import { expect, test, type Locator, type Page } from "@playwright/test";
import { openApiScene, prepareWorkbenchScene } from "./workbench-fixtures";

async function dragAcross(page: Page, locator: Locator) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("Selection target has no layout");
  await page.mouse.move(box.x + 1, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 12 });
  await page.mouse.up();
}

test("workbench drags avoid chrome selection while URL and response text stay selectable", async ({ page }) => {
  await prepareWorkbenchScene(page, "light");
  await page.goto("/");
  await openApiScene(page);

  const params = page.getByRole("button", { name: /^Params\b/ });
  await dragAcross(page, params);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("");

  await dragAcross(page, page.getByTitle("List members", { exact: true }));
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("");

  const tab = page.getByRole("tab", { name: /List members/ });
  await dragAcross(page, tab);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("");

  const url = page.getByRole("textbox", { name: "Request URL", exact: true });
  await dragAcross(page, url);
  expect(await url.evaluate((node: HTMLInputElement) =>
    (node.selectionEnd ?? 0) - (node.selectionStart ?? 0),
  )).toBeGreaterThan(0);
  await url.press("Control+A");
  expect(await url.evaluate((node: HTMLInputElement) =>
    node.value.slice(node.selectionStart ?? 0, node.selectionEnd ?? 0),
  )).toContain("api.example.test");

  // The second Headers control belongs to the response, after request config.
  await page.getByRole("button", { name: /^Headers/ }).last().click();
  const headerValue = page.getByText("no-store", { exact: true });
  await expect(headerValue).toBeVisible();
  await dragAcross(page, headerValue);
  expect(await page.evaluate(() => window.getSelection()?.toString())).toContain("no-store");
});
