# Tideline

## Overview

Tideline is the attribution layer for value flow on [Arc](https://docs.arc.io), Circle's
stablecoin-native L1. It ingests every token transfer on the chain (ERC-20 and ERC-721)
alongside CCTP V2 and Gateway activity, collapses raw logs into economic flows, and ties each
counterparty to a known entity with evidence.

It is built for a compliance officer's question, not for chart browsing:

> Show me every stablecoin flow touching this entity in the last 24 hours, and prove the
> attribution.

The positioning behind this is in [docs/MOAT-STRATEGY.md](docs/MOAT-STRATEGY.md).

## What it does

| Moat | Where it lives |
| --- | --- |
| **Attribution registry.** Entities, and per-address claims that each carry a method (`protocol_registry`, `self_attested`, `curated`, `heuristic`), a confidence and evidence. | `services/stream/src/db.rs`, `data/entities.seed.json` |
| **Clustering.** A Gateway burn signed by a different key links signer and depositor; a first-party attribution on one propagates to the other as a lower-confidence heuristic label, with the linking tx as evidence. | `enrich.rs` (links), `db.rs::propagate_links` |
| **Every asset, identified by address.** Any token that emits a transfer is discovered, its metadata read from chain and its flows stored in its own decimals. Only *listed* tokens (addresses Arc or the issuer publishes) own their symbol and carry a price; the rest are tracked by amount. A token named like a listed one is flagged, in the API, the UI and every narrative, because on Arc Mainnet several tokens call themselves USDC. | `tokens.rs`, `data/tokens.seed.json`, `/v1/assets` |
| **Decode + enrich.** A CCTP burn is one `cctp_outbound` flow from its real originator (forwarder contracts are seen through), not three token transfers. Each flow gets a size band and a plain-language narrative. | `decode.rs`, `enrich.rs` |
| **Privacy-model aware.** Arc confidential transfers hide amounts, not addresses. Shielded flows are stored with a null amount, still attributed by counterparty, counted separately, and never treated as zero volume. A disclosure API re-admits amounts revealed by a view-key holder. | `visibility` on flows, `POST /v1/disclosures`, `/coverage` |
| **Falsifiable claim.** "X% of live volume attributed within 5 seconds of settlement", recomputed from stored flows on every request, with latency measured only on blocks produced while tip-following. | `aggregate.rs::coverage`, `/coverage` |
| **Data license, not a token.** Public, licensed and admin API tiers; hashed keys; per-key daily metering; compliance report in JSON and CSV; live SSE stream. | `api.rs`, `/license` |

### What is and is not in the registry

The seed registry contains Arc's own protocol contracts on each network (ground truth from
the Arc docs) and the institutions publicly named as Arc participants. **Those institutions
ship with no addresses.** Tideline never guesses an attribution: an institution stays on the watchlist
until an address is attributed by signed self-attestation or by curation with evidence, and
its history is relabelled the moment that happens. Building that dataset is the proprietary
work; the `/coverage` page lists the highest-volume unattributed addresses as a worklist.

Arc's opt-in privacy is on Arc's roadmap and not live. The confidential-transfer event is
unpublished, so `CONFIDENTIAL_TRANSFER_TOPIC` stays empty until it ships; disclosures are
recorded as operator-asserted until Arc publishes a verifiable view-key proof format.

## Architecture

```
Arc RPC ──eth_getLogs──▶ ingest ─▶ decode ─▶ enrich ─▶ Postgres ◀─ attribution registry
                                                          │
                                   axum API (tiers, metering, SSE)
                                                          │
                              @tideline/sdk ─▶ Next.js app (server-side key)
```

- `services/stream` - Rust: ingestor, decoder, enrichment, registry, API.
- `apps/web` - Next.js 16 analyst UI.
- `packages/sdk` - typed REST client. `packages/schema` - types generated from Rust by ts-rs.
- `packages/arc` - chain config and addresses. `packages/ui`, `packages/config` - shared.

