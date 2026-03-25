# Install EDGEL

EDGEL v0.1.0 is designed to install from prebuilt binaries. End users do not need Rust.

## One-command install

```bash
curl -sSL https://edgel.sh | bash
```

Windows PowerShell installer:

```powershell
irm https://edgel.sh | iex
```

The installer script:

- detects OS and CPU architecture
- downloads the matching release archive
- installs `edgel` and `goldedge-browser`
- adds them to `~/.local/bin` by default

## Manual install

1. Download the matching archive from the GitHub release page.
2. Extract it.
3. Copy `edgel` and `goldedge-browser` into a directory on your `PATH`.

## Expected release artifacts

- `edgel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `edgel-v0.1.0-x86_64-apple-darwin.tar.gz`
- `edgel-v0.1.0-aarch64-apple-darwin.tar.gz`
- `edgel-v0.1.0-x86_64-pc-windows-msvc.zip`
- `edgel.ps1` (PowerShell installer)

## From a local checkout

If you are the release engineer, package binaries with:

```bash
scripts/release.sh
```
