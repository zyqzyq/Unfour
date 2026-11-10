# UI Design System

Unfour is a dense developer desktop workbench. The UI should feel closer to an
IDE or database client than a marketing dashboard.

## Product Style

- Lightweight desktop developer tool.
- High information density.
- Object-first navigation.
- Persistent state and clear feedback.
- Compact typography and stable panel geometry.
- Minimal decoration.

The adopted C2 design uses neutral work surfaces and a concentrated interaction
accent: violet in light mode, periwinkle in dark mode. The labeled module rail,
resource hierarchy, inset document frame, and filled selected states provide
orientation. Semantic success, warning, error, and information colors remain
distinct from the interaction accent. The packaged U application icon is
unchanged; internal feature/action icons continue to use the shared icon style.

Avoid:

- marketing-style dashboard layouts;
- oversized cards and hero sections;
- oversized or decorative border radii beyond the shared 6/8/12px scale;
- heavy shadows;
- excessive whitespace;
- decorative gradients;
- arbitrary module-specific accent colors.

## Shell Layout

Unfour uses one application shell. Feature modules provide trees, inspectors,
main panels, status content, and bottom-panel content through slots.

```text
GlobalToolbar 52px
├─ ActivityRail 72px (60px at narrow widths)
├─ Unified workspace frame (12px outer corners)
│  ├─ Resizable/collapsible Sidebar
│  └─ Editor region
│     ├─ MainWorkspace + optional RightInspector 260-420px
│     │  ├─ TabBar 42px
│     │  └─ Active module surface
│     └─ BottomPanel optional 180-360px
└─ StatusBar 28px
```

### Global Toolbar

The global toolbar contains global operations only:

- workspace switcher;
- active environment selector;
- MCP and application settings;
- host-provided global/account actions;
- desktop window controls.

Global search, the command palette, layout controls, and storage/status content
remain shell operations in their existing rail, shortcut, and status-bar entry
points; do not duplicate them merely to fill the title bar.

Do not place module-specific actions such as Send, Run SQL, Connect, Save
request, or Add connection in the global toolbar.

### Sidebar

The sidebar is for navigation and resource trees. Forms and editors do not
belong in the sidebar. Use dialogs, inspectors, or main tabs for edit-heavy
workflows.

The separate activity rail uses icons with translated short module labels.
Resource section headers remain visible while lists scroll. Filled accent-soft
selection and restrained hover surfaces distinguish the active resource.
Shell module sidebars use a single 4px content gutter (8px at search fields),
with headers aligned to the 42px document-tab rail. The API sidebar owns its
internal padding. All four module sidebars use the shared `surface` background,
distinct from the activity rail's `chrome`, with one straight right divider.
The outer workspace frame clips both left corners; module content must not
add its own outer background, border, or corner treatment when in the shell.
Flat root rows do not reserve an empty expand-arrow column; nested tree rows
retain their hierarchy indentation.

### Main Workspace

The main workspace owns the active module surface. Feature modules should not
create their own outer application shell.

Enclose the resource sidebar and editor region in one frame with shared 12px
outer corners, an 8px right/bottom inset, a subtle border, and a small shadow.
The sidebar/editor seam is straight with no gap or nested rounded frame. The
inspector and bottom panel stay within the editor region; the bottom panel
does not shorten the resource sidebar. When the sidebar is collapsed, the
same outer frame clips the editor's left corners. At narrow widths the right
inset reduces to 4px. Preserve resize, collapse, responsive split/stack
behavior, and persistent feature mounts.

### Inspector And Bottom Panel

The right inspector is for contextual metadata and low-frequency settings. The
bottom panel is for logs, traces, diagnostics, and task output. Both must be
collapsible and should preserve layout state where possible.

### Status Bar

The status bar is compact and always visible. It should show persistent state,
not one-off success messages.

## Shared Components

Reuse components from `packages/ui` before creating local variants. Check
`packages/ui/src/index.ts` for the exact export list.

Shared component categories include:

- buttons and icon buttons;
- badges and status indicators;
- inputs and selects;
- dialogs and confirmation dialogs;
- menus and popovers;
- tabs and tab bars;
- toolbar groups;
- tree views;
- data tables;
- shell layout helpers;
- empty, loading, and error states.

Feature packages may compose shared components, but `packages/ui` must remain
business-logic free.

## Component Rules

- Icon-only controls must use a shared icon button or equivalent accessible
  pattern with `aria-label` and tooltip text.
- Use `lucide-react` for icons. Do not mix icon libraries.
- Do not use emojis as functional icons.
- Module actions belong in module toolbars, not in the global toolbar.
- Each panel should expose no more than one primary action.
- Use a filled primary button, outlined secondary actions, and quiet icon
  actions. Inputs use the canvas surface and a clear focus treatment.
- Document tabs form a continuous flat 42px rail, with 128–224px-wide items.
  The outer document frame owns the corner radius; tabs have no inset card
  outline or top gutter. A panel-matched fill and short 2px bottom accent mark
  the active tab; short separators distinguish inactive neighbors. Dirty and
  loading markers remain visible. Close actions reserve their space and appear
  on the active tab, hover, keyboard focus, or devices without hover.
  Configuration tabs retain their compact inset selected states.
  Nested result-tab bars may keep their existing 36px height through the shared
  component's `className`; document-tab styling must not override that height.
