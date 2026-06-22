// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EnvironmentEditor } from "./EnvironmentEditor";
import { EnvironmentHints } from "./KeyValueEditor";

afterEach(cleanup);

function withI18n(children: ReactNode) {
  return <I18nProvider initialLocale="en">{children}</I18nProvider>;
}

describe("EnvironmentEditor", () => {
  it("keeps sensitive-looking variable values visible", () => {
    render(
      withI18n(
        <EnvironmentEditor
          environment={{
            id: "env-1",
            workspaceId: "ws-1",
            name: "Local",
            variables: [{ enabled: true, key: "auth_token", value: "secret" }],
            isActive: false,
            createdAt: "2026-01-01T00:00:00Z",
            updatedAt: "2026-01-01T00:00:00Z",
          }}
          onSave={vi.fn()}
        />,
      ),
    );

    expect(screen.getByDisplayValue("secret")).toHaveAttribute("type", "text");
  });
});

describe("EnvironmentHints", () => {
  it("does not describe sensitive-looking variables as masked", () => {
    render(
      withI18n(
        <EnvironmentHints
          variables={[{ enabled: true, key: "auth_token", value: "secret" }]}
        />,
      ),
    );

    expect(
      screen.queryByText(/Sensitive-looking values are masked locally/i),
    ).not.toBeInTheDocument();
  });
});
