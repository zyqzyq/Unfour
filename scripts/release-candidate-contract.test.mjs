import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const readWorkflow = (name) =>
  readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll(
    "\r\n",
    "\n",
  );

const readJob = (workflow, name) => {
  const marker = `  ${name}:\n`;
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1, `workflow must define ${name} job`);
  const bodyStart = start + marker.length;
  const nextJobOffset = workflow.slice(bodyStart).search(/\n  [A-Za-z0-9_-]+:/);
  const end = nextJobOffset === -1 ? workflow.length : bodyStart + nextJobOffset + 1;
  return workflow.slice(start, end);
};

test("Release Candidate is manual, read-only, and pins the requested ref", () => {
  const candidate = readWorkflow("release-candidate.yml");

  assert.match(candidate, /workflow_dispatch:/);
  assert.doesNotMatch(candidate, /^\s+push:/m);
  assert.match(candidate, /default: main/);
  assert.match(candidate, /permissions:\s+contents: read/);
  assert.match(candidate, /ref: \$\{\{ inputs\.ref \}\}/);
  assert.match(candidate, /commit=\$\(git rev-parse HEAD\)/);
  assert.match(candidate, /checkout_ref: \$\{\{ needs\.identity\.outputs\.commit \}\}/);
  assert.match(candidate, /artifact_prefix: release-candidate/);
});

test("Release Candidate and Release call the same signed Standard build core", () => {
  const candidate = readWorkflow("release-candidate.yml");
  const release = readWorkflow("release.yml");
  const build = readWorkflow("reusable-standard-build.yml");
  const reusableCall = /uses: \.\/\.github\/workflows\/reusable-standard-build\.yml/;

  assert.match(candidate, reusableCall);
  assert.match(release, reusableCall);
  assert.equal(build.match(/pnpm run tauri build/g)?.length, 1);
  assert.match(
    build,
    /pnpm run tauri build --config src-tauri\/tauri\.release\.conf\.json \$\{\{ matrix\.args \}\}/,
  );
  assert.match(build, /UNFOUR_RELEASE_CHANNEL: stable/);
  assert.match(build, /UNFOUR_DISTRIBUTION: standard/);
  assert.match(build, /TAURI_SIGNING_PRIVATE_KEY: \$\{\{ secrets\.TAURI_SIGNING_PRIVATE_KEY \}\}/);
  assert.match(build, /TAURI_SIGNING_PRIVATE_KEY_PASSWORD: \$\{\{ secrets\.TAURI_SIGNING_PRIVATE_KEY_PASSWORD \}\}/);
  assert.match(build, /TAURI_SIGNING_PRIVATE_KEY is required/);
  assert.match(build, /actions\/upload-artifact@v6/);

  for (const command of [
    "pnpm run lint",
    "pnpm run test",
    "pnpm run test:release-env",
    "pnpm run check",
    "pnpm run check:rust:ssh",
    "pnpm run test:rust",
    "pnpm run test:e2e",
  ]) {
    assert.ok(build.includes(command), `shared verify must run ${command}`);
  }
});

