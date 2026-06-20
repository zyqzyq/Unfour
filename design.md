# Unfour UI Design Entry

Unfour UI is a **Developer Desktop Workbench**: a lightweight IDE-style desktop
developer tool for dense, object-first developer workflows.

## Design Keywords

- High information density
- Object-first workflows
- Context retention
- Persistently visible state
- Minimal decoration
- Tool-like feel
- Keyboard friendly
- Clear primary action

## Reading Order

1. `design.md`
2. `docs/ui/ui-guidelines.md`
3. `docs/ui/ui-layouts.md`
4. `docs/ui/ui-tokens.md`
5. `docs/ui/ui-components.md`
6. `docs/ui/interaction-guidelines.md`
7. `docs/ui/ui-refactor-plan.md`

## Document Responsibilities

- `design.md`: UI design entry point, reading order, priority, and boundary
  guidance.
- `docs/ui/ui-guidelines.md`: Product style, common UI principles, state
  requirements, and scope control.
- `docs/ui/ui-layouts.md`: Layout rules for AppShell, Sidebar, MainWorkspace,
  Inspector, BottomPanel, StatusBar, and related shell regions.
- `docs/ui/ui-tokens.md`: Visual token source of truth, including colors,
  typography, spacing, heights, and radius.
- `docs/ui/ui-components.md`: Shared components, component usage rules, icon
  rules, and forbidden patterns.
- `docs/ui/interaction-guidelines.md`: Workbench model, object-first behavior,
  Tabs, TreeView, ContextMenu, DataTable, dangerous operations, and other
  complex interaction guidelines.
- `docs/ui/ui-refactor-plan.md`: Phased UI refactor plan. It is not the
  long-term source of truth for UI rules.

## Conflict Resolution

When UI documents overlap, use this priority order:

1. `design.md` defines the documentation entry point and priority order.
2. `docs/ui/ui-guidelines.md` defines product style and scope.
3. `docs/ui/ui-tokens.md` defines visual tokens. Any conflict about colors,
   font sizes, spacing, or radius is resolved by this file.
4. `docs/ui/ui-layouts.md` defines shell and layout structure.
5. `docs/ui/ui-components.md` defines shared components and component usage.
6. `docs/ui/interaction-guidelines.md` defines complex interaction goals and
   target module behavior.
7. `docs/ui/ui-refactor-plan.md` represents only the current phased execution
   plan.

## Boundaries

- Do not add a second UI token system.
- Do not add page-local button, input, tab, table, or dialog styles.
- Do not treat an external design system as Unfour's source of truth.
- Do not use marketing-style dashboards, large cards, large radius, heavy
  shadows, excessive whitespace, or random gradients.
- Do not change business logic, backend protocols, the Command Bus, or security
  boundaries for visual consistency.
- Do not introduce a new UI framework or large Data Grid.
- Do not place module-specific actions in `GlobalToolbar`.
