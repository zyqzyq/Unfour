# Context Map

Use this map when the task's owner or supporting context is unclear. Start
with the requested files and applicable local `AGENTS.md`; consult the owning
README for implementation landmarks. Follow a link below only when it answers
a question needed for the change. Stop loading context once ownership,
affected behavior, and validation needs are clear.

## Find the relevant context

| Task or question | Context to consult |
| --- | --- |
| Package ownership, imports, dependency direction, or moving responsibilities | [Package boundaries](../architecture/package-boundaries.md) |
| Repository layout or an unfamiliar call chain | [Project structure](../architecture/project-structure.md) |
| Persisted records, workspace isolation, or schema changes | [Data storage](../architecture/data-storage.md); the affected migration and its consumers |
| Credentials, redaction, permissions, or execution safety | [Security model](../architecture/security-model.md) |
| Cloud-bound mutations, binding ownership, outbox capture, or repair | [Cloud Sync invariants](../architecture/cloud-sync-invariants.md) |
| UI layout, components, styles, or interactions | [Design entry](../../design.md), then the visual or interaction sections it identifies |
| Command-bus dispatch or adapter contracts | `crates/unfour-command-bus/AGENTS.md`, its README, and the affected adapter |
| MCP architecture or policy | [MCP overview](../mcp/overview.md) |
| MCP tool names, schemas, or behavior | [MCP tools](../mcp/tools.md) |
| Connecting Codex to Unfour MCP | [Codex setup](../mcp/codex-setup.md) |
| Choosing checks or deciding whether work is complete | [Execution protocol](EXECUTION_PROTOCOL.md) |
| Release readiness or release claims | [Release verification](../testing/release-verification.md) and [release checklist](../release/release-checklist.md) |
| Manual coverage for an affected UI, platform, or live service | Relevant cases in [manual tests](../testing/manual-test-cases.md) |

For example, a documentation typo needs the surrounding text and link checks.
A database UI fix needs its local constraints and the relevant interaction
section; add backend or storage context only if the affected path reaches it.
A shared command contract change needs both its producers and consumers.

## Keep context useful

- Root instructions hold global invariants; local instructions hold scope,
  boundaries, and domain invariants. Architecture docs explain their details.
  READMEs provide implementation landmarks and examples, not extra workflows.
- When documents disagree with code, inspect the affected implementation and
  tests. Treat the mismatch as something to resolve or report, not permission
  to discard a documented security constraint.
- Use `docs/archive/` only for historical questions. Do not follow archived
  reading lists or treat old test results as current evidence.
- `.codex/config.toml` contains repository sandbox settings. It is not a
  second instruction entry point. Repository skills are optional workflow
  aids, not prerequisites for ordinary coding or a source of UI design rules.
- Maintain these pointers as ownership changes. Keep details at their owning
  source instead of copying them into every instruction file.
