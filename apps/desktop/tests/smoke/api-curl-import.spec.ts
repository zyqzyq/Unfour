import { expect, test } from "@playwright/test";

test("pasting cURL previews and fills the current blank request", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.goto("/");
  const url = page.getByRole("textbox", { name: "Request URL", exact: true });
  const paste = async (text: string) => {
    await url.focus();
    await page.evaluate(value => navigator.clipboard.writeText(value), text);
    await url.press("Control+v");
  };
  const dialog = page.getByRole("dialog");
  const command = `"curl.exe" 'https://example.test/api?a=1' -H 'Authorization: Bearer fixture' -d 'a=2'`;
  const tabs = page.getByRole("tab", { name: /Untitled Request/i });
  await expect(tabs).toHaveCount(1);
  await paste("https://example.test/plain");
  await expect(url).toHaveValue("https://example.test/plain");
  await page.getByRole("button", { name: "New Request", exact: true }).click();
  await expect(url).toHaveValue("");
  await paste(command);
  await expect(dialog.getByLabel("HTTP Method")).toHaveValue("POST");
  await expect(dialog.getByLabel("Authorization")).toHaveAttribute("type", "password");
  await expect(dialog.getByLabel("cURL command")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(url).toBeFocused();
  await expect(url).toHaveValue("");

  await paste("curl --unsupported https://example.test");
  await expect(dialog.getByRole("alert")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Apply to this request" })).toBeDisabled();
  await dialog.getByLabel("cURL command").fill(command);
  await dialog.getByRole("button", { name: "Preview" }).click();
  await expect(dialog.getByLabel("cURL command")).toHaveCount(0);
  await expect(dialog.getByLabel("Authorization")).toHaveAttribute("type", "password");
  await dialog.getByRole("button", { name: "Apply to this request" }).click();
  await expect(url).toHaveValue("https://example.test/api?a=1");
  await expect(url).toBeFocused();
  await expect(page.getByRole("combobox", { name: "HTTP method", exact: true })).toHaveValue("POST");
  await expect(tabs).toHaveCount(2);
  await expect(page.getByText("No response yet")).toBeVisible();
  await tabs.first().click();
  await expect(url).toHaveValue("https://example.test/plain");
});

test("cURL preview creates an unsaved request without execution", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  const unexpectedRequests: string[] = [];
  page.on("request", request => {
    if (request.url().startsWith("https://example.test")) unexpectedRequests.push(request.url());
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "API" }).click();
  await page.getByRole("button", { name: "Import cURL" }).click();
  const dialog = page.getByRole("dialog");
  const source = dialog.getByLabel("cURL command");
  await source.fill("curl https://example.test | whoami");
  await dialog.getByRole("button", { name: "Preview" }).click();
  await expect(dialog.getByRole("alert")).toBeVisible();
  await expect(source).toHaveValue("curl https://example.test | whoami");
  await source.fill("curl 'https://example.test/api?a=1&a=2' -H 'Authorization: Bearer fixture' -H 'Content-Type: application/json' --data-raw '{\"hello\":\"world\"}'");
  await dialog.getByRole("button", { name: "Preview" }).click();
  await expect(dialog.getByLabel("HTTP Method")).toHaveValue("POST");
  await expect(dialog.getByLabel("Authorization")).toHaveAttribute("type", "password");
  await source.press("Control+Enter");
  await expect(page.getByText("No response yet")).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("curl-preview.png"), fullPage: true });
  await dialog.getByRole("button", { name: "Create request" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByLabel("Request URL")).toHaveValue("https://example.test/api?a=1&a=2");
  await expect(page.getByText("No response yet")).toBeVisible();
  expect(unexpectedRequests).toEqual([]);
});
