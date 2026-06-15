# infra/goldsky

## Purpose

Goldsky is a candidate ingestion primitive for Tideline — streaming Arc on-chain
stablecoin events into our pipeline (`services/stream`) and/or a managed datastore.

## Pending Week 0 decision

Evaluate **Goldsky Mirror** vs **Envio HyperIndex** for the ingestion layer:

- Latency / reorg handling on Arc
- Cost at expected event volume
- Operational overhead vs. running our own `services/stream` ingestor
- Schema export ergonomics

Decision to be recorded here before indexing config is committed.

## Contracts to index

From `@tideline/arc` (Arc Testnet):

- USDC `0x3600000000000000000000000000000000000000`
- EURC `0x89B50855Aa3bE2F677cD6303Cec089B5F319D72a`
- USYC `0xe9185F0c5F296Ed1797AaE4238D26CCaBEadb86C`
- CCTP TokenMessengerV2 `0x8FE6B999Dc680CcFDD5Bf7EB0974218be2542DAA`
- Gateway Wallet `0x0077777d7EBA4688BDeF3E311b846F25870A19B9`
