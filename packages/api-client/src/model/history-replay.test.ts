import { describe, expect, it } from "vitest";
import { createNewRequestTab, emptyApiTabsState } from "./request-tabs";
import { historyReplayError } from "./history-replay";
import { validateBeforeSend, tabToInput } from "../hooks/useApiRequestTabs";

function historyTab() {
  const tab = createNewRequestTab(emptyApiTabsState("w"), "h").tabs[0];
  return { ...tab, source: "history" as const, draft: { ...tab.draft, url: "https://example.test/" } };
}

describe("historical snapshots are not original templates", () => {
  it("requires explicit review even for legacy history without markers or auth", () => {
    const tab = historyTab();
    expect(historyReplayError(tab)).toContain("Review Auth");
    expect(validateBeforeSend(tab)).toContain("Review Auth");
    tab.draft.historyReviewConfirmed = true;
    expect(historyReplayError(tab)).toBeNull();
    expect(tabToInput(tab, "w", { purpose: "save" })).not.toHaveProperty("historyReviewConfirmed");
  });

  it.each(["<redacted>", "%3Credacted%3E", "%253Credacted%253E", "\\u003credacted\\u003e", "[mask kind=jwt len=40]", "%5Bmask+empty%5D"])("blocks %s in query, body, headers and auth even after review", (marker) => {
    for (const patch of [
      { url: `https://example.test/?token=${marker}` },
      { body: `{"password":"${marker}"}` },
      { headers: [{ key: "Authorization", value: marker, enabled: false }] },
      { auth: { type: "bearer" as const, token: marker } },
    ]) {
      const tab = historyTab();
      Object.assign(tab.draft, patch, { historyReviewConfirmed: true });
      expect(historyReplayError(tab)).toContain("Replace or remove");
      expect(validateBeforeSend(tab)).toContain("Replace or remove");
    }
  });
});
