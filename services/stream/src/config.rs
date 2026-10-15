//! Runtime configuration, read once from the environment.

use alloy_primitives::{address, Address, B256};

use crate::tokens::PriceSource;

/// A tracked stablecoin on Arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub symbol: &'static str,
    pub address: Address,
    pub decimals: u32,
}

/// The Arc network a deployment indexes. Each network gets its own database: the guard in
/// `db::bind_network` refuses to mix them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn key(self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Testnet => "testnet",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Network::Mainnet => "Arc Mainnet",
            Network::Testnet => "Arc Testnet",
        }
    }

    pub fn chain_id(self) -> u64 {
        match self {
            Network::Mainnet => 5042,
            Network::Testnet => 5042002,
        }
    }

    pub fn from_chain_id(chain_id: u64) -> Option<Self> {
        [Network::Mainnet, Network::Testnet]
            .into_iter()
            .find(|n| n.chain_id() == chain_id)
    }

    pub fn explorer_url(self) -> &'static str {
        match self {
            Network::Mainnet => "https://explorer.arc.io",
            Network::Testnet => "https://explorer.testnet.arc.io",
        }
    }

    /// Public endpoints from docs.arc.io. More than one, because a public endpoint
    /// rate-limits and can ban a host outright; set `ARC_RPC_URL` to a keyed endpoint for
    /// production.
    fn default_rpc_urls(self) -> &'static str {
        match self {
            Network::Mainnet => {
                "https://rpc.mainnet.arc.io,https://rpc.drpc.mainnet.arc.io,\
https://rpc.blockdaemon.mainnet.arc.io,https://rpc.quicknode.mainnet.arc.io"
            }
            Network::Testnet => {
                "https://rpc.testnet.arc.io,https://rpc.drpc.testnet.arc.io,\
https://rpc.blockdaemon.testnet.arc.io"
            }
        }
    }

    pub fn contracts(self) -> Contracts {
        match self {
            Network::Mainnet => Contracts::arc_mainnet(),
            Network::Testnet => Contracts::arc_testnet(),
        }
    }
}

impl std::str::FromStr for Network {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            other => anyhow::bail!("ARC_NETWORK must be mainnet or testnet, not {other:?}"),
        }
    }
}

/// The contracts Tideline decodes on one network. Mirrors `@tideline/arc` addresses.
#[derive(Debug, Clone)]
pub struct Contracts {
    pub tokens: Vec<Token>,
    /// Arc's system emitter (EIP-7708): logs every native USDC movement at 18 decimals,
    /// including the plain native sends that the ERC-20 interface never logs.
    pub native_usdc_emitter: Address,
    pub cctp_token_messenger: Address,
    pub cctp_message_transmitter: Address,
    pub cctp_token_minter: Address,
    pub gateway_wallet: Address,
    pub gateway_minter: Address,
    pub usyc_teller: Address,
    pub fx_escrow: Address,
}

impl Contracts {
    /// Arc Mainnet (chain id 5042), from docs.arc.io contract addresses.
    pub fn arc_mainnet() -> Self {
        Self {
            tokens: vec![
                Token {
                    symbol: "USDC",
                    address: address!("3600000000000000000000000000000000000000"),
                    decimals: 6,
                },
                Token {
                    symbol: "EURC",
                    address: address!("bEf5f6d51CB62b58e6A8f77868681825C6fe21c1"),
                    decimals: 6,
                },
                Token {
                    symbol: "USYC",
                    address: address!("8a5D989Bbb96929F689B0200f435f53dA42bF490"),
                    decimals: 6,
                },
            ],
            native_usdc_emitter: address!("fffffffffffffffffffffffffffffffffffffffe"),
            cctp_token_messenger: address!("28b5a0e9C621a5BadaA536219b3a228C8168cf5d"),
            cctp_message_transmitter: address!("81D40F21F12A8F0E3252Bccb954D722d4c464B64"),
            cctp_token_minter: address!("fd78EE919681417d192449715b2594ab58f5D002"),
            gateway_wallet: address!("77777777Dcc4d5A8B6E418Fd04D8997ef11000eE"),
            gateway_minter: address!("2222222d7164433c4C09B0b0D809a9b52C04C205"),
            usyc_teller: address!("51A8CE47dC08ba5CD19c7aa84EA6fD6664f60f9b"),
            fx_escrow: address!("e2E5F173576B513d994073CCbDaCBE027d43DFe6"),
        }
    }

