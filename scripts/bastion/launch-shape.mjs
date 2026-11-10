import { pathToFileURL } from "node:url";

// Offline evidence tooling only. This is not Unfour's external-launch parser.
// Output is assembled from constants, booleans and positions: never input text.
const switches = new Map([
  ["-url", "url"], ["-newtab", "newtab"], ["-newwin", "newwin"],
  ["/sessionname", "sessionname"], ["/ini", "ini"],
  ["/hostkey", "hostkey"], ["/privatekey", "privatekey"],
  ["/command", "command"], ["/script", "script"],
]);
const schemes = new Set(["ssh", "sftp", "accessclient", "accessmac"]);
export const MAX_INPUT_BYTES = 131072;
const MAX_ARGS = 64;
const MAX_ARG_UNITS = 32768;
const failure = () => ({ schemaVersion: 1, error: "INVALID_EVIDENCE_INPUT" });

function slotShape(arg, index) {
  const quoted = arg.length >= 2 && arg.startsWith('"') && arg.endsWith('"');
  const text = quoted ? arg.slice(1, -1) : arg;
  const equals = text.indexOf("=");
  const head = (equals < 0 ? text : text.slice(0, equals)).toLowerCase();
  const option = switches.get(head);
  const schemeMatch = /^([a-z][a-z0-9+.-]*):\/\//i.exec(text);
  const shape = {
    index,
    kind: option ? "option" : schemeMatch ? "uri-shaped" : "opaque",
    empty: arg.length === 0,
    outerDoubleQuotes: quoted,
    containsWhitespace: /\s/u.test(arg),
    containsNonAscii: /[^\x00-\x7f]/u.test(arg),
    containsControl: /[\x00-\x1f\x7f]/u.test(arg),
    percentEncoding: !arg.includes("%") ? "absent"
      : /%(?![0-9a-f]{2})/i.test(arg) ? "malformed" : "hex-pairs",
  };
  if (option) {
    shape.option = option;
    shape.separator = equals < 0 ? "none" : "equals";
    // The value, including any URI nested in it, is always omitted.
  } else if (schemeMatch) {
    const scheme = schemeMatch[1].toLowerCase();
    shape.scheme = schemes.has(scheme) ? scheme : "other";
    const rest = text.slice(schemeMatch[0].length);
    // Lexical observations only. Slashes/@ may be part of ticket usernames;
    // do not infer host, username, password, port or an RFC URL parse from them.
    shape.delimiters = {
      at: rest.includes("@"), colon: rest.includes(":"),
      slash: rest.includes("/"), query: rest.includes("?"),
      fragment: rest.includes("#"), backslash: rest.includes("\\"),
    };
  }
  return shape;
}

/** Describe argv AFTER Windows argument splitting, with executable removed.
 * Never accepts a raw command line or attempts to decode a ticket/URI payload.
 * Positional boundaries survive; all arbitrary values are intentionally lost.
 */
export function summarizeArgv(argv) {
  if (!Array.isArray(argv) || argv.length > MAX_ARGS || argv.some(
    (arg) => typeof arg !== "string" || arg.length > MAX_ARG_UNITS,
  )) return failure();
  return { schemaVersion: 1, argumentCount: argv.length, slots: argv.map(slotShape) };
}

export function summarizeJson(input) {
  try {
    if (typeof input !== "string" || Buffer.byteLength(input, "utf8") > MAX_INPUT_BYTES) {
      return failure();
    }
    return summarizeArgv(JSON.parse(input));
  } catch {
    // JSON parser exceptions can quote secret-bearing input. Never forward them.
    return failure();
  }
}

async function main() {
  // No launch arguments or filenames: avoid creating another OS command-line copy.
  let result = failure();
  if (process.argv.length === 2) {
    let input = "";
    let bytes = 0;
    try {
      process.stdin.setEncoding("utf8");
      for await (const chunk of process.stdin) {
        bytes += Buffer.byteLength(chunk, "utf8");
        if (bytes > MAX_INPUT_BYTES) {
          input = "";
          break;
        }
        input += chunk;
      }
      if (bytes <= MAX_INPUT_BYTES) result = summarizeJson(input);
    } catch { /* Emit only the fixed error below. */ }
  }
  process.stdout.write(`${JSON.stringify(result)}\n`);
  if (result.error) process.exitCode = 2;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
