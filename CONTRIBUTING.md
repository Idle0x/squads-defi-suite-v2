# Contributing

## Repository structure

```
squads-defi-suite/
├── plugins/<name>/          # ZeroClaw WASM plugin crate
│   ├── Cargo.toml
│   ├── manifest.toml        # Plugin metadata for ZeroClaw registry
│   ├── SKILL.md             # Agent instruction file
│   ├── README.md            # Plugin documentation
│   └── src/                 # Rust source
├── squads-defi-core/        # Shared core crate (published on crates.io)
├── wit/v0/                  # WIT interface definitions
├── scripts/                 # Build and verification scripts
├── tests/                   # Integration tests
└── .github/workflows/       # CI/CD
```

## Design constraints

- **WASM target**: All plugins compile to `wasm32-wasip2`. The WIT interfaces are in `wit/v0/` and use the `tool-plugin` world.
- **No secrets in plugins**: Plugins hold no private keys. The host injects configuration via `__config`.
- **No environment variables in WASM**: WASM components cannot read environment variables. All configuration comes through `__config`.
- **Permit-based capabilities**: Each plugin declares required capabilities in `manifest.toml` (`http_client`, `config_read`). The runtime grants them at load time.

## Adding a new plugin

1. Create a new crate under `plugins/<name>/` with the standard structure.
2. Add the WIT bindings generation in the component module (see existing plugins for reference).
3. Implement the `tool-plugin` world: `PluginInfo` and `Tool` traits.
4. Create `manifest.toml` with the plugin name, version, wasm_path, capabilities, and permissions.
5. Add the plugin to the workspace `members` array in `Cargo.toml`.
6. Add the plugin to the `PLUGINS` array in `scripts/build.sh` and `scripts/package.sh`.
7. Write tests in `plugins/<name>/tests/`.
8. Create `SKILL.md` with agent-facing instructions.
9. Update `README.md` with configuration and usage documentation.

## Submitting changes

1. Run `cargo test --workspace` — all tests must pass.
2. Run `scripts/build.sh` — all WASM components must compile.
3. Run `scripts/verify.sh` — must report 0 errors.
4. Run `scripts/package.sh` — produces distribution zips.
5. Update `CHANGELOG.md` with the changes.
6. Open a pull request with a description of the change, including any config changes or new dependencies.

## Coding standards

- Tests are written in `tests/` as integration tests (public API only) and inline `#[cfg(test)]` for unit tests.
- Error types use `thiserror` derive macros.
- Config parsing uses `serde::Deserialize` with clear error messages.
- All plugin output is limited to 200 tokens (enforced by `squads-defi-core/src/shape.rs`).

## License

This project is licensed under the MIT License. By contributing, you agree to license your contributions under the same license.
