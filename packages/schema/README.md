# @tideline/schema

Shared TypeScript types generated from the Rust service.

`src/generated/` is the **export target of `ts-rs`** from `services/stream`. Running the
Rust test suite (`cargo test` in `services/stream`) emits `.ts` files here via the
`#[ts(export, export_to = "...")]` annotations. Do not hand-edit generated files.

No build step yet — types are consumed directly.
