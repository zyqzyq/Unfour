# Distribution architecture

Unfour has one source tree, one desktop product, and two distribution values.
Release channel (`test` or `stable`) selects services and storage; distribution
selects the update authority.

| Distribution | Package | Delivery | Update authority |
| --- | --- | --- | --- |
| `standard` | Windows NSIS, macOS packages, Linux x64 AppImage (Experimental) | GitHub Release, Cloudflare R2, and unfour.dev | Unfour updater at `https://releases.unfour.dev/stable/latest.json` |
| `microsoft-store` | Windows x64 MSIX | Manual Partner Center upload | Microsoft Store |

There is no second application or repository for Pro, and no Website package
kind. Account, entitlement, cloud sync, desktop, command bus, and `unfour-mcp`
are shared by both distributions. A paid account plan can still be called Pro;
that is an entitlement name, not a second client or repository boundary.
`standard` and `microsoft-store` describe distribution types, not Free or Pro
subscription tiers.

## Delivery stages

The four delivery stages have deliberately different responsibilities:

| Stage | Purpose | External side effects |
| --- | --- | --- |
| CI | Unit tests, Rust checks, release contracts, and migration/secret gates | None beyond CI logs |
| Release Candidate | Full verification plus real signed Standard Tauri bundles for Windows x64, macOS arm64/x64, and Linux x64 | Uploads per-platform GitHub Actions artifacts only |
| Release | Publishes an already reviewed Standard candidate to the GitHub Release and immutable R2 version path, then promotes the stable manifests | GitHub Release, R2 versioned files, then `stable/downloads.json` and finally `stable/latest.json` |
| Microsoft Store | Builds and validates the independent Windows x64 MSIX path | Manual Partner Center submission only |

The Release Candidate and Release workflows both call
`.github/workflows/reusable-standard-build.yml`. That reusable workflow owns
the complete verification suite, target matrix, real Tauri release command,
updater signing checks, canonical filenames, and per-platform Actions artifact
upload. Neither caller has a second copy of those build steps.

### Standard Release Candidate: build without publication

Run **Standard Release Candidate** manually in GitHub Actions. Its `ref` input
defaults to `main` and accepts a trusted branch or commit SHA. The identity job
resolves that input once and passes the exact commit SHA to every verification
and build job, so a branch moving during the run cannot mix commits across
platforms. Only use reviewed refs: the selected source is built with the
updater signing secret.

The run executes the same full verification and build core as a formal
release, including:

```text
pnpm run tauri build --config src-tauri/tauri.release.conf.json
```

The reusable workflow's actual `build` job binds the GitHub Environment
`production` and reads `TAURI_SIGNING_PRIVATE_KEY` plus
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` directly from that Environment. The
candidate caller does not forward signing secrets. The build fails explicitly
if the private key is empty. Successful runs upload:

- `release-candidate-x86_64-pc-windows-msvc`: canonical Windows x64 NSIS
  installer and `.sig`;
- `release-candidate-aarch64-apple-darwin`: canonical macOS arm64 DMG,
  `.app.tar.gz`, and `.app.tar.gz.sig`;
- `release-candidate-x86_64-apple-darwin`: canonical macOS x64 DMG,
  `.app.tar.gz`, and `.app.tar.gz.sig`; and
- `release-candidate-x86_64-unknown-linux-gnu`: canonical Linux x64 AppImage
  and `.sig` only.

The candidate workflow has `contents: read` permission. It does not create or
move a tag, create a GitHub Release, access R2, generate or publish
`latest.json` or `downloads.json`, or build/publish MSIX. Download the
Actions artifacts and complete the installer, startup, updater, OS trust, and
uninstall checks before formal publication.

## Standard: build once, publish twice

A plain `vX.Y.Z` tag is the only Stable release input. The workflow verifies
that Cargo, root/desktop package.json, and Tauri all contain the same `X.Y.Z`,
then builds each native target once with `distribution=standard`. Tauri creates
signed updater artifacts using signing secrets held in the GitHub Environment
`production` and read by the reusable workflow's `build` job.

The formal workflow resolves the release tag to an exact commit and calls the
same reusable build core used by Release Candidate. Each matrix job renames
its output to a canonical filename and uploads that exact file to the
aggregation job. `scripts/finalize-standard-release.mjs` discovers regular
files in `release-assets/`, requires the four canonical public installers and
all four signed updater artifacts, then generates `SHA256SUMS.txt`, Tauri
`latest.json`, and public `downloads.json`. A missing installer fails the
release; a macOS updater archive can never substitute for a missing DMG.
It uploads the same staged artifact bytes to both destinations:

```text
merge reviewed changes into main and set an unused X.Y.Z source version
  -> create vX.Y.Z tag (Standard Release)
  -> resolve exact tag commit
  -> reusable Standard verify and one native build per target
  -> finalize release-assets/SHA256SUMS.txt, latest.json, downloads.json
  -> upload immutable assets to Cloudflare R2 stable/X.Y.Z/
  -> download from R2 and verify SHA-256
  -> GitHub Release vX.Y.Z from the same staged files
  -> check versions of both live stable manifests
  -> R2 stable/downloads.json
  -> R2 stable/latest.json (uploaded last)
