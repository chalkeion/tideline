//! The token registry: every asset contract seen on the network, with what it takes to read
//! its amounts and, for listed tokens, to value them.
//!
//! Symbols are free text on chain: anyone can deploy a token called USDC, and on Arc Mainnet
//! someone has. A token is therefore identified by its address. Only a listed token, one
//! whose address Arc or its issuer publishes (`data/tokens.seed.json`), owns its symbol and
//! carries a price. Everything else is discovered from its own logs, tracked with exact
//! amounts, and left unpriced rather than guessed at.

use std::collections::HashMap;

use alloy_primitives::{Address, Bytes, U256};
use serde::Deserialize;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::config::Network;
use crate::rpc::RpcClient;

const SEED: &str = include_str!("../data/tokens.seed.json");

/// Longest symbol or name kept from a contract. Longer is spam or garbage.
const MAX_TEXT_CHARS: usize = 64;

/// Tokens whose metadata is read in one RPC batch (three calls each). Public Arc endpoints
/// answer about 20 calls of a batch and rate-limit the rest.
const DISCOVERY_BATCH: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standard {
    Erc20,
    Erc721,
}

impl Standard {
    pub fn key(self) -> &'static str {
        match self {
            Standard::Erc20 => "erc20",
            Standard::Erc721 => "erc721",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "erc20" => Some(Standard::Erc20),
            "erc721" => Some(Standard::Erc721),
            _ => None,
        }
    }
}

/// How a listed token is expressed in notional USD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceSource {
    /// USDC: 1.00 by definition.
    Usd,
    /// EURC: the ECB euro reference rate.
    Eur,
    /// USYC: the fund's published price report.
    Usyc,
    /// cirBTC: the bitcoin spot price.
    Btc,
    /// WETH: the ether spot price.
    Eth,
}

impl PriceSource {
    pub fn key(self) -> &'static str {
        match self {
            PriceSource::Usd => "usd",
            PriceSource::Eur => "eur",
            PriceSource::Usyc => "usyc",
            PriceSource::Btc => "btc",
            PriceSource::Eth => "eth",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            PriceSource::Usd,
            PriceSource::Eur,
            PriceSource::Usyc,
            PriceSource::Btc,
            PriceSource::Eth,
        ]
        .into_iter()
        .find(|p| p.key() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    pub address: Address,
    pub standard: Standard,
    pub symbol: Option<String>,
    pub name: Option<String>,
    /// `None` when the contract does not say: amounts stay in raw units. 0 for NFTs.
    pub decimals: Option<u32>,
    pub listed: bool,
    /// Entity slug of the issuer, for listed tokens.
    pub issuer: Option<String>,
    /// `None`: unpriced.
    pub price: Option<PriceSource>,
}

impl TokenInfo {
    /// What a flow's `asset` shows: the symbol, else the shortened address.
    pub fn label(&self) -> String {
        self.symbol
            .clone()
            .unwrap_or_else(|| crate::enrich::short_address(&format!("{:#x}", self.address)))
    }
}

pub type TokenMap = HashMap<Address, TokenInfo>;

#[derive(Deserialize)]
struct SeedFile {
    tokens: Vec<SeedToken>,
}

#[derive(Deserialize)]
struct SeedToken {
    network: String,
    address: String,
    symbol: String,
    name: String,
    decimals: u32,
    issuer: Option<String>,
    price: String,
}

/// The listed tokens of `network`.
pub fn listed(network: Network) -> anyhow::Result<Vec<TokenInfo>> {
    let seed: SeedFile = serde_json::from_str(SEED)?;
    seed.tokens
        .into_iter()
        .filter(|t| t.network == network.key())
        .map(|t| {
            Ok(TokenInfo {
                address: t.address.parse()?,
                standard: Standard::Erc20,
                symbol: Some(t.symbol),
                name: Some(t.name),
                decimals: Some(t.decimals),
                listed: true,
                issuer: t.issuer,
                price: Some(
                    PriceSource::parse(&t.price)
                        .ok_or_else(|| anyhow::anyhow!("tokens seed: unknown price {}", t.price))?,
                ),
            })
        })
        .collect()
}

/// Write the listed tokens of `network`. A listing overrides whatever discovery stored.
pub async fn seed(pool: &PgPool, network: Network) -> anyhow::Result<()> {
    for t in listed(network)? {
        sqlx::query(
            "INSERT INTO tokens (address, standard, symbol, name, decimals, listed, issuer_slug,
                 price_source)
             VALUES ($1, $2, $3, $4, $5, true, $6, $7)
             ON CONFLICT (address) DO UPDATE SET standard = EXCLUDED.standard,
                 symbol = EXCLUDED.symbol, name = EXCLUDED.name, decimals = EXCLUDED.decimals,
                 listed = true, issuer_slug = EXCLUDED.issuer_slug,
                 price_source = EXCLUDED.price_source",
        )
        .bind(format!("{:#x}", t.address))
        .bind(t.standard.key())
        .bind(&t.symbol)
        .bind(&t.name)
        .bind(t.decimals.map(|d| d as i32))
        .bind(&t.issuer)
        .bind(t.price.map(PriceSource::key))
        .execute(pool)
        .await?;
    }
    Ok(())
}

