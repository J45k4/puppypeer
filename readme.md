# puppyagent

Peer-to-peer agent that exposes file metadata and transfer operations over libp2p.

## File Metadata Protocol

The custom `/puppy/agent/0.0.1` request/response protocol supports:

- `ListDir { path }` → returns `DirEntries(Vec<FileEntry>)`
- `StatFile { path }` → returns `FileStat(FileEntry)`
- `ReadFile { path, offset, length }` → returns a ranged `FileChunk { offset, data, eof }`
- `WriteFile { path, offset, data }` → returns `WriteAck { bytes_written }`
- `ListCpus` → returns `Cpus(Vec<CpuInfo>)`
- `ListDisks` → returns `Disks(Vec<DiskInfo>)`
- `ListInterfaces` → returns `Interfaces(Vec<InterfaceInfo>)`
- `Error(String)` is used for any failure case

All messages are JSON encoded. `offset` and `length` are byte-based; `length` may be omitted to request up to a 4&nbsp;MiB chunk. `WriteFile` creates files as needed and zero-fills gaps before writing.

Each `FileEntry` includes the item name, whether it is a directory, optional extension, and file size when available.

`CpuInfo` exposes each logical processor's label, nominal frequency, and most recent usage sample.

`DiskInfo` contains filesystem metadata, capacity, usage percentage, cumulative read/write counts, and drive characteristics.

`InterfaceInfo` captures MAC address, IP networks, traffic counters, error counters, and MTU for every detected link.

`FileChunk.eof` indicates whether the returned chunk reached the end of the file, enabling callers to page through a file by issuing sequential `ReadFile` requests with increasing offsets.

## Control Plane Protocol

The `/puppy/control/1.0.0` JSON request/response protocol manages identity, sessions, and permissions for the data plane. Supported requests:

- `Authenticate { method }` → `AuthSuccess { session }` or `AuthFailure { reason }`
- `CreateUser { username, password, roles, permissions }` → `UserCreated { username }`
- `CreateToken { username, label, expires_in, permissions }` → `TokenIssued { token, token_id, username, permissions, expires_at }`
- `GrantAccess { username, permissions, merge }` → `AccessGranted { username, permissions }`
- `ListUsers` → `Users(Vec<UserSummary>)`
- `ListTokens { username }` → `Tokens(Vec<TokenInfo>)`
- `RevokeToken { token_id }` → `TokenRevoked { token_id }`
- `RevokeUser { username }` → `UserRemoved { username }`
- TODO `GetVersion` → `VersionInfo { version, commit, build_date, channel }`
- TODO `CheckUpdate { channel }` → `UpdateStatus { current, latest, available, notes_url }`
- TODO `UpdateVersion { version, channel, force }` → `UpdateStarted { job_id, target }`
- TODO `UpdateStatus { job_id }` → `UpdateState { status, pct, bytes_fetched, eta_secs, message }`
- TODO `RollbackUpdate` → `RollbackStarted { job_id }`
- TODO `BeginUploadUpdate { target, version, size, sha256, signature }` → `UploadSession { upload_id, offset }`
- TODO `UploadUpdateChunk { upload_id, offset, data }` → `ChunkAck { next_offset }`
- TODO `CommitUploadUpdate { upload_id }` → `UpdateStarted { job_id, target }`
- TODO `AbortUploadUpdate { upload_id }` → `UploadAborted { upload_id }`
- `Error(String)` is returned for unrecoverable failures

`AuthMethod` accepts either `Credentials { username, password }` or bearer `Token { token }`. Credential sessions expire after one hour; token sessions honour the optional `expires_in` deadline and can be revoked explicitly. The very first `CreateUser` call bootstraps the directory and automatically grants the `owner` role so that operators can mint additional accounts and tokens.

