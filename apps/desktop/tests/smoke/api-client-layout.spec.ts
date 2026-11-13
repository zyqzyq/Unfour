import { expect, test } from "@playwright/test";

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
