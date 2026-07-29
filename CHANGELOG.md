# Changelog

All notable changes to the Squads DeFi Suite will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2026-07-29

### Security

- **Critical: Jupiter APIs now called server-side by the plugin** — The LLM no longer constructs Jupiter URLs or fetches quotes externally. The plugin fetches the quote, token price, and swap instructions internally via `waki`. This eliminates LLM hallucination of quote data, fabricated swap instructions, and quote-swap mismatch attacks. Program ID validated against `JUP6LkbZ...` at the ingress point. (N1, N2, N3)
- **USD price fetched from Jupiter's price API internally** — The LLM no longer provides `usd_per_unit`. The plugin queries Jupiter's price API and the mint account for decimals, computing the price per smallest unit. Eliminates fabricated-price bypass of notional/daily-cap guardrails. (C2, L2)
- **HTTPS URL validation** — `rpc_url` and `jupiter_url` are validated for `https://` prefix in config parsing. Non-HTTPS URLs are rejected with a clear error. (D2)
- **Structured logging with timing** — All plugins now emit structured log events with `duration_ms` (wall-clock timing), `action` (Start/Complete/Fail), and `attrs` (JSON context: mints, amounts, error details). Enables performance monitoring and crash detection. (N5, N6, N10)

### Architecture

- **Unified Cargo workspace** — All 5 crates (core + 4 plugins) now share a single workspace. `cargo test --workspace` tests everything. Local `squads-defi-core` changes are immediately reflected via path dependencies. No more publish-to-test cycle. (B1)
- **SKILL.md files removed** — Agent-facing instructions are now embedded in each plugin's `description()` and `parameters_schema()`. The SKILL.md files were never loaded by ZeroClaw (plugins declare `capabilities = ["tool"]`, not `["tool", "skill"]`). (F1, F2, F3)
- **`squads_program_id` configurable** — Both `jupiter-swap-propose` and `vault-watch` now read the Squads program ID from config with a mainnet default. Removed hardcoded constant from function bodies. (N12)

### Changed

- **jupiter-swap-propose** — Plugin now accepts raw swap parameters (`input_mint`, `output_mint`, `amount`, `slippage_bps`) instead of pre-fetched JSON. Fetches quote, price, and swap instructions internally. All guardrails enforced in Rust.
- **vault-watch** — Added `refresh` parameter to `parameters_schema()`. Errors in RPC fetches now produce `warnings` field in JSON output instead of silently returning empty data. Configurable `squads_program_id`.
- **token-risk-check** — Added timing, structured logging, and HTTPS URL validation.
- **solana-pay-request** — Added timing and structured logging.

### Infrastructure

- **Config template fixed** — Replaced `[[plugins.entries]]` array-of-tables syntax with `[plugins.entries.<name>.config]` section format to avoid ZeroClaw issue #8636. Duplicate `config-template.toml` removed. (H1, N8)
- **WASM artifact validation** — CI now runs `wasm-tools validate --features component-model` on all built `.wasm` files. New `wasm_artifact_test.rs` files verify existence, size, and structural validity for all 4 plugins. (G1, I1, I2)
- **Registry index enriched** — `registry.json` now includes `description`, `author`, and `capabilities` fields, read from each plugin's manifest. (G2)
- **Package signing** — New `scripts/sign.sh` produces Ed25519 signatures for distribution zips. Signing step in CI release workflow (requires `PLUGIN_SIGNING_KEY` secret). (G3)
- **`.cwasm` precompilation** — `build.sh` generates pre-compiled WASM binaries via `wasmtime compile` when the CLI is available. (B3)
- **`.gitignore` updated** — Added `*.cwasm`, `dist/`, `registry.json`. (B2)

### Documentation

- Added cost tracking section to GETTING_STARTED.md. (H4)
- Consolidated config templates into single `config.toml.template`.
- CONTRIBUTING.md updated: workspace structure, SKILL.md guidance replaced with `description()`/`parameters_schema()` guidance, path dependency instructions for new plugins.

## [0.1.0] — 2026-07-25

### Added

- **jupiter-swap-propose** — Jupiter swap quote → guardrail check → unsigned Squads v4 multisig proposal. Six Rust-enforced guardrails: mint allowlist, max slippage, max price impact, max route hops, max notional, daily cap.
- **vault-watch** — Read-only treasury briefing: pending/ready/executed proposals, token balances, lending health factors. Suitable for daily cron scheduling.
- **solana-pay-request** — Build `solana:` payment URLs with config-enforced recipient address. SOL and SPL token support.
- **token-risk-check** — On-chain SPL token risk analysis: mint authority, freeze authority, and Token-2022 extension detection. Returns low/medium/high classification.

### Infrastructure

- Build pipeline: `build.sh` (WASM compilation), `verify.sh` (manifest cross-check), `package.sh` (distribution zips)
- CI: GitHub Actions build workflow (`build.yml`) — WASM build, test, verify, package
- SKILL.md agent instruction files for all 4 plugins
- Getting-started guide for absolute beginners
- Contributor guide for plugin development

### Security

- Plugin configuration is injected at runtime through `__config`. The host strips any value supplied by the LLM and substitutes operator-configured values.
- Plugins declare required capabilities in `manifest.toml`. The runtime grants `wasi:http` when `http_client` is declared.
- Ed25519 manifest signing supported (default: disabled).
