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

Avoid:

- marketing-style dashboard layouts;
- oversized cards and hero sections;
- large border radii;
- heavy shadows;
- excessive whitespace;
- decorative gradients;
- arbitrary module-specific accent colors.

## Shell Layout

Unfour uses one application shell. Feature modules provide trees, inspectors,
main panels, status content, and bottom-panel content through slots.

```text
GlobalToolbar 38px
├─ Sidebar 240-320px
├─ MainWorkspace
│  ├─ TabBar 34px
│  └─ Active module surface
├─ RightInspector optional 260-420px
├─ BottomPanel optional 180-360px
└─ StatusBar 24px
```

### Global Toolbar

The global toolbar contains global operations only:

- workspace switcher;
- global search and command palette;
- layout toggles;
- sync/storage/status indicators;
- desktop window controls.

Do not place module-specific actions such as Send, Run SQL, Connect, Save
request, or Add connection in the global toolbar.

### Sidebar

The sidebar is for navigation and resource trees. Forms and editors do not
belong in the sidebar. Use dialogs, inspectors, or main tabs for edit-heavy
workflows.

### Main Workspace

The main workspace owns the active module surface. Feature modules should not
create their own outer application shell.

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
- Secondary actions should move into dropdown menus, context menus, command
  palette entries, or secondary action groups.
- Components should expose composition slots instead of importing feature
  packages.
- Shared components may accept icons as `ReactNode`, but must not import
  feature business logic.

## Tokens

New UI tokens use the `--u-*` prefix.

Core color tokens:

```css
--u-color-bg
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
| Global toolbar | 38px |
| Section toolbar | 34px |
| Tab bar | 34px |
| Sidebar row | 28px |
| Table row | 30px |
| Input | 32px |
| Button | 30px |
| Compact button | 28px |
| Status bar | 24px |

## Typography

| Token | Value |
| --- | --- |
| UI text | 13px |
| Small text | 12px |
| Label text | 12px |
| Compact title | 14px |
| Code text | 13px |

Use monospace fonts for terminal, SQL editor, JSON viewer, logs, request
payloads, and code snippets. Do not use large title text in workspace pages.

## Borders, Radius, And Shadows

- Use subtle borders to separate structural regions.
- Prefer one border between major regions rather than borders around every
  nested block.
- Keep radius values at 8px or below.
- Use shadows only for dropdowns, popovers, dialogs, and tooltips.
- Avoid shadows for normal panels, sidebars, tab bars, tables, and main
  workspace sections.

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
