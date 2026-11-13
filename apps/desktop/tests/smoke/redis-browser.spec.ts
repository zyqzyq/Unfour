import { expect, test } from "@playwright/test";

test("opens Redis alongside SQL and starts a Flow from an inspected key", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  // Keep real UI, connection CRUD, tabs and command transport. Only the three
  // Redis network operations require a native driver outside this browser.
  await page.route("**/browser-mocks/redis.ts*", async (route) => {
    const response = await route.fetch();
    const body = (await response.text()).replace("switch (command) {", `switch (command) {
      case "redis_key_inspect": return {exists:true,keyType:"string",pttlMs:-1,length:2,fieldFound:null,preview:{base64:"ODk=",truncated:false}};
    `);
    await route.fulfill({ response, body });
  });
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Database", exact: true }).click();
  const sidebar = page.getByRole("complementary");
  await sidebar.getByRole("button", { name: "New database connection", exact: true }).press("Enter");
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Database type").press("End");
  await dialog.getByLabel("Name", { exact: true }).fill("Product cache");
  await dialog.getByLabel("Name", { exact: true }).press("Enter");
  await expect(sidebar.getByRole("button", { name: "Product cache", exact: true })).toBeVisible();
  await page.getByRole("textbox", { name: "Exact key name" }).fill("product:1001");
  await page.getByRole("button", { name: "Inspect key" }).click();
  await expect(page.getByText("89", { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Query 1" }).click();
  await expect(page.getByRole("textbox", { name: "Exact key name" })).toBeHidden();
  await page.getByRole("tab", { name: "Product cache" }).click();
  await expect(page.getByRole("textbox", { name: "Exact key name" })).toHaveValue("product:1001");
  await expect(page.getByText("89", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Create Flow from this key" }).click();
  await expect(page.getByLabel("Flow name", { exact: true })).toHaveValue("Redis key check");
});
