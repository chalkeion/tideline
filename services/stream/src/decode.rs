//! Raw EVM logs → typed stablecoin events.
//!
//! Every signature here was checked against live Arc Testnet and Mainnet logs; the
//! fixtures in the tests below are real logs.

use alloy_primitives::{b256, Address, B256, U256};

use crate::config::Contracts;
use crate::rpc::RawLog;
use crate::tokens::Standard;

/// `Transfer(address indexed from, address indexed to, uint256 value)`
pub const TRANSFER: B256 =
    b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
/// `DepositForBurn(address indexed burnToken, uint256 amount, address indexed depositor,
/// bytes32 mintRecipient, uint32 destinationDomain, bytes32 destinationTokenMessenger,
/// bytes32 destinationCaller, uint256 maxFee, uint32 indexed minFinalityThreshold, bytes hookData)`
pub const CCTP_DEPOSIT_FOR_BURN: B256 =
    b256!("0c8c1cbdc5190613ebd485511d4e2812cfa45eecb79d845893331fedad5130a5");
/// `MintAndWithdraw(address indexed mintRecipient, uint256 amount, address indexed mintToken,
/// uint256 feeCollected)`
pub const CCTP_MINT_AND_WITHDRAW: B256 =
    b256!("50c55e915134d457debfa58eb6f4342956f8b0616d51a89a3659360178e1ab63");
/// `MessageReceived(address indexed caller, uint32 sourceDomain, bytes32 indexed nonce,
/// bytes32 sender, uint32 indexed finalityThresholdExecuted, bytes messageBody)`
pub const CCTP_MESSAGE_RECEIVED: B256 =
    b256!("ff48c13eda96b1cceacc6b9edeedc9e9db9d6226afbc30146b720c19d3addb1c");
/// `Deposited(address indexed token, address indexed depositor, address indexed sender,
/// uint256 value)`
pub const GATEWAY_DEPOSITED: B256 =
    b256!("4174a9435a04d04d274c76779cad136a41fde6937c56241c09ab9d3c7064a1a9");
/// `GatewayBurned(address indexed token, address indexed depositor, bytes32 indexed
/// transferSpecHash, uint32 destinationDomain, bytes32 destinationRecipient, address signer,
/// uint256 value, uint256 fee, uint256 fromAvailable, uint256 fromWithdrawing)`
pub const GATEWAY_BURNED: B256 =
    b256!("12ee2719e7e2dec9f2a0041286b66669153dff0d36719f692b8bbaa4dfe0aa87");
/// `AttestationUsed(address indexed token, address indexed recipient, bytes32 indexed
/// transferSpecHash, uint32 sourceDomain, bytes32 sourceDepositor, bytes32 sourceSigner,
/// uint256 value)`
pub const GATEWAY_ATTESTATION_USED: B256 =
    b256!("bb312ce0cc311b2cb0746e09ccd2f91fdb9e2ac755d2f11c65300eb0d0fffd63");

/// topic0 values the ingestor asks the node for.
pub fn watched_topics(confidential: Option<B256>) -> Vec<B256> {
    let mut topics = vec![
        TRANSFER,
        CCTP_DEPOSIT_FOR_BURN,
        CCTP_MINT_AND_WITHDRAW,
        CCTP_MESSAGE_RECEIVED,
        GATEWAY_DEPOSITED,
        GATEWAY_BURNED,
        GATEWAY_ATTESTATION_USED,
    ];
    topics.extend(confidential);
    topics
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// An ERC-20 `Transfer`, from any contract on the network.
    Transfer {
        token: Address,
        from: Address,
        to: Address,
        value: U256,
    },
    /// An ERC-721 `Transfer`: the same topic0 with the token id indexed as well.
    NftTransfer {
        token: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    /// A native USDC movement, logged by Arc's system emitter at 18 decimals. Every ERC-20
    /// USDC `Transfer` has one of these as a twin; a plain native send has only this.
    NativeTransfer {
        from: Address,
        to: Address,
        value: U256,
    },
    /// An amount-shielded transfer: Arc confidential transfers keep both addresses public
    /// and encrypt only the amount.
    ShieldedTransfer {
        token: Address,
        from: Address,
        to: Address,
    },
    CctpDepositForBurn {
        token: Address,
        depositor: Address,
        amount: U256,
        mint_recipient: B256,
        destination_domain: u32,
        max_fee: U256,
        min_finality_threshold: u32,
    },
    CctpMintAndWithdraw {
        token: Address,
        recipient: Address,
        amount: U256,
        fee: U256,
    },
    CctpMessageReceived {
        source_domain: u32,
        /// `messageSender` from the BurnMessageV2 body: the depositor on the source chain.
        message_sender: Option<B256>,
        finality_threshold: u32,
    },
    GatewayDeposited {
        token: Address,
        depositor: Address,
        sender: Address,
        value: U256,
    },
    GatewayBurned {
        token: Address,
        depositor: Address,
        signer: Address,
        destination_domain: u32,
        destination_recipient: B256,
        value: U256,
        fee: U256,
    },
    GatewayAttestationUsed {
        token: Address,
        recipient: Address,
        source_domain: u32,
        source_depositor: B256,
        value: U256,
    },
}

impl Event {
    /// The token contract a transfer moves, and its standard.
    pub fn token(&self) -> Option<(Address, Standard)> {
        match self {
            Event::Transfer { token, .. } | Event::ShieldedTransfer { token, .. } => {
                Some((*token, Standard::Erc20))
            }
            Event::NftTransfer { token, .. } => Some((*token, Standard::Erc721)),
            _ => None,
        }
    }
}

/// A decoded event with its position on chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub tx_hash: B256,
    pub log_index: u32,
    pub block_number: u64,
    pub event: Event,
}

