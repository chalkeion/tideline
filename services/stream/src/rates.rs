//! Reference rates that express listed non-USD assets as notional USD.
//!
//! EURC is valued at the ECB euro reference rate, USYC at the fund's latest published price
//! report, and cirBTC and WETH at the bitcoin and ether spot prices. All are refreshed in
//! the background. The configured static EURC and USYC rates apply until a source answers;
//! cirBTC and WETH stay unpriced until then. A rate keeps its last good value if its source
//! stops.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::ReferenceRates;
use crate::schema::ReferenceRatesInfo;

const ECB_URL: &str = "https://api.frankfurter.dev/v1/latest?base=EUR&symbols=USD";
const USYC_URL: &str = "https://usyc.hashnote.com/api/price-reports";
const SPOT_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum&vs_currencies=usd&include_last_updated_at=true";
const ECB_SOURCE: &str = "ECB euro reference rate";
const SPOT_SOURCE: &str = "CoinGecko spot price";
const USYC_SOURCE: &str = "USYC fund price report";
const STATIC_SOURCE: &str = "configured static rate";

/// The ECB publishes once per working day and USYC reprices daily. Spot prices move
/// faster, but a flow is valued once, when ingested, so hourly is the honest resolution
/// within the free API's rate limit.
const REFRESH: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
struct Provenance {
    source: &'static str,
    /// Publication date (YYYY-MM-DD). None for a static rate.
    as_of: Option<String>,
}

impl Provenance {
    fn fixed() -> Self {
        Self {
            source: STATIC_SOURCE,
            as_of: None,
        }
    }

    fn unknown() -> Self {
        Self {
            source: "not yet available",
            as_of: None,
        }
    }
}

struct State {
    rates: ReferenceRates,
    eurc: Provenance,
    usyc: Provenance,
    spot: Provenance,
}

/// The rates in force, shared by the ingestor (which values flows) and the API.
pub struct LiveRates {
    state: RwLock<State>,
    http: reqwest::Client,
}

/// `(EUR/USD, publication date)` from a Frankfurter "latest" response.
fn parse_ecb(body: &Value) -> Option<(f64, String)> {
    let rate = body["rates"]["USD"].as_f64()?;
    let date = body["date"].as_str()?.to_string();
    // A value this far from parity is a broken response, not a rate.
    (0.5..2.0).contains(&rate).then_some((rate, date))
}

/// `(price, report date)` of the newest round in a USYC price-report response.
fn parse_usyc(body: &Value) -> Option<(f64, String)> {
    let (timestamp, price) = body["data"]
        .as_array()?
        .iter()
        .filter_map(|round| {
            let timestamp: i64 = round["timestamp"].as_str()?.parse().ok()?;
            let price: f64 = round["price"].as_str()?.parse().ok()?;
            Some((timestamp, price))
        })
        .max_by_key(|(timestamp, _)| *timestamp)?;
    let date = DateTime::<Utc>::from_timestamp(timestamp, 0)?
        .format("%Y-%m-%d")
        .to_string();
    (0.9..5.0).contains(&price).then_some((price, date))
}

/// `(BTC/USD, ETH/USD, last update)` from a CoinGecko simple-price response.
fn parse_spot(body: &Value) -> Option<(f64, f64, String)> {
    let btc = body["bitcoin"]["usd"].as_f64()?;
    let eth = body["ethereum"]["usd"].as_f64()?;
    let updated = body["bitcoin"]["last_updated_at"].as_i64()?;
    let at = DateTime::<Utc>::from_timestamp(updated, 0)?
        .format("%Y-%m-%d %H:%M UTC")
        .to_string();
    // Orders of magnitude off is a broken response, not a market.
    ((1_000.0..10_000_000.0).contains(&btc) && (10.0..1_000_000.0).contains(&eth))
        .then_some((btc, eth, at))
}

impl LiveRates {
    /// Starts on the configured rates; nothing is fetched until [`Self::refresh`].
    pub fn new(configured: ReferenceRates) -> anyhow::Result<Self> {
        Ok(Self {
            state: RwLock::new(State {
                rates: configured,
                eurc: Provenance::fixed(),
                usyc: Provenance::fixed(),
                spot: Provenance::unknown(),
            }),
            http: reqwest::Client::builder()
                .gzip(true)
                .timeout(Duration::from_secs(15))
                .user_agent(concat!("tideline/", env!("CARGO_PKG_VERSION")))
                .build()?,
        })
    }

    pub fn current(&self) -> ReferenceRates {
        self.state.read().unwrap().rates
    }

    pub fn info(&self) -> ReferenceRatesInfo {
        let state = self.state.read().unwrap();
        ReferenceRatesInfo {
            eurc_usd: state.rates.eurc_usd,
            usyc_usd: state.rates.usyc_usd,
            eurc_source: state.eurc.source.to_string(),
            eurc_as_of: state.eurc.as_of.clone(),
            usyc_source: state.usyc.source.to_string(),
            usyc_as_of: state.usyc.as_of.clone(),
            btc_usd: state.rates.btc_usd,
            eth_usd: state.rates.eth_usd,
            spot_source: state.spot.source.to_string(),
            spot_as_of: state.spot.as_of.clone(),
            note: "USD values are notional: each flow is valued at the reference rate in \
                   force when it was ingested (USDC = 1.00). Only listed tokens are priced; \
                   other tokens carry exact amounts and no USD value."
                .into(),
        }
    }