const TOKEN_SELECT: &str =
    "SELECT address, standard, symbol, name, decimals, listed, issuer_slug, price_source
     FROM tokens";

fn token_from_row(r: &PgRow) -> Option<TokenInfo> {
    Some(TokenInfo {
        address: r.get::<String, _>("address").parse().ok()?,
        standard: Standard::parse(r.get("standard"))?,
        symbol: r.get("symbol"),
        name: r.get("name"),
        decimals: r.get::<Option<i32>, _>("decimals").map(|d| d as u32),
        listed: r.get("listed"),
        issuer: r.get("issuer_slug"),
        price: r
            .get::<Option<String>, _>("price_source")
            .as_deref()
            .and_then(PriceSource::parse),
    })
}

/// Every token the database knows.
pub async fn load(pool: &PgPool) -> anyhow::Result<TokenMap> {
    let rows = sqlx::query(TOKEN_SELECT).fetch_all(pool).await?;
    Ok(rows
        .iter()
        .filter_map(token_from_row)
        .map(|t| (t.address, t))
        .collect())
}

/// One token by its lowercase address.
pub async fn get(pool: &PgPool, address: &str) -> sqlx::Result<Option<TokenInfo>> {
    let row = sqlx::query(&format!("{TOKEN_SELECT} WHERE address = $1"))
        .bind(address)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().and_then(token_from_row))
}

/// A token contract as first seen in a block range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sighting {
    pub address: Address,
    pub standard: Standard,
    pub block: u64,
}

/// Register every token in `seen` that `tokens` does not know yet: read its metadata from
/// the chain and store it, so its flows are read with its own decimals. Returns how many
/// tokens were new.
pub async fn discover(
    pool: &PgPool,
    rpc: &RpcClient,
    tokens: &mut TokenMap,
    seen: &[Sighting],
) -> anyhow::Result<usize> {
    let mut new: Vec<Sighting> = Vec::new();
    for s in seen {
        if !tokens.contains_key(&s.address) && !new.iter().any(|n| n.address == s.address) {
            new.push(*s);
        }
    }
    for chunk in new.chunks(DISCOVERY_BATCH) {
        let addresses: Vec<Address> = chunk.iter().map(|s| s.address).collect();
        let metadata = rpc.token_metadata(&addresses).await?;
        for (s, [name, symbol, decimals]) in chunk.iter().zip(metadata) {
            let info = TokenInfo {
                address: s.address,
                standard: s.standard,
                symbol: symbol.as_ref().and_then(|b| abi_string(b)),
                name: name.as_ref().and_then(|b| abi_string(b)),
                decimals: match s.standard {
                    Standard::Erc20 => decimals.as_ref().and_then(|b| abi_decimals(b)),
                    Standard::Erc721 => Some(0),
                },
                listed: false,
                issuer: None,
                price: None,
            };
            sqlx::query(
                "INSERT INTO tokens (address, standard, symbol, name, decimals, first_seen_block)
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (address) DO NOTHING",
            )
            .bind(format!("{:#x}", info.address))
            .bind(info.standard.key())
            .bind(&info.symbol)
            .bind(&info.name)
            .bind(info.decimals.map(|d| d as i32))
            .bind(s.block as i64)
            .execute(pool)
            .await?;
            tracing::info!(
                token = %info.address,
                symbol = info.symbol.as_deref().unwrap_or("?"),
                standard = info.standard.key(),
                "new token"
            );
            tokens.insert(s.address, info);
        }
    }
    Ok(new.len())
}

/// Read metadata again for tokens stored without a symbol, or an ERC-20 without decimals.
/// Most are tokens that really lack them; the rest were stored before a transient failure
/// was told apart from a missing function. Run once at startup; a failure only logs.
pub async fn repair(pool: &PgPool, rpc: &RpcClient, tokens: &mut TokenMap) {
    let incomplete: Vec<TokenInfo> = tokens
        .values()
        .filter(|t| {
            !t.listed
                && (t.symbol.is_none() || (t.standard == Standard::Erc20 && t.decimals.is_none()))
        })
        .cloned()
        .collect();
    let mut repaired = 0;
    for chunk in incomplete.chunks(DISCOVERY_BATCH) {
        let addresses: Vec<Address> = chunk.iter().map(|t| t.address).collect();
        let metadata = match rpc.token_metadata(&addresses).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("token metadata repair stopped: {e}");
                return;
            }
        };
        for (old, [name, symbol, decimals]) in chunk.iter().zip(metadata) {
            let info = TokenInfo {
                symbol: symbol.as_ref().and_then(|b| abi_string(b)),
                name: name.as_ref().and_then(|b| abi_string(b)),
                decimals: match old.standard {
                    Standard::Erc20 => decimals.as_ref().and_then(|b| abi_decimals(b)),
                    Standard::Erc721 => Some(0),
                },
                ..old.clone()
            };
            if info == *old {
                continue;
            }
            let updated = sqlx::query(
                "UPDATE tokens SET symbol = $2, name = $3, decimals = $4
                 WHERE address = $1 AND NOT listed",
            )
            .bind(format!("{:#x}", info.address))
            .bind(&info.symbol)
            .bind(&info.name)
            .bind(info.decimals.map(|d| d as i32))
            .execute(pool)
            .await;
            if let Err(e) = updated {
                tracing::warn!("token metadata repair stopped: {e}");
                return;
            }
            repaired += 1;
            tokens.insert(info.address, info);
        }
    }
    if repaired > 0 {
        tracing::info!(repaired, "token metadata repaired");
    }
}