```

Neither stable manifest is promoted until all versioned artifacts have been
uploaded and re-downloaded successfully, SHA-256 checks pass, and the GitHub
Release succeeds. The final promotion reads both live manifests and compares
versions by numeric SemVer fields. A `404` permits first publication of that
manifest; a newer or equal candidate is allowed, while a candidate older than
either manifest fails. Other read or parse failures block both promotions.
Equal-version reruns reach this gate only after existing immutable objects
under `stable/X.Y.Z/` pass the byte and checksum verification above.

The two pointer writes are sequential, not atomic. If downloads promotion
succeeds but updater promotion fails, downloads can temporarily be newer.
Checking both versions prevents a later older run from rolling downloads back;
a rerun of the same version can complete promotion after verification.

The formal `publish` job also binds the `production` Environment and reads
`R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_ACCOUNT_ID`, and `R2_BUCKET`
for Cloudflare R2 publication. GitHub and R2 never invoke separate builds.
Both manifests point to the immutable R2 version path. All six values belong
in the GitHub Environment `production`; none belongs in repository files.
Release Candidate only has the
shared signed-build capability and does not receive R2 or GitHub Release
publication authority.

### Independent stable metadata contracts

| URL | Consumer and contract |
| --- | --- |
| `https://releases.unfour.dev/stable/latest.json` | Tauri Updater only: `version`, `notes`, `pub_date`, and signed `platforms` entries |
| `https://releases.unfour.dev/stable/downloads.json` | Website / Download Worker: `version` and public installer `downloads` URLs |
| `https://releases.unfour.dev/stable/{version}/*` | Immutable release assets and `SHA256SUMS.txt` |

`latest.json` and `downloads.json` are mutable stable channel metadata, not
immutable release binaries. Both are excluded from the versioned R2 upload and
from `SHA256SUMS.txt`; the checksum file also excludes itself. Checksums cover
the staged installers, updater archives, and signatures. GitHub Release also
attaches the generated manifest snapshots, but they are not checksum entries.
Both stable pointers use `Content-Type: application/json` and
`Cache-Control: no-cache`.

The public download schema is:

```json
{
  "version": "1.2.3",
  "downloads": {
    "windows-x64": {
      "url": "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_windows_x64.exe"
    },
    "macos-arm64": {
      "url": "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_macos_arm64.dmg"
    },
    "macos-x64": {
      "url": "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_macos_x64.dmg"
    },
    "linux-x64": {
      "url": "https://releases.unfour.dev/stable/1.2.3/Unfour_1.2.3_linux_x64.AppImage"
    }
  }
}
```

Website and Download Worker consumers must read these URLs directly rather
than infer filenames from a version or use updater URLs as installer links.
The release script accepts only plain stable `X.Y.Z` versions and absolute
HTTP(S) base URLs without credentials, query strings, or fragments; it removes
trailing slashes and preserves a custom base path before appending `stable/`.

| Target | Public download key / installer | Tauri updater key / artifact |
| --- | --- | --- |
| Windows x64 | `windows-x64` / `.exe` | `windows-x86_64` / same `.exe` with signature |
| macOS arm64 | `macos-arm64` / `.dmg` | `darwin-aarch64` / `.app.tar.gz` with signature |
| macOS x64 | `macos-x64` / `.dmg` | `darwin-x86_64` / `.app.tar.gz` with signature |
| Linux x64 | `linux-x64` / `.AppImage` | `linux-x86_64` / same `.AppImage` with signature |

On macOS, `.dmg` is the user installer and `.app.tar.gz` is exclusively the
Tauri updater artifact. No website installer fields are added to `latest.json`,
and its platform keys and URL mappings remain unchanged.

For the already published Stable version, the operator will manually create
and upload `stable/downloads.json` to R2 as a one-time migration. There is no
backfill workflow, historical rebuild, tag change, or automatic migration.
Automatic generation begins with the next new Standard Release tag containing
this change. No new GitHub Actions secrets, R2 bucket, or R2 permission is
required beyond the existing Standard publishing setup.

