# Microsoft Store MSIX release

The Store package is built and validated locally on Windows x64, then uploaded
manually. The Standard workflow must never publish to Microsoft Store.

## Contract

- Source version is one plain `X.Y.Z`; package identity is `X.Y.Z.0`.
- Build profile is `stable` plus `microsoft-store`.
- Store manages updates. The Tauri updater plugin, checks, downloads, and NSIS
  update installation are disabled at backend and frontend boundaries.
- The full-trust package contains `unfour.exe`, `unfour-mcp.exe`, build
  metadata, and Store assets.
- The manifest registers `unfour://` and the stable `unfour-mcp.exe` execution
  alias, and declares `runFullTrust`.
- Identity Name, Publisher, and Publisher Display Name come exactly from
  Partner Center; tracked example values are deliberately invalid.

## Build and validate

Install the Windows SDK so `MakeAppx.exe` is available. From the repository
root, provide the exact identity through parameters, environment variables, or
an untracked config derived from `scripts/msix/msix.config.example.json`:

```powershell
$env:MSIX_IDENTITY_NAME = "<Partner Center Identity Name>"
$env:MSIX_PUBLISHER = "<Partner Center Publisher>"
$env:MSIX_PUBLISHER_DISPLAY_NAME = "<Partner Center Publisher Display Name>"
$env:MSIX_DISPLAY_NAME = "Unfour"

pnpm run msix:build
pnpm run msix:validate
```

`scripts/msix/build-msix.ps1` builds the frontend, desktop binary, and unified
MCP sidecar, asks the binary to export compile-time metadata, stages the
manifest/assets, packages with `MakeAppx`, validates, and writes an adjacent
SHA-256 file. Output is under `target/msix/`. An unsigned Store candidate is
for Partner Center upload, not direct installation.

For local installed testing, use `new-test-certificate.ps1`, build with the PFX
mode, trust only the exported public certificate on the disposable test
machine, and remove it afterward. PFX files, certificate passwords, base64
certificates, and private keys are ignored and forbidden by the secret audit.

## Required installed checks

Record the package hash, Windows build, identity, and result for:

1. Install, launch, close, relaunch, upgrade, and uninstall.
2. GitHub login with Unfour closed and running; both must deliver the complete
   `unfour://auth/callback` URI.
3. Settings reports Store-managed updates; Standard update controls and
   network requests are absent.
4. `Get-Command unfour-mcp.exe`, MCP initialize, tools/list, one real tool call,
   and clean exit after stdin closes.
5. API, SSH, SQLite Database, account sign-out, cloud push/pull, conflict,
   snapshot, and basic second-device convergence.
6. Install alongside Standard NSIS in both orders and check protocol ownership,
   shared Stable data, alias behavior, and independent uninstall.

Static validation proves package structure and compiled policy, not Windows
activation, Store servicing, or Partner Center acceptance. Unrun items remain
`NOT VERIFIED`.
