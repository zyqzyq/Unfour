import { expect, test } from "@playwright/test";

test("opens Redis alongside SQL and starts a Flow from an inspected key", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  // Keep the UI, connection CRUD, tabs and command transport. A native driver
  // handles the exact-key read in the desktop build.
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

test("reviews a local Redis key creation, refreshes its value and confirms deletion", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/redis.ts*", async (route) => {
    const response = await route.fetch();
    const body = (await response.text()).replace("switch (command) {", `switch (command) {
      case "redis_key_inspect": {
        const entry = window.__cacheFixture?.[args.keyBase64];
        const value = entry?.valueBase64;
        return {exists:value !== undefined, keyType:value === undefined ? "none" : "string",
          pttlMs:entry?.ttlMs ?? -2, length:value === undefined ? null : atob(value).length, fieldFound:null,
          preview:value === undefined ? null : {base64:value,truncated:false}};
      }
      case "redis_key_mutate": {
        if (!args.confirmed || args.expectedRevision !== 1) throw Error("expected scoped confirmation");
        if (args.keyBase64 !== btoa("product:42")) throw Error("unexpected key");
        if (args.mutation.kind === "createString") {
          if (args.mutation.ttlMs !== 120000) throw Error("expected a 120-second expiry");
          window.__cacheFixture = {[args.keyBase64]: args.mutation};
        }
        else if (args.mutation.kind === "deleteKey" && args.mutation.expectedType === "string") delete window.__cacheFixture[args.keyBase64];
        else throw Error("unexpected mutation");
        return;
      }
    `);
    await route.fulfill({ response, body });
  });
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Database", exact: true }).click();
  await page.getByRole("complementary").getByRole("button", { name: "New database connection", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Database type").press("End");
  await dialog.getByLabel("Name", { exact: true }).fill("Writable cache");
  await dialog.getByLabel("Allow manual Redis writes on this device").check();
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await page.getByRole("button", { name: "New key", exact: true }).click();
  await dialog.getByLabel("Exact key name").fill("product:42");
  await dialog.getByLabel("New value", { exact: true }).fill("sample-price");
  await dialog.getByLabel("Time to live (seconds, optional)").fill("120");
  await dialog.getByRole("button", { name: "Review change" }).click();
  await expect(dialog.getByText("SET · NX · PX")).toBeVisible();
  await dialog.getByRole("button", { name: "Apply change" }).click();
  await expect(page.getByText("sample-price", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit key…" }).click();
  await page.getByRole("menuitem", { name: "Delete key" }).click();
  await expect(dialog.getByText("UNLINK")).toBeVisible();
  await dialog.getByRole("button", { name: "Apply change" }).click();
  await expect(page.getByText(/product:42.*missing or expired/)).toBeVisible();
});
