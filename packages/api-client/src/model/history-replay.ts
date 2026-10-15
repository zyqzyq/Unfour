import { createTranslator, type TFunction } from "@unfour/ui";
import type { ApiRequestTab } from "./request-tabs";

export function historyReplayError(tab: ApiRequestTab, t: TFunction = createTranslator("en")): string | null {
  if (tab.source === "history" && !tab.draft.historyReviewConfirmed) return t("api.history.reviewRequired");
  // Check payload fields, including disabled entries, before saving a new
  // definition. A marker must never quietly become a reusable credential.
  const { url, headers, query, body, formBody, multipartParts, auth } = tab.draft;
  const serialized = JSON.stringify({ url, headers, query, body, formBody, multipartParts, auth }).toLowerCase().replace(/\\+/g, "\\");
  if (["<redacted>", "%3credacted%3e", "%253credacted%253e", "\\u003credacted\\u003e", "[mask ", "%5bmask"].some((marker) => serialized.includes(marker))) {
    return t("api.history.redactedBlocked");
  }
  return null;
}

export function localizeReplayError(message: string, t: TFunction): string {
  if (message.includes("API_REDACTED_REQUEST")) return t("api.history.redactedBlocked");
  if (message.includes("API_AUTH_MISSING")) return t("api.history.authMissing");
  return message;
}
