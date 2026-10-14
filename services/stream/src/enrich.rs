//! Decoded events → enriched flows.
//!
//! One on-chain action (a bridge, a Gateway deposit) emits several logs. Enrichment
//! collapses each transaction's logs into the economic flows they represent, so a CCTP
//! burn is one `cctp_outbound` flow from the real originator rather than three token
//! transfers, and then describes the flow the way a compliance analyst would.

use std::collections::BTreeMap;

use alloy_primitives::{Address, B256, U256};
use serde_json::{json, Value};

use crate::config::{Contracts, ReferenceRates};
use crate::decode::{bytes32_as_address, Decoded, Event};

pub const ARC_DOMAIN: i32 = 26;

/// A flow ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFlow {
    pub tx_hash: String,
    pub log_index: i32,
    pub block_number: i64,
    pub asset: &'static str,
    pub kind: &'static str,
    pub from_address: String,
    pub to_address: String,
    /// Raw token units; `None` when the amount is shielded.
    pub amount: Option<U256>,
    pub usd_value: Option<f64>,
    pub visibility: &'static str,
    pub src_domain: Option<i32>,
    pub dst_domain: Option<i32>,
    pub details: Value,
}

/// An observed control relationship between two addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLink {
    pub address_a: String,
    pub address_b: String,
    pub link_type: &'static str,
    pub evidence_tx: String,
}

pub fn addr(a: &Address) -> String {
    format!("{a:#x}")
}

/// A cross-chain party: the EVM address when it is one, else the raw bytes32.
fn remote(value: &B256) -> String {
    match bytes32_as_address(value) {
        Some(a) => addr(&a),
        None => format!("{value:#x}"),
    }
}

/// Raw 6-decimal token units → notional USD.
pub fn usd_value(amount: U256, decimals: u32, rate: f64) -> f64 {
    let units = u128::try_from(amount).unwrap_or(u128::MAX) as f64;
    units / 10f64.powi(decimals as i32) * rate
}

/// CCTP V2 domain → chain name, from
/// https://developers.circle.com/cctp/concepts/supported-chains-and-domains.
pub fn cctp_domain_name(domain: i32) -> String {
    let name = match domain {
        0 => "Ethereum",
        1 => "Avalanche",
        2 => "OP Mainnet",
        3 => "Arbitrum",
        5 => "Solana",
        6 => "Base",
        7 => "Polygon PoS",
        8 => "Sui",
        9 => "Aptos",
        10 => "Unichain",
        11 => "Linea",
        12 => "Codex",
        13 => "Sonic",
        14 => "World Chain",
        15 => "Monad",
        16 => "Sei",
        17 => "BNB Smart Chain",
        18 => "XDC",
        19 => "HyperEVM",
        21 => "Ink",
        22 => "Plume",
        25 => "Starknet",
        26 => "Arc",
        27 => "Stellar",
        28 => "EDGE",
        29 => "Injective",
        30 => "Morph",
        31 => "Pharos",
        32 => "Cronos",
        33 => "Plasma",
        37 => "X Layer",
        other => return format!("domain {other}"),
    };
    name.to_string()
}

/// Which plain token transfers are already accounted for by a higher-level event in the
/// same transaction, per token.
#[derive(Default)]
struct Suppress {
    burns: bool,
    mints: bool,
    touching: Vec<Address>,
}

/// Native USDC is logged at 18 decimals, the ERC-20 interface at 6.
const NATIVE_UNITS_PER_ERC20_UNIT: u64 = 1_000_000_000_000;

/// The native USDC movements of a transaction that no ERC-20 `Transfer` accounts for,
/// re-expressed as USDC transfers in 6-decimal units.
///
/// Arc's system emitter logs every USDC movement, while the ERC-20 contract logs only
/// calls made through its interface. Each ERC-20 log therefore has a native twin, and what
/// is left over is a native send, which reading the ERC-20 contract alone would miss.
fn native_only_sends(events: &[&Decoded], contracts: &Contracts) -> Vec<Decoded> {
    let Some(usdc) = contracts.usdc() else {
        return Vec::new();
    };
    let scale = U256::from(NATIVE_UNITS_PER_ERC20_UNIT);
    let mut twins: Vec<(Address, Address, U256)> = events
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transfer {
                token,
                from,
                to,
                value,
            } if *token == usdc.address => Some((*from, *to, value.saturating_mul(scale))),
            _ => None,
        })
        .collect();
    events
        .iter()
        .filter_map(|e| {
            let Event::NativeTransfer { from, to, value } = &e.event else {
                return None;
            };
            if let Some(twin) = twins.iter().position(|t| t == &(*from, *to, *value)) {
                twins.swap_remove(twin);
                return None;
            }
            Some(Decoded {
                event: Event::Transfer {
                    token: usdc.address,
                    from: *from,
                    to: *to,
                    // Whole ERC-20 units: anything below 0.000001 USDC is dropped.
                    value: *value / scale,
                },
                ..(*e).clone()
            })
        })
        .collect()
}

