#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const forbiddenFiles = [
  /(^|\/)\.env(?:\.|$)/i,
  /(^|\/)(?:updater_secret\.key|.*\.(?:pfx|p12|p8|key))$/i,
  /(^|\/)(?:service-account|client_secret|google-credentials)[^/]*\.json$/i,
];
const contentRules = [
  ["private key material", /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/],
  ["Supabase service-role JWT", /eyJ[a-zA-Z0-9_-]{20,}\.eyJ[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}/],
  ["GitHub access token", /\b(?:ghp|gho|ghu|ghs|github_pat)_[A-Za-z0-9_]{20,}\b/],
  ["Google API key", /\bAIza[0-9A-Za-z_-]{30,}\b/],
  ["Stripe/Creem-style secret key", /\b(?:sk_live|creem_(?:live|test))_[A-Za-z0-9_-]{16,}\b/i],
  ["Cloudflare API token", /\b(?:CF_API_TOKEN|CLOUDFLARE_API_TOKEN)\s*[:=]\s*["']?[A-Za-z0-9_-]{20,}/i],
  ["literal client secret", /\b(?:oauth|google|partner_center|azure|msix)?_?client_secret\s*[:=]\s*["'][^"'\s${}<]{8,}["']/i],
  ["literal signing secret", /\b(?:TAURI_SIGNING_PRIVATE_KEY|MSIX_SIGNING_CERTIFICATE_BASE64|MSIX_SIGNING_CERTIFICATE_PASSWORD|R2_SECRET_ACCESS_KEY|SUPABASE_SERVICE_ROLE_KEY|CREEM_API_KEY|CREEM_WEBHOOK_SECRET)\s*[:=]\s*["'][^"'\s${}<]{8,}["']/i],
];
const textExtensions = new Set([
  "", ".cjs", ".css", ".env", ".html", ".js", ".json", ".jsx", ".md", ".mjs",
  ".ps1", ".rs", ".sql", ".toml", ".ts", ".tsx", ".txt", ".yaml", ".yml",
]);
const redactionFixtures = new Map([
  ["packages/command-client/src/logger.test.ts", new Set(["private key material"])],
  ["crates/http-engine/src/script_runtime_tests.rs", new Set(["literal client secret"])],
]);

// Published russh 0.61.1 includes PEM format markers and public test keys.
// Only this rule and these exact upstream bytes qualify for the exception.
const russhKeyFixtures = new Map([
  ["vendor/russh/src/keys/format/mod.rs", "7ca4a796a523df1063f0ea0bac1b6d869288dd100d041a6455f8bd7d73b15678"],
  ["vendor/russh/src/keys/format/tests.rs", "47a0f71ac07b8bf2604bcdf47e18b2a29a91035432f24c41ec57b639b391e055"],
  ["vendor/russh/src/keys/mod.rs", "a68f9e90eed19a372f82245d722b46c5808a8e8a5f1fa15510e92d8f7d5e034f"],
]);

function trackedFiles() {
  return execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    { cwd: repoRoot },
  )
    .toString("utf8")
    .split("\0")
    .filter(Boolean);
}

const findings = [];
const files = trackedFiles();
for (const file of files) {
  const normalized = file.replaceAll("\\", "/");
  for (const rule of forbiddenFiles) {
    if (rule.test(normalized)) findings.push(`${normalized}: forbidden sensitive filename`);
  }
  if (
    normalized === "scripts/audit-public-secrets.mjs" ||
    !textExtensions.has(extname(file).toLowerCase())
  ) continue;
  let bytes;
  try {
    bytes = readFileSync(resolve(repoRoot, file));
  } catch {
    continue;
  }
  const content = bytes.toString("utf8");
  for (const [label, rule] of contentRules) {
    if (rule.test(content) && !redactionFixtures.get(normalized)?.has(label)) {
      const expected = label === "private key material" && russhKeyFixtures.get(normalized);
      if (expected && createHash("sha256").update(bytes).digest("hex") === expected) continue;
      findings.push(`${normalized}: ${label}`);
    }
  }
}

if (findings.length) {
  console.error("Public-repository secret audit failed (values intentionally omitted):");
  for (const finding of [...new Set(findings)].sort()) console.error(`- ${finding}`);
  process.exit(1);
}
console.log(`Public-repository secret audit passed (${files.length} publishable files scanned).`);
