import assert from "node:assert/strict";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  classifyLineCount,
  containsTestCode,
  countLines,
  formatIssue,
  looksLikeGeneratedOrBuildArtifact,
  scanLargeFiles,
  shouldSkipPath,
} from "./check-large-files.mjs";

function lines(count) {
  return Array.from({ length: count }, (_, index) => `line ${index + 1}`).join("\n");
}

test("classifyLineCount marks P1, P0, and Critical thresholds", () => {
  assert.equal(classifyLineCount(500), null);
  assert.deepEqual(classifyLineCount(501), {
    category: "P1",
    severity: "p1",
    threshold: 500,
  });
  assert.deepEqual(classifyLineCount(801), {
    category: "P0",
    severity: "p0",
    threshold: 800,
  });
  assert.deepEqual(classifyLineCount(1201), {
    category: "Critical",
    severity: "critical",
    threshold: 1200,
  });
});

test("countLines handles empty and newline-terminated files", () => {
  assert.equal(countLines(""), 0);
  assert.equal(countLines("one"), 1);
  assert.equal(countLines("one\n"), 1);
  assert.equal(countLines("one\ntwo"), 2);
});

test("shouldSkipPath excludes generated, vendored, lock, and build output paths", () => {
  const root = path.join(tmpdir(), "unfour-large-file-test");

  assert.equal(shouldSkipPath(path.join(root, "node_modules/pkg/index.ts"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "dist/index.js"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "target/debug/lib.rs"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "src/generated/types.ts"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "src/client.generated.ts"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "src/app.min.js"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "src/app.bundle.css"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "pnpm-lock.yaml"), root), true);
  assert.equal(shouldSkipPath(path.join(root, "src/app.ts"), root), false);
});

test("scanLargeFiles reports only supported files in audit roots over thresholds", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "unfour-large-files-"));

  try {
    await mkdir(path.join(root, "apps/desktop/src"), { recursive: true });
    await mkdir(path.join(root, "crates/core/src"), { recursive: true });
    await mkdir(path.join(root, "docs"), { recursive: true });
    await mkdir(path.join(root, "src"), { recursive: true });
    await mkdir(path.join(root, "apps/node_modules/pkg"), { recursive: true });
    await mkdir(path.join(root, "apps/dist"), { recursive: true });
    await mkdir(path.join(root, "apps/generated"), { recursive: true });

    await writeFile(path.join(root, "apps/desktop/src", "p1.ts"), lines(501));
    await writeFile(path.join(root, "apps/desktop/src", "p0.tsx"), lines(801));
    await writeFile(path.join(root, "crates/core/src", "critical.rs"), lines(1201));
    await writeFile(path.join(root, "docs", "audit.md"), lines(550));
    await writeFile(path.join(root, "apps/desktop/src", "ok.rs"), lines(500));
    await writeFile(path.join(root, "apps/desktop/src", "ignored.txt"), lines(2000));
    await writeFile(path.join(root, "src", "outside-root.ts"), lines(2000));
    await writeFile(path.join(root, "apps/node_modules/pkg/index.ts"), lines(2000));
    await writeFile(path.join(root, "apps/dist/index.js"), lines(2000));
    await writeFile(path.join(root, "apps/generated/types.ts"), lines(2000));

    const issues = await scanLargeFiles(root);

    assert.deepEqual(
      issues.map((issue) => ({
        path: issue.path,
        lineCount: issue.lineCount,
        category: issue.category,
        threshold: issue.threshold,
        containsTestCode: issue.containsTestCode,
        possibleGeneratedArtifact: issue.possibleGeneratedArtifact,
      })),
      [
        {
          path: "crates/core/src/critical.rs",
          lineCount: 1201,
          category: "Critical",
          threshold: 1200,
          containsTestCode: false,
          possibleGeneratedArtifact: false,
        },
        {
          path: "apps/desktop/src/p0.tsx",
          lineCount: 801,
          category: "P0",
          threshold: 800,
          containsTestCode: false,
          possibleGeneratedArtifact: false,
        },
        {
          path: "docs/audit.md",
          lineCount: 550,
          category: "P1",
          threshold: 500,
          containsTestCode: false,
          possibleGeneratedArtifact: false,
        },
        {
          path: "apps/desktop/src/p1.ts",
          lineCount: 501,
          category: "P1",
          threshold: 500,
          containsTestCode: false,
          possibleGeneratedArtifact: false,
        },
      ],
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("test-code and generated-artifact detectors avoid external test-module false positives", () => {
  assert.equal(containsTestCode("crates/app/src/service.rs", "#[cfg(test)]\nmod tests;\n"), false);
  assert.equal(containsTestCode("crates/app/src/service_tests.rs", "fn helper() {}\n"), true);
  assert.equal(containsTestCode("crates/app/src/service.rs", "#[test]\nfn it_works() {}\n"), true);
  assert.equal(containsTestCode("packages/app/src/util.ts", "/token/.test(key);\n"), false);
  assert.equal(looksLikeGeneratedOrBuildArtifact("packages/app/src/file.ts", "// @generated\n"), true);
});

test("formatIssue includes actionable guidance", () => {
  const message = formatIssue({
    path: "src/large.tsx",
    lineCount: 900,
    category: "P0",
    threshold: 800,
    containsTestCode: false,
    possibleGeneratedArtifact: false,
  });

  assert.match(message, /src\/large\.tsx/);
  assert.match(message, /900 lines/);
  assert.match(message, /threshold >800/);
  assert.match(message, /tests no/);
  assert.match(message, /generated\/build artifact no/);
  assert.match(message, /responsibility boundaries/);
});

test("formatIssue marks critical severity and grandfathered files", () => {
  const criticalMessage = formatIssue({
    path: "src/huge.rs",
    lineCount: 1800,
    category: "Critical",
    threshold: 1200,
    containsTestCode: true,
    possibleGeneratedArtifact: false,
    baselineAllowed: true,
    baselineMaxLines: 1900,
    baselineReason: "Historical module; awaiting split.",
  });

  assert.match(criticalMessage, /Critical/);
  assert.match(criticalMessage, /tests yes/);
  assert.match(criticalMessage, /grandfathered <= 1900/);
  assert.match(criticalMessage, /Historical module; awaiting split\./);
});
