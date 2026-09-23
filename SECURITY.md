# Security

Report vulnerabilities privately through the repository's GitHub security
advisory reporting feature when available. Do not attach tokens, real MCP
environment variables or unredacted inventories to public issues. If private
reporting is unavailable, open an issue requesting a private contact channel
without exploit details.

The application manages local files. Read-only scans must not launch MCP
servers or third-party scripts. A marketplace listing is not a trust guarantee.
File mutations must be previewed, confirmed, checked against current state,
backed up and verified. A failed rollback must be reported explicitly.

Popularity metadata must not be presented as security certification. Backups
can contain configuration secrets and must remain local and outside Git.

On Windows, backups are restricted to the current user and SYSTEM using a
protected, verified DACL before file changes begin. This is not a security
boundary against administrators or malicious processes running as the same
user. The application rejects junction/reparse write paths and rechecks files,
package membership and observed ownership. It does not guarantee protection
against all concurrent path replacement races or sudden power loss.
