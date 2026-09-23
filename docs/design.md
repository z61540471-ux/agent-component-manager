# Agent Component Manager Technical Design

## Architecture

The application is a Windows-first Tauri desktop application with a React UI and Rust local core.

```text
React UI
  Inventory / Component Detail / Marketplace / Change Preview / Settings
        | Tauri commands
Rust core
  agents / components / scanners / registries / planner / executor / storage
        | local filesystem and configuration files
Codex CLI + Claude Code state
```

The UI never writes agent configuration directly. Tauri commands call the Rust core, which validates paths and returns serializable results.

## Module boundaries

- `agents`: agent-specific roots, config locations, enablement rules, and target bindings.
- `components`: normalized Skill, MCP, and Plugin records and status calculation.
- `scanners`: read-only discovery and content hashing.
- `registries`: GitHub, MCP Registry, and Claude Plugin Marketplace clients. Registry clients return normalized metadata and never execute install commands.
- `planner`: resolves an install/update/uninstall request into a Change Plan containing reads, creates, edits, deletes, backups, warnings, and required confirmation.
- `executor`: applies an approved plan, writes backups, performs atomic file updates where possible, rescans affected targets, and rolls back on failure.
- `storage`: SQLite schema, cache, inventory snapshots, and operation history.

## Unified component model

Each component has a stable local ID, kind (`skill`, `mcp`, `plugin`), name, description, version, source, source URL, repository URL, optional Star/download metrics with provenance, installed targets, scopes, install path, status, content hash, and last-seen timestamp.

The model records both observed filesystem state and effective agent bindings. This prevents a file that exists but is shadowed, disabled, or outside an active discovery root from being reported as active.

### Implementation clarifications

- Keep physical location identity, component identity and per-Agent bindings separate. Junction aliases to one resolved location are not independent duplicate installs.
- Represent enablement and diagnostic findings separately; a component may be enabled and also duplicated. Unknown runtime loading is reported as unknown, not asserted active.
- Plugin-owned Skills remain children of their owning package. They cannot be independently removed from a plugin cache.
- An MCP configuration registration is not an installed binary or running process. Removing registration does not remove shared npm/Python packages or kill processes.
- Project scans cover user-selected project roots, not an unrestricted full-disk crawl. Exclude dependency trees and prevent reparse-point traversal loops.
- Export and UI redact configuration secrets. Raw configuration belongs only in local protected backups, never public source control or exported inventory.
- Source resolution must preserve exact provenance. GitHub repository popularity is repository-level, not an individual Skill download counter.
- SQLite inventory indexes are rebuildable; operation history, confirmed plans and backup references are not recreated by a scan and must be preserved.
- Automated rollback guarantees cover app-owned file/configuration changes only. Package-manager or external-command side effects require explicit capability reporting, not a promise of full rollback.

## Scan flow

1. Detect or load configured Codex and Claude Code roots.
2. Enumerate global and project Skill directories, MCP configuration files, and Plugin manifests.
3. Parse supported manifests and `SKILL.md` frontmatter.
4. Hash relevant files and normalize records.
5. Compare names, sources, paths, hashes, and bindings to classify duplicate content, conflicts, drift, broken entries, and disabled components.
6. Persist the snapshot to SQLite and expose it to the UI.

Scan errors are retained per path and do not discard successful records from other roots.

## Marketplace flow

Each registry adapter implements search, detail lookup, and install-source resolution. The first adapters are GitHub, the official MCP Registry, and Claude Plugin Marketplace. GitHub Star and download values are optional and labeled with their source and retrieval time. Missing metrics are shown as unavailable rather than synthesized.

## Change Plan and safety

All mutating operations follow:

```text
request -> validate -> resolve -> Change Plan -> user confirmation
       -> backup -> apply -> rescan -> record result
```

Change Plans list every created, modified, and deleted file, configuration key changes, required permissions, backup paths, and rollback capability. The executor refuses to apply a plan whose target files changed after planning unless the user regenerates the plan. Third-party commands are displayed for review and are not automatically executed in the MVP.

## Persistence

SQLite stores normalized components, observed locations, registry cache, scan issues, operation plans, operation results, and backup references. The database is local-only and can be rebuilt from a fresh scan.

## UI structure

- Overview: inventory counts, active targets, duplicate/conflict counts, and broken entries.
- Inventory: searchable and filterable list by agent, kind, scope, status, source, and path.
- Component detail: metadata, bindings, paths, hash, scan issues, and available actions.
- Marketplace: search and detail pages with source attribution and metrics.
- Change Preview: file-level diff and confirmation controls.
- Settings: roots, registry refresh, backup directory, cache, and logs.

## Error handling

- Missing roots: show setup guidance and allow manual path selection.
- Parse failure: retain the path as a scan issue with a raw preview where safe.
- Registry unavailable: show cached results and stale timestamp.
- Write failure: stop at the failed operation, restore backups, rescan, and report partial rollback if restoration fails.
- Permission failure: show the exact target and required operation without retrying silently.

## Validation strategy

- Unit tests for frontmatter/config parsing, normalization, duplicate classification, plan generation, and rollback bookkeeping.
- Fixture-based scanner tests for representative Codex and Claude layouts.
- Registry adapter contract tests using recorded responses and malformed payloads.
- Rust integration tests for backup/apply/rescan/rollback in temporary directories.
- UI tests for inventory filters and Change Plan confirmation.
- Windows smoke test covering discovery, read-only scan, dry-run plan, and a reversible fixture installation.

## Prior art and external references

- Skill Manager: https://github.com/mode-io/skill-manager
- SkillBuddy: https://github.com/konnga/skill-buddy
- Agent Skills VS Code extension: https://marketplace.visualstudio.com/items?itemName=JakeSterns.agent-skills-vscode
- Official MCP Registry: https://registry.modelcontextprotocol.io/docs
- Claude plugin management: https://support.claude.com/en/articles/13837433-manage-plugins-for-your-organization
