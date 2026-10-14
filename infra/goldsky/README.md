# infra/goldsky

## Decision

Ingestion runs in-house: `services/stream` polls the Arc RPC with `eth_getLogs`, one filtered
call per block range (all tracked contracts, all tracked topics), gzip on. Arc has
deterministic finality, so there is no reorg handling to outsource, and the flow and cursor
commit in one transaction.

Goldsky Mirror or Envio HyperIndex remain options for deep historical backfill, where a
public RPC is the bottleneck. Nothing here is required to run Tideline.

## Contracts indexed

From `@tideline/arc` (`mainnetAddresses`; `testnetAddresses` for Arc Testnet):

- USDC `0x3600000000000000000000000000000000000000`
- EURC `0xbEf5f6d51CB62b58e6A8f77868681825C6fe21c1`
- USYC `0x8a5D989Bbb96929F689B0200f435f53dA42bF490`
- CCTP TokenMessengerV2 `0x28b5a0e9C621a5BadaA536219b3a228C8168cf5d`
- CCTP MessageTransmitterV2 `0x81D40F21F12A8F0E3252Bccb954D722d4c464B64`
- Gateway Wallet `0x77777777Dcc4d5A8B6E418Fd04D8997ef11000eE`
- Gateway Minter `0x2222222d7164433c4C09B0b0D809a9b52C04C205`
- Native USDC system emitter `0xffffFFFfFFffffffffffffffFfFFFfffFFFfFFfE` (EIP-7708): every
  USDC movement, including native sends the ERC-20 contract never logs
