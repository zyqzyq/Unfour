// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RequestParamsTabs } from "./RequestParamsTabs";
import type { ApiAuthConfig } from "../model/types";

afterEach(cleanup);

function renderAuth(auth: ApiAuthConfig) {
  return render(
    <I18nProvider initialLocale="en">
      <RequestParamsTabs
        auth={auth}
        body=""
        bodyMode="none"
        formBody={[]}
        headers={[]}
        onAuthChange={vi.fn()}
        onBodyChange={vi.fn()}
        onBodyModeChange={vi.fn()}
        onFormBodyChange={vi.fn()}
        onHeadersChange={vi.fn()}
        onQueryChange={vi.fn()}
        onRawBodyTypeChange={vi.fn()}
        onTabChange={vi.fn()}
        query={[]}
        rawBodyType="json"
        tab="auth"
      />
    </I18nProvider>,
  );
}

describe("RequestParamsTabs auth inputs", () => {
  it("shows auth secret values as editable text instead of password fields", () => {
    const { rerender } = renderAuth({ type: "bearer", token: "secret-token" });

    expect(screen.getByLabelText("Token")).toHaveAttribute("type", "text");

    rerender(
      <I18nProvider initialLocale="en">
        <RequestParamsTabs
          auth={{ type: "basic", username: "user", password: "secret-password" }}
          body=""
          bodyMode="none"
          formBody={[]}
          headers={[]}
          onAuthChange={vi.fn()}
          onBodyChange={vi.fn()}
          onBodyModeChange={vi.fn()}
          onFormBodyChange={vi.fn()}
          onHeadersChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          query={[]}
          rawBodyType="json"
          tab="auth"
        />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Password")).toHaveAttribute("type", "text");

    rerender(
      <I18nProvider initialLocale="en">
        <RequestParamsTabs
          auth={{
            type: "api-key",
            addTo: "header",
            key: "x-api-key",
            value: "secret-key",
          }}
          body=""
          bodyMode="none"
          formBody={[]}
          headers={[]}
          onAuthChange={vi.fn()}
          onBodyChange={vi.fn()}
          onBodyModeChange={vi.fn()}
          onFormBodyChange={vi.fn()}
          onHeadersChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          query={[]}
          rawBodyType="json"
          tab="auth"
        />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Value")).toHaveAttribute("type", "text");
  });
});
