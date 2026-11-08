# Graphite / Jade — independent UI experiment

**Historical record — C2 adopted.** The redesigned workbench layout was adopted
with the later C2 palette: neutral surfaces, violet light-mode interaction
accents, and neutral/periwinkle dark mode. Temporary palette URL switches have
been removed. This document preserves the original Graphite / Jade proposal
and its conclusions for comparison; current rules live in
[design-system.md](design-system.md) and
[interaction-guidelines.md](interaction-guidelines.md).

This branch implements the independently authorized November 8, 2026 visual
experiment. It intentionally replaces the previous visual direction for this
experiment, without changing the repository's ownership, security, i18n,
execution, persistence, or interaction contracts. It is not a release decision.

## Design intent

Unfour should feel like one precise desktop instrument. A quiet outer frame
holds workspace and resource context; an inset working surface holds the active
task. Jade marks focus and primary actions. Warm mineral light and graphite dark
share the same hierarchy, geometry, and semantic colors.

The identity comes from the labeled vertical module rail,
recessed resource navigation, rounded document frame, and restrained jade
selection. Content and controls remain the focus: no decorative dashboard,
marketing copy, imagery, hidden expert features, or replacement demo screens.

Following review, the packaged U application icon remains unchanged to preserve
existing brand recognition. The experimental four-part shell brand mark was
removed to avoid a competing identity. Internal feature and action icon design
remains part of the experiment, alongside the new palette, typography and layout.

## Foundations

Tokens remain canonical in `packages/ui/src/styles/`. Features consume semantic
tokens instead of introducing their own theme systems.

| Role | Light | Dark |
| --- | --- | --- |
| Outer chrome | `#e8ece8` | `#111719` |
| Working canvas | `#ffffff` | `#1b2225` |
| Surface | `#f8faf8` | `#20282c` |
| Context band | `#f0f4f1` | `#252e32` |
| Primary action/focus | `#087563` | `#59d9b2` |
| Main text | `#23352f` | `#e2ebe8` |
| Secondary text | `#52685e` | `#afc1bb` |
| Boundary | `#d8e1db` | `#344145` |

- Use jade for interaction; retain distinct success, warning, danger, info and
  capability colors. Status always includes text or an existing semantic icon.
- UI fonts: Segoe UI Variable / Segoe UI, native Apple fonts, and CJK fallbacks.
  Code: Cascadia Code / SFMono-Regular / Consolas. No downloaded fonts.
- UI body remains 12–13px; task titles use 15–16px and settings headings 19px.
  Monospace is reserved for technical content and values.
- Spacing uses the existing 4px scale. Controls use 6px corners; groups 8px;
  frames/dialogs 12px. Small shadows separate layers without adding decoration.
- Title bar 52px, contextual toolbar 44px, tab strip 42px, sidebar rows 32px,
  data rows 34px, inputs 34px, buttons 32px, compact actions 28px, status bar 28px.
  Toolbars may wrap rather than conceal controls when space is constrained.

## Composition and components

- **Shell:** workspace and environment stay in the title bar. The 72px rail
  exposes translated short module labels; at narrower widths it becomes 60px.
  The 8px frame inset distinguishes the document from resource navigation.
  Existing sidebar resize/collapse, persistent feature mounts, native window
  controls, command palette and status slots remain wired as before.
- **Buttons and inputs:** one filled primary action, outlined secondary actions,
  quiet icon actions. Inputs use the canvas background with a clear border and
  focus ring. Disabled actions retain their existing disabled behavior.
- **Tabs and selectors:** document tabs are raised into the adjacent surface;
  configuration selectors use inset rounded selected states. Close, reorder,
  unsaved markers, context menus and keyboard focus are preserved.
- **Dialogs and menus:** larger padding, distinct title/footer bands, 12px
  dialog corners, subtle blurred overlay. Content scrolls inside constrained
  dialogs. Menu items retain keyboard and context-menu semantics.
- **Tables and trees:** readable row spacing, calm separators and typed metadata.
  Selection remains separate from hover. Copy, resize, sorting/filter controls,
  drag/drop and context menus are retained.

## Module application

- **API:** request identity above a unified method/address capsule, prominent
  Send/Save, scrollable configuration selectors, clear key/value grids, grouped
  response status/time/size, consistent body/auth/multipart/script surfaces.
  The existing responsive request/response split remains in use.
- **Database:** resource hierarchy and connection context have clear headers.
  SQL actions and connection/catalog/schema context use separate wrapping rows.
  Editors, result tabs, table filter/pagination/structure, and connection safety
  settings use the same surface grammar.
- **SSH:** terminal remains dark in both themes. Session panes expose the
  existing connection identity/status, with breathing room around the fitted
  terminal. SFTP, task editors, step cards, and run views use shared controls.
- **Flow:** dotted canvas, capability icons, numbered step cards, compact
  start/end nodes, selected jade outline, grouped zoom tools, and a framed
  inspector. At small content widths the inspector stacks below the canvas;
  authoring, connections, variables, runs and revision behavior are retained.
- **Workspace / Environment / Settings:** existing UI inherits the same tokens,
  controls and dialogs; settings gain a clearer navigation/content split and
  grouped settings cards. Theme preference and locale persistence are unchanged.

## Tradeoffs and limits

The labeled rail and larger toolbars deliberately trade some data density for
orientation and reachable controls. The API request/response layout and Flow
inspector can stack on small windows, increasing vertical scrolling. Font
rendering follows the operating system. Terminal ANSI colors and native window
control behavior remain their existing implementation.

This is a frontend visual experiment, not a backend or architecture refactor.
No dependencies, schemas, command contracts, service logic, credential handling,
cloud ownership, or synchronization logic are changed. Browser fixture evidence
does not establish real database/SSH/network connectivity or native Tauri
behavior; see the verification record for the exact limits.

Verification and screenshot comparisons:
[experiment record](../testing/ui-redesign-experiment-2026-11-08.md).
