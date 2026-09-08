// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider, type Locale } from "@unfour/ui";
import { SettingsAbout } from "./SettingsAbout";

const getAppInfo = vi.hoisted(() => vi.fn());

vi.mock("@unfour/command-client", () => ({ getAppInfo }));

function renderAbout(children?: ReactNode, locale: Locale = "en") {
  return render(
    <I18nProvider initialLocale={locale} storageKey={`test.settings-about.${locale}.locale`}>
      <SettingsAbout>{children}</SettingsAbout>
    </I18nProvider>,
  );
}

beforeEach(() => {
  getAppInfo.mockResolvedValue({
    name: "Unfour",
    version: "0.9.4",
    distribution: "microsoft-store",
    channel: "stable",
    commit: "0123456789abcdef-dirty",
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("SettingsAbout", () => {
  it("keeps application identity and embedded update state on one About page", async () => {
    renderAbout(<div>Update available</div>);

    expect(screen.getByRole("heading", { name: "About" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Application" })).toBeTruthy();
    expect(screen.getByText("Update available")).toBeTruthy();
    expect(screen.getByRole("link", { name: /unfour\.dev/i })).toBeTruthy();
    expect(screen.getByRole("link", { name: /github\.com\/zyqzyq\/unfour$/i })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Links" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Feedback & Support" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Actions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy version info" })).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByText("0.9.4")).toBeTruthy();
      expect(screen.getByText("Microsoft Store")).toBeTruthy();
      expect(screen.getByText("Stable")).toBeTruthy();
      expect(screen.getByText("0123456789ab-dirty")).toBeTruthy();
    });
  });

  it("places feedback links after the updates slot and before copy actions", () => {
    renderAbout(<div>Update available</div>);

    const updates = screen.getByText("Update available");
    const feedback = screen.getByRole("heading", { name: "Feedback & Support" });
    const actions = screen.getByRole("heading", { name: "Actions" });

    expect(updates.compareDocumentPosition(feedback) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(feedback.compareDocumentPosition(actions) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    const sendFeedback = screen.getByRole("link", { name: "Send feedback" });
    expect(sendFeedback).toHaveAttribute("href", "https://github.com/zyqzyq/Unfour/discussions");
    expect(sendFeedback).toHaveAttribute("target", "_blank");

    const reportIssue = screen.getByRole("link", { name: "Report an issue" });
    expect(reportIssue).toHaveAttribute(
      "href",
      "https://github.com/zyqzyq/Unfour/issues/new?template=bug_report.yml",
    );
    expect(reportIssue).toHaveAttribute("target", "_blank");
  });

  it("keeps the feedback block available for the standard distribution", async () => {
    getAppInfo.mockResolvedValue({
      name: "Unfour",
      version: "0.9.4",
      distribution: "standard",
      channel: "test",
      commit: null,
    });
    renderAbout();

    await waitFor(() => {
      expect(screen.getByText("Standard")).toBeTruthy();
    });
    expect(screen.getByRole("heading", { name: "Feedback & Support" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Send feedback" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy version info" })).toBeTruthy();
  });

  it("localizes the feedback block in zh-CN", () => {
    renderAbout(undefined, "zh-CN");

    expect(screen.getByRole("heading", { name: "反馈与支持" })).toBeTruthy();
    expect(
      screen.getByText("Unfour 目前由我独立开发，每一条反馈我都会看。"),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "发送反馈" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "报告问题" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "复制版本信息" })).toBeTruthy();
  });
});
