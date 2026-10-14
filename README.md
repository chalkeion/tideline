# Tideline

## Overview

Tideline is the attribution layer for stablecoin flow on [Arc](https://docs.arc.io), Circle's
stablecoin-native L1. It ingests USDC, EURC, USYC, CCTP V2 and Gateway activity, collapses
raw logs into economic flows, and ties each counterparty to a known entity with evidence.

It is built for a compliance officer's question, not for chart browsing:

> Show me every stablecoin flow touching this entity in the last 24 hours, and prove the
> attribution.

The positioning behind this is in [docs/MOAT-STRATEGY.md](docs/MOAT-STRATEGY.md).

## What it does

| Moat | Where it lives |
| --- | --- |
| **Attribution registry.** Entities, and per-address claims that each carry a method (`protocol_registry`, `self_attested`, `curated`, `heuristic`), a confidence and evidence. | `services/stream/src/db.rs`, `data/entities.seed.json` |
| **Clustering.** A Gateway burn signed by a different key links signer and depositor; a first-party attribution on one propagates to the other as a lower-confidence heuristic label, with the linking tx as evidence. | `enrich.rs` (links), `db.rs::propagate_links` |
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
- **Reference rates.** EURC is valued at the ECB euro reference rate and USYC at the fund's
  published price report, refreshed hourly and applied when a flow is ingested.
  `EURC_USD_RATE` and `USYC_USD_RATE` apply until a source answers; `LIVE_RATES=false` pins
  them. `/v1/overview` reports each rate with its source and publication date.
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
| `GET /v1/flows`, `/v1/flows/:id` | public | Filters: `asset kind entity address visibility min_usd since_hours attributed before after limit` |
| `GET /v1/entities`, `/v1/entities/:slug`, `/v1/addresses/:address` | public | Evidence fields are licensed |
| `GET /v1/compliance/report?entity=&window_hours=&format=csv` | licensed | The compliance question |
| `GET /v1/flows/stream` | licensed | Server-sent events |
| `POST /v1/attributions`, `/v1/attestations`, `/v1/disclosures`, `/v1/keys` | admin | Registry curation |

## Deploy

Three parts: Postgres, the stream service (one process: ingestor and API), and the web app.

- **Service.** `services/stream/Dockerfile` builds a self-contained image; migrations and
  the seed registry are compiled in.

  ```bash
  docker build -t tideline-stream services/stream
  docker run -p 8080:8080 -e DATABASE_URL=... -e TIDELINE_ADMIN_KEY=... tideline-stream
  ```

  Set `ARC_NETWORK`, `BACKFILL_BLOCKS=172800` for a full day of history on first start, and
  a keyed endpoint first in `ARC_RPC_URL` for anything long-running. Run a single instance
  per network: ingest progress and rate limits are held in memory.
- **Web.** Any Next.js host, with `apps/web` as the root directory. Set `TIDELINE_API_URL`
  to the service and `TIDELINE_API_KEY` to a licensed key from `keys create`, not the
  admin key.

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