/// An ABI-encoded `string` return value, or a `bytes32` one (older tokens such as MKR).
/// Control characters are dropped and the result is capped; blank is `None`.
pub fn abi_string(data: &[u8]) -> Option<String> {
    let raw: &[u8] = if data.len() >= 64 {
        let offset = usize::try_from(U256::from_be_slice(&data[..32])).ok()?;
        let len_end = offset.checked_add(32)?;
        let len = usize::try_from(U256::from_be_slice(data.get(offset..len_end)?)).ok()?;
        data.get(len_end..len_end.checked_add(len)?)?
    } else if data.len() == 32 {
        let end = data.iter().position(|b| *b == 0).unwrap_or(32);
        &data[..end]
    } else {
        return None;
    };
    let text: String = String::from_utf8_lossy(raw)
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT_CHARS)
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// An ABI-encoded `decimals()` return value. Above 77 does not fit a NUMERIC(78) amount.
pub fn abi_decimals(data: &[u8]) -> Option<u32> {
    let value = U256::from_be_slice(data.get(..32)?);
    u32::try_from(value).ok().filter(|d| *d <= 77)
}

/// Decode a hex `eth_call` result.
pub fn hex_bytes(hex: &str) -> Option<Bytes> {
    hex.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        hex_bytes(s).unwrap().to_vec()
    }

    #[test]
    fn reads_dynamic_and_bytes32_strings() {
        // "USDC" as an ABI string.
        let usdc = hex(concat!(
            "0x0000000000000000000000000000000000000000000000000000000000000020",
            "0000000000000000000000000000000000000000000000000000000000000004",
            "5553444300000000000000000000000000000000000000000000000000000000"
        ));
        assert_eq!(abi_string(&usdc).as_deref(), Some("USDC"));
        // "MKR" as bytes32.
        let mkr = hex("0x4d4b520000000000000000000000000000000000000000000000000000000000");
        assert_eq!(abi_string(&mkr).as_deref(), Some("MKR"));
        // Empty string, nothing, and a length that runs past the data.
        let empty = hex(concat!(
            "0x0000000000000000000000000000000000000000000000000000000000000020",
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));
        assert_eq!(abi_string(&empty), None);
        assert_eq!(abi_string(&[]), None);
        let overrun = hex(concat!(
            "0x0000000000000000000000000000000000000000000000000000000000000020",
            "00000000000000000000000000000000000000000000000000000000000000ff"
        ));
        assert_eq!(abi_string(&overrun), None);
    }

    #[test]
    fn strips_control_characters_and_caps_length() {
        let mut data = vec![0u8; 31];
        data.push(0x20);
        let mut len = vec![0u8; 31];
        len.push(200);
        data.extend(len);
        data.extend(std::iter::repeat_n(b'A', 198));
        data.extend(*b"\n\x07");
        data.resize(data.len().div_ceil(32) * 32, 0);
        let text = abi_string(&data).unwrap();
        assert_eq!(text.len(), MAX_TEXT_CHARS);
        assert!(text.chars().all(|c| c == 'A'));
    }

    #[test]
    fn decimals_are_bounded() {
        let six = hex("0x0000000000000000000000000000000000000000000000000000000000000006");
        assert_eq!(abi_decimals(&six), Some(6));
        let huge = hex("0x00000000000000000000000000000000000000000000000000000000000000ff");
        assert_eq!(abi_decimals(&huge), None);
        assert_eq!(abi_decimals(&[]), None);
    }

    #[test]
    fn every_network_lists_the_circle_stablecoins_with_matching_decimals() {
        for network in [Network::Mainnet, Network::Testnet] {
            let listed = listed(network).unwrap();
            for token in network.contracts().tokens {
                let entry = listed
                    .iter()
                    .find(|t| t.address == token.address)
                    .unwrap_or_else(|| panic!("{} lacks {}", network.key(), token.symbol));
                assert_eq!(entry.symbol.as_deref(), Some(token.symbol));
                assert_eq!(entry.decimals, Some(token.decimals));
            }
            let mut addresses: Vec<_> = listed.iter().map(|t| t.address).collect();
            addresses.sort();
            addresses.dedup();
            assert_eq!(addresses.len(), listed.len(), "duplicate listing");
            assert!(listed.iter().all(|t| t.price.is_some()));
        }
    }
}
