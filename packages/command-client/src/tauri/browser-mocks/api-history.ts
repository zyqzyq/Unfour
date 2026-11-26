import type { ApiRequestInput, KeyValue } from "../../types";
import { resolveInput, resolveTemplate } from "./helpers";

const masked = "<redacted>";

function credentialField(key: string) {
  if (/^(auth|x-auth)$/i.test(key.trim())) return true;
  return /authorization|cookie|password|passwd|token|secret|apikey|privatekey|licensekey|passphrase|credential|signature|^sig$/i
    .test(key.trim().replace(/[-_ ]/g, ""));
}

/** The browser preview has no Rust backend. Mask its history at the mock command boundary. */
export function mockHistoryRequest(
  request: ApiRequestInput,
  variables: (KeyValue & { isSecret: boolean })[],
) {
  const displayVariables = variables.map(variable =>
    variable.isSecret || credentialField(variable.key) ? { ...variable, value: masked } : variable,
  );
  const safe = resolveInput(request, displayVariables);
  const apiKey = mockApiKeySlot(request.authJson, displayVariables);
  const sensitiveRow = (row: KeyValue, location: "header" | "query") => {
    const key = location === "header" ? row.key.trim().toLowerCase() : row.key;
    return credentialField(key) || (apiKey?.location === location && apiKey.key === key);
  };
  // A masked variable in the authority or port can make a valid sent URL unparsable.
  const url = URL.canParse(safe.url) ? new URL(safe.url) : null;
  const query: KeyValue[] = [];
  if (url) {
    url.searchParams.forEach((value, key) => query.push({ key, value, enabled: true }));
    url.search = "";
    url.hash = "";
    if (url.password) url.password = masked;
  }
  query.push(...safe.query.filter(row => row.enabled && row.key));
  const headers = safe.headers.filter(row => row.enabled && row.key);
  const credentials = variables.filter(v => v.isSecret || credentialField(v.key)).map(v => v.value);
  for (const header of headers.filter(row => sensitiveRow(row, "header"))) {
    credentials.push(header.value);
    if (/^(proxy-)?authorization$/i.test(header.key.trim())) {
      credentials.push(header.value.replace(/^\S+\s+/, ""));
    }
  }
  credentials.push(...query.filter(row => sensitiveRow(row, "query")).map(row => row.value));
  const scrub = historyValueScrubber(credentials);
  const mask = (row: KeyValue, location: "header" | "query") => ({
    ...row,
    key: scrub(row.key),
    value: sensitiveRow(row, location) ? masked : scrub(row.value),
  });
  return {
    url: scrub(url?.toString() ?? masked),
    requestHeadersJson: JSON.stringify(headers.map(row => mask(row, "header"))),
    requestQueryJson: JSON.stringify(query.map(row => mask(row, "query"))),
  };
}

// One credential can also occur under a business field name or in the URL path.
function historyValueScrubber(credentials: string[]) {
  const variants = new Set<string>();
  for (const value of credentials) {
    if (!value || value === masked) continue;
    const encoded = new URLSearchParams([["v", value]]).toString().slice(2);
    variants.add(value);
    variants.add(encoded);
    variants.add(encoded.replace(/\+/g, "%20"));
  }
  return (value: string) => {
    for (const secret of variants) {
      if (value.includes(secret)) return masked;
    }
    return value;
  };
}

function mockApiKeySlot(authJson: string | undefined, variables: KeyValue[]) {
  try {
    const auth = JSON.parse(authJson ?? "null");
    if (auth?.type === "api-key" && typeof auth.key === "string") {
      const key = resolveTemplate(auth.key, variables).trim();
      return auth.addTo === "query" ? { location: "query", key } : { location: "header", key: key.toLowerCase() };
    }
  } catch {
    // The preview does not execute Auth configurations.
  }
}