### Linux (Experimental)

The Standard release currently publishes one Linux format: x86_64 (x64)
AppImage only. Linux remains Experimental. Ubuntu 22.04 or newer is the current
Linux runtime/test baseline; Ubuntu 20.04 is not supported. This is not a
compatibility guarantee for every distribution with glibc 2.35 or newer.
The canonical public assets are:

- `Unfour_X.Y.Z_linux_x64.AppImage`
- `Unfour_X.Y.Z_linux_x64.AppImage.sig`

The updater manifest has one Linux entry, `linux-x86_64`, pointing to the
AppImage. Tauri may still generate `.deb` and `.rpm` files during the Linux
build, but they remain CI intermediates: they are not copied into
`release-assets/`, `SHA256SUMS.txt`, GitHub Releases, Cloudflare R2, or
either stable manifest. Linux ARM64 is not a published Standard target.

The shared Standard `build` job pins `x86_64-unknown-linux-gnu` to
`ubuntu-22.04` for both RC and Release. Linux dependency installation and staging
select that target, not a moving runner label. Its Rust cache key also includes
the runner label so earlier Ubuntu 24.04 native dependencies cannot be restored
from the former Linux build cache. Windows/macOS runners and cache keys are
unchanged. The independent `verify` job can stay on `ubuntu-latest`: it uploads
no native artifacts for packaging, uses a separate Rust cache key, and the
actual build recompiles the release MCP sidecar via Tauri's `beforeBuildCommand`.
Identity and publication jobs do not compile release binaries either.

Ubuntu's Jammy package indexes list amd64 builds of
[`libwebkit2gtk-4.1-dev`](https://packages.ubuntu.com/jammy/libwebkit2gtk-4.1-dev),
[`libsoup-3.0-dev`](https://packages.ubuntu.com/jammy-updates/libsoup-3.0-dev), and
[`libjavascriptcoregtk-4.1-dev`](https://packages.ubuntu.com/jammy/libjavascriptcoregtk-4.1-dev).
[Tauri's AppImage guidance](https://v2.tauri.app/distribute/appimage/) also names
Ubuntu 22.04 as a suitable WebKitGTK 4.1 build baseline. This is package-level
evidence, not a successful hosted-runner apt/build or runtime test. Keep the
standard Jammy updates/security repositories enabled. If apt
cannot resolve the required packages, stop and record its output; do not fall
back to `ubuntu-latest`.

Runner lifecycle follow-up: GitHub has
[announced](https://github.com/actions/runner-images/issues/14254) deprecation of
`ubuntu-22.04` beginning 2026-09-17 and retirement on 2027-04-17. Before then,
explicitly review how to preserve or revise the Linux build/runtime baseline;
runner retirement must not trigger an unreviewed upgrade to `ubuntu-latest`.

`pnpm run test:release-env` guards the pinned Linux runner, target-based steps,
and native-cache isolation. No broad bundled-ELF ABI scanner is added: a check
of only the main executable would miss runtime-loaded dependencies, while an
unvalidated scan of every bundled GTK/WebKitGTK library could misclassify symbol
definitions or unused libraries. The runner/cache contract is not an ABI or
runtime proof. Require a newly built candidate to pass real Ubuntu 22.04 x64
startup/module/relaunch checks and an Ubuntu 24.04 x64 launch smoke, plus the
existing Linux updater smoke. See
[manual cases](../testing/manual-test-cases.md#linux-appimage-experimental).

The published v0.9.0 AppImage predates this fix and is not rebuilt or replaced.
Its Ubuntu 20.04 runtime failure and the still-unverified Ubuntu 22.04+
regression are recorded in
[release verification](../testing/release-verification.md#linux-appimage-compatibility).

## Microsoft Store

MSIX remains an intentional manual path: local Windows x64 build, local
validation, then manual upload and certification. The Standard release
workflow neither builds MSIX nor calls Store publication tooling. See
`docs/release/msix.md`.

The Store binary is compiled with `channel=stable` and
`distribution=microsoft-store`. It uses the same APIs and Stable data profile,
but does not register the Tauri updater plugin. Both updater commands reject
Store builds, the frontend hides installer controls, and package validation
requires `updaterEnabled=false` plus a null updater endpoint.

## Version contract

- Product source version: exactly `X.Y.Z` in Cargo, package.json, and Tauri.
- Standard tag and artifact version: the same `X.Y.Z`.
- MSIX identity version: deterministic `X.Y.Z.0`.
- A previously published tag must never be moved or reused. Bump the canonical
  source version before the next release when its current tag already exists.