    /// Arc Testnet (chain id 5042002).
    pub fn arc_testnet() -> Self {
        Self {
            tokens: vec![
                Token {
                    symbol: "USDC",
                    address: address!("3600000000000000000000000000000000000000"),
                    decimals: 6,
                },
                Token {
                    symbol: "EURC",
                    address: address!("89B50855Aa3bE2F677cD6303Cec089B5F319D72a"),
                    decimals: 6,
                },
                Token {
                    symbol: "USYC",
                    address: address!("e9185F0c5F296Ed1797AaE4238D26CCaBEadb86C"),
                    decimals: 6,
                },
            ],
            native_usdc_emitter: address!("fffffffffffffffffffffffffffffffffffffffe"),
            cctp_token_messenger: address!("8FE6B999Dc680CcFDD5Bf7EB0974218be2542DAA"),
            cctp_message_transmitter: address!("E737e5cEBEEBa77EFE34D4aa090756590b1CE275"),
            cctp_token_minter: address!("b43db544E2c27092c107639Ad201b3dEfAbcF192"),
            gateway_wallet: address!("0077777d7EBA4688BDeF3E311b846F25870A19B9"),
            gateway_minter: address!("0022222ABE238Cc2C7Bb1f21003F0a260052475B"),
            usyc_teller: address!("9fdF14c5B14173D74C08Af27AebFf39240dC105A"),
            fx_escrow: address!("d68256f4D69C6BbEcB873D8588AE0Dc6B8E22E10"),
        }
    }

    pub fn token(&self, address: &Address) -> Option<&Token> {
        self.tokens.iter().find(|t| &t.address == address)
    }

    /// The ERC-20 interface over the native USDC balance.
    pub fn usdc(&self) -> Option<&Token> {
        self.tokens.iter().find(|t| t.symbol == "USDC")
    }

    /// Every contract whose logs the ingestor subscribes to.
    pub fn watched(&self) -> Vec<Address> {
        let mut out: Vec<Address> = self.tokens.iter().map(|t| t.address).collect();
        out.extend([
            self.native_usdc_emitter,
            self.cctp_token_messenger,
            self.cctp_message_transmitter,
            self.gateway_wallet,
            self.gateway_minter,
        ]);
        out
    }
}

/// Reference rates used to express listed non-USD assets as notional USD. The configured
/// EURC and USYC values are the fallback; `rates::LiveRates` replaces them with published
/// ones while it can. Bitcoin and ether move too fast for a static fallback, so cirBTC and
/// WETH stay unpriced until a spot source answers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReferenceRates {
    pub eurc_usd: f64,
    pub usyc_usd: f64,
    pub btc_usd: Option<f64>,
    pub eth_usd: Option<f64>,
}

