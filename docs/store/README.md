# Store listing assets

This folder holds Microsoft Store listing assets. It is not part of the Tauri
or MSIX package. Do not copy these files into `apps/desktop/src-tauri/icons/`
or change packaging to consume them.

## App icon

`assets/app-icon-300.png` is the Microsoft Store listing icon:

- exactly 300 × 300 px
- PNG with the current Unfour logo, transparent corners, and existing margins
- generated from `apps/desktop/src-tauri/icons/icon.png` (512 × 512), not from
  a smaller tile asset

Do not redesign the logo or add text, shadows, backgrounds, or other decoration.

## Screenshots

Store screenshots must show the real Unfour desktop UI. Do not use AI-generated
interface mockups.

Recommended capture format:

- 1920 × 1080 PNG
- actual product windows, not redesigned marketing art

## Shared source

Store listing, README, and the website should reuse the same real product
assets. Prefer `docs/screenshots/` for UI captures and
`apps/desktop/src-tauri/icons/icon.png` as the logo master. Do not maintain a
second generated or AI-drawn set for Store, GitHub, or unfour.dev.