    async fn fetch(&self, url: &str) -> Result<Value, reqwest::Error> {
        self.http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    /// Fetch every source once. A source that fails or answers nonsense leaves its rate as
    /// it was.
    pub async fn refresh(&self) {
        let (ecb, usyc, spot) = tokio::join!(
            self.fetch(ECB_URL),
            self.fetch(USYC_URL),
            self.fetch(SPOT_URL)
        );
        let spot = spot
            .map_err(|e| e.to_string())
            .and_then(|body| parse_spot(&body).ok_or_else(|| "unexpected response".to_string()));
        let ecb = ecb
            .map_err(|e| e.to_string())
            .and_then(|body| parse_ecb(&body).ok_or_else(|| "unexpected response".to_string()));
        let usyc = usyc
            .map_err(|e| e.to_string())
            .and_then(|body| parse_usyc(&body).ok_or_else(|| "unexpected response".to_string()));

        let mut state = self.state.write().unwrap();
        match ecb {
            Ok((rate, date)) => {
                state.rates.eurc_usd = rate;
                state.eurc = Provenance {
                    source: ECB_SOURCE,
                    as_of: Some(date),
                };
            }
            Err(e) => tracing::warn!("EURC reference rate not refreshed: {e}"),
        }
        match usyc {
            Ok((price, date)) => {
                state.rates.usyc_usd = price;
                state.usyc = Provenance {
                    source: USYC_SOURCE,
                    as_of: Some(date),
                };
            }
            Err(e) => tracing::warn!("USYC reference rate not refreshed: {e}"),
        }
        match spot {
            Ok((btc, eth, at)) => {
                state.rates.btc_usd = Some(btc);
                state.rates.eth_usd = Some(eth);
                state.spot = Provenance {
                    source: SPOT_SOURCE,
                    as_of: Some(at),
                };
            }
            Err(e) => tracing::warn!("BTC and ETH spot prices not refreshed: {e}"),
        }
    }

    /// Refresh on a timer until the task is dropped.
    pub async fn run(self: Arc<Self>) {
        loop {
            tokio::time::sleep(REFRESH).await;
            self.refresh().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn ecb_rate_is_read_and_sanity_checked() {
        let body = json!({ "amount": 1.0, "base": "EUR", "date": "2026-10-08", "rates": { "USD": 1.1186 } });
        assert_eq!(parse_ecb(&body), Some((1.1186, "2026-10-08".to_string())));
        assert_eq!(
            parse_ecb(&json!({ "date": "2026-10-08", "rates": { "USD": 118.6 } })),
            None
        );
        assert_eq!(parse_ecb(&json!({ "rates": {} })), None);
    }

    #[test]
    fn spot_prices_are_read_and_sanity_checked() {
        let body = json!({
            "bitcoin": { "usd": 82404.0, "last_updated_at": 1791979200 },
            "ethereum": { "usd": 2461.28, "last_updated_at": 1791979200 },
        });
        let (btc, eth, at) = parse_spot(&body).unwrap();
        assert_eq!((btc, eth), (82404.0, 2461.28));
        assert!(at.ends_with("UTC"), "{at}");
        let broken = json!({
            "bitcoin": { "usd": 0.5, "last_updated_at": 1791979200 },
            "ethereum": { "usd": 2461.28 },
        });
        assert_eq!(parse_spot(&broken), None);
    }

    #[test]
    fn usyc_price_comes_from_the_newest_round() {
        let body = json!({ "data": [
            { "roundId": "528", "price": "1.1395", "timestamp": "1791376391" },
            { "roundId": "529", "price": "1.13963880332885057", "timestamp": "1791462791" },
            { "roundId": "x", "price": "not a number", "timestamp": "1791549191" },
        ] });
        let (price, date) = parse_usyc(&body).unwrap();
        assert!((price - 1.1396388).abs() < 1e-6);
        assert_eq!(date, "2026-10-08");
        assert_eq!(parse_usyc(&json!({ "data": [] })), None);
        assert_eq!(
            parse_usyc(&json!({ "data": [{ "price": "0", "timestamp": "1791462791" }] })),
            None
        );
    }

    #[test]
    fn configured_rates_apply_until_a_source_answers() {
        let configured = ReferenceRates {
            eurc_usd: 1.08,
            usyc_usd: 1.10,
            btc_usd: None,
            eth_usd: None,
        };
        let rates = LiveRates::new(configured).unwrap();
        assert_eq!(rates.current(), configured);
        let info = rates.info();
        assert_eq!(info.eurc_source, STATIC_SOURCE);
        assert_eq!(info.usyc_as_of, None);
        // No static spot price: cirBTC and WETH wait for a source.
        assert_eq!(rates.current().price(crate::tokens::PriceSource::Btc), None);
    }
}
