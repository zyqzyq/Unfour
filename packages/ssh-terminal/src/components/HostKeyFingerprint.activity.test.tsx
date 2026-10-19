// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FeedbackProvider, I18nProvider } from "@unfour/ui";
import { HostKeyFingerprint } from "./HostKeyFingerprint";

const mocks = vi.hoisted(() => ({
  getFingerprint: vi.fn(),
  resetFingerprint: vi.fn(),
}));

vi.mock("@unfour/command-client", () => ({
  getSshHostFingerprint: mocks.getFingerprint,
  resetSshHostFingerprint: mocks.resetFingerprint,
}));

const fingerprint = {
  workspaceId: "ws",
  host: "example.com",
  port: 22,
  fingerprint: "SHA256:kept",
  createdAt: "2026-10-19T00:00:00.000Z",
};

function renderFingerprint() {
  return render(
    <I18nProvider initialLocale="en" storageKey="host-key-activity-test">
      <FeedbackProvider>
        <HostKeyFingerprint host="example.com" port={22} workspaceId="ws" />
      </FeedbackProvider>
    </I18nProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.getFingerprint.mockResolvedValue(fingerprint);
});

afterEach(cleanup);

it("clears the trusted fingerprint and warns when activity logging fails", async () => {
  mocks.resetFingerprint.mockResolvedValue({
    deleted: true,
    warnings: ["ACTIVITY_RECORD_FAILED"],
  });
  renderFingerprint();
  expect(await screen.findByText("SHA256:kept")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Reset" }));
  expect(await screen.findByText(/activity log could not be saved/i)).toBeVisible();
  expect(await screen.findByText(/No trusted fingerprint/)).toBeVisible();
  expect(mocks.resetFingerprint).toHaveBeenCalledExactlyOnceWith({
    workspaceId: "ws",
    host: "example.com",
    port: 22,
  });
  expect(screen.queryByText("SHA256:kept")).not.toBeInTheDocument();
});

it("keeps the reset error when the fingerprint was not removed", async () => {
  mocks.resetFingerprint.mockRejectedValue(new Error("fingerprint store unavailable"));
  renderFingerprint();
  expect(await screen.findByText("SHA256:kept")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Reset" }));
  expect(await screen.findByText("fingerprint store unavailable")).toBeVisible();
  expect(screen.queryByText(/activity log could not be saved/i)).not.toBeInTheDocument();
  expect(mocks.resetFingerprint).toHaveBeenCalledOnce();
});
