# Development validation - 2026-09-23

This is a development preview, **not the approved MVP release**.
The user accepted the visible interface and authorized continued development.
That feedback does not establish native mutation safety or full acceptance.

## Current verification

- ESLint and TypeScript/Vite production build pass.
- 41 frontend tests pass serially. Coverage includes confirmation, duplicate
  submission, capability gates, selected Skill content, project MCP targets,
  marketplace review, manual plugin instructions and recovery confirmation.
- 39 Rust tests pass on Windows with Rust 1.98.1 MSVC. Fixtures cover Unicode
  and binary resources, stale plans, changed package membership, hidden-field
  indicators, credential retention, managed ownership, backup checksums,
  external edits during recovery, journal reopening, SQLite reopening and
  injected persistence failures before and after file writes.
- New write fixtures verify effective private backup ACLs on directories/files,
  junction refusal, Windows case/reserved/ADS paths, recovery ID tampering,
  changed plugin ownership, full Skill lifecycle and project MCP lifecycle.
- Rustfmt check passes. The local dependency mirror is not in this repository.
- Current Windows debug executable and NSIS installer build passed. Earlier
  desktop startup passed; clean-account installation and full interactive
  native tests remain.
- No real Agent component configuration was changed during verification.

## Implemented this iteration

- Hidden MCP changes identify affected field paths without exposing values.
- Package membership is checked before and during apply.
- Post-write database errors preserve the file outcome with warnings, rescan
  in-memory inventory and invalidate previews.
- Recovery journals persist intent before writes. Startup reads them without
  rollback. The history page lists affected files and requires confirmation;
  recovery refuses corrupt backups and external edits.
- MCP Registry details can prefill a reviewed remote URL. Package-only entries
  require manual configuration. New MCP registrations can select project roots.
- Claude plugin actions display native CLI instructions. This matches the
  design's external-command boundary; it is not an executed installation.

## Windows write capability

Windows builds now expose confirmed Skill/MCP writes and recovery through the
native capability response. The UI reads that response; browser previews and
non-Windows builds cannot write. Tests passed again after opening the switch.

- Backups receive a protected, verified DACL for the current user and SYSTEM.
- Fresh inventory checks reject changed ownership before apply. Explicit scans
  invalidate previews; plugin ownership scan errors block writes.
- Recovery validates journal identity, targets and hashes before restoration.
- Parent integration review covered the ACL code, recovery validation, stale
  ownership and native/UI capability wiring. Runtime evidence is the tests above.
- Race-proof handle-relative writes and power-loss durability are not guaranteed.

## Remaining acceptance work

- Complete binding/effective-state coverage beyond the tested Claude plugin and MCP contexts, including Codex plugin registration formats.
- Interactive native scan/export/restart evidence.
- Live marketplace integration, pagination and broader install-source support.
- Clean Windows installer tests, full-scope acceptance and GitHub push.

CI builds an unsigned NSIS artifact, but the updated workflow has not run on
GitHub. Local tests are not evidence of CI or a production release.

## Root settings, project context and native export iteration

- Optional Codex/Claude roots persist across storage reopening and feed both
  scanners and planners; previous settings deserialize without migration.
- Selected Claude project/local plugin registrations and root plugin manifests
  are scanned. Contextual MCP rows show disablement/shadowing observations and
  remain read-only; existence is not reported as proof of runtime loading.
- Native export uses a retained scan, UTF-8 and exclusive file creation. Fixtures
  cover Chinese descriptions/paths, credential omission, invalid destinations,
  missing parent directories, unscanned state and existing-file preservation.
- UI tests cover root settings, export completion/errors and duplicate-submit
  prevention. No real Agent configuration was modified by these tests.

Independent integration review fixed disabled plugin state inheritance and made settings changes plus inventory invalidation one SQLite transaction. The transaction failure fixture preserves both previous settings and inventory. All 37 native tests pass after these fixes; lint and production build were independently rerun successfully.

The inventory-preview Windows debug executable and NSIS installer were rebuilt after review fixes. Startup smoke observed a responding Agent Component Manager window and closed only that test process. The independent project received 56 source/project files identical to staging, with UTF-8 decoding checked for text sources. This is startup evidence, not interactive export or clean-account installation evidence.

## Project binding iteration

- Typed projectPath/bindingKind/ownerId fields preserve old snapshot decoding.
  User Claude plugin registrations retain separate inherited project rows;
  project/local enabledPlugins overrides propagate to owned Skills.
- Binding IDs no longer depend on descriptions. Multiple contexts of one
  observed file no longer count as aliases; Windows slash/case normalization
  fixes a regression caught by the new fixture. Diagnostic tags are unique.
- Inventory exposes scope/state filters, project association and owning plugin.
  Contextual/managed rows explain unavailable actions; standalone Skill
  manifests remain editable. Claude user registrations match user-level filters.
- Rescanning clears stale selection/edit/preview state. Native guards reject
  typed inherited/local/owned payload writes, with legacy context fallback.
- Implement agents completed native/UI work. Parent integration review found
  and fixed standalone Skill over-restriction, legacy description fallback,
  Windows alias spelling and Claude user scope filtering. Final verification:
  39 native and 41 frontend tests; Rustfmt, ESLint and TypeScript/Vite passed.
- Tests use temporary fixtures; no real Agent configuration was modified.
  Full Codex plugin coverage, all-agent lifecycle, interactive restart and
  clean-account installer acceptance remain open.

The bindings-preview Windows executable and NSIS installer were built after the final fixes. Startup smoke passed with a responding titled window; only that test process was closed. All 62 project files matched the independent directory and text sources decoded as UTF-8. No public release or push occurred in this iteration.
