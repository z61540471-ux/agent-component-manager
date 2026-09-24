# Native implementation contracts

Commands: `preview`, `preview_package`, `preview_mcp`, `preview_github`, `preview_plugin`, `read_skill`, `mcp_detail`, `capabilities`, `apply_plan`.

- All executable previews are held server-side. Local package import supports Codex and Claude Code user roots; GitHub import currently targets Codex. Raw binary/configuration bytes are excluded from IPC and SQLite serialization. Configuration previews show a structural diff of server names, transport origin, flags and Skill path bindings; credentials, arguments, full URLs and unrelated configuration are omitted. Backups contain original bytes locally.
- Full Skill import snapshots every file, including binary resources, from a bounded local directory or a GitHub commit resolved once before downloads. Links/submodules and VCS/dependency trees fail closed. Uninstall previews all package files; empty directories remain.
- GitHub imports use immutable commit paths; a repository result is not automatically an installable skill. The user selects the directory containing SKILL.md.
- Codex Skill enablement updates only its path binding. Claude has no documented equivalent per-skill switch; unsupported action is explicit. Shared alias payload mutation is refused. Plugin/synced/system children are not individually writable.
- MCP writes modify configuration registration only. Codex user/project supports install, update, enable/disable, uninstall. Claude user/shared-project supports install, update, uninstall; local project entries and per-project disablement remain native-manager operations. Updates merge fields to preserve existing credentials; transport changes require explicit uninstall/install.
- `preview_plugin` returns a manual Claude CLI command only, verified with installed CLI help for install/update/uninstall/enable/disable and their scope flags. No plugin cache/index edits or third-party execution occurs. Empty change plans cannot apply.
- `mcp_detail(name,version)` resolves official registry version metadata and offers configuration only for a streamable HTTP remote; required credentials still need review. Package-only entries do not fabricate runtime installation instructions.
- `apply_plan` persists intent before mutation, writes synchronized backups and a recovery journal before each target write, rechecks file hashes and package membership, rereads resulting bytes, rescans, and records history. Post-write database errors return the original outcome with warnings and keep in-memory history. Plans are invalidated even when persistence fails.
- Startup inspects unfinished journals read-only. `recovery` lists affected paths/hashes; `recover_operation` requires explicit confirmation and the native write gate. Recovery verifies backup checksums and refuses to overwrite external edits. Completed rollback is excluded from pending records. The history page presents a separate file list and confirmation for recovery.
- MCP Registry details feed a reviewed remote URL into the configuration editor; package-only entries remain manual configuration. New MCP registrations can select configured project roots. Claude plugin inventory entries expose manual lifecycle commands for known registration identities/scopes.

- Windows backups use a protected DACL granting full control only to the current process user and SYSTEM, inherited by backup files. The effective ACL is verified before credential-bearing backups are written; permission failure blocks apply. Junctions, alternate data streams, reserved names, ambiguous paths and unverifiable ancestors are refused.
- Applying a retained plan rescans ownership/content first. A changed inventory invalidates the plan; plugin scan errors block writes. Explicit scans invalidate existing previews. Recovery journals validate identity, unique targets and checksum formats before restoration.
- Windows builds advertise `mutationsEnabled: true` after the verified write iteration. Other platforms remain disabled. Every apply/recovery still requires confirmation; plugin/manual-command plans are not executable.
- A disabled Claude plugin binding marks its child Skill bindings disabled while retaining managed ownership. This does not claim runtime loading for other observed cache entries.

Limitations: TOML comments and JSON formatting are not preserved. Power-loss directory durability, race-proof handle-relative writes against a malicious same-user process, Codex plugin registration writes, automatic runtime installation and automatic native-plugin rollback are not promised. Empty Skill directories remain after uninstall.

## Root settings and inventory export

Settings accepts optional `codexHome` and `claudeHome`, with serde defaults for
previous saved records. Scanning and planning share the same root resolvers.
Claude's user MCP file remains separate at `home/.claude.json`. Selected
projects expose local MCP entries and inherited user bindings with contextual
disablement/shadowing observations; those contextual rows are read-only.
Registered project/local plugins are filtered to selected project roots;
project plugin manifests are observations, not proof of runtime activation.

`export_inventory({destination})` writes only the retained normalized inventory
after a scan. It validates an absolute local `.json` path and existing parent,
uses exclusive creation, writes UTF-8 and synchronizes the file. Existing files
are never replaced. A failed write reports that a partial export may remain.
No configuration arguments, environment values, headers or backup bytes are
included. File paths and component descriptions are inventory metadata.

## Project bindings and ownership

Component records carry optional `projectPath`, `bindingKind` and `ownerId`.
The scanner preserves the user plugin registration and emits inherited rows
for each selected project, applying user, project and local enablement settings
in that order. Owned Skills reference their plugin observation and inherit its
disabled state. These rows describe configuration, not actual runtime loading.

Binding IDs exclude mutable descriptions. Observed paths are compared with
Windows separator/case normalization when classifying aliases; several bindings
to the same observed path are not aliases. Distinct link paths remain distinct.
Diagnostic tags are unique. Legacy snapshots without binding fields deserialize
successfully; old contextual MCP descriptions retain a conservative read-only
fallback until rescanning.

Native Skill planners reject explicit ownership and inherited/local context,
in addition to existing package/path checks. Frontend action policy mirrors
these restrictions. A standalone Skill manifest is still editable: the
manifest observation restriction applies to plugin/MCP observations, not to
every component discovered from a file.

## Codex plugin observations

The scanner accepts portable plugin.json and legacy .codex-plugin/plugin.json
manifests in the Codex cache. Cache packages have owned Skill/MCP observations;
MCP headers, arguments and environment values never enter the inventory.
Declared resource paths must be relative and stay inside the package, including
existing links. Unsupported declarations produce a scan issue.

User/project config.toml plugin tables produce separate configured or disabled
rows, including inherited user settings for selected projects. These rows do
not establish trust, the active cache version or runtime loading. Cache rows
remain cached even when similarly named settings exist. This deliberately
avoids treating retained older versions as active installations.

Codex plugin installation/uninstallation and config mutations remain unsupported;
no cache registration is fabricated. The scanner also observes selected
workspace-local caches at `<project>/.codex/plugins/cache`, retaining project
scope and ownership without claiming trust or runtime loading. Local catalog
source resolution remains outside this iteration. Format evidence:
https://developers.openai.com/plugins/build/plugins (inspected 2026-09-23).
