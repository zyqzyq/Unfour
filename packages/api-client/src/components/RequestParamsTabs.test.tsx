// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RequestParamsTabs } from "./RequestParamsTabs";
import type { ApiAuthConfig } from "../model/types";

vi.mock("@monaco-editor/react", () => ({
  default: ({
    onChange,
    value,
  }: {
    onChange: (value: string) => void;
    value: string;
  }) => (
    <textarea
      aria-label="script editor"
      onChange={(event) => onChange(event.target.value)}
      value={value}
    />
  ),
}));

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
        onPostResponseScriptChange={vi.fn()}
        onPreRequestScriptChange={vi.fn()}
        onQueryChange={vi.fn()}
        onRawBodyTypeChange={vi.fn()}
        onTabChange={vi.fn()}
        query={[]}
        rawBodyType="json"
        postResponseScript=""
        preRequestScript=""
        tab="auth"
      />
    </I18nProvider>,
  );
}

describe("RequestParamsTabs auth inputs", () => {
  it("switches request timeout between inherit, custom, and unlimited", () => {
    const onTimeoutChange = vi.fn();
    render(
      <I18nProvider initialLocale="en">
        <RequestParamsTabs
          auth={{ type: "none" }}
          body=""
          bodyMode="none"
          formBody={[]}
          headers={[]}
          onAuthChange={vi.fn()}
          onBodyChange={vi.fn()}
          onBodyModeChange={vi.fn()}
          onFormBodyChange={vi.fn()}
          onHeadersChange={vi.fn()}
          onPostResponseScriptChange={vi.fn()}
          onPreRequestScriptChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          onTimeoutChange={onTimeoutChange}
          postResponseScript=""
          preRequestScript=""
          query={[]}
          rawBodyType="json"
          tab="settings"
          timeoutMs={null}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("radio", { name: /Custom timeout/ }));
    expect(onTimeoutChange).toHaveBeenCalledWith(0);
    expect(screen.getByLabelText("Request timeout in milliseconds")).toBeDisabled();
  });

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
          onPostResponseScriptChange={vi.fn()}
          onPreRequestScriptChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          query={[]}
          rawBodyType="json"
          postResponseScript=""
          preRequestScript=""
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
          onPostResponseScriptChange={vi.fn()}
          onPreRequestScriptChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          query={[]}
          rawBodyType="json"
          postResponseScript=""
          preRequestScript=""
          tab="auth"
        />
      </I18nProvider>,
    );
    expect(screen.getByLabelText("Value")).toHaveAttribute("type", "text");
  });

  it("loads and edits both request-level script phases", () => {
    const onPostResponseScriptChange = vi.fn();
    render(
      <I18nProvider initialLocale="en">
        <RequestParamsTabs
          auth={{ type: "none" }}
          body=""
          bodyMode="none"
          formBody={[]}
          headers={[]}
          onAuthChange={vi.fn()}
          onBodyChange={vi.fn()}
          onBodyModeChange={vi.fn()}
          onFormBodyChange={vi.fn()}
          onHeadersChange={vi.fn()}
          onPostResponseScriptChange={onPostResponseScriptChange}
          onPreRequestScriptChange={vi.fn()}
          onQueryChange={vi.fn()}
          onRawBodyTypeChange={vi.fn()}
          onTabChange={vi.fn()}
          postResponseScript="pm.test('post', () => {})"
          preRequestScript="console.log('pre')"
          query={[]}
          rawBodyType="json"
          tab="scripts"
        />
      </I18nProvider>,
    );

    expect(screen.getByLabelText("script editor")).toHaveValue("console.log('pre')");
    fireEvent.click(screen.getByRole("button", { name: "Post-response" }));
    expect(screen.getByLabelText("script editor")).toHaveValue(
      "pm.test('post', () => {})",
    );
    fireEvent.change(screen.getByLabelText("script editor"), {
      target: { value: "console.warn('changed')" },
    });
    expect(onPostResponseScriptChange).toHaveBeenCalledWith("console.warn('changed')");
  });
});
