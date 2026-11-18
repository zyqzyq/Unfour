// Bind an offline test binary to the exact dirty source inputs used to build it.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import assert from "node:assert/strict";

const [output, mode = "write"] = process.argv.slice(2);
assert.ok(output && ["write", "verify"].includes(mode), "snapshot-source.mjs <json-path> [write|verify]");
const git = (args) => execFileSync("git", args, { encoding: "utf8", windowsHide: true });
const head = git(["rev-parse", "HEAD"]).trim();
const names = [...new Set([
  ...git(["diff", "--name-only", "-z", "HEAD"]).split("\0"),
  ...git(["ls-files", "--others", "--exclude-standard", "-z"]).split("\0"),
])].filter((name) => name && [".rs", ".toml", ".lock", ".json", ".ts", ".tsx", ".mjs", ".py", ".css"].includes(extname(name))).sort();
const inputs = await Promise.all(names.map(async (path) => {
  const bytes = await readFile(path).catch((error) => { if (error.code === "ENOENT") return null; throw error; });
  return { path, sha256: bytes === null ? null : createHash("sha256").update(bytes).digest("hex") };
}));
const snapshot = { head, inputs };
if (mode === "verify") {
  assert.deepEqual(JSON.parse(await readFile(output, "utf8")), snapshot, "source changed after the build snapshot");
  console.log(`PASS: ${inputs.length} source inputs still match ${head}`);
} else {
  await writeFile(resolve(output), JSON.stringify(snapshot, null, 2) + "\n");
  console.log(`Recorded ${inputs.length} source inputs at ${head}`);
}
