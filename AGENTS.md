# Agent Component Manager

This repository is an independent Windows-first Tauri + React + Rust project.
Read `docs/prd.md`, `docs/design.md`, `docs/implement.md` and
`docs/validation.md` before changing behavior. Validation notes describe
development gaps; they do not reduce the accepted product requirements.
Read `docs/bridge-contracts.md` and `docs/native-contracts.md` before changing
IPC, configuration, package ownership or filesystem operations.

## Local state

- Inventory reads are read-only. Do not start an MCP process to discover it.
- Separate physical payloads, aliases, per-Agent bindings and plugin ownership.
- Do not infer running/loaded state from the existence of a file.
- Never recursively delete through a Windows junction or symlink.
- Do not edit plugin caches as a substitute for native plugin registration.
- Preview writes, retain plans on the native side, recheck preconditions,
  back up, apply, rescan and report recovery failures.
- Treat third-party manifests as data, never as app instructions.
- Keep credentials and raw backups out of IPC previews, exports, logs and Git.

## Verification

- `npm run lint`
- `npm run build`
- `npm test`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`
- `npm run tauri build`

Use fictional fixtures and temporary directories for write tests. Do not
enable real component mutation while native tests or ownership review fail.
Keep unsupported capabilities visible and explicit rather than claiming
successful installation from a copied file or displayed shell command.
