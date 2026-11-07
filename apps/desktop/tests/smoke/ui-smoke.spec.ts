import { expect, test } from "@playwright/test";

test("app shell renders and module switching stays stable", async ({ page }) => {
  const consoleErrors: string[] = [];

  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => {
    consoleErrors.push(error.message);
  });

  await page.goto("/");

  await expect(page.locator(".app-shell")).toBeVisible();
  await expect(page.getByRole("button", { name: /default workspace/i })).toBeVisible();

  const moduleNav = page.getByRole("navigation", { name: "Modules" });
  await expect(moduleNav).toBeVisible();
  await expect(moduleNav.getByRole("button", { name: "API" })).toHaveAttribute(
    "title",
    "API Client",
  );
  await expect(moduleNav.getByRole("button", { name: "SSH" })).toHaveAttribute(
    "title",
    "SSH Terminal",
  );
  await expect(
    moduleNav.getByRole("button", { name: "Database" }),
  ).toHaveAttribute("title", "Database");

  // Return to Database after another persistent feature has published its
  // shell slots; the first mount alone did not catch the native RC render loop.
  for (const label of ["API Client", "SSH Terminal", "Database", "Flow", "Database", "API Client", "Database"]) {
    await moduleNav.getByRole("button", { name: label }).click();
    await expect(page.locator(".app-shell")).toBeVisible();
    if (label === "Database") {
      await expect(page.getByRole("heading", { name: "Add your first database connection" })).toBeVisible();
      await expect(page.getByRole("contentinfo")).toContainText("No active database session");
    }
  }

  expect(consoleErrors).toEqual([]);
});
