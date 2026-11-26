import { expect, test } from "@playwright/test";

test("keeps the environment title, Save and save errors in view while editing the last variable", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/workspace.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      import { mockStore } from "./state.ts";
      const environment = mockStore.workspaceEnvironments[0];
      environment.name = "Long environment";
      environment.variables = Array.from({ length: 100 }, (_, index) => ({
        ...environment.variables[0], id: "var-" + index, key: "KEY_" + index, sortOrder: index,
      }));
      const update = handlers.get("workspace_environment_update");
      handlers.set("workspace_environment_update", () => {
        handlers.set("workspace_environment_update", update);
        throw new Error("Test save failed");
      });
    ` });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Active variable environment" }).click();
  await page.getByRole("menuitem", { name: "Manage variables" }).click();

  const title = page.getByRole("heading", { name: "Long environment", exact: true });
  const save = page.getByRole("button", { name: "Save", exact: true });
  const values = page.getByRole("textbox", { name: "Value", exact: true });
  await expect(values).toHaveCount(100);
  const lastValue = values.last();
  await lastValue.scrollIntoViewIfNeeded();
  await lastValue.fill("edited at the bottom");
  await expect(values.first()).not.toBeInViewport();
  await expect(title).toBeInViewport();
  await expect(save).toBeInViewport();

  await save.click();
  const error = page.getByRole("alert").filter({ hasText: "Test save failed" });
  await expect(error).toBeInViewport();
  await expect(lastValue).toBeInViewport();
  await expect(lastValue).toHaveValue("edited at the bottom");

  await values.first().scrollIntoViewIfNeeded();
  await expect(lastValue).not.toBeInViewport();
  await expect(error).toBeInViewport();
  await lastValue.scrollIntoViewIfNeeded();

  await save.click();
  await expect(error).toHaveCount(0);
  await expect(save).toBeDisabled();
  await expect(lastValue).toBeInViewport();
  await expect(lastValue).toHaveValue("edited at the bottom");
});

test("filters by variable name and saves hidden edits alongside visible ones", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.goto("/");
  await page.getByRole("button", { name: "Active variable environment" }).click();
  await page.getByRole("menuitem", { name: "Manage variables" }).click();
  const filter = page.getByRole("textbox", { name: "Filter by variable name" });
  const keys = page.getByRole("textbox", { name: "Key", exact: true });
  const values = page.getByRole("textbox", { name: "Value", exact: true });
  await expect(keys).toHaveCount(2);
  await values.first().fill("hidden edit");
  await filter.fill(" SOURCE ");
  await expect(keys).toHaveCount(1);
  await expect(keys).toHaveValue("source");
  await keys.fill("renamed");
  await keys.press("Tab");
  await expect(values).toBeFocused();
  await values.fill("visible edit");
  await filter.focus();
  await expect(page.getByText("No variable names match the filter.")).toBeVisible();
  await page.getByRole("button", { name: "Add variable" }).click();
  await expect(keys).toBeFocused();
  await keys.fill("source_image");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(keys).toHaveCount(3);
  await expect(values.nth(0)).toHaveValue("hidden edit");
  await expect(values.nth(1)).toHaveValue("visible edit");
  await expect(keys.nth(2)).toHaveValue("source_image");

  await filter.fill("e");
  await keys.first().fill("url");
  await page.getByRole("button", { name: "Delete variable renamed" }).click();
  await expect(page.getByRole("button", { name: "Delete variable renamed" })).toHaveCount(0);
  await filter.focus();
  await expect(keys).toHaveCount(1);
  await expect(keys).toHaveValue("source_image");
});