## Run it

Prereqs: Node 22+, pnpm 9+, Rust stable, Postgres 14+.

```bash
cp .env.example .env            # set TIDELINE_ADMIN_KEY and TIDELINE_API_KEY (same value is fine locally)
podman run -d --name tideline-pg -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=tideline \
  -p 5432:5432 docker.io/library/postgres:17-alpine     # or docker

pnpm install
(cd services/stream && cargo run --release)   # migrates, seeds, ingests, serves :8080
pnpm --filter @tideline/web dev                # http://localhost:3000
```

Both the service and the web app read the root `.env`.

### Data sources

- **USDC.** USDC is Arc's native asset. The ERC-20 contract logs only calls made through
  its interface, so Tideline also reads Arc's system emitter (EIP-7708), which logs every
  movement, and keeps the native sends that have no ERC-20 twin. Nothing is counted twice.
- **Network.** `ARC_NETWORK` is `mainnet` (chain 5042, the default) or `testnet` (chain
  5042002). The decoded contracts, the protocol entries in the seed registry, explorer links
  and default endpoints all follow it, and `GET /v1/network` reports it. A database is bound
  to the network it first indexes and the service will not start against the other, so run
  one service and one database per network.
- **Chain.** `ARC_RPC_URL` takes a comma-separated list; empty means the public endpoints
  for `ARC_NETWORK`. At startup every endpoint must report the expected chain id. The
  ingestor uses one endpoint at a time and fails over to the next on any error, so a public
  endpoint that rate-limits or bans the host does not stop ingestion. Endpoint URLs never
  reach the logs or the API, so a keyed URL is safe to use. On testnet the older
  `ARC_TESTNET_RPC_URL` is still read.
- **Tokens.** With `TRACK_ALL_TOKENS=true` (the default) the ingestor reads every `Transfer`
  log on the chain, not only listed contracts. A new token's `name()`, `symbol()` and
  `decimals()` are read once, in small batches (public endpoints rate-limit inside a
  batch), and stored in `tokens`; a call that reverts is recorded as missing, any other
  failure is retried. `TRACK_ALL_TOKENS=false` limits ingestion to listed tokens.
- **Reference rates.** Listed tokens only. EURC is valued at the ECB euro reference rate,
  USYC at the fund's published price report, and cirBTC and WETH at CoinGecko's bitcoin and
  ether spot prices, all refreshed hourly and applied when a flow is ingested.
  `EURC_USD_RATE` and `USYC_USD_RATE` apply until a source answers; cirBTC and WETH stay
  unpriced until then. `LIVE_RATES=false` pins EURC and USYC and leaves cirBTC and WETH
  unpriced. `/v1/overview` reports each rate with its source and date. Unlisted tokens are
  never valued: no rate Tideline could defend exists for them.
- **History.** A fresh database holds `BACKFILL_BLOCKS` of history (172800 is one day).
  Windowed responses carry a `scope` that says where history starts, and the web app says
  so whenever a window reaches back further than the data does.

### Curate the registry

```bash
# Curated attribution (evidence is mandatory)
curl -X POST localhost:8080/v1/attributions -H "Authorization: Bearer $TIDELINE_ADMIN_KEY" \
  -H 'content-type: application/json' \
  -d '{"entity_slug":"visa","address":"0x...","label":"Settlement treasury","confidence":0.9,
       "source":"Public disclosure","evidence":{"url":"https://...","note":"..."}}'

# Self-attestation: the owner signs (EIP-191) the message
#   Tideline attribution: address <lowercase address> is controlled by <entity slug>.
curl -X POST localhost:8080/v1/attestations -H "Authorization: Bearer $TIDELINE_ADMIN_KEY" \
  -H 'content-type: application/json' \
  -d '{"entity_slug":"visa","address":"0x...","signature":"0x..."}'

# Issue a data-license key
(cd services/stream && cargo run -- keys create "Risk desk" licensed)
```

