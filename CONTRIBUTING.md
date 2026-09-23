# Contributing

This is a Windows-first Tauri, React and Rust project. Keep Agent adapters,
marketplace discovery and filesystem mutation independent.

Before proposing a change, describe the observable problem and expected result.
Use fictional configuration fixtures; never commit real user inventories,
credentials, machine paths, SQLite databases or backups.

Changes to installation, links or configuration writes require regression tests
covering stale plans, shared ownership, failed writes and rollback. Parsing tests
should include malformed input and UTF-8 Chinese descriptions. Test against
temporary directories, never against a contributor's live Agent configuration.

Run `npm run lint`, `npm run build`, `npm test`,
`cargo fmt --manifest-path src-tauri/Cargo.toml --check` and
`cargo test --manifest-path src-tauri/Cargo.toml --locked` before submitting.
On machines under heavy native build load, UI tests can run serially with
`node node_modules/vitest/vitest.mjs run --maxWorkers=1 --no-file-parallelism`.
Windows CI builds an unsigned NSIS installer as a temporary workflow artifact;
that artifact is not a production release or signing guarantee.

Pull requests should state the supported platforms, validation performed and
remaining limitations. Preserve third-party licenses when incorporating code.
