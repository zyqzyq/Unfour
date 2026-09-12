# App Shell Scope

Own the frontend desktop workbench composition root: global layout, top-level
slots, workspace switcher wiring, module navigation, command palette,
diagnostics actions, and module mount surfaces.

- Mount feature modules and pass shell props/children through shared layout
  primitives. Feature internals stay in their owning packages.
- `DesktopApp.tsx` is a composition root, not a home for request execution,
  SQL editors, SSH session state, feature mock data, or feature-specific UI.
- Wire the shared i18n provider at composition level; feature packages must
  not import app-shell to translate their UI.
