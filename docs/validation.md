# Development validation - 2026-09-23

This is a Windows development preview, not the complete accepted MVP.

## Latest verification

- 45 frontend tests passed; ESLint and TypeScript/Vite production build passed.
- 45 native tests passed on Windows/MSVC. Three network tests are ignored by
  normal test runs, then explicitly executed: all three passed.
- Live GitHub, official MCP Registry and Claude official marketplace searches
  each returned 30 first-page and 30 second-page items through the native adapter.
- Rustfmt check passed. Native fixture coverage includes Unicode/binary package
  preservation, credential omission/preservation, ownership and stale checks,
  protected backup ACLs, rollback/recovery, SQLite reopening and injected failures.
- The published baseline de7a543 passed GitHub Windows checks and installer
  generation: https://github.com/z61540471-ux/agent-component-manager/actions/runs/35845239485
  That CI result covers the baseline only, not the additions in this iteration.
- Tests did not change real Agent configurations or execute plugin/MCP scripts.

## Current behavior

Windows builds allow confirmed Skill/MCP file changes and recovery. Plans stay
in native memory; rechecks, private backups and journaling precede writes.
Persistence failures preserve the observed outcome and recovery evidence.
Other platforms and browser previews cannot apply component changes.

Optional Agent roots feed scanners and planners. Project bindings carry typed
project/ownership fields; inherited Claude plugin settings propagate to owned
Skills. Description changes preserve identity; Windows slash/case spelling does
not create false aliases. UI filters include scope, observed state and findings.
Owned/contextual rows explain unavailable actions. Standalone Skills remain editable.

Native export writes a retained inventory snapshot as UTF-8 JSON, refuses
existing files and excludes raw MCP configuration/backup bytes.

The desktop UI provides inventory, detail, marketplace, preview, settings and
history views. Scan failures, empty results, stale registry pages, permission
errors and rollback/recovery outcomes remain visible instead of being replaced
by a generic success state.

## Market and Codex plugin iteration

- Market results page by source, query and cursor; exact-page cache fallback
  preserves timestamps. Failures retain prior pages; stale requests are discarded.
  Limits: 4 MiB response, 1,000 GitHub results, 34 pages per UI search.
- Rate-limit errors include Retry-After/reset metadata when supplied.
- Codex portable plugin.json and legacy compatibility cache manifests are
  recognized. Their Skills and MCP entries retain explicit package ownership.
- Codex user/project plugin settings appear separately from cached versions;
  configured/disabled does not claim installation, project trust or runtime loading.
- Relative package resources reject escapes and existing links outside the
  package. Unsupported manifest declarations appear as scan issues.
- Parent integration review covered cache isolation, pagination/response limits,
  UI stale-result handling, cache ownership, secret omission and resource paths.

## Remaining acceptance

- Codex local catalog resolution and complete cross-Agent lifecycle/binding
  coverage. Selected workspace-local Codex plugin caches are now observed with
  project scope and ownership; plugin writes remain unsupported;
  Claude project marketplace catalogs are observed with project scope and
  ownership; Claude plugin actions provide manual native CLI instructions.
- Interactive desktop scan/export/restart validation and clean-account Windows
  installation. SQLite reopen tests and startup smoke are narrower evidence.
- Full-scope review and acceptance before a production release.

## Optional online check

Run only when public network access is available:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml registry::live_checks -- --ignored --nocapture --test-threads=1
```

Normal tests do not require these endpoints. The local dependency mirror is not
part of the repository. The Windows installer is unsigned development output.
Power-loss durability and handle-relative protection against malicious same-user
filesystem races are not guaranteed. Empty directories may remain after uninstall.

Latest Windows debug executable and NSIS installer built successfully in artifacts/windows-debug/market-preview/. Startup smoke observed a responding titled window and closed only that test process. All 65 project files matched staging with UTF-8 validation.