test("production Environment owns signing and publication secrets", () => {
  const candidate = readWorkflow("release-candidate.yml");
  const release = readWorkflow("release.yml");
  const build = readWorkflow("reusable-standard-build.yml");
  const buildJob = readJob(build, "build");
  const releaseBuildJob = readJob(release, "standard-build");
  const candidateBuildJob = readJob(candidate, "standard-build");
  const publishJob = readJob(release, "publish");

  assert.match(buildJob, /^    environment: production$/m);
  assert.match(publishJob, /^    environment: production$/m);
  assert.doesNotMatch(build, /^    secrets:/m);
  assert.doesNotMatch(releaseBuildJob, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.doesNotMatch(candidateBuildJob, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(releaseBuildJob, /^    secrets: inherit$/m);
  assert.match(candidateBuildJob, /^    secrets: inherit$/m);
  assert.match(buildJob, /TAURI_SIGNING_PRIVATE_KEY: \$\{\{ secrets\.TAURI_SIGNING_PRIVATE_KEY \}\}/);
  assert.match(buildJob, /TAURI_SIGNING_PRIVATE_KEY_PASSWORD: \$\{\{ secrets\.TAURI_SIGNING_PRIVATE_KEY_PASSWORD \}\}/);
  assert.match(publishJob, /AWS_ACCESS_KEY_ID: \$\{\{ secrets\.R2_ACCESS_KEY_ID \}\}/);
  assert.match(publishJob, /AWS_SECRET_ACCESS_KEY: \$\{\{ secrets\.R2_SECRET_ACCESS_KEY \}\}/);
  assert.match(publishJob, /R2_ACCOUNT_ID: \$\{\{ secrets\.R2_ACCOUNT_ID \}\}/);
  assert.match(publishJob, /R2_BUCKET: \$\{\{ secrets\.R2_BUCKET \}\}/);
});

test("reusable Standard build does not declare workflow_call secrets", () => {
  const build = readWorkflow("reusable-standard-build.yml");

  assert.match(build, /workflow_call:\n    inputs:/);
  assert.doesNotMatch(build, /workflow_call:[\s\S]*?\n    secrets:/);
});

test("Standard Linux release build baseline must remain pinned to ubuntu-22.04", () => {
  const buildJob = readJob(readWorkflow("reusable-standard-build.yml"), "build");
  // Inspect only the Linux include entry, not a snapshot of the workflow.
  const linuxEntries = buildJob.match(
    /^\s*-\s*\{[^\n]*\btarget:\s*x86_64-unknown-linux-gnu\b[^\n]*\}\s*$/gm,
  ) ?? [];
  const message = "Linux GLIBC compatibility baseline: Standard Linux release builds must remain pinned to ubuntu-22.04; changing it requires an explicit support-policy review and runtime verification.";

  assert.equal(linuxEntries.length, 1, message);
  assert.match(linuxEntries[0], /\bplatform:\s*ubuntu-22\.04\s*[,}]/, message);
  assert.match(buildJob, /^    runs-on:\s*\$\{\{\s*matrix\.platform\s*\}\}\s*$/m, message);
});

test("Linux dependency installation and AppImage staging follow the target, not the runner label", () => {
  const buildJob = readJob(readWorkflow("reusable-standard-build.yml"), "build");
  const steps = buildJob.split(/\n      - /);
  for (const name of ["Install Tauri system dependencies", "Stage canonical Linux assets"]) {
    const step = steps.find((entry) => entry.startsWith(`name: ${name}\n`));
    assert.ok(step, `Linux build must retain ${name}`);
    assert.match(
      step,
      /^        if:\s*matrix\.target == 'x86_64-unknown-linux-gnu'\s*$/m,
      `${name} must follow the Linux target so a runner change cannot skip it`,
    );
  }
});

test("Linux release native artifacts stay isolated from verify and older runner caches", () => {
  const workflow = readWorkflow("reusable-standard-build.yml");
  const verifyJob = readJob(workflow, "verify");
  const buildJob = readJob(workflow, "build");

  assert.doesNotMatch(verifyJob, /actions\/upload-artifact|pnpm run tauri build/);
  assert.doesNotMatch(buildJob, /actions\/download-artifact/);
  assert.ok(
    buildJob.includes("key: ${{ matrix.target == 'x86_64-unknown-linux-gnu' && format('{0}-{1}', matrix.target, matrix.platform) || matrix.target }}"),
    "Linux GLIBC compatibility baseline: Rust cache keys must isolate the Linux runner version from old native artifacts while retaining other platform keys",
  );
});

test("shared staging enforces the four canonical signed platform outputs", () => {
  const build = readWorkflow("reusable-standard-build.yml");

  for (const target of [
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
  ]) {
    assert.match(build, new RegExp(`target: ${target}`));
  }
  for (const name of [
    "windows_x64.exe",
    "macos_\\$\\{ASSET_ARCH\\}.dmg",
    "macos_\\$\\{ASSET_ARCH\\}.app.tar.gz",
    "macos_\\$\\{ASSET_ARCH\\}.app.tar.gz.sig",
    "linux_x64.AppImage",
    "linux_x64.AppImage.sig",
  ]) {
    assert.match(build, new RegExp(name.replaceAll(".", "\\.")));
  }
  assert.match(build, /Join-Path release-assets "\$name\.sig"/);
  assert.match(build, /target\/aarch64-apple-darwin\/release\/bundle/);
  assert.match(build, /target\/x86_64-apple-darwin\/release\/bundle/);
  assert.match(build, /find "\$BUNDLE_DIR\/appimage"/);
  assert.doesNotMatch(build, /bundle\/(?:deb|rpm)|linux_x64\.(?:deb|rpm)/);
});

test("Release Candidate contains no publication or Store side effects", () => {
  const candidate = readWorkflow("release-candidate.yml");
  const build = readWorkflow("reusable-standard-build.yml");
  const candidatePath = `${candidate}\n${build}`;

  assert.doesNotMatch(candidatePath, /softprops\/action-gh-release|gh release/i);
  assert.doesNotMatch(candidatePath, /aws s3|cloudflare|\bR2_/i);
  assert.doesNotMatch(candidatePath, /stable\/(?:latest|downloads)\.json|finalize-standard-release|check-update-order/i);
  assert.doesNotMatch(candidatePath, /\bgit\s+(?:tag|push)\b/i);
  assert.doesNotMatch(candidatePath, /build-msix|msix:build|Partner Center/i);
  assert.doesNotMatch(candidatePath, /contents: write/);
});