### Update semantics
`UpdateVersion` downloads binaries from GitHub Releases. The `version` may be an explicit SemVer tag like `v0.3.4` or omitted/empty to select the latest release on the specified `channel` (defaults to `stable`). Supported channels: `stable` (latest non-prerelease) and `prerelease`. Assets are expected to follow the pattern `puppyagent-${target}-${version}.tar.gz` accompanied by `.sha256` and `.sig` files. The node verifies the SHA-256 checksum and an Ed25519 signature against a built-in public key before installation. Updates are applied atomically by unpacking to a new directory, switching a versioned symlink, and then restarting the service. If the new binary fails a health check on startup, the node automatically rolls back to the previous version. `UpdateStarted` returns a `job_id`; clients can poll progress via `UpdateStatus { job_id }` until a terminal `status` of `success` or `error`.

**Offline / pushed updates over libp2p**: For peers without outbound internet access, the same tarball can be uploaded over the control plane. Use `BeginUploadUpdate` to declare metadata (`size`, `sha256`, and detached Ed25519 `signature`). The server returns an `upload_id` and `offset`. Transfer proceeds with `UploadUpdateChunk` until complete, then `CommitUploadUpdate` verifies checksum and signature and starts the same atomic swap + restart flow. `AbortUploadUpdate` cancels and deletes a partial upload. Upload sessions support resume via the `offset` field and expire if idle.

Capability checks reuse a shared set of `PermissionGrant` values:

- `Owner` – unrestricted access to all resources
- `SoftwareUpdate` – allows checking for updates, performing updates, and rolling back
- `Viewer` – read-only access to metadata, system, disk, and network queries
- `Files { path, access }` – scoped filesystem access where `access` is either `Read` or `ReadWrite`
- `SystemInfo`, `DiskInfo`, `NetworkInfo` – individual toggles for CPU, storage, and interface queries

Owner sessions implicitly satisfy every check. Tokens inherit the permissions supplied at issuance; user sessions inherit the permissions currently assigned to the account.

`SoftwareUpdate` is required for `CheckUpdate`, `UpdateVersion`, `UpdateStatus`, and `RollbackUpdate`. The `Owner` role implicitly includes `SoftwareUpdate`.

## Features

- Fetch folder content
- Fetch filesystem path metadata its size, modification date ....
- Read and write files
- Query CPU, network interfaces..
- Stream display content
- Control mouse and keyboard
- Synchronize folder with full or selective syncing 
- Self-update to a specific version or latest release
- Verified downloads from GitHub Releases (SHA-256 + Ed25519)
- Automatic rollback on failed update
- Offline/air-gapped updates via libp2p upload

## Update Command

Invoke `puppyagent update` to download and apply the latest release. Pass `--version <tag>` to target a specific GitHub release tag (for example, `puppyagent update --version 20240214`). The updater still verifies the downloaded binary signature before installing it.

## Authentication

Every connection still performs the libp2p handshake so peers can verify the remote `PeerId`. User- and token-level authentication now flows through `/puppy/control/1.0.0::Authenticate`, which returns a `SessionInfo` payload (session id, username, roles, permissions, and expiration). When no users exist the node operates in an open mode for bootstrapping; after the first account is created every file-metadata request must present an active session. Credential sessions last one hour, while token sessions honour their configured expiry or can be revoked instantly.

## Permissions

Permissions are expressed with the `PermissionGrant` values documented above. File and system operations query the stored set on every request so that revocations and role updates take effect immediately. Grants can be merged or fully replaced through the `GrantAccess` control-plane call, and revoking a token immediately tears down any sessions that were created from it.


## Development

- Format with `cargo fmt`
- Test with `cargo test --frozen --offline`
- Lint with `cargo clippy`

## Release packaging
GitHub Releases should include platform-specific tarballs named `puppyagent-${target}-${version}.tar.gz` and sidecar files `${asset}.sha256` and `${asset}.sig`. The signature is created over the tarball bytes using the project’s release Ed25519 key. At runtime the updater downloads these three files, verifies the checksum and signature, and only then swaps the active binary. Targets might be like `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, or `x86_64-pc-windows-msvc`. Keep assets small by excluding debug symbols from release archives.

For offline uploads, clients must provide the same tarball, its `.sha256`, and detached `.sig`. The server checks the declared `size`, verifies hash and signature, and only then activates the version. Temporary staging directories are used for resumable uploads before promotion.
