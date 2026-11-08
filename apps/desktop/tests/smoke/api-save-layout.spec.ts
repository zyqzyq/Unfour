import { expect, test } from "@playwright/test";

test("API save keeps its footer visible after validation at a compact window size", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 600 });
  await page.addInitScript(() => {
    localStorage.setItem("unfour.locale", "en");
    localStorage.setItem("unfour.theme", "light");
  });
  // Disposable browser metadata fills the location picker; no request is sent.
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      for (let index = 0; index < 8; index++) mockStore.collections.push({
        id: 'save-layout-' + index, workspaceId: 'mock-workspace',
        name: 'Collection ' + index, description: null,
        createdAt: '2026-01-01', updatedAt: '2026-01-01', deletedAt: null
      });
      mockStore.savedRequests.push({
        id: 'save-layout-duplicate', workspaceId: 'mock-workspace',
        collectionId: 'save-layout-0', parentFolderId: null, name: 'Duplicate request',
        method: 'GET', url: 'https://example.test/save-layout',
        headersJson: '[]', queryJson: '[]', bodyKind: 'none', body: '', deletedAt: null
      });
    ` });
  });
  await page.goto("/");
  await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("https://example.test/save-layout");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  const dialog = page.getByRole("dialog");
  const name = dialog.getByRole("textbox", { name: "Name", exact: true });
  await name.fill("Duplicate request");
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog.getByText(/already exists in this location/)).toBeVisible();

  // Check bounds before scrolling or trial clicks: Playwright can scroll an
  // overflow-hidden ancestor and otherwise conceal a clipped-footer defect.
  const frame = await dialog.boundingBox();
  expect(frame).not.toBeNull();
  for (const label of ["Cancel", "Save"]) {
    const button = dialog.getByRole("button", { name: label, exact: true });
    const bounds = await button.boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds!.y).toBeGreaterThanOrEqual(frame!.y);
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(frame!.y + frame!.height);
  }
  const error = dialog.getByText(/already exists in this location/);
  await error.scrollIntoViewIfNeeded();
  await expect(error).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: testInfo.outputPath("api-save-validation-compact.png"), animations: "disabled" });

  await name.fill("Saved request");
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /Saved request/ })).toBeVisible();
});
