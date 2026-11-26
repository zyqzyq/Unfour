import { expect, test } from "@playwright/test";

test("copies the response body and shows clipboard feedback in the response toolbar", async ({ page }) => {
  let finishWrite!: () => void;
  const firstWrite = new Promise<void>((resolve) => { finishWrite = resolve; });
  await page.exposeFunction("waitForFirstCopy", () => firstWrite);
  await page.addInitScript(() => {
    localStorage.setItem("unfour.locale", "en");
    let attempts = 0;
    Object.defineProperty(navigator, "clipboard", { value: {
      async writeText(value: string) {
        sessionStorage.setItem("copy-attempts", String(++attempts));
        if (attempts === 1) {
          await (window as unknown as { waitForFirstCopy: () => Promise<void> }).waitForFirstCopy();
          throw new DOMException("Denied", "NotAllowedError");
        }
        sessionStorage.setItem("test-clipboard", value);
      },
    } });
  });
  const body = '{ "message": "original spacing" }';
  await page.route("https://example.test/copy", (route) => route.fulfill({ contentType: "application/json", body }));
  await page.goto("/");
  await page.getByRole("textbox", { name: "Request URL" }).fill("https://example.test/copy");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  const copy = page.getByRole("button", { name: "Copy response body" });
  await copy.focus();
  await page.keyboard.press("Enter");
  await expect(copy).toHaveAttribute("aria-busy", "true");
  await expect(copy).toBeFocused();
  await page.keyboard.press("Enter");
  expect(await page.evaluate(() => sessionStorage.getItem("copy-attempts"))).toBe("1");
  finishWrite();
  await expect(page.getByText("Could not copy the response body.")).toBeVisible();
  await expect(copy).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(copy).toHaveAttribute("title", "Response body copied");
  expect(await page.evaluate(() => sessionStorage.getItem("test-clipboard"))).toBe(body);
  await expect(copy).toBeFocused();
  await expect(copy).toHaveAttribute("title", "Copy response body");
});

test("request history folds down and gives its space back to collections", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "zh-CN"));
  await page.setViewportSize({ width: 1024, height: 640 });
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      for (let i = 1; i <= 20; i++) {
        mockStore.history.push({ id: 'h' + i, workspaceId: 'mock-workspace', name: 'Request ' + i,
          method: 'GET', url: 'https://example.test/' + i, createdAt: '2026-10-01T10:00:00Z',
          status: 200, durationMs: 10 });
      }
    ` });
  });
  await page.goto("/");
  const expand = page.getByRole("button", { name: "展开请求历史" });
  const collapse = page.getByRole("button", { name: "收起请求历史" });
  await expect(expand).toBeInViewport({ ratio: 1 });
  const folded = (await expand.boundingBox())!;
  await expand.press("Enter");
  await expect(page.getByText("Request 1", { exact: true })).toBeInViewport();
  const opened = (await collapse.boundingBox())!;
  expect(opened.y).toBeLessThan(folded.y - 100);
  await page.getByText("Request 20", { exact: true }).scrollIntoViewIfNeeded();
  expect((await collapse.boundingBox())!.y).toBe(opened.y);
  await page.screenshot({ path: testInfo.outputPath("history-expanded.png") });

  await page.reload();
  await expect(collapse).toBeVisible();
  await collapse.press("Space");
  await expect(expand).toBeFocused();
  expect((await expand.boundingBox())!.y).toBe(folded.y);
  await page.screenshot({ path: testInfo.outputPath("history-collapsed.png") });
});

test("API shows the environment tunnel, supports a request override and keeps the original URL", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockStore.sshConnections.push({ id:"qa-jump", name:"QA jump", workspaceId:"mock-workspace",
        host:"qa.example.test", port:22, username:"qa", authKind:"agent", revision:3 });
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "API" }).click();
  const url = page.getByLabel("Request URL");
  await url.fill("https://orders.internal/status");
  const dialog = page.getByRole("dialog");
  await page.getByRole("button", { name: "Network route · Environment · Direct" }).click();
  await dialog.getByLabel("Default for Default").selectOption("ssh:qa-jump");
  await dialog.getByRole("button", { name: "Apply", exact: true }).click();
  const ssh = page.getByRole("button", { name: "Network route · Environment · SSH · QA jump" });
  await expect(ssh).toBeVisible();
  await expect(url).toHaveValue("https://orders.internal/status");
  // Browser mode must not silently send an SSH-routed request directly.
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.getByText("This build does not include native SSH forwarding.").first()).toBeVisible();
  await ssh.click();
  await dialog.getByLabel("This request").selectOption("direct");
  await dialog.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(page.getByRole("button", { name: "Network route · Direct" })).toBeVisible();
  await expect(url).toHaveValue("https://orders.internal/status");
});

test("api client keeps a Postman-like vertical workbench layout", async ({
  page,
}, testInfo) => {
  const consoleErrors: string[] = [];

  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => {
    consoleErrors.push(error.message);
  });

  await page.setViewportSize({ height: 900, width: 1440 });
  await page.goto("/");

  const moduleNav = page.getByRole("navigation", { name: "Modules" });
  await expect(moduleNav).toBeVisible();
  await moduleNav.getByRole("button", { name: "API" }).click();

  await expect(
    page.getByRole("tablist", { name: "Open API requests" }),
  ).toBeVisible();
  await expect(page.getByLabel("Request URL")).toBeVisible();
  await expect(page.getByRole("button", { name: /^Send$/i })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Save$/i })).toBeVisible();

  await expect(page.getByText("Search or run command")).toHaveCount(0);
  await expect(page.getByText(/^unsaved$/i)).toHaveCount(0);

  const requestAnchor = page.getByLabel("Request URL");
  const responseEmptyState = page.getByText("No response yet");
  await expect(requestAnchor).toBeVisible();
  await expect(responseEmptyState).toBeVisible();

  const requestBox = await requestAnchor.boundingBox();
  const responseBox = await responseEmptyState.boundingBox();
  expect(requestBox).not.toBeNull();
  expect(responseBox).not.toBeNull();
  expect(responseBox!.y).toBeGreaterThan(requestBox!.y + 60);

  await page.screenshot({
    fullPage: true,
    path: testInfo.outputPath("api-client-layout.png"),
  });

  expect(consoleErrors).toEqual([]);
});
