# Roadmap

## Now: shipped

- Arc Mainnet and Testnet behind `ARC_NETWORK`, one database per network.
- Every token on Arc (ERC-20 and ERC-721), identified by contract address; listed tokens
  priced, everything else tracked by amount; impostor symbols flagged.

## Next: every USDC and EURC transfer on every chain Circle issues them on

Sized on 2026-10-14 from Circle's contract-address pages, explorer counters, Visa Onchain
Analytics, and our own samples of live RPC data.

### The job

- **Chains:** USDC is on 38 chains and EURC on 9. 27 are EVM; the other 11 span about ten
  runtimes: Solana, Sui, Aptos, Stellar, Noble, Algorand, Hedera, NEAR, Polkadot Asset Hub,
  Starknet and XRPL.
- **Volume:** about 15-18M raw USDC transfers a day.
  - Solana is about half (6.6-9.9M).
  - Base is about a quarter (4.1M; our samples gave 3.4-5.0M).
  - Arbitrum 1.25M, OP Mainnet 1.2M, Polygon 0.8M, Ethereum 0.56M.
  - Every other chain is under 0.2M, and EURC is about 0.1M across all chains.
- **Bots:** raw counts are 7-76x Visa's bot-filtered counts. 39% of Ethereum USDC/USDT
  transfers are sub-cent address-poisoning spam (Goldsky).
- **Value is concentrated.** In our Base and Ethereum samples, the ~9% of transfers worth
  $10K or more carry 96-97% of the value.
- **History:** about 7-11B transfers all-time; Base alone has over 2B.

### What limits it, measured

1. **Storage format.** The current Postgres `flows` row costs 565 B, which at this volume is
   10 GB a day. A compact columnar layout (dictionary-encoded addresses, delta-encoded
   blocks, zstd) measured 7.8 B per transfer, ~13 B with the address dictionary, and 35 B
   if tx hashes are kept. That is ~230 MB a day, ~85 GB a year, about $1.30 a month on
   Cloudflare R2 for each year kept.
2. **Solana.** Querying the USDC mint misses plain `Transfer` instructions, so every block
   has to be read. The public RPC rate-limits `getBlock` (HTTP 429). The cheapest complete
   stream found is about $98 a month (Chainstack Yellowstone gRPC).
3. **EVM data sources: free, but fragile.**
   - Envio HyperSync's free tier covers Arc and every large EVM chain. It needs a token, and
     its "fair use" limit is unpublished.
   - dRPC's free tier allows ~10.5M `eth_getLogs` a month; polling 20 chains every 10 s
     needs ~5.2M.
   - Alchemy and QuickNode free tiers cap ranges at 10 and 5 blocks.
   - Paid fallback: dRPC at about $31 a month.
4. **Non-EVM long tail.** The cost is adapters, not money. Retention is short:
   - Stellar RPC keeps about 7 days; Sui's public GraphQL keeps 30-90 days.
   - Hedera's mirror node has no token filter.
   - NEAR Lake stopped in March 2026.

   So live capture has to start before history disappears.
5. **Free history.**
   - Ethereum: BigQuery's free 1 TiB a month, ethPandaOps Xatu, and AWS public data.
   - Base and Arbitrum: AWS public data.
   - Everything else: HyperSync (slow on the free tier), or nothing.
6. **Hosting.** Raw RPC JSON for the top six EVM chains is ~6.5 GB a day. Ingestion runs on a
   server, not a home connection.

### Design

- **Ledger:** every transfer on every chain, stored as compact columnar files, uploaded to
  object storage hourly. No tx hashes: (chain, block, tx index) identifies a transaction,
  and the hash is fetched when someone opens it.
- **Flows:** the Postgres table that exists today keeps only flows worth a narrative:
  attributed parties, $1K and up, cross-chain, watched entities.
- **Dust:** transfers under $1 are rolled up per sender, receiver and hour, and an
  address-poisoning detector runs over them.
- **One config-driven EVM adapter** for all 27 EVM chains, reusing the failover client and
  token-metadata code.
- **Cross-chain edges:**
  - CCTP V2 pairs a burn on one chain with its mint on another, a deterministic link.
  - The same EVM address on 27 chains is the same controller.
- **History** starts at go-live. Ethereum, Base and Arbitrum are backfilled from free bulk
  data; other addresses are backfilled lazily when they are attributed.

### Phases

| Phase | Coverage | Share of transfers | Cost per month |
|---|---|---|---|
| 1 | 27 EVM chains incl. Arc; HyperSync free, dRPC fallback | ~45-50% | $0-5 (server + R2 past 10 GB) |
| 2 | 10 non-EVM chains | +~1% | same |
| 3 | Solana via gRPC stream | ~100% | +~$98 |

### TODO

- [ ] Decide the monthly budget ceiling: $0, ~$10, or ~$110 with Solana.
- [ ] Get an Envio HyperSync API token (envio.dev).
- [ ] Columnar ledger writer (Parquet/DuckDB) and an R2 uploader; read path through DuckDB.
- [ ] Chain registry config; generic EVM adapter; start with Base, Arbitrum, OP Mainnet,
      Polygon and Ethereum.
- [ ] Dust aggregation and an address-poisoning detector.
- [ ] CCTP V2 cross-chain pairing by (source domain, nonce).
- [ ] Non-EVM adapters by volume: Stellar, then Sui, Aptos, Noble, Algorand, Hedera, NEAR,
      XRPL, Polkadot Asset Hub, Starknet.
- [ ] Solana block-stream adapter, reading token balance changes so plain `Transfer` is caught.
- [ ] Backfill Ethereum, Base and Arbitrum from the AWS public bucket and BigQuery.
- [ ] Admin API to list a token (price source plus evidence) instead of editing the seed file.
