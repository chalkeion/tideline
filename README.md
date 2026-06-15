# Tideline

## Overview

Tideline is a stablecoin-flow intelligence platform built on [Arc](https://docs.arc.io),
Circle's stablecoin-native, EVM-compatible L1. It ingests, decodes, and enriches
on-chain stablecoin movements (USDC, EURC, USYC, CCTP, Gateway) into actionable flow
intelligence.

## Architecture

A Turborepo monorepo: a Next.js web app (`apps/web`), a Rust streaming/indexing
service (`services/stream`), and shared TypeScript packages (`packages/*`) for chain
config, schema, SDK, and UI. Internal design docs to be linked here once they exist.

## Stack

- **Tooling:** pnpm 9, Turborepo 2, TypeScript 5.6
- **Web:** Next.js 16 (App Router), React 19, Tailwind 3, viem 2
- **Service:** Rust (axum 0.7, tokio, sqlx 0.8, ts-rs 10, alloy-primitives 0.8)
- **Quality:** Vitest 2, ESLint 9 (flat config), Prettier 3, GitHub Actions CI

## Development

Prereqs: Node 22+, pnpm 9+, Rust stable.

```bash
pnpm install
pnpm dev
```

## Verification

```bash
pnpm install
pnpm typecheck
pnpm lint
pnpm test
pnpm build
cd services/stream && cargo fmt --check && cargo clippy -- -D warnings && cargo check && cargo test
```

## Links

- Arc docs: https://docs.arc.io
- Explorer: https://testnet.arcscan.app
- Faucet: https://faucet.circle.com