/// Collapse every decoded event of a block range into flows and clustering links.
pub fn build_flows(
    events: &[Decoded],
    contracts: &Contracts,
    rates: &ReferenceRates,
) -> (Vec<NewFlow>, Vec<NewLink>) {
    let mut by_tx: BTreeMap<(u64, B256), Vec<&Decoded>> = BTreeMap::new();
    for e in events {
        by_tx
            .entry((e.block_number, e.tx_hash))
            .or_default()
            .push(e);
    }
    let mut flows = Vec::new();
    let mut links = Vec::new();
    for tx_events in by_tx.values() {
        // From here on a native send is a USDC transfer like any other, so the bridge and
        // Gateway rules below see through it the same way.
        let native = native_only_sends(tx_events, contracts);
        let mut events: Vec<&Decoded> = tx_events
            .iter()
            .copied()
            .filter(|e| !matches!(e.event, Event::NativeTransfer { .. }))
            .chain(&native)
            .collect();
        events.sort_by_key(|e| e.log_index);
        let native: Vec<u32> = native.iter().map(|e| e.log_index).collect();
        build_tx_flows(&events, &native, contracts, rates, &mut flows, &mut links);
    }
    (flows, links)
}

fn build_tx_flows(
    events: &[&Decoded],
    // Log indexes of the transfers that were native sends.
    native: &[u32],
    contracts: &Contracts,
    rates: &ReferenceRates,
    flows: &mut Vec<NewFlow>,
    links: &mut Vec<NewLink>,
) {
    let Some(first) = events.first() else { return };
    let tx_hash = format!("{:#x}", first.tx_hash);
    let block_number = first.block_number as i64;
    let mut suppress: BTreeMap<Address, Suppress> = BTreeMap::new();
    // Transfer log indexes consumed as the funding leg of a bridge.
    let mut consumed: Vec<u32> = Vec::new();

    let flow = |log_index: u32,
                token: &Address,
                kind: &'static str,
                from: String,
                to: String,
                amount: U256,
                src_domain: Option<i32>,
                dst_domain: Option<i32>,
                details: Value|
     -> Option<NewFlow> {
        let token = contracts.token(token)?;
        Some(NewFlow {
            tx_hash: tx_hash.clone(),
            log_index: log_index as i32,
            block_number,
            asset: token.symbol,
            kind,
            from_address: from,
            to_address: to,
            amount: Some(amount),
            usd_value: Some(usd_value(amount, token.decimals, rates.rate(token.symbol))),
            visibility: "public",
            src_domain,
            dst_domain,
            details,
        })
    };

    // CCTP inbound mints pair, in order, with the MessageReceived logs of the same tx.
    let mut received = events.iter().filter_map(|e| match &e.event {
        Event::CctpMessageReceived {
            source_domain,
            message_sender,
            finality_threshold,
        } => Some((*source_domain, *message_sender, *finality_threshold)),
        _ => None,
    });

    for e in events {
        match &e.event {
            Event::CctpDepositForBurn {
                token,
                depositor,
                amount,
                mint_recipient,
                destination_domain,
                max_fee,
                min_finality_threshold,
            } => {
                let s = suppress.entry(*token).or_default();
                s.burns = true;
                s.touching
                    .extend([contracts.cctp_token_messenger, contracts.cctp_token_minter]);
                // A forwarder contract is often the depositor of record. When exactly the
                // burned amount was transferred into it in this tx, that sender is the
                // economic originator.
                let funding = events.iter().find_map(|f| match &f.event {
                    Event::Transfer {
                        token: t,
                        from,
                        to,
                        value,
                    } if t == token
                        && to == depositor
                        && value == amount
                        && !consumed.contains(&f.log_index) =>
                    {
                        Some((f.log_index, *from))
                    }
                    _ => None,
                });
                let mut details = json!({
                    "protocol": "cctp_v2",
                    "depositor": addr(depositor),
                    "max_fee": max_fee.to_string(),
                    "min_finality_threshold": min_finality_threshold,
                    "fast_transfer": *min_finality_threshold < 2000,
                });
                let from = match funding {
                    Some((idx, originator)) => {
                        consumed.push(idx);
                        details["forwarder"] = json!(addr(depositor));
                        originator
                    }
                    None => *depositor,
                };
                let to = remote(mint_recipient);
                details["self_bridge"] = json!(to == addr(&from));
                flows.extend(flow(
                    e.log_index,
                    token,
                    "cctp_outbound",
                    addr(&from),
                    to,
                    *amount,
                    Some(ARC_DOMAIN),
                    Some(*destination_domain as i32),
                    details,
                ));
            }
            Event::CctpMintAndWithdraw {
                token,
                recipient,
                amount,
                fee,
            } => {
                let s = suppress.entry(*token).or_default();
                s.mints = true;
                s.touching
                    .extend([contracts.cctp_token_messenger, contracts.cctp_token_minter]);
                let msg = received.next();
                let sender = msg.and_then(|(_, sender, _)| sender);
                let from = sender
                    .as_ref()
                    .map(remote)
                    .unwrap_or_else(|| addr(&Address::ZERO));
                let mut details = json!({
                    "protocol": "cctp_v2",
                    "fee_collected": fee.to_string(),
                    "self_bridge": from == addr(recipient),
                });
                if let Some((_, _, finality)) = msg {
                    details["finality_threshold_executed"] = json!(finality);
                    details["fast_transfer"] = json!(finality < 2000);
                }
                flows.extend(flow(
                    e.log_index,
                    token,
                    "cctp_inbound",
                    from,
                    addr(recipient),
                    *amount,
                    msg.map(|(domain, _, _)| domain as i32),
                    Some(ARC_DOMAIN),
                    details,
                ));
            }
            Event::GatewayDeposited {
                token,
                depositor,
                sender,
                value,
            } => {
                suppress
                    .entry(*token)
                    .or_default()
                    .touching
                    .push(contracts.gateway_wallet);
                flows.extend(flow(
                    e.log_index,
                    token,
                    "gateway_deposit",
                    addr(sender),
                    addr(&contracts.gateway_wallet),
                    *value,
                    None,
                    None,
                    json!({ "protocol": "gateway", "credited_to": addr(depositor) }),
                ));
            }
            Event::GatewayBurned {
                token,
                depositor,
                signer,
                destination_domain,
                destination_recipient,
                value,
                fee,
            } => {
                let s = suppress.entry(*token).or_default();
                s.burns = true;
                s.touching.push(contracts.gateway_wallet);
                // The signer spent the depositor's unified balance: same controlling party.
                if signer != depositor {
                    links.push(NewLink {
                        address_a: addr(depositor),
                        address_b: addr(signer),
                        link_type: "gateway_delegate",
                        evidence_tx: tx_hash.clone(),
                    });
                }
                flows.extend(flow(
                    e.log_index,
                    token,
                    "gateway_outbound",
                    addr(depositor),
                    remote(destination_recipient),
                    *value,
                    Some(ARC_DOMAIN),
                    Some(*destination_domain as i32),
                    json!({
                        "protocol": "gateway",
                        "signer": addr(signer),
                        "fee": fee.to_string(),
                    }),
                ));
            }
            Event::GatewayAttestationUsed {
                token,
                recipient,
                source_domain,
                source_depositor,
                value,
            } => {
                let s = suppress.entry(*token).or_default();
                s.mints = true;
                s.touching.push(contracts.gateway_minter);
                flows.extend(flow(
                    e.log_index,
                    token,
                    "gateway_inbound",
                    remote(source_depositor),
                    addr(recipient),
                    *value,
                    Some(*source_domain as i32),
                    Some(ARC_DOMAIN),
                    json!({ "protocol": "gateway" }),
                ));
            }
            Event::ShieldedTransfer { token, from, to } => {
                if let Some(t) = contracts.token(token) {
                    flows.push(NewFlow {
                        tx_hash: tx_hash.clone(),
                        log_index: e.log_index as i32,
                        block_number,
                        asset: t.symbol,
                        kind: "transfer",
                        from_address: addr(from),
                        to_address: addr(to),
                        amount: None,
                        usd_value: None,
                        visibility: "shielded",
                        src_domain: None,
                        dst_domain: None,
                        details: json!({ "privacy": "confidential_transfer" }),
                    });
                }
            }
            Event::Transfer { .. }
            | Event::NativeTransfer { .. }
            | Event::CctpMessageReceived { .. } => {}
        }
    }

    for e in events {
        let Event::Transfer {
            token,
            from,
            to,
            value,
        } = &e.event
        else {
            continue;
        };
        if value.is_zero() || consumed.contains(&e.log_index) {
            continue;
        }
        if let Some(s) = suppress.get(token) {
            if (s.burns && to.is_zero())
                || (s.mints && from.is_zero())
                || s.touching.contains(from)
                || s.touching.contains(to)
            {
                continue;
            }
        }
        let is_usyc = contracts.token(token).is_some_and(|t| t.symbol == "USYC");
        let kind = if is_usyc && (from.is_zero() || *from == contracts.usyc_teller) {
            "usyc_subscription"
        } else if is_usyc && (to.is_zero() || *to == contracts.usyc_teller) {
            "usyc_redemption"
        } else if *to == contracts.usyc_teller {
            // The stablecoin leg paid into the Teller.
            "usyc_subscription"
        } else if *from == contracts.usyc_teller {
            "usyc_redemption"
        } else if *from == contracts.fx_escrow || *to == contracts.fx_escrow {
            "fx_settlement"
        } else if from.is_zero() {
            "mint"
        } else if to.is_zero() {
            "burn"
        } else {
            "transfer"
        };
        flows.extend(flow(
            e.log_index,
            token,
            kind,
            addr(from),
            addr(to),
            *value,
            None,
            None,
            if native.contains(&e.log_index) {
                json!({ "usdc_interface": "native" })
            } else {
                json!({})
            },
        ));
    }
}

