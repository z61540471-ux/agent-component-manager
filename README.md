# Agent Component Manager

A Windows-first, local inventory for Skills, MCP registrations and Plugins
across Codex and Claude Code. Built with Tauri, React, TypeScript and Rust.

**Status: in development.** See the validation notes before using write
operations on real configurations. Marketplace visibility is not a guarantee
that an item supports installation on every Agent.

## Product scope

- An inventory-first interface with component descriptions, locations, source
  attribution and diagnostic findings.
- Global and selected project scopes, with per-Agent bindings.
- GitHub, official MCP Registry and Claude plugin marketplace discovery.
- Preview and confirmation before mutations, local backups and verification.
- Local SQLite persistence and redacted JSON export.

GitHub stars refer to repositories. Unavailable download/version metadata must
remain unavailable. MCP registration does not imply a running server or an
installed runtime package. Plugin-owned resources are managed through their
owning plugin, not by deleting individual cache files.

## Development

Prerequisites: Node.js, npm, the Rust stable MSVC toolchain, Visual Studio C++
Build Tools and the Windows WebView2 runtime.

```powershell
npm ci
npm run tauri dev
```

```powershell
npm run lint
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri build
```

`npm run dev` starts the frontend only; native filesystem operations require
the Tauri desktop process.

The current development build supports local package and MCP configuration
previews and pinned GitHub Skill imports. Windows builds support confirmed
Skill and MCP configuration writes with private backups and recovery. Claude plugin
actions currently show manual CLI guidance; they are not executed by the app.
See [validation](docs/validation.md) and [native contracts](docs/native-contracts.md)
for tested behavior and outstanding acceptance work.

MCP market details can prefill a reviewed remote URL, and registrations can
target selected projects. The operation history includes recovery records with
explicit confirmation; it never rolls back automatically when the app starts.

In Settings, select your user home and project directories. Optional Codex and
Claude component directories override their default locations. Leaving Codex
blank uses CODEX_HOME when available; leaving Claude blank uses home/.claude.
Claude user MCP registrations remain in home/.claude.json. Save and scan again
after changing these paths.

The inventory separates a component's registration from observations in each
selected project. Project associations and owning plugins appear in details;
scope and state filters help isolate disabled or managed entries.
Read-only context and plugin-owned entries explain why direct modification is
unavailable. A configuration observation does not establish runtime loading.

After a desktop scan, use Export JSON and enter an absolute `.json` path in an
existing directory. Export saves the inventory snapshot without raw MCP
configuration. Existing files are preserved; choose a new filename to export
again. Browser preview does not have native filesystem access.

## Project layout

- `src/`: desktop interface and native bridge.
- `src-tauri/`: Rust scanning, persistence, registry and change operations.
- `docs/`: approved requirements, design, plan and validation notes.

## Prior art

Product research includes [Skill Manager](https://github.com/mode-io/skill-manager),
[SkillBuddy](https://github.com/konnga/skill-buddy), and
[Agent Skills Marketplace & Manager](https://marketplace.visualstudio.com/items?itemName=JakeSterns.agent-skills-vscode).
These projects informed the inventory and discovery direction; mentioning them
does not imply affiliation or endorsement.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
Licensed under [MIT](LICENSE).

Market search supports loading subsequent pages, exact-page cache fallback and
rate-limit messages. Codex plugin configuration is shown separately from cached
portable/legacy packages, with ownership links for their Skills and MCP entries.
Codex plugin actions remain read-only; use Codex to install or remove them.
