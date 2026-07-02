# Unfour UI Components

Shared UI lives in `packages/ui`. Feature packages may compose these components,
but must not create local replacements for common controls.

## Current Shared Components

| Component | Status | Notes |
| --------- | ------ | ----- |
| `Badge` | Existing | Uses tone variants for compact status display. |
| `Button` | Existing | Shared button primitive. New usage should prefer token-backed sizes. |
| `Input` | Existing | Shared single-line input. |
| `IconButton` | Added for shell | Icon-only button with required accessible label and tooltip text. |
| `GlobalToolbar` | Added for shell | Application-level toolbar only. |
| `Sidebar` | Added for shell | Navigation and tree container. |
| `TabBar` | Added for shell | Workspace tabs across all modules. |
| `MainWorkspace` | Added for shell | Central tab content area. |
| `RightInspector` | Added for shell | Optional collapsible right panel. |
| `BottomPanel` | Added for shell | Optional collapsible diagnostics/output panel. |
| `StatusBar` | Added for shell | Compact application state strip. |
| `SplitPane` | Added for shell | Resizable split helper for future persistence. |
| `CommandPalette` | Added for shell | Global command entry surface. |

## Component Rules

- Icon-only controls must use `IconButton` or another shared component that
  provides `aria-label` and tooltip text.
- Module actions belong in section toolbars inside module panels, not in
  `GlobalToolbar`.
- Tree rows, tabs, panel headers, and status items should use token-backed
  dimensions.
- Components should expose composition slots instead of importing feature
  packages.
- Shared components may accept icons as `ReactNode`, but must not import feature
  business logic.

## Known Gaps

- `Select`, `Textarea`, `Checkbox`, `Switch`, `Toolbar`, `TreeView`,
  `DataTable`, `PropertyGrid`, `Dialog`, `Popover`, `ContextMenu`, and richer
  `DropdownMenu` wrappers still need to move into `packages/ui`.
- Existing feature panels still contain local select, tab-like toggles, panel
  header, list item, empty state, and inline status implementations.
- Existing feature panels still include hardcoded Tailwind colors and sizes.

## Icon Rules

- Use `lucide-react` only.
- Do not mix icon libraries.
- Do not use emojis as functional icons.
- All icon-only buttons MUST provide tooltips.
- Use consistent stroke width across the application.
- Avoid decorative icons without functional meaning.

Recommended sizes:

| Context      | Size |
| ------------ | ---: |
| Toolbar      | 16px |
| Button       | 15px |
| Sidebar row  | 15px |
| Table action | 14px |
| Status bar   | 14px |

## Component-Related Forbidden Patterns

- Do not create page-local variants of common components when `packages/ui` already provides them.
- Do not introduce page-specific styling for shared patterns.
- Do not use multiple icon libraries.
- Do not create inconsistent button styles across modules.