## API

| Endpoint | Tier | |
| --- | --- | --- |
| `GET /v1/overview`, `/v1/coverage` | public | Aggregates and the headline claim. `?window_hours=` |
| `GET /v1/flows`, `/v1/flows/:id` | public | Filters: `asset kind entity address visibility min_usd since_hours attributed before after limit`. `asset` is a token address, or the symbol of a listed token |
| `GET /v1/assets`, `/v1/assets/:address` | public | Every listed token and every token that moved, with flows, amount moved, USD volume (listed only) and the most active addresses. `?window_hours=` |
| `GET /v1/entities`, `/v1/entities/:slug`, `/v1/addresses/:address` | public | Evidence fields are licensed |
| `GET /v1/compliance/report?entity=&window_hours=&format=csv` | licensed | The compliance question |
| `GET /v1/flows/stream` | licensed | Server-sent events |
| `POST /v1/attributions`, `/v1/attestations`, `/v1/disclosures`, `/v1/keys` | admin | Registry curation |

## Deploy

Three parts: Postgres, the stream service (one process: ingestor and API), and the web app.
The service needs a host that keeps it running (it is an ingestor, not a request handler);
the web app runs anywhere Next.js does. One working setup is Railway for Postgres and the
service, and Vercel for the web app, both deploying from this repository.

**1. Postgres and the stream service (Railway).**

1. New project, then add a PostgreSQL database.
2. Add a service from this GitHub repo. Set its root directory to `services/stream`; it
   builds from the Dockerfile, with the health check from `services/stream/railway.json`.
   Migrations and the seed registry are compiled in.
3. Variables on the service:

   | Variable | Value |
   | --- | --- |
   | `DATABASE_URL` | `${{Postgres.DATABASE_URL}}` |
   | `TIDELINE_ADMIN_KEY` | a long random value (`openssl rand -hex 32`) |
   | `ARC_NETWORK` | `mainnet` |
   | `RETENTION_DAYS` | `3`: with every token tracked, Arc Mainnet adds about 1 GB of flows a day |
   | `BACKFILL_BLOCKS` | `7200` (one hour; 172800 is a day) |
   | `POLL_INTERVAL_MS` | `1000` |
   | `ARC_RPC_URL` | optional: a keyed endpoint first, then the public ones |

4. Generate a public domain for the service and check `https://<service>/health`.
5. Mint a licensed key for the web app:

   ```bash
   curl -X POST https://<service>/v1/keys -H "Authorization: Bearer $TIDELINE_ADMIN_KEY" \
     -H 'content-type: application/json' -d '{"name":"web","tier":"licensed"}'
   ```

Run a single instance per network: ingest progress and rate limits are held in memory.
Without Railway, any container host works:

```bash
docker build -t tideline-stream services/stream
docker run -p 8080:8080 -e DATABASE_URL=... -e TIDELINE_ADMIN_KEY=... tideline-stream
```

**2. Web app (Vercel).** Import the repo, set the root directory to `apps/web` (the
framework and the pnpm workspace are detected), and set:

| Variable | Value |
| --- | --- |
| `TIDELINE_API_URL` | `https://<service>` |
| `TIDELINE_API_KEY` | the licensed key from step 1.5, not the admin key |

The key stays on the server: the browser only ever talks to the Next.js app.

## Verification

```bash
pnpm typecheck && pnpm lint && pnpm test && pnpm build
cd services/stream
cargo fmt --check && cargo clippy --all-targets -- -D warnings
TEST_DATABASE_URL=postgresql://postgres:postgres@localhost:5432/postgres cargo test
```

`cargo test` regenerates `packages/schema/src/generated`. The database-backed API test is
skipped when `TEST_DATABASE_URL` is unset.

## Links

- Arc docs: https://docs.arc.io
- Explorer: https://explorer.arc.io (testnet: https://explorer.testnet.arc.io)
- Faucet: https://faucet.circle.com
