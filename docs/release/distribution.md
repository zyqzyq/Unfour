# Distribution architecture

Unfour has one source tree, one desktop product, and two distribution values.
Release channel (`test` or `stable`) selects services and storage; distribution
selects the update authority.

| Distribution | Package | Delivery | Update authority |
| --- | --- | --- | --- |
| `standard` | Windows NSIS, macOS packages, Linux x64 AppImage (Experimental) | GitHub Release, Cloudflare R2, and unfour.dev | Unfour updater at `https://release.unfour.dev/stable/latest.json` |
| `microsoft-store` | Windows x64 MSIX | Manual Partner Center upload | Microsoft Store |

There is no Pro application edition and no Website package kind. Account,
entitlement, cloud sync, desktop, command bus, and `unfour-mcp` are shared by
both distributions. A paid account plan can still be called Pro; that is an
entitlement name, not a second client or repository boundary.

## Standard: build once, publish twice

A plain `vX.Y.Z` tag is the only Stable release input. The workflow verifies
that Cargo, root/desktop package.json, and Tauri all contain the same `X.Y.Z`,
then builds each native target once with `distribution=standard`. Tauri creates
signed updater artifacts using a private key held only in GitHub Actions
secrets.

Each matrix job renames its output to a canonical filename and uploads that
exact file to the aggregation job. The aggregation job creates one
`SHA256SUMS.txt` and one Tauri `latest.json`. It uploads the same staged bytes
to both destinations:

```text
vX.Y.Z
  -> verify
  -> one native build per target
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

GitHub and R2 never invoke separate builds. `latest.json` points to the
immutable R2 version path, while unfour.dev may link to the same R2 objects.
Required CI secrets are `TAURI_SIGNING_PRIVATE_KEY`, optional signing-key
password, `R2_ACCOUNT_ID`, `R2_BUCKET`, `R2_ACCESS_KEY_ID`, and
`R2_SECRET_ACCESS_KEY`. None belongs in repository files.

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
