import { expect, test } from "@playwright/test";

test("edge insertion menu escapes Canvas clipping near its right edge", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  const canvas = page.getByLabel("Flow Canvas", { exact: true });
  const trigger = page.getByRole("button", { name: "Insert node · Start → API Request", exact: true });
  await expect(trigger).toBeVisible();
  const bounds = (await canvas.boundingBox())!;
  const button = (await trigger.boundingBox())!;
  // Pan the actual viewport, leaving the insertion control just inside its edge.
  const pane = (await page.locator(".react-flow__pane").boundingBox())!;
  const start = { x: pane.x + 30, y: pane.y + pane.height - 30 };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x + bounds.x + bounds.width - 24 - (button.x + button.width / 2), start.y, { steps: 12 });
  await page.mouse.up();
  await trigger.click();
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await expect(canvas.getByRole("menu")).toHaveCount(0);
  await expect(menu).toHaveCount(1);
  await expect.poll(async () => menu.evaluate((element) => {
    const canvas = document.querySelector('[aria-label="Flow Canvas"]')!;
    const rect = element.getBoundingClientRect();
    return rect.right > canvas.getBoundingClientRect().right;
  })).toBe(true);
  // Hit-testing every item checks actual paint/occlusion, including outside Canvas.
  for (const item of await menu.getByRole("menuitem").all()) {
    await expect(item).toBeVisible();
    expect(await item.evaluate((element) => {
      const rect = element.getBoundingClientRect();
      return element.contains(document.elementFromPoint(rect.right - 4, rect.top + rect.height / 2));
    })).toBe(true);
  }
  await page.screenshot({ path: "test-results/flow-edge-menu-portal.png" });
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

test("default Flow Canvas supports Inspector inputs, refs, insertion, deletion and save", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await expect(page.getByLabel("Flow Canvas", { exact: true })).toBeVisible();
  const inspector = page.getByRole("complementary", { name: "Inspector" });
  await inspector.getByRole("button", { name: "Add input", exact: true }).click();
  await inspector.getByLabel("Input name", { exact: true }).fill("endpoint");
  await inspector.getByRole("combobox", { name: "Type", exact: true }).selectOption("string");
  await page.locator('.react-flow__node').filter({ hasText: "API Request" }).click();
  await expect(inspector.getByRole("textbox", { name: "Step name", exact: true })).toBeVisible();
  await inspector.getByLabel("Add parameter").selectOption("url");
  await inspector.getByLabel("URL · Type").selectOption("variable");
  await inspector.getByLabel("URL · Variable").selectOption({ label: "Start · endpoint" });
  await expect(inspector.getByText("endpoint", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Insert node · Start → API Request", exact: true }).click();
  await page.getByRole("menuitem", { name: "Condition", exact: true }).click();
  await expect(inspector.getByRole("combobox", { name: "If true", exact: true })).toHaveValue(await inspector.getByRole("combobox", { name: "If false", exact: true }).inputValue());
  await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("Choose path");
  await page.getByRole("button", { name: "Fit view" }).click();
  await expect(page.locator(".react-flow__node")).toHaveCount(4);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r1", { exact: true })).toBeVisible();
  await page.screenshot({ path: "test-results/flow-canvas-inspector.png" });
  await inspector.getByRole("button", { name: "Remove", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Affected paths");
  await dialog.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(page.locator(".react-flow__node")).toHaveCount(3);
  await expect(inspector.getByLabel("Input name")).toHaveValue("endpoint");
  await page.getByRole("button", { name: "Insert node · API Request → End", exact: true }).click();
  await page.getByRole("menuitem", { name: "Condition", exact: true }).click();
  await inspector.getByLabel("Condition · Value · Variable").selectOption({ label: "Step outputs · API Request · body" });
  await page.getByRole("button", { name: "Fit view" }).click();
  await page.locator(".react-flow__node").filter({ hasText: "API Request" }).click();
  await inspector.getByRole("button", { name: "Remove", exact: true }).first().click();
  await expect(dialog).toContainText("Update these referencing steps before deleting");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator(".react-flow__node")).toHaveCount(4);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r2", { exact: true })).toBeVisible();
  const node = page.locator(".react-flow__node").filter({ hasText: "API Request" });
  const box = await node.boundingBox();
  if (!box) throw new Error("Node has no layout");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 + 40, { steps: 5 });
  await page.mouse.up();
  await expect(page.getByText("r2", { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test("edge plus inserts independently on ordinary and Condition branch connections", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  const inspector = page.getByRole("complementary", { name: "Inspector" });
  await page.getByRole("button", { name: "Insert node · API Request → End", exact: true }).click();
  await page.getByRole("menuitem", { name: "Condition", exact: true }).click();
  await page.getByRole("button", { name: "Fit view" }).click();
  await page.getByRole("button", { name: "Insert node · Condition True → End", exact: true }).click();
  await page.getByRole("menuitem", { name: "Wait", exact: true }).click();
  await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("True wait");
  await page.getByRole("button", { name: "Fit view" }).click();
  await page.getByRole("button", { name: "Insert node · Condition False → End", exact: true }).click();
  await page.getByRole("menuitem", { name: "Wait", exact: true }).click();
  await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("False wait");
  await page.getByRole("button", { name: "Fit view" }).click();
  await page.locator('.react-flow__node').filter({ hasText: "Condition" }).click();
  await expect(inspector.getByLabel("If true").locator("option:checked")).toHaveText("True wait");
  await expect(inspector.getByLabel("If false").locator("option:checked")).toHaveText("False wait");
  await page.screenshot({ path: "test-results/flow-canvas-branches.png" });
});
