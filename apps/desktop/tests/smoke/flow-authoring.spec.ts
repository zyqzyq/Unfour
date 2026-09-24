import { expect, test } from "@playwright/test";

test("Flow uses saved API overrides, detected SSH inputs and a required SQL editor", async ({ page }) => {
  // Disposable browser fixture. No external endpoints or native execution.
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockStore.savedRequests.push({id:'flow-api',workspaceId:'mock-workspace',name:'Health',method:'GET',url:'https://example.test/health',headersJson:JSON.stringify([{key:'Accept',value:'application/json',enabled:true}]),queryJson:JSON.stringify([{key:'page',value:'1',enabled:true}]),bodyKind:'json',body:'{"ready":true}',deletedAt:null});
      mockStore.databaseConnections.push({id:'flow-db',workspaceId:'mock-workspace',name:'Local database',driver:'sqlite',readOnly:true});
      mockStore.sshConnections.push({id:'flow-ssh',workspaceId:'mock-workspace',name:'Fixed connection',host:'127.0.0.1',port:22,username:'fixture'});
      mockStore.sshTasks.push({task:{id:'flow-task',workspaceId:'mock-workspace',name:'Deploy',deletedAt:null},localBinding:{defaultConnectionId:'flow-ssh',lastUsedConnectionId:'unrelated'},steps:[{enabled:true,stepType:'command',configJson:{command:'echo {{VERSION}}',workingDirectory:'/tmp'}}]});
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.locator(".react-flow__node").filter({ hasText: "API Request" }).click();
  const inspector = page.getByRole("complementary", { name: "Inspector" });
  await inspector.getByLabel("Referenced resource").selectOption("flow-api");
  await expect(inspector.getByText("GET · Health")).toBeVisible();
  const headers = inspector.getByRole("group", { name: "Headers" });
  await headers.getByRole("button", { name: "Override", exact: true }).click();
  await headers.getByLabel("Headers 1", { exact: true }).fill("text/plain");
  await expect(headers.getByText(/Accept: application\/json/)).toBeVisible();
  await expect(headers.getByLabel("Headers 1 · Type")).toHaveCount(0);
  await page.screenshot({ path: "test-results/flow-authoring-api.png" });
  await inspector.getByLabel("Capability").selectOption("database");
  await inspector.getByLabel("Connection").selectOption("flow-db");
  await expect(inspector.getByLabel("SQL", { exact: true })).toHaveJSProperty("tagName", "TEXTAREA");
  await inspector.getByLabel("SQL", { exact: true }).fill("SELECT 1; SELECT 2;");
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await inspector.getByLabel("SQL", { exact: true }).fill("SELECT 1;\n-- one statement");
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeEnabled();
  await page.screenshot({ path: "test-results/flow-authoring-database.png" });
  await inspector.getByLabel("Capability").selectOption("ssh");
  await inspector.getByLabel("Referenced resource").selectOption("flow-task");
  await expect(inspector.getByLabel("Connection")).toHaveValue("flow-ssh");
  await expect(inspector.getByLabel("VERSION", { exact: true })).toBeVisible();
  await expect(inspector.getByRole("button", { name: "Add field" })).toHaveCount(0);
  await inspector.getByLabel("VERSION", { exact: true }).fill("v1");
  await page.screenshot({ path: "test-results/flow-authoring-ssh.png" });
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r1", { exact: true })).toBeVisible();
});
