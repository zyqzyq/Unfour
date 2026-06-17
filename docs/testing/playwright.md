# Playwright

Unfour uses `@playwright/test` as the Playwright entry point. The project does
not depend on a root-resolvable `playwright` package, which is expected with the
current pnpm dependency layout.

## Commands

Run all end-to-end tests from the repository root:

```bash
pnpm run test:e2e
```

Run a focused spec:

```bash
pnpm exec playwright test tests/e2e/ui-smoke.spec.ts --project=chromium
```

Open the Playwright UI:

```bash
pnpm run test:e2e:ui
```

Check the installed CLI version:

```bash
pnpm exec playwright --version
```

## Programmatic Use

Use `@playwright/test` for imports:

```ts
import { expect, test } from "@playwright/test";
```

For one-off Node checks, resolve `@playwright/test` rather than `playwright`:

```bash
node -e "const pw = require('@playwright/test'); console.log(Boolean(pw.chromium))"
```

`require.resolve("playwright")` can return `NOT_FOUND` in this repository even
when Playwright is working, because pnpm keeps the transitive `playwright`
package under `@playwright/test`.

## Local App Startup

`playwright.config.ts` starts the Vite desktop frontend automatically:

```ts
webServer: {
  command: "pnpm --filter @unfour/desktop dev --host 127.0.0.1",
  url: "http://127.0.0.1:1420",
}
```

Specs should call `page.goto("/")` and rely on the configured `baseURL`.

## Screenshots

Use `testInfo.outputPath(...)` for screenshots or other generated files. The
repository ignores `test-results` and `playwright-report`, so these artifacts do
not need to be committed.

```ts
await page.screenshot({
  fullPage: true,
  path: testInfo.outputPath("api-client-layout.png"),
});
```

## Troubleshooting

- If `pnpm exec playwright --version` works but `require("playwright")` fails,
  use `@playwright/test`; this is the supported project entry point.
- If the web server is already running on port `1420`, Playwright reuses it
  outside CI.
- If browser binaries are missing, run `pnpm exec playwright install` manually.
  This downloads external browser assets and should not be done silently by
  automation.