- Secondary actions should move into dropdown menus, context menus, command
  palette entries, or secondary action groups.
- Components should expose composition slots instead of importing feature
  packages.
- Shared components may accept icons as `ReactNode`, but must not import
  feature business logic.

## Tokens

### Single source of truth

All shared design tokens (base `--u-*` primitives, light/dark theme semantic
values, and the `--panel-*` / `--app-*` / `--border` / `--text` / `--accent` /
`--sidebar-*` / `--danger` / `--success` / `--warning` / `--focus` / `--neutral`
/ `--badge-*` aliases) have **one default source of truth**:
`@unfour/ui/styles.css` (the files under `packages/ui/src/styles/`).

- The host app (`apps/desktop`) MUST import the
  shared tokens first, before their own local `styles.css`:

  ```ts
  import "@unfour/ui/styles.css";
  import "./styles.css";
  ```

- Host apps MAY **override** a token for host-only or Pro-only needs, but they
  MUST NOT **redefine** shared tokens. The token source of truth always lives
  in `@unfour/ui/styles.css`, never in a host app. Pro-only variables in the
  `--pro-*` namespace are allowed.

- `scripts/check-shared-tokens.mjs` enforces this: it fails the build if any
  `apps/*/src/styles.css` redefines a shared token. Run it via
  `pnpm run check:tokens` (it is part of `pnpm run check`).

New UI tokens use the `--u-*` prefix.

Core color tokens:

```css
--u-color-bg
--u-color-chrome
--u-color-canvas
--u-color-surface
--u-color-surface-subtle
--u-color-surface-muted
--u-color-surface-hover
--u-color-surface-active
--u-color-border
--u-color-border-strong
--u-color-text
--u-color-text-muted
--u-color-text-soft
--u-color-primary
--u-color-primary-hover
--u-color-primary-soft
--u-color-danger
--u-color-warning
--u-color-success
--u-color-focus
```

### Adopted C2 palette

| Role / token suffix | Light | Dark |
| --- | --- | --- |
| `bg` | `#f2f3f5` | `#18191d` |
| `chrome` | `#e7e8eb` | `#121316` |
| `canvas` | `#ffffff` | `#1f2025` |
| `surface` | `#fafafa` | `#25262c` |
| `surface-subtle` | `#f2f2f3` | `#2b2c33` |
| `surface-muted` | `#e9eaec` | `#31333b` |
| `surface-hover` | `#e1e3e6` | `#383b44` |
| `surface-active` | `#d5d8dd` | `#414551` |
| `border` | `#d7d9dd` | `#393c46` |
| `border-strong` | `#a9adb5` | `#666d7e` |
| `text` | `#24262b` | `#eceef3` |
| `text-muted` | `#50545d` | `#bfc3cf` |
| `text-soft` | `#575d68` | `#aeb4c3` |
| `primary` | `#6e40bd` | `#abb7ff` |
| `primary-hover` | `#5d32a8` | `#c3cbff` |
| `primary-foreground` | `#ffffff` | `#1a2148` |
| `primary-soft` | `#eee7fa` | `#2c3457` |
| `focus` | `#7640cd` | `#abb7ff` |

All suffixes above belong to `--u-color-*`. Full semantic, badge, scrollbar,
terminal, and overlay values remain in the shared CSS source. C2 is the default
design, not an extra theme choice. Light/dark/system selection, persistence,
and locale settings retain their existing behavior; temporary palette URL
switches are not part of the product.

Surface roles:

- `chrome`: title bar, activity rail, outer workspace surround, and status bar.
  Document-tab rails use `surface-subtle` to keep the editor header quiet.
- `surface`: forms, ordinary panels, and every module's resource sidebar.
  The shared sidebar owns this background rather than individual features.
- `bg`: application background; `surface-subtle`: section toolbars, grouped
  settings bands, and table headers. Do not globally swap these roles.
- `canvas`: data table bodies, SQL, API response, and code/editor canvases.
  Keep toolbars and headers separate from these working areas.
- `primary` / `primary-soft`: primary actions, active navigation, selected
  resources/tabs, and editor selections. Use `focus` for keyboard focus,
  active-cell outlines, editor cursors, and matching-bracket borders.
- Terminal canvases remain dark in both application themes: background
  `#16171a`, foreground `#e0e2e6`, cursor `#acb5ff`. Use the terminal tokens
  for their controls rather than a local light-theme override.

Shared `Tabs` defaults to `surface`. Set `activeBackground="subtle"` when the
next region is a section toolbar, or `"terminal"` for a terminal pane (including
its light-theme foreground). The active fill matches that adjacent region.
API request tabs join their `canvas` request bar; underline-style
configuration/response tabs retain their own hierarchy.

