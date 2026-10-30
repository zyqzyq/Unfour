import { emptyDraft } from "./request-tabs";
import { queryFromUrl, syncUrlQuery } from "../request-utils";
import type { RequestDraft } from "./types";

export type CurlErrorCode = "CURL_SYNTAX" | "CURL_OPTION" | "CURL_FILE" | "CURL_URL" | "CURL_SEMANTICS";
export class CurlImportError extends Error {
  constructor(readonly code: CurlErrorCode) { super(code); }
}
function fail(code: CurlErrorCode): never { throw new CurlImportError(code); }

// A deliberately bounded lexer, never a shell interpreter. Reject expansions even
// inside quotes: ambiguity is preferable to silently importing different bytes.
function tokenize(source: string): string[] {
  if (source.length > 262144 || source.includes(String.fromCharCode(0)) || /\$|`|![^!\r\n]+!|\{\{/.test(source)) fail("CURL_SYNTAX");
  if (/%[A-Za-z_][^%]*%/.test(source.replace(/%[0-9a-f]{2}/gi, ""))) fail("CURL_SYNTAX");
  const outsideSingleQuotes = source.replace(/'[^']*'/g, "");
  if (/(?:^|\s)\^"/.test(outsideSingleQuotes)) return tokenizeCaretCmd(source);
  const cmd = /\^\r?\n|\^\\?"/.test(outsideSingleQuotes);
  if (cmd && source.includes("%")) fail("CURL_SYNTAX");
  const input = source.trim();
  const tokens: string[] = [];
  let word = "", quote = "", started = false;
  const flush = () => { if (started) tokens.push(word); word = ""; started = false; };
  for (let i = 0; i < input.length; i++) {
    const c = input[i], next = input[i + 1];
    if ((c === "\\" && quote !== "'" || c === "^" && cmd) && (next === "\n" || next === "\r" && input[i + 2] === "\n")) {
      i += next === "\r" ? 2 : 1; continue;
    }
    if (cmd && c === "^") {
      if (next === "\\" && input[i + 2] === '"') { word += '"'; i += 2; started = true; continue; }
      if (next === '"') { word += '"'; i++; started = true; continue; }
      fail("CURL_SYNTAX");
    }
    if (c === "\\" && quote !== "'") {
      if (!next) fail("CURL_SYNTAX");
      if (!quote || /["\\]/.test(next)) { word += next; i++; started = true; continue; }
      word += c; continue;
    }
    if (quote) {
      if (c === quote) quote = ""; else word += c;
      continue;
    }
    if (c === '"' || c === "'") { quote = c; started = true; continue; }
    if (/[\r\n|&;<>()[\]{}#~^]/.test(c)) fail("CURL_SYNTAX");
    if (/\s/.test(c)) { flush(); continue; }
    word += c; started = true;
  }
  if (quote) fail("CURL_SYNTAX");
  flush();
  if (!/^(curl|curl\.exe)$/.test(tokens.shift() ?? "")) fail("CURL_SYNTAX");
  if (tokens.some(token => token.includes("{{"))) fail("CURL_SYNTAX");
  return tokens;
}

// Chrome/Edge's "Copy as cURL (cmd)" quotes every argument with ^".
// Only decode its literal quoting grammar; other cmd features are rejected.
function tokenizeCaretCmd(source: string): string[] {
  if (source.includes("%")) fail("CURL_SYNTAX");
  const tokens: string[] = [];
  let word = "", quoted = false, started = false;
  const flush = () => { if (started) tokens.push(word); word = ""; started = false; };
  const input = source.trim();
  for (let i = 0; i < input.length; i++) {
    const c = input[i];
    if (input.startsWith('^\\^"', i) && quoted) { word += '"'; i += 3; continue; }
    if (input.startsWith('^"', i)) { quoted = !quoted; started = true; i++; continue; }
    if (c === "^") {
      const next = input[++i];
      if (!quoted && next === "\n") continue;
      if (!quoted && next === "\r" && input[i + 1] === "\n") { i++; continue; }
      if (!quoted || !next || !"&|<>(){}^".includes(next)) fail("CURL_SYNTAX");
      word += next; continue;
    }
    // Backslash sequences beyond the DevTools escaped quote are ambiguous.
    if (c === "\\" || c === '"' || c === "'" || /[\r\n|&<>()]/.test(c)) fail("CURL_SYNTAX");
    if (!quoted && /[|&;<>()[\]{}#~]/.test(c)) fail("CURL_SYNTAX");
    if (!quoted && /\s/.test(c)) { flush(); continue; }
    word += c; started = true;
  }
  if (quoted) fail("CURL_SYNTAX");
  flush();
  if (!/^(curl|curl\.exe)$/.test(tokens.shift() ?? "")) fail("CURL_SYNTAX");
  if (tokens.some(token => token.includes("{{"))) fail("CURL_SYNTAX");
  return tokens;
}

export function parseCurl(source: string): RequestDraft {
  const args = tokenize(source);
  const draft = emptyDraft();
  let url: string | undefined, method: string | undefined, cookie: string | undefined, get = false;
  const data: string[] = [];
  for (let i = 0; i < args.length; i++) {
    let arg = args[i], attached: string | undefined;
    if (arg.startsWith("--") && arg.includes("=")) {
      const index = arg.indexOf("="); attached = arg.slice(index + 1); arg = arg.slice(0, index);
    } else if (/^-[XHdb].+/.test(arg)) { attached = arg.slice(2); arg = arg.slice(0, 2); }
    const value = () => {
      const result = attached ?? args[++i];
      if (result === undefined) fail("CURL_SYNTAX");
      return result;
    };
    if (arg === "-G" || arg === "--get") { if (attached !== undefined) fail("CURL_OPTION"); get = true; }
    else if (arg === "-X" || arg === "--request") {
      if (method) fail("CURL_SEMANTICS");
      method = value();
      if (!/^(GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS)$/.test(method)) fail("CURL_SEMANTICS");
    } else if (arg === "-H" || arg === "--header") {
      const header = value();
      if (header.startsWith("@")) fail("CURL_FILE");
      const colon = header.indexOf(":");
      const key = header.slice(0, colon), content = header.slice(colon + 1).trim();
      if (colon < 1 || !/^[!#$%&'*+.^_`|~\w-]+$/.test(key) || !content || /[\r\n]/.test(content)) fail("CURL_SEMANTICS");
      draft.headers.push({ key, value: content, enabled: true });
    } else if (arg === "-b" || arg === "--cookie") {
      if (cookie !== undefined) fail("CURL_SEMANTICS");
      cookie = value();
      if (!cookie.includes("=")) fail("CURL_FILE");
      if (/[\r\n]/.test(cookie)) fail("CURL_SEMANTICS");
    } else if (["-d", "--data", "--data-raw", "--data-binary"].includes(arg)) {
      const body = value();
      if (arg !== "--data-raw" && body.startsWith("@")) fail("CURL_FILE");
      data.push(body);
    } else if (arg === "--url" || !arg.startsWith("-")) {
      if (url !== undefined) fail("CURL_URL");
      url = arg === "--url" ? value() : arg;
    } else if (["-F", "--form", "--form-string", "-T", "--upload-file"].includes(arg)) fail("CURL_FILE");
    else fail("CURL_OPTION");
  }
  if (cookie !== undefined) {
    if (draft.headers.some(x => x.key.toLowerCase() === "cookie")) fail("CURL_SEMANTICS");
    draft.headers.push({ key: "Cookie", value: cookie, enabled: true });
  }
  if (!url || !/^https?:\/\//i.test(url) || /[\s{}[\]]/.test(url)) fail("CURL_URL");
  try { const parsed = new URL(url); if (parsed.username || parsed.password || parsed.hash) fail("CURL_URL"); } catch { fail("CURL_URL"); }
  draft.method = method ?? (get ? "GET" : data.length ? "POST" : "GET");
  if (data.length && !get && ["GET", "HEAD"].includes(draft.method)) fail("CURL_SEMANTICS");
  let queryUrl = url;
  if (get && data.length) {
    if (data.some(x => /[#\r\n]/.test(x))) fail("CURL_SEMANTICS");
    queryUrl += (url.includes("?") ? "&" : "?") + data.join("&");
  }
  try { decodeURIComponent(queryUrl.slice(queryUrl.indexOf("?") + 1)); } catch { fail("CURL_URL"); }
  draft.query = queryFromUrl(queryUrl);
  if (draft.query.some(x => x.key.includes("{{") || x.value.includes("{{"))) fail("CURL_SYNTAX");
  if (draft.query.some(x => !x.key.trim() || x.key !== x.key.trim())) fail("CURL_SEMANTICS");
  draft.url = syncUrlQuery(url, draft.query);
  if (data.length && !get) {
    const contentTypes = draft.headers.filter(x => x.key.toLowerCase() === "content-type");
    if (contentTypes.length > 1 || contentTypes.some(x => /multipart\//i.test(x.value))) fail("CURL_SEMANTICS");
    if (!contentTypes.length) draft.headers.push({ key: "Content-Type", value: "application/x-www-form-urlencoded", enabled: true });
    // Keep form bytes verbatim; the form editor would normalize encoding and
    // drop empty keys. Raw text + explicit Content-Type preserves these bodies.
    draft.body = data.join("&");
    draft.bodyMode = "raw";
    draft.rawBodyType = "text";
    if (contentTypes.some(x => /\bjson\b/i.test(x.value))) {
      try { JSON.parse(draft.body); draft.rawBodyType = "json"; } catch { /* preserve invalid JSON as text */ }
    }
  }
  return draft;
}
