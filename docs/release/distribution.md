# Distribution architecture

Unfour has one source tree, one desktop product, and two distribution values.
Release channel (`test` or `stable`) selects services and storage; distribution
selects the update authority.

| Distribution | Package | Delivery | Update authority |
| --- | --- | --- | --- |
| `standard` | Windows NSIS, macOS packages, Linux x64 AppImage (Experimental) | GitHub Release, Cloudflare R2, and unfour.dev | Unfour updater at `https://releases.unfour.dev/stable/latest.json` |
| `microsoft-store` | Windows x64 MSIX | Manual Partner Center upload | Microsoft Store |

There is no Pro application edition and no Website package kind. Account,
entitlement, cloud sync, desktop, command bus, and `unfour-mcp` are shared by
both distributions. A paid account plan can still be called Pro; that is an
entitlement name, not a second client or repository boundary.

## Delivery stages

The four delivery stages have deliberately different responsibilities:

| Stage | Purpose | External side effects |
| --- | --- | --- |
| CI | Unit tests, Rust checks, release contracts, and migration/secret gates | None beyond CI logs |
| Release Candidate | Full verification plus real signed Standard Tauri bundles for Windows x64, macOS arm64/x64, and Linux x64 | Uploads per-platform GitHub Actions artifacts only |
| Release | Publishes an already reviewed Standard candidate to the GitHub Release and immutable R2 version path, then promotes the updater manifest | GitHub Release, R2 versioned files, and finally `stable/latest.json` |
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
move a tag, create a GitHub Release, access R2, generate or publish an online
`latest.json`, change `stable/latest.json`, or build/publish MSIX. Download the
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
aggregation job. The aggregation job creates one
`SHA256SUMS.txt` and one Tauri `latest.json`. It uploads the same staged bytes
to both destinations:

```text
vX.Y.Z
  -> resolve exact tag commit
  -> reusable Standard verify and one native build per target
  -> release-assets/
       -> Cloudflare R2 stable/X.Y.Z/ (downloaded again and SHA-256 checked)
       -> GitHub Release vX.Y.Z
       -> R2 stable/latest.json (uploaded last)
```

The final manifest promotion reads the live `stable/latest.json` first and
compares versions by numeric SemVer fields. A missing manifest is allowed for
the first release; a newer or equal candidate is allowed, while an older
candidate fails. Equal-version reruns reach this gate only after the existing
immutable objects under `stable/X.Y.Z/` have passed the byte and checksum
verification above.

The formal `publish` job also binds the `production` Environment and reads
`R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_ACCOUNT_ID`, and `R2_BUCKET`
for Cloudflare R2 publication. GitHub and R2 never invoke separate builds.
`latest.json` points to the immutable R2 version path, while unfour.dev may
link to the same R2 objects. All six values belong in the GitHub Environment
`production`; none belongs in repository files. Release Candidate only has the
shared signed-build capability and does not receive R2 or GitHub Release
publication authority.

### Linux (Experimental)

The Standard release currently publishes one Linux format: x64 AppImage.
The canonical public assets are:

- `Unfour_X.Y.Z_linux_x64.AppImage`
- `Unfour_X.Y.Z_linux_x64.AppImage.sig`

The updater manifest has one Linux entry, `linux-x86_64`, pointing to the
AppImage. Tauri may still generate `.deb` and `.rpm` files during the Linux
build, but they remain CI intermediates: they are not copied into
`release-assets/`, `SHA256SUMS.txt`, GitHub Releases, Cloudflare R2, or
`latest.json`. Linux ARM64 is not a published Standard target.

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
