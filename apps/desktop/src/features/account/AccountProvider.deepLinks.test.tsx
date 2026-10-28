// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { AccountStateSnapshot } from "./accountTypes";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  getCurrent: vi.fn(),
  onOpenUrl: vi.fn(),
  stop: vi.fn(),
  listener: undefined as ((urls: string[]) => void) | undefined,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-deep-link", () => ({ getCurrent: mocks.getCurrent, onOpenUrl: mocks.onOpenUrl }));
vi.mock("./accountForeground", () => ({ listenForAccountForeground: vi.fn(async () => vi.fn()) }));

import { AccountProvider } from "./AccountProvider";
import { useAccount } from "./useAccount";

const signedOut: AccountStateSnapshot = {
  account: { kind: "signedOut" }, syncContext: { kind: "inactive" },
};
const signedIn: AccountStateSnapshot = {
  account: {
    kind: "signedIn",
    profile: {
      id: "recovered", username: "recovered", displayName: "Recovered",
      email: "recovered@example.test", avatarUrl: null, entitlements: [], devices: [],
    },
  },
  syncContext: { kind: "inactive" },
};
const privateCallback = `unfour://auth/callback?authorizationCode=${"P".repeat(43)}&state=${"S".repeat(43)}`;

function Probe() {
  const account = useAccount();
  return <>
    <span data-testid="state">{account.state.kind}</span>
    <span data-testid="failure">{JSON.stringify(account.failure)}</span>
    <button type="button" onClick={account.signIn}>sign in</button>
    <button type="button" onClick={account.retry}>recover</button>
  </>;
}

beforeEach(() => {
  vi.resetAllMocks();
  mocks.listener = undefined;
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  mocks.invoke.mockImplementation(async (command: string) => {
    if (command === "account_get_state") return signedOut;
    if (command === "account_begin_sign_in") return { kind: "signingIn" };
    if (command === "account_handle_deep_link") return signedIn;
    throw new Error("Unexpected account command");
  });
  mocks.getCurrent.mockResolvedValue(null);
  mocks.onOpenUrl.mockImplementation(async (listener: (urls: string[]) => void) => {
    mocks.listener = listener;
    return mocks.stop;
  });
});

afterEach(() => {
  cleanup();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("AccountProvider Deep Link recovery", () => {
  it.each(["sign in", "recover"])("starts fresh authorization via %s after cold-start lookup fails and receives a new callback", async (action) => {
    mocks.getCurrent.mockRejectedValueOnce(new Error(privateCallback));
    const view = render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("failure")).toHaveTextContent("callback_read_failed"));
    expect(screen.getByTestId("failure")).toHaveTextContent("callbackRead");
    expect(screen.getByTestId("failure")).toHaveTextContent("restartSignIn");
    expect(screen.getByTestId("failure")).not.toHaveTextContent("authorizationCode");
    expect(mocks.stop).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: action }));
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("account_begin_sign_in"));
    expect(screen.getByTestId("state")).toHaveTextContent("signingIn");
    expect(screen.getByTestId("failure")).toHaveTextContent("null");

    const callback = privateCallback.replace("P".repeat(43), (action === "sign in" ? "C" : "R").repeat(43));
    mocks.listener?.([callback]);
    await waitFor(() => expect(screen.getByTestId("state")).toHaveTextContent("signedIn"));
    expect(mocks.invoke).toHaveBeenCalledWith("account_handle_deep_link", { url: callback });
    expect(screen.getByTestId("failure")).toHaveTextContent("null");
    expect(mocks.onOpenUrl).toHaveBeenCalledTimes(1);
    expect(mocks.stop).not.toHaveBeenCalled();

    view.unmount();
    expect(mocks.stop).toHaveBeenCalledTimes(1);
  });

  it("still requires an app restart when the live listener cannot be registered", async () => {
    mocks.onOpenUrl.mockRejectedValueOnce(new Error(privateCallback));
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("failure")).toHaveTextContent("callback_listener_failed"));
    expect(screen.getByTestId("failure")).toHaveTextContent("callbackListener");
    expect(screen.getByTestId("failure")).toHaveTextContent("restartApp");
    expect(screen.getByTestId("failure")).not.toHaveTextContent("authorizationCode");

    fireEvent.click(screen.getByRole("button", { name: "sign in" }));
    fireEvent.click(screen.getByRole("button", { name: "recover" }));
    expect(mocks.invoke).not.toHaveBeenCalledWith("account_begin_sign_in");
    expect(mocks.getCurrent).not.toHaveBeenCalled();
    expect(screen.getByTestId("failure")).toHaveTextContent("restartApp");
  });
});
