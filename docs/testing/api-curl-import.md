# API Client cURL import — 2026-10-30

## Behavior and ownership

The request-tab toolbar opens the existing shared Dialog. Parsing and preview
are frontend-only. Confirmation passes a RequestDraft to the existing
newRequest action, creating a workspace-scoped, unsaved tab with empty scripts,
no response, and no execution. Editing source invalidates preview. Parse errors
retain source; cancellation/success clears it. No import IPC, persistence,
activity logging, command execution, or network request is introduced.

Sensitive headers use the same classification and SecretInput as the request
editor. Values stay in the transient draft; explicit later Save/Send uses the
existing backend credential-reference and redaction path. The source textarea
necessarily shows the pasted command. Errors contain only fixed categories,
never source, parameter values, URLs or credentials.

## V1 support

- One literal `curl` or `curl.exe` command with one HTTP(S) URL.
- POSIX single/double quotes, concatenated quoted segments, escaped quotes,
  backslash LF/CRLF continuation; basic cmd caret LF/CRLF continuation and
  Chrome/Edge caret-quoted arguments (`^"` with inner `^\^"`).
- `-X/--request`, `-H/--header`, `-d/--data`, `--data-raw`,
  `--data-binary`, `-G/--get`, `--url`, and inline `-b/--cookie`.
  Value-taking long options accept `=value`; supported short options accept
  attached values.
- GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS. Data implies POST unless an
  explicit method or `--get` overrides it. Repeated data is joined with `&`.
- Duplicate header and query rows retain order. Query decoding/encoding follows
  the existing request parameter editor, including its normalization of escapes.
- JSON, text and URL-encoded form bodies. Form bytes remain raw text with an
  explicit Content-Type rather than being decoded/re-encoded by the form editor.
  An absent data Content-Type becomes application/x-www-form-urlencoded, as curl
  does. Invalid JSON remains raw text rather than blocking subsequent editing.

## Explicitly unsupported

All options not listed above fail, including `--compressed`, `-L/--location`,
`-k/--insecure`, `-I/--head`, `-u/--user`, `--data-urlencode`, proxy/TLS/timeout
options, config files, output flags and multiple transfers. No unknown option is
silently skipped. Multipart (`-F/--form`), upload (`-T/--upload-file`), header or
body `@file` references and cookie files fail; `--data-raw '@literal'` is literal.

Shell pipelines, redirections, chaining, substitutions, variables, ANSI-C `$'…'`
quoting, PowerShell syntax and ambiguous escapes fail. Detected cmd mode rejects
percent characters and complex backslash escapes; users can paste the Bash
variant instead. Bare cmd operators are rejected even within caret delimiters.
Literal `{{…}}` tokens and decoded query values are rejected to avoid accidental
workspace-variable expansion. The limit is 262144 JavaScript string characters.

Requests that the existing model cannot represent safely fail: GET/HEAD with
body, empty/removal header syntax, duplicate explicit methods, multiple
Content-Type headers with a body, multipart Content-Type bodies, mixed cookie
option/header sources, empty/whitespace query names, malformed query encoding,
URL credentials/fragments/globs, and multiple URLs.

## Verification

- PASS: API Client plus shared i18n Vitest suite — 28 files, 260 tests; subsequent
  focused parser run — 39 tests after the final cmd-operator regression was added.
- PASS: Chromium Playwright `api-curl-import.spec.ts` — failure/retry, preview,
  masking, shortcut isolation, new tab, no response/network execution; screenshot
  visually inspected. Chinese Dialog flow and cancel focus return covered in jsdom.
- PASS: `pnpm run build` (TypeScript and Vite). Existing bundle-size warning remains.
- PASS: ESLint on changed TypeScript files, zero errors. Warnings remain for the
  existing large page/hook and the bounded parser's three branch-heavy functions.
- PASS: `pnpm run check:large-files` (zero blocking files), `git diff --check`.
- Independent read-only review found cmd quoting, literal caret, template decoding
  and bare cmd operator issues; each was fixed and regression-tested.

No Rust execution semantics, OpenAPI/Collection import flow or dependency was
changed. Native Tauri manual interaction and real HTTP execution were not tested;
the browser regression uses the existing browser backend. The importer creates a
request draft, not a full curl runtime: later Send follows Unfour's existing
transport settings and query normalization. Unsupported cmd quoting is rejected
rather than approximated.