fn word(data: &[u8], index: usize) -> Option<&[u8]> {
    data.get(index * 32..index * 32 + 32)
}

fn word_u256(data: &[u8], index: usize) -> Option<U256> {
    word(data, index).map(U256::from_be_slice)
}

fn word_u32(data: &[u8], index: usize) -> Option<u32> {
    word_u256(data, index).and_then(|v| u32::try_from(v).ok())
}

fn word_b256(data: &[u8], index: usize) -> Option<B256> {
    word(data, index).map(B256::from_slice)
}

fn word_address(data: &[u8], index: usize) -> Option<Address> {
    word(data, index).map(|w| Address::from_slice(&w[12..]))
}

fn topic_address(topics: &[B256], index: usize) -> Option<Address> {
    topics.get(index).map(|t| Address::from_slice(&t[12..]))
}

fn topic_u32(topics: &[B256], index: usize) -> Option<u32> {
    topics
        .get(index)
        .and_then(|t| u32::try_from(U256::from_be_bytes(t.0)).ok())
}

/// The EVM address inside a CCTP/Gateway `bytes32`, if it is one (12 leading zero bytes).
/// Non-EVM recipients (e.g. Solana) use all 32 bytes and return `None`.
pub fn bytes32_as_address(value: &B256) -> Option<Address> {
    value[..12]
        .iter()
        .all(|b| *b == 0)
        .then(|| Address::from_slice(&value[12..]))
}

/// `messageSender` sits after version(4) + burnToken(32) + mintRecipient(32) + amount(32)
/// in a BurnMessageV2 body.
fn burn_message_sender(body: &[u8]) -> Option<B256> {
    body.get(100..132).map(B256::from_slice)
}