Monaco uses the shared `configureEditorThemes` helper in `packages/ui`, registered
by each API editor before mounting and by the SQL editor on mount. Its concrete
color values must stay aligned with the workbench's shared CSS tokens. Register both light and dark
themes so an already-mounted editor follows settings changes; API must render
correctly before Database has ever been opened. Syntax roles use semantic
secondary, success, warning, info, and text tokens; do not recolor syntax just
to match a primary button. This helper contains visual configuration only, not
editor commands or feature behavior.

Core size tokens:

```css
--u-size-global-toolbar
--u-size-section-toolbar
--u-size-tabbar
--u-size-sidebar-row
--u-size-table-row
--u-size-input
--u-size-button
--u-size-button-compact
--u-size-statusbar
```

Core radius tokens:

```css
--u-radius-sm
--u-radius-md
--u-radius-lg
```

Legacy aliases may remain temporarily, but new code should consume `--u-*`
tokens through shared components.

## Spacing And Density

Use compact desktop spacing:

| Token | Value |
| --- | ---: |
| `space-1` | 4px |
| `space-2` | 8px |
| `space-3` | 12px |
| `space-4` | 16px |
| `space-5` | 20px |
| `space-6` | 24px |

Prefer `space-1` through `space-4` for normal workbench UI. Avoid spacing
larger than `space-6` inside workspace pages.

## Heights

| Element | Target |
| --- | ---: |
| Global toolbar | 52px |
| Section toolbar | 44px |
| Tab bar | 42px |
| Sidebar row | 32px |
| Table row | 34px |
| Input | 34px |
| Button | 32px |
| Compact button | 28px |
| Status bar | 28px |

These are shared geometry targets, not fixed heights for wrapping content.
Keep primary actions and existing secondary controls reachable in narrow
layouts; toolbars may wrap and feature panels may stack.

## Typography

| Token | Value |
| --- | --- |
| UI text | 13px |
| Small text | 12px |
| Label text | 12px |
| Task title | 15–16px |
| Settings section heading | 19px |
| Code text | 13px |
| Caption (`--u-font-size-caption`) | 11px |

Use the native `--u-font-sans` stack: Segoe UI Variable / Segoe UI, native Apple
fonts, PingFang SC / Microsoft YaHei UI, and system fallbacks. The shared
`--u-font-mono` stack starts with Cascadia Code / SFMono-Regular / Consolas;
monospace editor fallbacks remain supported. No downloaded font is required.
Use monospace for terminal, SQL, JSON, logs, request payloads, and technical
values. Keep workspace titles within the compact sizes above.

Use `text-muted` for readable compact metadata such as table types, row counts,
history times, and editor hints. Reserve `text-soft` for decorative or disabled
details; do not combine it with 10/11px text that users need to read. Apply the
caption token selectively without changing row heights or adding density modes.

## Borders, Radius, And Shadows

- Use subtle borders to separate structural regions.
- Prefer one border between major regions rather than borders around every
  nested block.
- Use 6px corners for controls, 8px for groups (including the request address),
  and 12px for document frames and dialogs. Use the shared radius tokens.
- Use the small shared shadow for the inset document frame and subtle control
  elevation; use overlay elevation for menus, popovers, dialogs, and tooltips.
- Avoid stacked shadows on nested panels, sidebars, tab bars, and tables.
- Dialogs use distinct title/footer bands, a restrained overlay, and internal
  scrolling when content exceeds the available window height.

## Module Composition

- API: request identity above the method/address group; prominent Send and
  nearby Save; scrollable request configuration tabs; grouped response status,
  duration, and size. Retain the responsive request/response split.
  The editable identity uses 13px medium text without a redundant Name label,
  so it does not compete with the document tab or the request address.
  Method and URL share a single 38px-high address outline and focus treatment,
  separated by a short divider. Method color belongs to its text, not a nested
  border. Send matches this height; Save remains a quiet adjacent icon action.
- Database: distinguish connection/schema navigation, SQL actions, and active
  connection/catalog/schema context. Editors, result tabs, grids, filters,
  pagination, and connection dialogs share the surface and control hierarchy.
- SSH: retain dark terminal canvases and visible connection/session state.
  SFTP, task editors, and run views use shared controls and spacing.
- Flow: dotted canvas, capability icons, numbered step cards, compact start/end
  nodes, accent selection, grouped canvas controls, and a contextual inspector.
  Stack the inspector below the canvas when its content width requires it.
- Workspace, Environment, and Settings: use the same controls and dialogs;
  keep settings navigation separate from grouped content. Preserve all existing
  theme, locale, connection, and persistence behavior.

## Required States

Each feature must evaluate the following states and implement the ones that
apply to its workflow:

- initial state;
- nothing selected;
- empty state;
- loading;
- success;
- failure;
- network error;
- timeout;
- no data;
- long content;
- unsupported format;
- disabled state;
- confirmation required;
- disconnected or closed;
- reconnecting.

State must be visible near the affected object or action. Do not rely only on
temporary notifications.

## Scope Control

- UI optimization must not change backend protocols.
- UI optimization must not change command-bus behavior.
- A shared component should have at least two clear consumers before it moves
  into `packages/ui`.
- Visual consistency must not force API Client, Database, and SSH Terminal into
  identical workflows.
- Do not introduce a large UI framework or large data grid without an explicit
  task and documented reason.
