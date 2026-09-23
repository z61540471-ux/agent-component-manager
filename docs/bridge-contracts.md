# Desktop bridge contracts

## 1. Scope and trigger

Apply these contracts when changing React editors, Tauri commands, persisted
plans or filesystem writes. Native implementation details are in
`native-contracts.md`; current verification evidence is in `validation.md`.

## 2. Signatures

- `read_skill({ id }) -> string`
- `export_inventory({ destination }) -> string` (saved absolute JSON path)
- Settings adds optional `codexHome` and `claudeHome`; older saved settings remain readable.
- `preview({ action, id, name, content }) -> Plan`
- `preview_package({ action, id, name, sourcePath, agent }) -> Plan`
- `preview_mcp({ action, id, name, content, agent, project }) -> Plan`
- `preview_github({ repository, revision, subdirectory, name, action, id }) -> Plan`
- `preview_plugin({ action, name, scope }) -> Plan` (manual guidance only)
- `apply_plan({ id, confirmed }) -> Operation`
- `recovery() -> RecoveryRecord[]` (read-only)
- `recover_operation({ id, confirmed }) -> Operation` (same native write gate)
- `capabilities() -> { mutationsEnabled, skillPackages, mcpConfiguration,
  githubPinnedImport, plugins, claudeSkillEnablement }`

## 3. Payload and persistence contracts

Component observations add optional `projectPath`, `bindingKind` and `ownerId`
fields. Missing fields in previous SQLite records deserialize as null.
`bindingKind` distinguishes `registration`, `inherited`, `local` and `manifest`.
`ownerId` identifies the owning plugin observation for a managed Skill.
Identity and write eligibility must use these fields, never human-readable
descriptions. Description edits must not create a new registration identity.
Inherited/local context observations are not independent writable registrations.

IPC uses camelCase fields. A Plan has `id`, `title`, `changes`, `warnings`,
and `createdAt` (Unix seconds). A change contains `path`, `beforeHash`,
`before` and `after`; absent before/after is null. Native raw byte snapshots
are skipped by serialization. Never reconstruct executable plans from the
redacted IPC/SQLite document; apply only the plan retained in native memory.

An Operation has `id`, `title`, `status`, `message`, and `at`. Successful
application uses status `completed`, not `success`. Configuration backup
bytes can contain credentials and stay outside the repository.

Post-write persistence warnings use `completed_with_warnings` or
`failed_with_warnings`; the original file outcome is preserved. The frontend
retains that result even if a subsequent history/inventory refresh fails.
Recovery records expose affected paths and hashes, never raw backup bytes.
`rolled_back` is terminal and must not appear as pending on restart.

SQLite stores JSON in `state(key PRIMARY KEY, value)` and
`operations(id PRIMARY KEY, value)`. Saving settings and clearing the persisted
inventory is one transaction; failure preserves both previous values. Only
after commit does memory change and retained plans become invalid.

Explicit Agent roots take precedence over discovery defaults. Codex falls back
to CODEX_HOME, then home/.codex; Claude component storage falls back to
home/.claude. Claude user MCP configuration remains home/.claude.json.
Inventory export uses the native retained scan, requires an absolute local
JSON destination with an existing parent, and refuses to replace an existing
file. It never exports raw MCP configuration or backup bytes.

## 4. Validation and error matrix

| Condition | Required behavior |
| --- | --- |
| Native mutation gate disabled | Reject apply before any component write |
| Confirmation absent | Reject apply |
| Unknown or consumed plan ID | Require a new preview |
| Target differs from captured hash | Reject stale plan |
| Component ownership/content inventory changes | Reject retained plan and require new preview |
| Explicit rescan occurs | Invalidate retained previews |
| Windows backup ACL cannot be verified | Fail before component writes |
| Package gains or loses a file after preview | Reject stale package snapshot |
| SQLite fails before intent is persisted | Do not modify component files |
| SQLite fails after filesystem apply | Rescan memory, invalidate plans, retain warning and recovery evidence |
| Recovery sees an external edit or corrupt backup | Preserve current file and report recovery conflict |
| Plugin/cache/synced child | Refuse independent payload mutation |
| Inherited/contextual registration | Show its project association; refuse independent writes |
| Plugin description changes | Preserve binding identity |
| Invalid MCP JSON/TOML | Fail without replacing the original file |
| Manual plugin plan | No executable changes and no success claim |
| Error from apply | Display error; require renewed confirmation |

## 5. Good, base and bad cases

- Good: import a directory containing UTF-8 SKILL.md and binary assets;
  preview all files and retain exact bytes.
- Base: update a Claude MCP URL; preserve omitted headers and unknown fields.
- Bad: preview a file, modify it externally, then apply the old ID; the
  external edit must survive.
- Good: a user plugin stays installed while its project binding and owned
  Skills show disabled in the project that overrides enablement.
- Bad: treating two project bindings to one package as two removable copies.

## 6. Required tests

- Native fixtures: byte preservation, stale-plan rejection, credential
  omission from serialization, credential preservation in merged config,
  plugin ownership refusal and failure-injection rollback.
- Restart recovery listing must be read-only. Confirmed rollback must work
  without the original in-memory Plan and reject mismatched backup hashes.
- UI: disabled gate and unchecked confirmation prevent application;
  double clicks invoke once; errors remain visible; updating a Skill begins
  with the selected content.
- Windows build: valid ICO resource exists. Frontend tests alone cannot
  establish native correctness or installer readiness.
- Binding fixtures: stable IDs after description edits, distinct selected
  projects, plugin state inheritance, old snapshot compatibility and denied
  writes to contextual rows. UI must explain read-only actions before preview.
- Windows write fixtures verify effective backup directory/file ACLs, junction
  refusal, case aliases, recovery identity tampering, full Skill/MCP lifecycle
  and newly registered plugin ownership invalidating a previous plan.

## 7. Wrong versus correct

Wrong: rename a shared SKILL.md to disable it in one Agent.

Correct: change the supported Agent-specific binding. If that Agent has no
supported per-Skill disable contract, show the capability as unsupported.

Wrong: report a displayed plugin command or MCP registration as an installed,
running runtime.

Correct: report precisely the registration or manual instruction produced.
