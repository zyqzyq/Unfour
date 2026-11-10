import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { MAX_INPUT_BYTES, summarizeArgv, summarizeJson } from "./launch-shape.mjs";

const fixture = JSON.parse(readFileSync(new URL("./fixtures/launch-cases.json", import.meta.url), "utf8"));
const cli = fileURLToPath(new URL("./launch-shape.mjs", import.meta.url));

for (const example of fixture.cases) {
  test(`synthetic shape: ${example.id}`, () => {
    const result = summarizeArgv(example.argv);
    assert.equal(result.argumentCount, example.argv.length);
    assert.deepEqual(result.slots.map((slot) => slot.index), example.argv.map((_, i) => i));
    assert.deepEqual(result.slots.map((slot) => slot.kind), example.kinds);
    const output = JSON.stringify(result);
    for (const forbidden of ["SYNTHETIC", "example.invalid", "合成", "Synthetic Folder"]) {
      assert.equal(output.includes(forbidden), false);
    }
  });
}

test("preserves lexical distinctions without claiming a URL parse", () => {
  const result = summarizeArgv([
    '-URL', '"ssh://SYNTHETIC%40USER:SYNTHETIC@asset.example.invalid:2222"',
    "/sessionname=合成 名称", "ssh://SYNTHETIC:%GG@[broken", "ssh://SYNTHETIC\u0000",
  ]);
  assert.equal(result.slots[0].option, "url");
  assert.equal(result.slots[1].outerDoubleQuotes, true);
  assert.equal(result.slots[1].percentEncoding, "hex-pairs");
  assert.deepEqual(result.slots[1].delimiters, {
    at: true, colon: true, slash: false, query: false, fragment: false, backslash: false,
  });
  assert.equal(result.slots[2].separator, "equals");
  assert.equal(result.slots[2].containsWhitespace, true);
  assert.equal(result.slots[2].containsNonAscii, true);
  assert.equal(result.slots[3].percentEncoding, "malformed");
  assert.equal(result.slots[4].containsControl, true);
  assert.equal("username" in result.slots[1], false);
});

test("arbitrary values cannot enter output, even when URI parsing would fail", () => {
  const sentinel = "PRIVATE-SENTINEL-9ca763";
  const result = summarizeArgv([
    `ssh://${sentinel}:${sentinel}@[broken`, `ssh://${sentinel}/x@host`,
    `${sentinel}://payload`, `--${sentinel}`, `/hostkey=${sentinel}`,
    `ssh://x@${sentinel}?token=${sentinel}#${sentinel}`, sentinel,
  ]);
  assert.equal(JSON.stringify(result).includes(sentinel), false);
  assert.equal(result.slots[2].scheme, "other");
});

test("bad JSON, wrong types and bounds return one constant error", () => {
  const expected = { schemaVersion: 1, error: "INVALID_EVIDENCE_INPUT" };
  for (const value of ["{PRIVATE_SENTINEL", '"PRIVATE_SENTINEL"', '{"argv":[]}', "[null]", "[1]"]) {
    assert.deepEqual(summarizeJson(value), expected);
  }
  assert.deepEqual(summarizeArgv(Array(65).fill("x")), expected);
  assert.deepEqual(summarizeArgv(["x".repeat(32769)]), expected);
  assert.deepEqual(summarizeJson(" ".repeat(MAX_INPUT_BYTES + 1)), expected);
  assert.equal(summarizeArgv(Array(64).fill("x")).argumentCount, 64);
  assert.equal(summarizeArgv(["x".repeat(32768)]).argumentCount, 1);
});

test("stdin CLI emits safe JSON, rejects command-line input and never echoes failures", () => {
  for (const { args, input, status } of [
    { args: [], input: JSON.stringify(fixture.cases[1].argv), status: 0 },
    { args: [], input: "{PRIVATE_SENTINEL", status: 2 },
    { args: ["PRIVATE_SENTINEL"], input: "[]", status: 2 },
    { args: [], input: "x".repeat(MAX_INPUT_BYTES + 1), status: 2 },
  ]) {
    const result = spawnSync(process.execPath, [cli, ...args], { input, encoding: "utf8" });
    assert.equal(result.status, status);
    assert.equal(result.stderr, "");
    assert.equal(result.stdout.includes("PRIVATE_SENTINEL"), false);
    assert.equal(result.stdout.includes("SYNTHETIC"), false);
    assert.equal(JSON.parse(result.stdout).schemaVersion, 1);
  }
});
