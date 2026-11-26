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