impl ReferenceRates {
    /// USD per whole token for a price source; `None` while the rate is unknown.
    pub fn price(&self, source: PriceSource) -> Option<f64> {
        match source {
            PriceSource::Usd => Some(1.0),
            PriceSource::Eur => Some(self.eurc_usd),
            PriceSource::Usyc => Some(self.usyc_usd),
            PriceSource::Btc => self.btc_usd,
            PriceSource::Eth => self.eth_usd,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub network: Network,
    pub database_url: String,
    /// Arc RPC endpoints, tried in order. The ingestor fails over to the next on error.
    pub rpc_urls: Vec<String>,
    pub port: u16,
    /// Run the chain ingestor alongside the API.
    pub ingest_enabled: bool,
    /// Ingest transfers of every token on the network. Off: listed tokens only.
    pub track_all_tokens: bool,
    /// Free CoinGecko demo key for the cirBTC and WETH spot prices. Without it the
    /// keyless API is tried, then Coinbase.
    pub coingecko_api_key: Option<String>,
    /// Seconds an aggregate response (overview, coverage, assets) is served from memory
    /// before it is refreshed in the background. 0 disables the cache.
    pub cache_seconds: u64,
    /// Delete flows older than this many hours, checked hourly. `None`: keep everything.
    /// With every token tracked, Arc Mainnet adds about 1 GB of flows a day.
    pub retention_hours: Option<u32>,
    /// Blocks to backfill behind the head on first start (no stored cursor).
    pub backfill_blocks: u64,
    /// Blocks to stay behind the head. Arc has deterministic finality, so 0 is safe.
    pub confirmations: u64,
    pub max_chunk_blocks: u64,
    pub poll_interval_ms: u64,
    pub rates: ReferenceRates,
    /// Refresh `rates` from published sources. Off: the configured values are used as is.
    pub live_rates: bool,
    /// Bootstrap admin API key. When unset, admin endpoints are unreachable until a key
    /// is minted with `keys create`.
    pub admin_key: Option<String>,
    /// topic0 of Arc's confidential-transfer event. Arc's opt-in privacy is not live yet
    /// and the event is unpublished, so this stays unset until it is; once set, matching
    /// logs on tracked tokens are ingested as amount-shielded flows.
    pub confidential_transfer_topic: Option<B256>,
    pub contracts: Contracts,
}

/// A comma-separated endpoint list, blanks dropped.
fn parse_rpc_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .collect()
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set (see .env.example)"))?;
        let confidential_transfer_topic = match std::env::var("CONFIDENTIAL_TRANSFER_TOPIC") {
            Ok(v) if !v.trim().is_empty() => Some(
                v.trim()
                    .parse::<B256>()
                    .map_err(|e| anyhow::anyhow!("CONFIDENTIAL_TRANSFER_TOPIC: {e}"))?,
            ),
            _ => None,
        };
        let network = match std::env::var("ARC_NETWORK") {
            Ok(v) if !v.trim().is_empty() => v.parse::<Network>()?,
            _ => Network::Mainnet,
        };
        let mut rpc_urls = parse_rpc_urls(&std::env::var("ARC_RPC_URL").unwrap_or_default());
        // Before the network switch the endpoint list was testnet-only, and named for it.
        let legacy = parse_rpc_urls(&std::env::var("ARC_TESTNET_RPC_URL").unwrap_or_default());
        if rpc_urls.is_empty() && !legacy.is_empty() {
            if network == Network::Testnet {
                rpc_urls = legacy;
            } else {
                tracing::warn!(
                    "ignoring ARC_TESTNET_RPC_URL: ARC_NETWORK is {}",
                    network.key()
                );
            }
        }
        if rpc_urls.is_empty() {
            rpc_urls = parse_rpc_urls(network.default_rpc_urls());
        }
        Ok(Self {
            network,
            database_url,
            rpc_urls,
            port: env_parse("PORT", 8080),
            ingest_enabled: env_parse("INGEST_ENABLED", true),
            track_all_tokens: env_parse("TRACK_ALL_TOKENS", true),
            cache_seconds: env_parse("CACHE_SECONDS", 30),
            coingecko_api_key: std::env::var("COINGECKO_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty()),
            // RETENTION_HOURS wins; RETENTION_DAYS is the coarse form.
            retention_hours: Some(env_parse("RETENTION_HOURS", 0))
                .filter(|h| *h > 0)
                .or_else(|| Some(env_parse("RETENTION_DAYS", 0u32) * 24).filter(|h| *h > 0)),
            backfill_blocks: env_parse("BACKFILL_BLOCKS", 600),
            confirmations: env_parse("CONFIRMATIONS", 0),
            max_chunk_blocks: env_parse("MAX_CHUNK_BLOCKS", 100).max(1),
            poll_interval_ms: env_parse("POLL_INTERVAL_MS", 500),
            rates: ReferenceRates {
                eurc_usd: env_parse("EURC_USD_RATE", 1.08),
                usyc_usd: env_parse("USYC_USD_RATE", 1.10),
                btc_usd: None,
                eth_usd: None,
            },
            live_rates: env_parse("LIVE_RATES", true),
            admin_key: std::env::var("TIDELINE_ADMIN_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty()),
            confidential_transfer_topic,
            contracts: network.contracts(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_url_lists_are_split_and_trimmed() {
        assert_eq!(
            parse_rpc_urls(" https://a.example , ,https://b.example/v2/key "),
            ["https://a.example", "https://b.example/v2/key"]
        );
        assert!(parse_rpc_urls("").is_empty());
        assert_eq!(parse_rpc_urls(Network::Mainnet.default_rpc_urls()).len(), 4);
        assert_eq!(parse_rpc_urls(Network::Testnet.default_rpc_urls()).len(), 3);
    }

    #[test]
    fn networks_parse_and_round_trip_their_chain_id() {
        assert_eq!(" Mainnet ".parse::<Network>().unwrap(), Network::Mainnet);
        assert_eq!("testnet".parse::<Network>().unwrap(), Network::Testnet);
        assert!("sepolia".parse::<Network>().is_err());
        for n in [Network::Mainnet, Network::Testnet] {
            assert_eq!(Network::from_chain_id(n.chain_id()), Some(n));
            for url in parse_rpc_urls(n.default_rpc_urls()) {
                assert!(url.contains(n.key()), "{url} is not a {} endpoint", n.key());
            }
        }
    }

    #[test]
    fn mainnet_and_testnet_share_only_usdc_and_the_system_emitter() {
        let (main, test) = (Contracts::arc_mainnet(), Contracts::arc_testnet());
        let shared: Vec<_> = main
            .watched()
            .into_iter()
            .filter(|a| test.watched().contains(a))
            .collect();
        assert_eq!(
            shared,
            [main.usdc().unwrap().address, main.native_usdc_emitter]
        );
        assert_ne!(main.usyc_teller, test.usyc_teller);
        assert_ne!(main.fx_escrow, test.fx_escrow);
    }
}
