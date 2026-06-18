# Docs Audit

This audit records documentation ownership, stale references, and known
conflicts after the package-context cleanup. It is a routing aid, not a second
package progress tracker.

## Current Source of Truth

| Area | Source of Truth | Notes |
|---|---|---|
| Global long-term agent rules | `AGENTS.md` | Package boundaries, command-bus expectations, dependency discipline, and reporting requirements. |
| AI/Codex reading strategy | `docs/agents/START_HERE.md` | Defines scoped reading order and legacy fallback behavior. |
| Current package and crate status | `docs/project/PACKAGE_STATUS.md` | Authoritative for current package/crate status; supersedes status details in historical checkpoint docs. |
| Frontend package boundaries | `packages/*/AGENTS.md` and `packages/*/README.md` | Local long-term rules and quick package orientation. |
| Command bus boundary | `crates/unfour-command-bus/AGENTS.md`, `crates/unfour-command-bus/README.md`, `docs/engineering/command-bus.md` | Command bus is a Rust crate, not a frontend package. |
| Architecture explanation | `docs/architecture/*` | Reference docs only; should not override `docs/project/PACKAGE_STATUS.md`. |

## Legacy or Possibly Stale Docs

| File | Issue | Suggested Action |
|---|---|---|
| `docs/agents/PROJECT_MAP.md` | Historical structural snapshot; previously contained old package names and stale crate status. | Keep as architecture/reference map with a top-level status-source notice. |
| `docs/agents/PROJECT_STATE.md` | Checkpoint-style state with dated counts, verification totals, and release status. | Treat as legacy checkpoint context unless explicitly refreshed. |
| `docs/agents/NEXT_STEPS.md` | Dated next-step queue; can duplicate current status and open issues. | Use only when `docs/project/NEXT_STEPS.md` is absent and the task needs legacy checkpoint context. |
| `docs/agents/OPEN_ISSUES.md` | Dated issue list; can duplicate `PACKAGE_STATUS.md` risks. | Use only as legacy issue context; do not let it override package status. |
| `docs/engineering/architecture.md` | Contains older Database/SSH "reserved" status language. | Mark as engineering history or update in a separate architecture refresh. |
| `docs/engineering/progress.md` | Maintains historical progress separate from `PACKAGE_STATUS.md`. | Consider archiving or marking historical in a later pass. |
| `docs/engineering/task-breakdown.md` | Contains old PostgreSQL/MySQL task status. | Treat as historical planning unless refreshed. |
| `docs/user/USER_GUIDE.md` | Some user-facing capability notes still describe PostgreSQL/MySQL and secret storage as future/reserved. | Review separately before publishing user docs. |
| `docs/superpowers/*` | Plans/specs preserve historical task intent and may mention created files or old work phases. | Keep as historical artifacts; do not use as current package status. |

## Naming Conflicts

| Old Name | Current Name | Status |
|---|---|---|
| `packages/api-debugger` | `packages/api-client` | Corrected in `docs/agents/PROJECT_MAP.md`; current package exists. |
| `packages/terminal` | `packages/ssh-terminal` | Corrected in `docs/agents/PROJECT_MAP.md`; current package exists. |
| `packages/workspace` | `packages/workspace-core` and `packages/workspace-local` | Corrected in `docs/agents/PROJECT_MAP.md`; split package names exist. |
| `packages/command-bus` | `crates/unfour-command-bus` | Corrected in `docs/architecture/package-boundaries.md`; no frontend package exists. |
| `packages/extension-contracts` | No current package or crate | Marked as not present and planning status待确认. |

## Conflicting Statements

| File A | File B | Conflict | Resolution |
|---|---|---|---|
| `docs/agents/PROJECT_MAP.md` | Actual `packages/*` directories | Listed `api-debugger`, `terminal`, and `workspace` directories that do not exist. | Updated to `api-client`, `ssh-terminal`, `workspace-core`, and `workspace-local`. |
| `docs/agents/PROJECT_MAP.md` | `docs/project/PACKAGE_STATUS.md` | Described Database PostgreSQL/MySQL as reserved and SSH native as not connected. | Updated PROJECT_MAP to reference current capabilities and defer status to `PACKAGE_STATUS.md`. |
| `docs/architecture/package-boundaries.md` | Actual workspace layout | Listed `packages/command-bus` as a missing frontend package, while the real command bus is a Rust crate. | Replaced with `crates/unfour-command-bus` and noted that no frontend command-bus package exists. |
| `docs/architecture/package-boundaries.md` | Current `packages/api-client` source | Claimed `ResponseTabs.tsx` had a local `EmptyState`. | Reworded as a resolved historical note; the file now imports `EmptyState` from `packages/ui`. |
| `docs/agents/START_HERE.md` | Actual `docs/project/*` files | Referenced `docs/project/PROJECT_STATE.md`, `NEXT_STEPS.md`, and `OPEN_ISSUES.md`, which do not exist. | Clarified that `docs/agents/*` fallbacks are legacy checkpoint context and must not override `PACKAGE_STATUS.md`. |
| `docs/engineering/architecture.md` | `docs/project/PACKAGE_STATUS.md` | Older Database/SSH reserved language conflicts with current implemented surfaces. | Not changed in this pass; mark for future architecture refresh. |
| `docs/user/USER_GUIDE.md` | `docs/project/PACKAGE_STATUS.md` | User guide still contains older reserved/future capability language. | Not changed in this pass; recommend a separate user-doc review. |

## Missing Referenced Docs

| Referenced Path | Referenced By | Suggested Action |
|---|---|---|
| `docs/project/PROJECT_STATE.md` | `docs/agents/START_HERE.md` | Do not create automatically; use `docs/agents/PROJECT_STATE.md` as legacy fallback only when needed. |
| `docs/project/NEXT_STEPS.md` | `docs/agents/START_HERE.md` | Do not create automatically; use `docs/agents/NEXT_STEPS.md` as legacy fallback only when needed. |
| `docs/project/OPEN_ISSUES.md` | `docs/agents/START_HERE.md` | Do not create automatically; use `docs/agents/OPEN_ISSUES.md` as legacy fallback only when needed. |
| `packages/extension-contracts` | `docs/architecture/package-boundaries.md` before this pass and `docs/project/PACKAGE_STATUS.md` | Not a current workspace member; confirm planning intent before creating anything. |
| `crates/extension-contracts` | Inferred from extension-contracts discussion | Not a current workspace member; confirm planning intent before creating anything. |

## Repeated Status Maintenance

| Status Topic | Repeated In | Resolution |
|---|---|---|
| Package/crate current status | `docs/project/PACKAGE_STATUS.md`, `docs/agents/PROJECT_STATE.md`, `docs/engineering/progress.md`, `docs/agents/PROJECT_MAP.md` | Treat `PACKAGE_STATUS.md` as authoritative; other docs are checkpoint/history/reference. |
| SSH live verification | `PACKAGE_STATUS.md`, `PROJECT_STATE.md`, `OPEN_ISSUES.md`, `NEXT_STEPS.md` | Keep `PACKAGE_STATUS.md` as package status; use issue/next-step docs only as legacy checkpoint context. |
| Workspace-local re-export | `PACKAGE_STATUS.md`, `package-boundaries.md`, `packages/workspace-local/README.md`, `PROJECT_MAP.md` | Consistent after this pass: workspace-local is a compatibility re-export until real local behavior is scoped. |
| Extension contracts | `PACKAGE_STATUS.md`, `package-boundaries.md` | Consistent after this pass: no current package/crate; planning status待确认. |