/// Notional-USD size band of a flow.
pub fn size_tier(usd: f64) -> &'static str {
    if usd >= 1_000_000.0 {
        "institutional"
    } else if usd >= 10_000.0 {
        "commercial"
    } else if usd >= 100.0 {
        "retail"
    } else {
        "micro"
    }
}

pub fn format_usd(usd: f64) -> String {
    let abs = usd.abs();
    if abs >= 1e9 {
        format!("${:.2}B", usd / 1e9)
    } else if abs >= 1e6 {
        format!("${:.1}M", usd / 1e6)
    } else if abs >= 1e3 {
        format!("${:.1}K", usd / 1e3)
    } else {
        format!("${usd:.2}")
    }
}

pub fn short_address(address: &str) -> String {
    if address.len() > 12 && address.is_ascii() {
        format!("{}…{}", &address[..6], &address[address.len() - 4..])
    } else {
        address.to_string()
    }
}

/// Everything the narrative depends on. Entity names are resolved at read time, so a new
/// attribution rewrites the story of every historical flow it touches.
pub struct Narrative<'a> {
    pub kind: &'a str,
    pub asset: &'a str,
    pub usd_value: Option<f64>,
    pub visibility: &'a str,
    pub from_address: &'a str,
    pub to_address: &'a str,
    pub from_entity: Option<&'a str>,
    pub to_entity: Option<&'a str>,
    pub src_domain: Option<i32>,
    pub dst_domain: Option<i32>,
    pub self_bridge: bool,
}

