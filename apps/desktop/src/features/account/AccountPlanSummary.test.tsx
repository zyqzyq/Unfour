// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { AccountProfile } from "./accountTypes";

const mocks = vi.hoisted(() => ({
  openAccountUpgrade: vi.fn(),
  openWebAccount: vi.fn(),
  refreshAccount: vi.fn(),
}));

const translations: Record<string, string> = {
  "account.plan": "Plan",
  "account.freePlan": "Unfour Free",
  "account.proPlan": "Unfour Pro",
  "account.active": "Active",
  "account.upgradeToPro": "Upgrade to Pro",
  "account.accountAndBilling": "Account & Billing",
  "account.refreshAccount": "Refresh account",
  "account.refreshing": "Refreshing…",
  "account.refreshFailed": "Account refresh failed.",
  "account.cloudSyncRequiresPro": "Cloud Sync requires Unfour Pro.",
  "account.openAccountFailed": "Open account failed",
  "account.openUpgradeFailed": "Upgrade page failed to open",
  "account.billingSignInRequired": "Sign in again before managing billing.",
  "account.billingApiUnavailable": "Billing is temporarily unavailable.",
  "account.billingInvalidResponse": "The billing service returned an invalid payment link.",
  "account.billingAlreadyActive": "This account already has an active subscription.",
  "account.billingCustomerNotFound": "No billing account is available.",
  "account.billingRequestFailed": "The billing request failed.",
};

vi.mock("@unfour/ui", () => ({
  Button: ({ children, size: _size, variant: _variant, ...props }: {
    children: ReactNode;
    size?: string;
    variant?: string;
  } & ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}>{children}</button>,
  useI18n: () => ({
    locale: "en",
    t: (key: string, values?: Record<string, string>) => {
      if (key === "account.activeUntil") return `Active until ${values?.date}`;
      return translations[key] ?? key;
    },
  }),
}));
vi.mock("./accountApi", () => ({
  getAccountCommandErrorCode: (error: unknown) => {
    if (typeof error !== "object" || error === null || !("code" in error)) return null;
    return String((error as { code: unknown }).code);
  },
  openAccountUpgrade: mocks.openAccountUpgrade,
  openWebAccount: mocks.openWebAccount,
}));
vi.mock("./useAccount", () => ({
  useAccount: () => ({
    refreshAccount: mocks.refreshAccount,
    refreshing: false,
  }),
}));

import { AccountPlanSummary } from "./AccountPlanSummary";

function profile(entitlements: AccountProfile["entitlements"]): AccountProfile {
  return {
    id: "account",
    email: "account@example.test",
    username: "account",
    displayName: "Account",
    avatarUrl: null,
    entitlements,
    devices: [],
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.openAccountUpgrade.mockResolvedValue(undefined);
  mocks.openWebAccount.mockResolvedValue(undefined);
  mocks.refreshAccount.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("AccountPlanSummary", () => {
  it("shows the Free plan and opens the checkout command without URL input", () => {
    render(<AccountPlanSummary profile={profile([])} />);
    expect(screen.getByText("Unfour Free")).toBeInTheDocument();
    expect(screen.getByText("Cloud Sync requires Unfour Pro.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));
    expect(mocks.openAccountUpgrade).toHaveBeenCalledWith();
  });

  it("shows the Pro expiry and opens the portal command without URL input", () => {
    render(<AccountPlanSummary profile={profile([{
      code: "cloud_sync",
      status: "active",
      validUntil: "2026-09-23T00:00:00.000Z",
    }])} />);
    expect(screen.getByText("Unfour Pro")).toBeInTheDocument();
    expect(screen.getByText("Active until Sep 23, 2026")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Account & Billing" }));
    expect(mocks.openWebAccount).toHaveBeenCalledWith();
  });

  it("shows a suspended entitlement as Free", () => {
    render(<AccountPlanSummary profile={profile([{
      code: "cloud_sync",
      status: "suspended",
      validUntil: null,
    }])} />);

    expect(screen.getByText("Unfour Free")).toBeInTheDocument();
    expect(screen.getByText("Cloud Sync requires Unfour Pro.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Upgrade to Pro" })).toBeInTheDocument();
    expect(screen.queryByText("Active")).not.toBeInTheDocument();
  });

  it("offers a manual account refresh without starting a new login", () => {
    render(<AccountPlanSummary profile={profile([])} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh account" }));
    expect(mocks.refreshAccount).toHaveBeenCalledTimes(1);
  });

  it("shows a clear sign-in recovery when the desktop session is missing", async () => {
    mocks.openAccountUpgrade.mockRejectedValueOnce({ code: "signed_out" });
    render(<AccountPlanSummary profile={profile([])} />);
    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Sign in again before managing billing.",
    );
  });

  it("distinguishes an invalid API payment URL from a browser open failure", async () => {
    mocks.openAccountUpgrade.mockRejectedValueOnce({ code: "invalid_billing_url" });
    const { rerender } = render(<AccountPlanSummary profile={profile([])} />);
    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The billing service returned an invalid payment link.",
    );

    mocks.openWebAccount.mockRejectedValueOnce({ code: "billing_portal_open_failed" });
    rerender(<AccountPlanSummary profile={profile([{
      code: "cloud_sync",
      status: "active",
      validUntil: null,
    }])} />);
    fireEvent.click(screen.getByRole("button", { name: "Account & Billing" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Open account failed");
  });
});
