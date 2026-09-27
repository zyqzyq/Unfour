import { expect, test } from "@playwright/test";

test("Flow condition and Wait Until use grouped variables and preserve legacy predicates", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  // Disposable authoring fixture; no native execution or external requests.
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockStore.savedRequests.push({id:'status',workspaceId:'mock-workspace',name:'Get status',method:'GET',url:'https://example.test/status',headersJson:'[]',queryJson:'[]',bodyKind:'none',body:null,deletedAt:null});
    ` });
  });
  await page.route("**/browser-mocks/flow.ts*", async (route) => {
    await route.fulfill({ contentType: "application/javascript", body: `
      import { UNHANDLED } from "./types.ts";
      let definition;
      export function handleFlowMock(command, args) {
        if (command === 'flow_list') {
          definition ??= {id:'fixture',workspaceId:args.workspaceId,name:'Deployment readiness',revision:2,
            inputs:[{name:'desiredStatus',type:'string',required:true,secret:false,default:'ready'}],steps:[
              {id:'fetch',name:'Get deployment',kind:'action',timeoutMs:60000,next:null,action:{capability:'api',resourceId:'status',connectionId:null,arguments:{}}},
              {id:'wait',name:'Wait for ready',kind:'waitUntil',timeoutMs:60000,next:null,intervalMs:5000,maxAttempts:null,probeErrorPolicy:'failImmediately',intervalStrategy:'fixed',probe:{capability:'api',resourceId:'status',connectionId:null,arguments:{}},successWhen:{left:{$ref:'/probe/body/status'},op:'eq',right:'ready'},failureWhen:null},
              {id:'check',name:'Check status',kind:'condition',timeoutMs:1000,next:null,ifTrue:'$end',ifFalse:'$end',predicate:{left:{$ref:'/steps/wait/result/body/status'},op:'eq',right:'ready'}}
            ]};
          return [definition];
        }
        if (command === 'flow_save') { definition = {...args.input,revision:args.input.revision+1}; return definition; }
        if (command === 'flow_runs_list') return [];
        return UNHANDLED;
      }
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "Deployment readiness", exact: true }).click();
  await expect(page.getByText("Environment is selected at run time and shared by the entire Flow.")).toBeVisible();
  await page.getByTestId("rf__node-wait").click();
  const inspector = page.getByRole("complementary", { name: "Inspector" }).locator("section:visible");
  await expect(inspector.getByText("Uses Flow environment")).toBeVisible();
  await expect(inspector.getByLabel("Check every (ms)")).toHaveValue("5000");
  await expect(inspector.getByLabel("Timeout (ms)")).toHaveValue("60000");
  const source = inspector.getByLabel("Success condition · Value · Variable");
  await expect(source.locator('optgroup[label="Previous Steps"] option')).toHaveCount(5);
  await expect(source.locator('optgroup[label="Poll / Wait Until probe output"] option')).toHaveCount(5);
  await expect(source).toHaveValue("/probe/body");
  await expect(inspector.getByLabel("Nested field (optional, dot separated)")).toHaveValue("status");
  await inspector.getByLabel("Success condition · Compare with · Type").selectOption("variable");
  await inspector.getByLabel("Success condition · Compare with · Variable").selectOption("/inputs/desiredStatus");
  await page.screenshot({ animations: "disabled", path: "test-results/flow-control-wait-until.png" });
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r3", { exact: true })).toBeVisible();
  await page.getByTestId("rf__node-check").click();
  await expect(inspector.getByLabel("Condition · Value · Variable")).toHaveValue("/steps/wait/result/body");
  await inspector.getByLabel("Condition · Operator").selectOption("ne");
  await inspector.getByLabel("Condition · Compare with", { exact: true }).fill("failed");
  await expect(page.getByTestId("rf__node-check")).toContainText('Wait for ready · result.body.status Does not equal "failed"');
  await page.screenshot({ animations: "disabled", path: "test-results/flow-control-condition.png" });
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r4", { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});