/// Decode one log. Returns `None` for logs that are not a tracked event, or are malformed.
pub fn decode(
    log: &RawLog,
    contracts: &Contracts,
    confidential_topic: Option<B256>,
) -> Option<Decoded> {
    let topic0 = *log.topics.first()?;
    let topics = &log.topics;
    let data: &[u8] = &log.data;
    let is_token = contracts.token(&log.address).is_some();

    let event = if topic0 == TRANSFER && log.address == contracts.native_usdc_emitter {
        Event::NativeTransfer {
            from: topic_address(topics, 1)?,
            to: topic_address(topics, 2)?,
            value: word_u256(data, 0)?,
        }
    } else if topic0 == TRANSFER && topics.len() == 3 {
        // Every token on the network is tracked; the registry decides what it is.
        Event::Transfer {
            token: log.address,
            from: topic_address(topics, 1)?,
            to: topic_address(topics, 2)?,
            value: word_u256(data, 0)?,
        }
    } else if topic0 == TRANSFER && topics.len() == 4 {
        Event::NftTransfer {
            token: log.address,
            from: topic_address(topics, 1)?,
            to: topic_address(topics, 2)?,
            token_id: U256::from_be_bytes(topics[3].0),
        }
    } else if Some(topic0) == confidential_topic && is_token {
        Event::ShieldedTransfer {
            token: log.address,
            from: topic_address(topics, 1)?,
            to: topic_address(topics, 2)?,
        }
    } else if topic0 == CCTP_DEPOSIT_FOR_BURN && log.address == contracts.cctp_token_messenger {
        Event::CctpDepositForBurn {
            token: topic_address(topics, 1)?,
            depositor: topic_address(topics, 2)?,
            min_finality_threshold: topic_u32(topics, 3)?,
            amount: word_u256(data, 0)?,
            mint_recipient: word_b256(data, 1)?,
            destination_domain: word_u32(data, 2)?,
            max_fee: word_u256(data, 5)?,
        }
    } else if topic0 == CCTP_MINT_AND_WITHDRAW && log.address == contracts.cctp_token_messenger {
        Event::CctpMintAndWithdraw {
            recipient: topic_address(topics, 1)?,
            token: topic_address(topics, 2)?,
            amount: word_u256(data, 0)?,
            fee: word_u256(data, 1)?,
        }
    } else if topic0 == CCTP_MESSAGE_RECEIVED && log.address == contracts.cctp_message_transmitter {
        let body_offset = usize::try_from(word_u256(data, 2)?).ok()?;
        let body_len = data
            .get(body_offset..body_offset.checked_add(32)?)
            .map(U256::from_be_slice)
            .and_then(|v| usize::try_from(v).ok())?;
        let body_start = body_offset + 32;
        let body = data.get(body_start..body_start.checked_add(body_len)?)?;
        Event::CctpMessageReceived {
            source_domain: word_u32(data, 0)?,
            message_sender: burn_message_sender(body),
            finality_threshold: topic_u32(topics, 3)?,
        }
    } else if topic0 == GATEWAY_DEPOSITED && log.address == contracts.gateway_wallet {
        Event::GatewayDeposited {
            token: topic_address(topics, 1)?,
            depositor: topic_address(topics, 2)?,
            sender: topic_address(topics, 3)?,
            value: word_u256(data, 0)?,
        }
    } else if topic0 == GATEWAY_BURNED && log.address == contracts.gateway_wallet {
        Event::GatewayBurned {
            token: topic_address(topics, 1)?,
            depositor: topic_address(topics, 2)?,
            destination_domain: word_u32(data, 0)?,
            destination_recipient: word_b256(data, 1)?,
            signer: word_address(data, 2)?,
            value: word_u256(data, 3)?,
            fee: word_u256(data, 4)?,
        }
    } else if topic0 == GATEWAY_ATTESTATION_USED && log.address == contracts.gateway_minter {
        Event::GatewayAttestationUsed {
            token: topic_address(topics, 1)?,
            recipient: topic_address(topics, 2)?,
            source_domain: word_u32(data, 0)?,
            source_depositor: word_b256(data, 1)?,
            value: word_u256(data, 3)?,
        }
    } else {
        return None;
    };

    Some(Decoded {
        tx_hash: log.transaction_hash,
        log_index: u32::try_from(log.log_index.to::<u64>()).ok()?,
        block_number: log.block_number.to::<u64>(),
        event,
    })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub fn log(address: &str, topics: &[&str], data: &str, tx: &str, log_index: u64) -> RawLog {
        serde_json::from_value(serde_json::json!({
            "address": address,
            "topics": topics,
            "data": data,
            "blockNumber": "0x3ef8478",
            "blockTimestamp": "0x6ac6b7ca",
            "transactionHash": tx,
            "logIndex": format!("0x{log_index:x}"),
        }))
        .unwrap()
    }

    pub const TX_A: &str = "0xbc49fe0bc06cffe2a85264cde533a9f6e1232dbfccd7a26679bf72ebe6be5dad";

    /// Real Arc Testnet log: 1.02 USDC burned for Base via CCTP V2.
    pub fn deposit_for_burn() -> RawLog {
        log(
            "0x8fe6b999dc680ccfdd5bf7eb0974218be2542daa",
            &[
                "0x0c8c1cbdc5190613ebd485511d4e2812cfa45eecb79d845893331fedad5130a5",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000c5567a5e3370d4dbfb0540025078e283e36a363d",
                "0x00000000000000000000000000000000000000000000000000000000000003e8",
            ],
            "0x00000000000000000000000000000000000000000000000000000000000f90600000000000000000000000001474a77ac12fade821e9df4b5afca288849b2e8a00000000000000000000000000000000000000000000000000000000000000060000000000000000000000008fe6b999dc680ccfdd5bf7eb0974218be2542daa00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000031bcd00000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000020636374702d666f72776172640000000000000000000000000000000000000000",
            TX_A,
            7,
        )
    }

    /// Real Arc Mainnet log: 105 USDC burned for Base via CCTP V2.
    pub fn mainnet_deposit_for_burn() -> RawLog {
        log(
            "0x28b5a0e9c621a5badaa536219b3a228c8168cf5d",
            &[
                "0x0c8c1cbdc5190613ebd485511d4e2812cfa45eecb79d845893331fedad5130a5",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000b3fa262d0fb521cc93be83d87b322b8a23daf3f0",
                "0x00000000000000000000000000000000000000000000000000000000000003e8",
            ],
            "0x0000000000000000000000000000000000000000000000000000000006422c4000000000000000000000000082af7c2617c0d7b7454c601f7f4b7e1e06eb866e000000000000000000000000000000000000000000000000000000000000000600000000000000000000000028b5a0e9c621a5badaa536219b3a228c8168cf5d0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000d54f00000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000020636374702d666f72776172640000000000000000000000000000000000000000",
            "0xefed716c60d7e164064daeb46730fedd489173fdae0664a6fa1bf57288f5b426",
            20,
        )
    }

    /// Real Arc Mainnet log: Gateway burn of 26,220.606176 USDC for Avalanche.
    pub fn mainnet_gateway_burned() -> RawLog {
        log(
            "0x77777777dcc4d5a8b6e418fd04d8997ef11000ee",
            &[
                "0x12ee2719e7e2dec9f2a0041286b66669153dff0d36719f692b8bbaa4dfe0aa87",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000f70da97812cb96acdf810712aa562db8dfa3dbef",
                "0xf55c705e0ff1146b7eb6df0b0423614b6b68da8ab71472a4dd19fab142e663d9",
            ],
            "0x0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000f70da97812cb96acdf810712aa562db8dfa3dbef000000000000000000000000f70da97812cb96acdf810712aa562db8dfa3dbef000000000000000000000000000000000000000000000000000000061adeb2e00000000000000000000000000000000000000000000000000000000000000dac000000000000000000000000000000000000000000000000000000061adec08c0000000000000000000000000000000000000000000000000000000000000000",
            "0x7fc88ddfc8665716be8aba9cf0b98a6c808ab74d4cabb67986f7187706ec4411",
            2,
        )
    }

    /// Real Arc Testnet log: Gateway burn of 0.2 USDC, signer differs from depositor.
    pub fn gateway_burned() -> RawLog {
        log(
            "0x0077777d7eba4688bdef3e311b846f25870a19b9",
            &[
                "0x12ee2719e7e2dec9f2a0041286b66669153dff0d36719f692b8bbaa4dfe0aa87",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc7",
                "0xab94c8aa37fdc6c7bcd705534d050606777ad86cfe10e51682cd7275dbdbcf2d",
            ],
            "0x000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc700000000000000000000000010f48f71dee384425ac52ea3dbf5180a95d3f5ab0000000000000000000000000000000000000000000000000000000000030d400000000000000000000000000000000000000000000000000000000000004e790000000000000000000000000000000000000000000000000000000000035bb90000000000000000000000000000000000000000000000000000000000000000",
            "0x0ac7304575f550b70f544c4c3192674fc90efd9f2cff8ea761e6de884d66f445",
            3,
        )
    }

    /// Real Arc Testnet log: Gateway deposit of 0.3 USDC.
    pub fn gateway_deposited() -> RawLog {
        log(
            "0x0077777d7eba4688bdef3e311b846f25870a19b9",
            &[
                "0x4174a9435a04d04d274c76779cad136a41fde6937c56241c09ab9d3c7064a1a9",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc7",
                "0x000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc7",
            ],
            "0x00000000000000000000000000000000000000000000000000000000000493e0",
            "0x8aeb435d19b8354fe12e07c82f417a70225a0f0af062ba5452f477c684bf2a22",
            2,
        )
    }

    /// Real Arc Testnet log: Gateway attestation minting 0.2 USDC.
    pub fn attestation_used() -> RawLog {
        log(
            "0x0022222abe238cc2c7bb1f21003f0a260052475b",
            &[
                "0xbb312ce0cc311b2cb0746e09ccd2f91fdb9e2ac755d2f11c65300eb0d0fffd63",
                "0x0000000000000000000000003600000000000000000000000000000000000000",
                "0x000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc7",
                "0xab94c8aa37fdc6c7bcd705534d050606777ad86cfe10e51682cd7275dbdbcf2d",
            ],
            "0x000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000631fdaafe9ff675bf7a122013d1555513363dcc700000000000000000000000010f48f71dee384425ac52ea3dbf5180a95d3f5ab0000000000000000000000000000000000000000000000000000000000030d40",
            "0xeea4e0322de5c2a3125f3b0767f6b7293596e209a7953e78433d32b57dc39b5f",
            4,
        )
    }

    pub const SYSTEM_EMITTER: &str = "0xfffffffffffffffffffffffffffffffffffffffe";
    pub const TX_NATIVE: &str =
        "0x062f4aac0be3920a8ac7bba873d2e4b5c85525839cea5f64511d3971310910fe";
    pub const TX_TWIN: &str = "0xb2ce1544c5f5677aec06266e5383c9143eba7c2248d8f4adfd67c3804426b749";

    /// Real Arc Testnet log: 10.068033162188713984 USDC sent natively (as `msg.value`) to
    /// a swap router. The ERC-20 contract logs nothing for it.
    pub fn native_send() -> RawLog {
        log(
            SYSTEM_EMITTER,
            &[
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                "0x00000000000000000000000059825bb2aa01018adf67aa95741d23cae307431a",
                "0x000000000000000000000000e72f8175ab0991dbb778f6de62009c5bf97c17f7",
            ],
            "0x0000000000000000000000000000000000000000000000008bb8d6d17b1c7000",
            TX_NATIVE,
            0,
        )
    }

    /// Real Arc Testnet logs: one ERC-20 `transfer()` of 1.921835 USDC, which the system
    /// emitter (18 decimals) and the ERC-20 contract (6 decimals) both log.
    pub fn erc20_transfer_with_native_twin() -> [RawLog; 2] {
        let topics = [
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "0x0000000000000000000000000e8ffdbfd3a96171986ee53249426f08af86ddc2",
            "0x000000000000000000000000f50a3e9cabc8dd64019e5d1660718d3032236f26",
        ];
        [
            log(
                SYSTEM_EMITTER,
                &topics,
                "0x0000000000000000000000000000000000000000000000001aabbac00be9b000",
                TX_TWIN,
                0,
            ),
            log(
                "0x3600000000000000000000000000000000000000",
                &topics,
                "0x00000000000000000000000000000000000000000000000000000000001d532b",
                TX_TWIN,
                1,
            ),
        ]
    }

    pub fn transfer(token: &str, from: &str, to: &str, value: u64, tx: &str, idx: u64) -> RawLog {
        let pad = |a: &str| format!("0x{:0>64}", a.trim_start_matches("0x"));
        log(
            token,
            &[
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                &pad(from),
                &pad(to),
            ],
            &format!("0x{value:064x}"),
            tx,
            idx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use alloy_primitives::address;

    fn dec(log: &RawLog) -> Event {
        decode(log, &Contracts::arc_testnet(), None).unwrap().event
    }

    #[test]
    fn decodes_cctp_deposit_for_burn() {
        let Event::CctpDepositForBurn {
            token,
            depositor,
            amount,
            mint_recipient,
            destination_domain,
            min_finality_threshold,
            ..
        } = dec(&deposit_for_burn())
        else {
            panic!("wrong variant")
        };
        assert_eq!(token, address!("3600000000000000000000000000000000000000"));
        assert_eq!(
            depositor,
            address!("c5567a5e3370d4dbfb0540025078e283e36a363d")
        );
        assert_eq!(amount, U256::from(1_020_000u64));
        assert_eq!(destination_domain, 6);
        assert_eq!(min_finality_threshold, 1000);
        assert_eq!(
            bytes32_as_address(&mint_recipient),
            Some(address!("1474a77ac12fade821e9df4b5afca288849b2e8a"))
        );
    }

    #[test]
    fn decodes_gateway_events() {
        let Event::GatewayBurned {
            depositor,
            signer,
            destination_domain,
            value,
            fee,
            ..
        } = dec(&gateway_burned())
        else {
            panic!("wrong variant")
        };
        assert_eq!(
            depositor,
            address!("631fdaafe9ff675bf7a122013d1555513363dcc7")
        );
        assert_eq!(signer, address!("10f48f71dee384425ac52ea3dbf5180a95d3f5ab"));
        assert_eq!(destination_domain, 26);
        assert_eq!(value, U256::from(200_000u64));
        assert_eq!(fee, U256::from(0x4e79u64));

        let Event::GatewayDeposited { value, .. } = dec(&gateway_deposited()) else {
            panic!("wrong variant")
        };
        assert_eq!(value, U256::from(300_000u64));

        let Event::GatewayAttestationUsed {
            source_domain,
            value,
            ..
        } = dec(&attestation_used())
        else {
            panic!("wrong variant")
        };
        assert_eq!(source_domain, 26);
        assert_eq!(value, U256::from(200_000u64));
    }

    #[test]
    fn mainnet_logs_decode_only_with_mainnet_contracts() {
        let mainnet = Contracts::arc_mainnet();
        for log in [mainnet_deposit_for_burn(), mainnet_gateway_burned()] {
            assert!(decode(&log, &mainnet, None).is_some());
            // Same events at other addresses: the testnet set must not claim them.
            assert!(decode(&log, &Contracts::arc_testnet(), None).is_none());
        }

        let Event::CctpDepositForBurn {
            depositor,
            amount,
            destination_domain,
            ..
        } = decode(&mainnet_deposit_for_burn(), &mainnet, None)
            .unwrap()
            .event
        else {
            panic!("wrong variant")
        };
        assert_eq!(
            depositor,
            address!("b3fa262d0fb521cc93be83d87b322b8a23daf3f0")
        );
        assert_eq!(amount, U256::from(105_000_000u64));
        assert_eq!(destination_domain, 6);

        let Event::GatewayBurned {
            destination_domain,
            value,
            fee,
            ..
        } = decode(&mainnet_gateway_burned(), &mainnet, None)
            .unwrap()
            .event
        else {
            panic!("wrong variant")
        };
        assert_eq!(destination_domain, 1);
        assert_eq!(value, U256::from(26_220_606_176u64));
        assert_eq!(fee, U256::from(3_500u64));
    }

    #[test]
    fn decodes_native_usdc_sends_at_18_decimals() {
        let Event::NativeTransfer { from, to, value } = dec(&native_send()) else {
            panic!("wrong variant")
        };
        assert_eq!(from, address!("59825bb2aa01018adf67aa95741d23cae307431a"));
        assert_eq!(to, address!("e72f8175ab0991dbb778f6de62009c5bf97c17f7"));
        assert_eq!(value, U256::from(10_068_033_162_188_713_984u128));
    }

    #[test]
    fn decodes_transfers_from_any_token_and_nfts() {
        let token = "0x52159c27e5da9a7643125ffab46053637c2a88de";
        let log = transfer(
            token,
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
            42,
            TX_A,
            1,
        );
        let Event::Transfer {
            token: t, value, ..
        } = dec(&log)
        else {
            panic!("wrong variant")
        };
        assert_eq!(format!("{t:#x}"), token);
        assert_eq!(value, U256::from(42u64));

        // ERC-721: the token id is the fourth topic and data is empty.
        let mut nft = log.clone();
        nft.topics.push(B256::from(U256::from(7u64)));
        nft.data = Bytes::new();
        let Event::NftTransfer { token_id, .. } = dec(&nft) else {
            panic!("wrong variant")
        };
        assert_eq!(token_id, U256::from(7u64));
        assert_eq!(
            dec(&nft).token().map(|(_, s)| s),
            Some(crate::tokens::Standard::Erc721)
        );

        // Two topics is neither standard.
        let mut odd = log;
        odd.topics.truncate(2);
        assert!(decode(&odd, &Contracts::arc_testnet(), None).is_none());
    }

    #[test]
    fn ignores_events_from_untracked_contracts() {
        let mut log = gateway_deposited();
        log.address = address!("00000000000000000000000000000000000000aa");
        assert!(decode(&log, &Contracts::arc_testnet(), None).is_none());
    }

    #[test]
    fn truncated_data_is_rejected_not_panicked() {
        let mut log = deposit_for_burn();
        log.data = log.data.slice(..40);
        assert!(decode(&log, &Contracts::arc_testnet(), None).is_none());
    }

    #[test]
    fn confidential_topic_yields_shielded_transfer() {
        let topic = b256!("00000000000000000000000000000000000000000000000000000000c0ffee00");
        let mut log = transfer(
            "0x3600000000000000000000000000000000000000",
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000b2",
            0,
            TX_A,
            1,
        );
        log.topics[0] = topic;
        log.data = Bytes::from_static(&[0xde, 0xad]);
        let decoded = decode(&log, &Contracts::arc_testnet(), Some(topic)).unwrap();
        assert!(matches!(decoded.event, Event::ShieldedTransfer { .. }));
        // Without the topic configured the same log is simply not tracked.
        assert!(decode(&log, &Contracts::arc_testnet(), None).is_none());
    }

    use alloy_primitives::Bytes;
}
