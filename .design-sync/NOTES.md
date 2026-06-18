# Design Sync Notes — @unfour/ui

## Repo quirks

- **pnpm workspace, package not self-installed**: `@unfour/ui` is not installed in its own `node_modules/@unfour/ui/` (pnpm workspace packages are never self-installed). Always pass `--entry packages/ui/src/index.ts` to the build command; without it the converter fails with ENOENT trying to find `packages/ui/node_modules/@unfour/ui/package.json`.
- **Node modules location**: Use `packages/ui/node_modules` as `--node-modules` — react and all UI deps are installed there.
- **DTS from dist/types**: TypeScript types are compiled to `packages/ui/dist/types/index.d.ts`. The build reads these via ts-morph. The JS entry is source-only (`./src/index.ts`); esbuild compiles TS directly.
- **Shell components excluded**: `ActivityBar`, `AppShellFrame`, `BottomPanel`, `CommandPalette`, `GlobalToolbar`, `MainWorkspace`, `RightInspector`, `Sidebar`, `SidebarHeader`, `SidebarRow`, `SidebarSection`, `SplitPane`, `StatusBar`, `TabBar`, and `I18nProvider` are excluded via `componentSrcMap` — they are internal IDE chrome and not design primitives. New shell exports should be added to the exclusion list.
- **Inter font**: Served by a runtime font service, not shipped with the bundle. Suppressed via `runtimeFontPrefixes: ["Inter"]`.

## Known render warns

- 16 components on the floor card (sub-components without authored previews): `ContextMenuContent`, `ContextMenuItem`, `ContextMenuTrigger`, `DialogBody`, `DialogClose`, `DialogContent`, `DialogDescription`, `DialogFooter`, `DialogHeader`, `DialogTitle`, `DialogTrigger`, `DialogXClose`, `DropdownMenuContent`, `DropdownMenuItem`, `DropdownMenuTrigger`, `ToolbarGroup`. These are leaf/sub-components that only render correctly inside their parent and are deliberately floor-carded.

## Platform-injected files

- `support.js` at the project root is a claude.ai/design runtime file (dc-runtime). It is platform-managed; do not delete it and do not include it in `deletes` for future upload plans. It will appear in `list_files` but is not part of the design system build.

## Re-sync infrastructure (added 2026-06-18)

- **`packages/ui/tsconfig.dts.json`**: Added to generate `dist/types/` declaration files. The base tsconfig has `noEmit: true` and `allowImportingTsExtensions: true` which are incompatible with emitting. This override tsconfig sets `allowImportingTsExtensions: false`, `noEmit: false`, `declaration: true`, `emitDeclarationOnly: true`, `outDir: "dist/types"`. Run via `npx tsc -p packages/ui/tsconfig.dts.json`.
- **`.design-sync/build-css.mjs`**: Compiles `apps/desktop/src/styles.css` (Tailwind v4 source with all `--u-color-*` tokens) to `packages/ui/dist/styles.css` using `@tailwindcss/node`. Scans `packages/**/*.{ts,tsx,js,jsx}` for class candidates. Output path is `packages/ui/dist/styles.css` — this is what `cssEntry: "dist/styles.css"` resolves to (PKG_DIR is `packages/ui/` from the `package.json` walk up from `src/`).
- **buildCmd is fully self-contained**: `node .design-sync/build-css.mjs && npx tsc -p packages/ui/tsconfig.dts.json && node .ds-sync/package-build.mjs ... && node .design-sync/strip-tw-vars.mjs`. No manual pre-steps needed.
- **Playwright is in `.ds-sync/node_modules/`**: Installed with `npm i -D playwright@1.61.0` in `.ds-sync/`. Chromium cache at `~/.cache/ms-playwright/`. Re-run `npx playwright install chromium` in `.ds-sync/` if cache is lost.
- **CSS generation timing**: The `buildCmd` must run BEFORE `resync.mjs` starts — the driver's internal `package-build.mjs` step checks for `cssEntry` during its own build pass. Do not generate CSS while the driver is already running (it will miss it and emit `[CSS_RUNTIME]`).

## Re-sync risks

- **Token vocabulary can drift**: conventions.md token names were updated on 2026-06-18 (accent→primary, badge tone names changed to success/warning/danger/info with ring suffix). Validate token names in conventions.md against `_ds_bundle.css` on each sync using `grep -o "\-\-u-color-[a-z-]*" ds-bundle/_ds_bundle.css | sort -u`.
- **New shell exports**: If new components are added to `shell.tsx` and exported from `index.ts`, they should be added to `componentSrcMap` exclusions — otherwise they'll appear in the design system as shell primitives the design agent shouldn't use.
- **Playwright version**: Playwright 1.61.0 is installed in `.ds-sync/node_modules/`. Chromium cache is at `~/.cache/ms-playwright/`. If the render check fails with "Executable doesn't exist", run `npx playwright install chromium` from `.ds-sync/`.
- **dist/types must stay current**: If `dist/types/` is stale, DTS extraction will have outdated props. The `buildCmd` includes `npx tsc -p packages/ui/tsconfig.dts.json` so this is handled automatically on each re-sync.