impl Narrative<'_> {
    /// Both sides resolve to the same known entity.
    fn internal(&self) -> bool {
        match (self.from_entity, self.to_entity) {
            (Some(a), Some(b)) => a == b,
            // The same address on both chains, owned by a known entity.
            (Some(_), None) | (None, Some(_)) => self.self_bridge,
            (None, None) => false,
        }
    }

    pub fn render(&self) -> String {
        let src = self.src_domain.map(cctp_domain_name).unwrap_or_default();
        let dst = self.dst_domain.map(cctp_domain_name).unwrap_or_default();
        let internal = self.internal();
        let action = match self.kind {
            "cctp_outbound" if internal => format!("treasury rebalance to {dst} via CCTP"),
            "cctp_outbound" if self.self_bridge => format!("self-bridge to {dst} via CCTP"),
            "cctp_outbound" => format!("bridge-out to {dst} via CCTP"),
            "cctp_inbound" if internal => format!("treasury rebalance from {src} via CCTP"),
            "cctp_inbound" if self.self_bridge => format!("self-bridge from {src} via CCTP"),
            "cctp_inbound" => format!("bridge-in from {src} via CCTP"),
            "gateway_deposit" => "deposit into Gateway unified balance".to_string(),
            "gateway_outbound" => format!("Gateway transfer out to {dst}"),
            "gateway_inbound" => format!("Gateway mint from {src}"),
            "usyc_subscription" => "USYC subscription leg".to_string(),
            "usyc_redemption" => "USYC redemption leg".to_string(),
            "fx_settlement" => "StableFX settlement leg".to_string(),
            "mint" => "issuance".to_string(),
            "burn" => "redemption burn".to_string(),
            _ if internal => "internal treasury movement".to_string(),
            _ => "transfer".to_string(),
        };
        let size = match (self.visibility, self.usd_value) {
            ("shielded", _) | (_, None) => "Amount-shielded".to_string(),
            (_, Some(usd)) => format!("{} {}", format_usd(usd), size_tier(usd)),
        };
        let party = |entity: Option<&str>, address: &str| match entity {
            Some(name) => name.to_string(),
            None => short_address(address),
        };
        let from = party(self.from_entity, self.from_address);
        let to = party(self.to_entity, self.to_address);
        let parties = match self.kind {
            "mint" => format!("to {to}"),
            "burn" => format!("by {from}"),
            _ => format!("{from} → {to}"),
        };
        format!("{size} {} {action}: {parties}", self.asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{decode, fixtures::*};
    use crate::rpc::RawLog;

    const USDC: &str = "0x3600000000000000000000000000000000000000";
    const ZERO: &str = "0x0000000000000000000000000000000000000000";
    const MESSENGER: &str = "0x8fe6b999dc680ccfdd5bf7eb0974218be2542daa";
    const DEPOSITOR: &str = "0xc5567a5e3370d4dbfb0540025078e283e36a363d";
    const ALICE: &str = "0x00000000000000000000000000000000000a11ce";

    fn run(logs: &[RawLog]) -> (Vec<NewFlow>, Vec<NewLink>) {
        let contracts = Contracts::arc_testnet();
        let rates = ReferenceRates {
            eurc_usd: 1.08,
            usyc_usd: 1.10,
        };
        let events: Vec<Decoded> = logs
            .iter()
            .filter_map(|l| decode(l, &contracts, None))
            .collect();
        build_flows(&events, &contracts, &rates)
    }

    #[test]
    fn cctp_burn_collapses_to_one_flow_from_the_originator() {
        let (flows, _) = run(&[
            transfer(USDC, ALICE, DEPOSITOR, 1_020_000, TX_A, 4),
            transfer(USDC, DEPOSITOR, MESSENGER, 1_020_000, TX_A, 5),
            transfer(USDC, MESSENGER, ZERO, 1_020_000, TX_A, 6),
            deposit_for_burn(),
        ]);
        assert_eq!(flows.len(), 1, "{flows:#?}");
        let f = &flows[0];
        assert_eq!(f.kind, "cctp_outbound");
        assert_eq!(f.from_address, ALICE);
        assert_eq!(f.to_address, "0x1474a77ac12fade821e9df4b5afca288849b2e8a");
        assert_eq!(f.dst_domain, Some(6));
        assert_eq!(f.details["forwarder"], DEPOSITOR);
        assert!((f.usd_value.unwrap() - 1.02).abs() < 1e-9);
    }

    #[test]
    fn unrelated_transfer_in_a_bridge_tx_is_kept() {
        let (flows, _) = run(&[
            transfer(
                USDC,
                ALICE,
                "0x00000000000000000000000000000000000000b0",
                5_000_000,
                TX_A,
                1,
            ),
            deposit_for_burn(),
        ]);
        let kinds: Vec<_> = flows.iter().map(|f| f.kind).collect();
        assert_eq!(kinds.len(), 2);
        assert!(kinds.contains(&"transfer") && kinds.contains(&"cctp_outbound"));
    }

    #[test]
    fn a_native_usdc_send_becomes_a_flow_in_erc20_units() {
        let (flows, _) = run(&[native_send()]);
        assert_eq!(flows.len(), 1, "{flows:#?}");
        let f = &flows[0];
        assert_eq!((f.asset, f.kind), ("USDC", "transfer"));
        assert_eq!(f.from_address, "0x59825bb2aa01018adf67aa95741d23cae307431a");
        assert_eq!(f.to_address, "0xe72f8175ab0991dbb778f6de62009c5bf97c17f7");
        // 10.068033162188713984 USDC at 18 decimals, kept to the 6 the ERC-20 uses.
        assert_eq!(f.amount, Some(U256::from(10_068_033u64)));
        assert_eq!(f.details["usdc_interface"], "native");
    }

    #[test]
    fn an_erc20_transfer_is_not_counted_again_through_its_native_twin() {
        let (flows, _) = run(&erc20_transfer_with_native_twin());
        assert_eq!(flows.len(), 1, "{flows:#?}");
        assert_eq!(flows[0].amount, Some(U256::from(1_921_835u64)));
        assert_eq!(flows[0].log_index, 1);
        assert_eq!(flows[0].details, json!({}));
    }

    #[test]
    fn a_natively_funded_bridge_still_collapses_to_one_flow() {
        // ALICE pays the forwarder with a native send instead of an ERC-20 transfer.
        let native_funding = {
            let pad = |a: &str| format!("0x{:0>64}", a.trim_start_matches("0x"));
            log(
                SYSTEM_EMITTER,
                &[
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    &pad(ALICE),
                    &pad(DEPOSITOR),
                ],
                &format!("0x{:064x}", 1_020_000u128 * 1_000_000_000_000),
                TX_A,
                3,
            )
        };
        let (flows, _) = run(&[
            native_funding,
            transfer(USDC, DEPOSITOR, MESSENGER, 1_020_000, TX_A, 5),
            transfer(USDC, MESSENGER, ZERO, 1_020_000, TX_A, 6),
            deposit_for_burn(),
        ]);
        assert_eq!(flows.len(), 1, "{flows:#?}");
        assert_eq!(flows[0].kind, "cctp_outbound");
        assert_eq!(flows[0].from_address, ALICE);
    }

    #[test]
    fn gateway_burn_links_signer_to_depositor() {
        let (flows, links) = run(&[gateway_burned()]);
        assert_eq!(flows[0].kind, "gateway_outbound");
        assert_eq!(flows[0].dst_domain, Some(26));
        assert_eq!(
            links,
            vec![NewLink {
                address_a: "0x631fdaafe9ff675bf7a122013d1555513363dcc7".into(),
                address_b: "0x10f48f71dee384425ac52ea3dbf5180a95d3f5ab".into(),
                link_type: "gateway_delegate",
                evidence_tx: "0x0ac7304575f550b70f544c4c3192674fc90efd9f2cff8ea761e6de884d66f445"
                    .into(),
            }]
        );
    }

    #[test]
    fn gateway_deposit_absorbs_its_token_transfer() {
        let tx = "0x8aeb435d19b8354fe12e07c82f417a70225a0f0af062ba5452f477c684bf2a22";
        let (flows, _) = run(&[
            transfer(
                USDC,
                "0x631fdaafe9ff675bf7a122013d1555513363dcc7",
                "0x0077777d7eba4688bdef3e311b846f25870a19b9",
                300_000,
                tx,
                1,
            ),
            gateway_deposited(),
        ]);
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].kind, "gateway_deposit");
    }

    #[test]
    fn plain_transfers_classify_mint_burn_and_skip_zero_value() {
        let (flows, _) = run(&[
            transfer(USDC, ZERO, ALICE, 10, TX_A, 1),
            transfer(USDC, ALICE, ZERO, 10, TX_A, 2),
            transfer(USDC, ALICE, DEPOSITOR, 0, TX_A, 3),
        ]);
        let kinds: Vec<_> = flows.iter().map(|f| f.kind).collect();
        assert_eq!(kinds, vec!["mint", "burn"]);
    }

    #[test]
    fn narrative_names_entities_and_detects_rebalances() {
        let base = Narrative {
            kind: "cctp_outbound",
            asset: "USDC",
            usd_value: Some(12_000_000.0),
            visibility: "public",
            from_address: ALICE,
            to_address: DEPOSITOR,
            from_entity: Some("Acme Treasury"),
            to_entity: Some("Acme Treasury"),
            src_domain: Some(26),
            dst_domain: Some(0),
            self_bridge: false,
        };
        assert_eq!(
            base.render(),
            "$12.0M institutional USDC treasury rebalance to Ethereum via CCTP: Acme Treasury → Acme Treasury"
        );
        let shielded = Narrative {
            kind: "transfer",
            usd_value: None,
            visibility: "shielded",
            to_entity: None,
            ..base
        };
        let anonymous = Narrative {
            usd_value: Some(0.1),
            from_entity: None,
            to_entity: None,
            self_bridge: true,
            ..base
        };
        assert_eq!(
            anonymous.render(),
            "$0.10 micro USDC self-bridge to Ethereum via CCTP: 0x0000…11ce → 0xc556…363d"
        );
        assert_eq!(
            shielded.render(),
            "Amount-shielded USDC transfer: Acme Treasury → 0xc556…363d"
        );
    }
}
