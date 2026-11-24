# Release and updater workflows

The `release` workflow creates a release in `moiCR/rDM` using `CHANGELOG.md`, seeds `latest.json`, invokes the `Windows`, `Linux`, and `Extensions` workflows, and combines the signed Windows and Linux platform entries into the final `latest.json` manifest. Each platform workflow also supports manual runs against an existing release tag to replace assets and merge updated manifest entries without recreating the release.

## Key generation & GitHub configuration

Public key `371A49087FDA6373` is embedded in `crates/core/src/updater.rs`, so both local builds and CI builds verify signed releases out of the box.

Configure these values in `moiCR/rDM` (**Settings -> Secrets and variables -> Actions**):

- **Secret `RDM_SIGNING_PRIVATE_KEY`** *(required)*: contents of the private Minisign key paired with public key `371A49087FDA6373`. Used by `tauri signer sign` during CI builds.
- **Secret `RDM_SIGNING_PRIVATE_KEY_PASSWORD`** *(optional)*: password protecting `RDM_SIGNING_PRIVATE_KEY` (leave unset or empty if the key has no password).
- **Variable or Secret `RDM_UPDATER_PUBLIC_KEY`** *(optional)*: overrides the embedded public key at compile time if rotating keys without editing source first.

GitHub's `GITHUB_TOKEN` publishes releases and assets automatically.

## Verification & installation

- Each release publishes `latest.json` containing `version`, `notes`, `pub_date`, and platform entries (`url`, `signature`, and `size`).
- Downloads stream atomically into a private `.part` file (`0o700` on Unix) while reporting live byte progress (`AtomicU64`), enforcing both expected size and a 1 GiB ceiling before running streaming Minisign verification in a blocking worker.
- Trusted comments inside every `.sig` payload are verified against `version:<version>` (or `app-version:<version>`) and `file:<filename>` (when present) before the package is staged and installed:
  - **Windows x64**: silent Inno Setup install (`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS`), automatically relaunching rDM.
  - **Linux AppImage x64**: atomic swap with `.backup` rollback and automatic restart.
  - **Linux Debian / RPM x64**: detected via `package-format` marker or `dpkg-query` / `rpm`, elevated install via `pkexec` (`apt-get`, `dnf`, `yum`, or `zypper`), and automatic restart.

Release tags must match `version` in `Cargo.toml` (for example `1.3.0` and `v1.3.0`).
