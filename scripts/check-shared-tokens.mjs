#!/usr/bin/env node
/**
 * check-shared-tokens.mjs
 *
 * Guards the single-source-of-truth rule for Unfour shared design tokens.
 *
 * @unfour/ui/styles.css (packages/ui/src/styles/*) is the ONLY default source
 * of shared design tokens (--u-*, --panel-*, --app-*, and the semantic alias
 * families --border / --text / --accent / --sidebar-* / --danger / --success /
 * --warning / --focus / --neutral / --badge-*).
 *
 * Host apps (any CSS file under apps/<edition>/src/) MAY import
 * "@unfour/ui/styles.css" and MAY override tokens for host-only needs, but they
 * MUST NOT redefine shared tokens — otherwise drift returns and the two
 * editions diverge. Pro-only variables (--pro-*) are explicitly allowed.
 *
 * Usage:
 *   node scripts/check-shared-tokens.mjs
 *
 * Exit code 1 on any violation, 0 when clean.
 */

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

const uiStylesDir = join(root, "packages", "ui", "src", "styles");
const appsDir = join(root, "apps");

// Prefixes that are always considered shared, regardless of whether the exact
// name was seen in @unfour/ui (covers any future --u-*/--panel-*/--app-* token).
const FORBIDDEN_PREFIXES = ["--u-", "--panel-", "--app-"];
// Pro edition is allowed to own its own --pro-* namespace.
const ALLOWED_PREFIXES = ["--pro-"];

/** Strip /* ... *\/ block comments so commented-out tokens aren't flagged. */
function stripComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Collect every `--name: value;` custom-property declaration in a CSS string. */
function collectDeclarations(src) {
  const clean = stripComments(src);
  const decls = [];
  clean.split("\n").forEach((line, idx) => {
    const m = line.match(/^\s*(--[\w-]+)\s*:/);
    if (m) decls.push({ name: m[1], line: idx + 1 });
  });
  return decls;
}

/** Recursively collect every *.css file under a directory. */
function listCssFiles(dir, acc = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      listCssFiles(join(dir, entry.name), acc);
    } else if (entry.name.endsWith(".css")) {
      acc.push(join(dir, entry.name));
    }
  }
  return acc;
}

/** Gather the canonical shared token set from @unfour/ui/src/styles. */
function loadCanonicalTokens() {
  if (!existsSync(uiStylesDir)) {
    console.error(
      `[check-shared-tokens] Expected shared tokens dir not found: ${uiStylesDir}`,
    );
    process.exit(1);
  }
  const names = new Set();
  for (const file of readdirSync(uiStylesDir)) {
    if (!file.endsWith(".css")) continue;
    const decls = collectDeclarations(
      readFileSync(join(uiStylesDir, file), "utf8"),
    );
    for (const { name } of decls) names.add(name);
  }
  if (names.size === 0) {
    console.error(
      `[check-shared-tokens] No tokens found in ${uiStylesDir} — cannot establish canonical set.`,
    );
    process.exit(1);
  }
  return names;
}

function isForbidden(name, canonical) {
  if (name.startsWith("--pro-")) return false; // explicitly allowed
  if (canonical.has(name)) return true;
  return FORBIDDEN_PREFIXES.some((p) => name.startsWith(p));
}

function main() {
  const canonical = loadCanonicalTokens();
  console.log(
    `[check-shared-tokens] Canonical shared token source: ${uiStylesDir}`,
  );
  console.log(
    `[check-shared-tokens] Loaded ${canonical.size} shared token names.`,
  );

  if (!existsSync(appsDir)) {
    console.log(`[check-shared-tokens] No apps/ directory — nothing to check.`);
    return;
  }

  const violations = [];
  for (const entry of readdirSync(appsDir)) {
    const srcDir = join(appsDir, entry, "src");
    if (!existsSync(srcDir)) continue;
    for (const hostStyles of listCssFiles(srcDir)) {
      const decls = collectDeclarations(readFileSync(hostStyles, "utf8"));
      for (const { name, line } of decls) {
        if (isForbidden(name, canonical)) {
          violations.push({ file: hostStyles, line, name });
        }
      }
    }
  }

  if (violations.length === 0) {
    console.log(
      "[check-shared-tokens] OK — no host app redefines shared design tokens.",
    );
    return;
  }

  console.error(
    "\n[check-shared-tokens] FAILED — host app(s) redefine shared design tokens.",
  );
  console.error(
    "Shared tokens must come ONLY from @unfour/ui/styles.css. Host apps may",
  );
  console.error(
    "override, but must not redefine. Pro-only --pro-* variables are allowed.\n",
  );
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}  redefines  ${v.name}`);
  }
  console.error(
    `\n${violations.length} violation(s). Remove these definitions from the host app's CSS.`,
  );
  process.exit(1);
}

main();
