import { describe, expect, it } from "vitest";
import {
  activeWorkspaceEnvironmentName,
  activeWorkspaceEnvironmentId,
  defaultTaskRunInputs,
  mergeWorkspaceVariables,
  workspaceEnvironmentById,
} from "./task-run-inputs";

describe("task run workspace variable defaults", () => {
  it("merges workspace vars then overlays the active environment", () => {
    const environment = {
      id: "dev",
      name: "Dev",
      isActive: true,
      variables: [
        {
          key: "archive_name",
          value: "from-env",
          isEnabled: true,
          isSecret: false,
        },
        {
          key: "token",
          value: "secret-token",
          isEnabled: true,
          isSecret: true,
        },
      ],
    };
    const merged = mergeWorkspaceVariables(
      [
        {
          key: "source_image",
          value: "workspace-image",
          isEnabled: true,
          isSecret: false,
        },
        {
          key: "archive_name",
          value: "from-workspace",
          isEnabled: true,
          isSecret: false,
        },
        {
          key: "disabled_key",
          value: "nope",
          isEnabled: false,
          isSecret: false,
        },
      ],
      environment,
    );

    expect(merged.get("source_image")?.value).toBe("workspace-image");
    expect(merged.get("archive_name")?.value).toBe("from-env");
    expect(merged.get("token")).toEqual({
      key: "token",
      value: "secret-token",
      isSecret: true,
    });
    expect(merged.has("disabled_key")).toBe(false);
  });

  it("prefills matching placeholders case-insensitively and leaves the rest empty", () => {
    const variables = mergeWorkspaceVariables(
      [
        {
          key: "SOURCE_IMAGE",
          value: "nginx:latest",
          isEnabled: true,
          isSecret: false,
        },
        {
          key: "API_TOKEN",
          value: "secret",
          isEnabled: true,
          isSecret: true,
        },
      ],
      null,
    );

    expect(
      defaultTaskRunInputs(
        ["source_image", "target_image", "API_TOKEN"],
        variables,
      ),
    ).toEqual({
      inputs: {
        source_image: "nginx:latest",
        target_image: "",
        API_TOKEN: "secret",
      },
      secretNames: ["API_TOKEN"],
    });
  });

  it("reports the active environment name when present", () => {
    expect(
      activeWorkspaceEnvironmentName([
        { name: " Dev ", isActive: true, variables: [] },
        { name: "Prod", isActive: false, variables: [] },
      ]),
    ).toBe("Dev");
    expect(activeWorkspaceEnvironmentName([{ isActive: false, variables: [] }])).toBe(
      null,
    );
  });

  it("selects a run-only environment without changing active state", () => {
    const environments = [
      { id: "dev", name: "Dev", isActive: true, variables: [] },
      { id: "prod", name: "Prod", isActive: false, variables: [] },
    ];
    expect(activeWorkspaceEnvironmentId(environments)).toBe("dev");
    expect(workspaceEnvironmentById(environments, "prod")?.name).toBe("Prod");
    expect(workspaceEnvironmentById(environments, "")).toBeNull();
    expect(environments[0].isActive).toBe(true);
  });
});
