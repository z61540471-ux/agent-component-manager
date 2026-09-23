# Implementation Plan

## Phase 1: Repository and shell

- [x] Initialize the Tauri + React project in `D:/develop/package/agent-component-manager`.
- [x] Configure Windows development commands, Rust formatting, TypeScript linting, and test scripts.
- [x] Add MIT or Apache-2.0 license decision, README, CONTRIBUTING, SECURITY, and issue templates.
- [x] Add `.gitignore` for Tauri, Rust, Node, local SQLite, backups, and brainstorm artifacts.

## Phase 2: Core contracts and persistence

- [ ] Define normalized component, agent target, scan issue, Change Plan, and operation result types.
- [x] Create SQLite schema and repository layer.
- [x] Add JSON serialization for inventory export.

## Phase 3: Windows scanners

- [x] Implement Codex root discovery and Skill/MCP scanning.
- [x] Implement Claude Code root discovery and Skill/MCP/Plugin scanning.
- [ ] Add project-scope scanning, hashing, status calculation, and duplicate/conflict classification.
- [ ] Add fixture tests for the layouts used by the current machine.

## Phase 4: Registry adapters

- [ ] Implement GitHub search/detail/source resolution with rate-limit and cache handling.
- [x] Implement official MCP Registry search/detail/version resolution.
- [ ] Implement Claude Plugin Marketplace discovery and install-source resolution.
- [x] Normalize metadata and retain metric provenance.

## Phase 5: Planner and executor

- [ ] Generate plans for install, update, enable/disable, and uninstall.
- [x] Render file-level changes and warnings.
- [x] Implement backup, atomic writes where applicable, post-apply rescan, and rollback.
- [x] Reject stale plans when target files changed after planning.

## Phase 6: UI

- [x] Build inventory-first shell and navigation.
- [ ] Implement overview, inventory, component detail, marketplace, Change Preview, and settings pages.
- [ ] Add loading, empty, stale-cache, parse-error, permission-error, and rollback states.

## Phase 7: Verification and release

- [ ] Run Rust unit/integration tests, TypeScript checks, UI tests, and Windows smoke tests.
- [x] Build a Windows distributable.
- [ ] Review documentation and security boundaries.
- [ ] Commit and push the first public MVP to GitHub.

## Review gates

- Do not begin mutating implementation until the PRD, design, and plan are approved.
- Do not enable external install execution in MVP.
- Do not report a marketplace metric without naming its source and retrieval time.
- Do not apply a change without a visible Change Plan and user confirmation.

## Review evidence ? 2026-09-23

Current evidence: 29 Windows native tests, 20 frontend tests, native startup,
and debug NSIS installer generation passed. File previews, private backups,
stale checks, persistence-failure reporting and rollback are implemented and
verified with fixtures. Windows writes are enabled after preview/confirmation.
Registry live integration and full component/binding coverage remain incomplete;
the installer is an unsigned development build, not a production release.

The Claude scanner now includes selected project plugin registrations/manifests and contextual MCP state. Codex plugin schema coverage and broader effective binding handling remain incomplete; project scanning and full lifecycle checklist items remain open.

## Integration review reconciliation - 2026-09-23

Phase 1 project initialization and Windows check commands are now checked: the target repository has the Tauri/React sources, scripts, rustfmt support and Windows CI configuration. Other unchecked items remain open for complete model/binding coverage, recorded current-machine layout fixtures, registry source resolution/live integration, all-agent lifecycle support, complete UI states, full acceptance and publication.
