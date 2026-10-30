import { expect, test } from "@playwright/test";

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
