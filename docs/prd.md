# Agent Component Manager

## Objective

Build an open-source Windows-first desktop control center for locally installed AI agent components. The first release manages Skills, MCP Servers, and Plugins for Codex CLI and Claude Code, with a local inventory as the primary experience.

## User problem

Agent components are scattered across user and project directories, configuration files, plugin caches, and marketplaces. Users cannot easily answer what is installed, which agent loads it, whether copies are duplicated or drifted, where it came from, or how to remove it safely.

## Scope

### In scope

- Windows desktop application using Tauri, React, and Rust.
- Discovery of Codex CLI and Claude Code installations.
- Scan global and project-scoped Skills, MCP Servers, and Plugins.
- Unified inventory with source, description, version, path, scope, targets, status, and content hash.
- Detection of duplicate content, same-name conflicts, drift, broken paths, disabled components, and stale entries.
- Marketplace discovery from GitHub, the official MCP Registry, and the Claude Plugin Marketplace.
- Display of available marketplace metadata, including description, source, version, and Star/download metrics when the source provides them.
- Install, update, enable/disable, and uninstall previews represented as Change Plans.
- User confirmation before writes; pre-change backup; post-change rescan; rollback on failure.
- SQLite local index and operation history.
- JSON export of the local inventory.
- Public GitHub repository and open-source documentation.

### Out of scope for MVP

- Cloud accounts, sync, team permissions, or hosted service.
- macOS/Linux native support.
- Community marketplace publishing or moderation backend.
- Automatic third-party script execution.
- Automated security scoring model.

## Product principles

- Local inventory is the home screen and source of truth for what exists locally.
- Read-only scanning is separate from write operations.
- Every write is previewable and requires explicit confirmation.
- Market popularity metrics are metadata, not safety scores.
- Adapters are isolated so additional agents and registries can be added later.
- The application must distinguish files that exist from components that are currently effective.

## Acceptance criteria

- [x] 1. On Windows, the app detects Codex CLI and Claude Code roots or lets the user correct their paths.
- [ ] 2. The app scans global and project-level Skills, MCP configuration, and Plugin manifests.
- [ ] 3. The inventory identifies duplicate content, same-name conflicts, broken paths, disabled components, and drift.
- [x] 4. Marketplace search covers GitHub, the official MCP Registry, and Claude Plugin Marketplace.
- [x] 5. Results show description, source, version, and available Star/download metrics with the metric source identified.
- [ ] 6. Install, update, enable/disable, and uninstall all produce a Change Plan before modifying files.
- [x] 7. Applying a Change Plan creates backups, performs the change, rescans, and records the result.
- [x] 8. Failed changes restore affected files from backup where possible and report the remaining error.
- [ ] 9. The inventory and operation history survive application restart.
- [x] 10. The app does not execute third-party installation scripts without an explicit, separate user action.
- [x] 11. Users can export the inventory as JSON.
- [x] 12. The project includes open-source documentation, contribution guidance, and a clear Windows MVP limitation statement.

## Initial project constraints

- Project directory: `D:/develop/package/agent-component-manager`.
- Remote: `https://github.com/z61540471-ux/agent-component-manager.git`.
- Default home view: local inventory first.
- First supported agents: Codex CLI and Claude Code.
- Default write policy: preview, confirm, backup, apply, verify.

## Review evidence ? 2026-09-23

Checked criteria 4 and 5 are supported by registry search/normalization code and frontend source attribution; live network integration remains unverified. Criterion 10 is supported by source review: external commands are never executed. Criterion 12 is supported by README, CONTRIBUTING, SECURITY, LICENSE and the Windows limitation statement.

Criteria 7 and 8 now have Windows fixture evidence: complete Skill/MCP operations,
private ACL backups, persisted history, rescans, injected failures, verified
rollback and external-edit protection. The Windows write capability is enabled.
Criteria 1 and 11 now have implementation and fixture evidence: custom roots are persisted and shared by scan/planning; native JSON export retains Unicode, omits MCP credential values and refuses overwrite. The desktop UI calls that command and reports the saved path.
Criteria 2, 3 and 6 remain blocking for Codex plugin schema/project coverage and broader per-Agent lifecycle/effective bindings.
Criterion 9 remains pending user verification: SQLite reopen tests and startup loading are implemented, but an interactive desktop restart should confirm the last inventory and history remain visible. Interactive scan/export is also recommended smoke verification, beyond the automated export acceptance evidence.
