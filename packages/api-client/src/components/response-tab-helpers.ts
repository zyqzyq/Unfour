import type { ApiResponse, KeyValue } from "@unfour/command-client";
import { isSensitiveKey } from "../request-utils";
import type { deriveTabResponseState } from "../model/request-tabs";

export function redactKeyValues(items: KeyValue[]): KeyValue[] {
  return items.map((item) =>
    isSensitiveKey(item.key) ? { ...item, value: "<redacted>" } : item,
  );
}

export function responseCookies(response: ApiResponse | null) {
  return (
    response?.headers
      .filter((item) => item.key.toLowerCase() === "set-cookie")
      .flatMap((item) => parseSetCookieHeader(item.value)) ?? []
  );
}

export function parseSetCookieHeader(value: string): KeyValue[] {
  const [pair] = value.split(";");
  const separator = pair.indexOf("=");
  if (separator < 0) {
    return [];
  }
  return [
    {
      enabled: true,
      key: pair.slice(0, separator).trim(),
      value: pair.slice(separator + 1).trim(),
    },
  ];
}

export function responseStateLabel(
  state: ReturnType<typeof deriveTabResponseState>,
  t: (key: string) => string,
) {
  switch (state) {
    case "sending":
      return t("api.response.status.sending");
    case "network":
      return t("api.response.status.network");
    case "timeout":
      return t("api.response.status.timeout");
    case "failed":
      return t("api.response.status.failed");
    case "pre-script-error":
      return t("api.scripts.preErrorTitle");
    case "pre-script-timeout":
      return t("api.scripts.preTimeoutTitle");
    default:
      return state;
  }
}
