# Documentation Archive

This folder preserves historical documentation that is no longer part of the
active release documentation path.

Archived files may contain stale package names, dated verification counts,
completed task plans, checkpoint notes, or superseded status. They are useful
for archaeology, but they must not override active docs.

## Active Sources Of Truth

- Agent onboarding: `docs/agents/START_HERE.md`
- Package boundaries: `docs/architecture/package-boundaries.md`
- Project structure: `docs/architecture/project-structure.md`
- Data storage: `docs/architecture/data-storage.md`
- Security model: `docs/architecture/security-model.md`
- UI design system: `docs/ui/design-system.md`
- UI interaction model: `docs/ui/interaction-guidelines.md`
- MCP tools: `docs/mcp/tools.md`
- Release verification: `docs/testing/release-verification.md`
- Release checklist: `docs/release/release-checklist.md`

## Archived Categories

| Archive path | Reason |
| --- | --- |
| `docs/archive/agents/` | Historical project state, next steps, open issues, docs audit, checkpoint refresh procedure, and project map. |
| `docs/archive/project/` | Previous package status matrix with dated progress fields. |
| `docs/archive/engineering/` | Superseded architecture, storage, command-bus, security, progress, and task-breakdown notes. Useful content was consolidated into `docs/architecture/*` and `docs/mcp/*`. |
| `docs/archive/ui/` | Superseded UI guideline fragments and refactor plan. Useful rules were consolidated into `docs/ui/design-system.md` and `docs/ui/interaction-guidelines.md`. |
| `docs/archive/ai/` | Superseded single-page MCP document. Split into `docs/mcp/overview.md`, `docs/mcp/tools.md`, and `docs/mcp/codex-setup.md`. |
| `docs/archive/superpowers/` | Historical task spec and implementation plan artifacts. |

When in doubt, prefer active docs. If archived information is still useful,
extract it into the relevant active document before relying on it.
