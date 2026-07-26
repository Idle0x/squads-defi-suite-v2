# Changelog

All notable changes to the Squads DeFi Suite will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
