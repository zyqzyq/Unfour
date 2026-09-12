# Unfour UI Design Entry

Unfour UI is a developer desktop workbench: a lightweight IDE-style desktop
tool for dense, object-first workflows.

## Design Keywords

- High information density.
- Object-first workflows.
- Context retention.
- Persistently visible state.
- Minimal decoration.
- Tool-like feel.
- Keyboard friendly.
- Clear primary action.

## Use the relevant design context

These three documents are Unfour's only UI design authorities; the current
style is fixed. Use this entry for orientation,
[design-system.md](docs/ui/design-system.md) for visual/component rules, and
[interaction-guidelines.md](docs/ui/interaction-guidelines.md) for behavior.
Read the sections affected by the task; a small copy fix does not require a
full design review. Do not generate a replacement design system from a skill.

Historical UI audit, token, component, layout, and refactor-plan documents live
under `docs/archive/ui/`. They are preserved for context but are no longer the
active source of truth.

## Document Responsibilities

- `design.md`: UI design entry point, context routing, priority, and boundary
  guidance.
- `docs/ui/design-system.md`: Product style, layout model, tokens, component
  usage rules, and forbidden visual patterns.
- `docs/ui/interaction-guidelines.md`: Workbench interaction model,
  object-first behavior, states, keyboard expectations, dangerous operations,
  and module-specific interaction rules.

## Conflict Resolution

When UI documents overlap, use this priority order:

1. `design.md` defines the documentation entry point and priority order.
2. `docs/ui/design-system.md` defines shared visual and component rules.
3. `docs/ui/interaction-guidelines.md` defines interaction behavior and
   module-specific workflow guidance.
4. Package docs may add local constraints, but must not override shared UI
   rules or package boundaries.

## Boundaries

- Do not add a second UI token system.
- Do not add page-local button, input, tab, table, or dialog styles when
  `packages/ui` provides the shared primitive.
- Do not treat an external design system as Unfour's source of truth.
- Do not use marketing-style dashboards, large cards, large radius, heavy
  shadows, excessive whitespace, or random gradients.
- Do not change business logic, backend protocols, the command bus, or security
  boundaries for visual consistency.
- Do not introduce a new UI framework or large data grid without an explicit
  task and documented reason.
- Do not place module-specific actions in the global toolbar.
