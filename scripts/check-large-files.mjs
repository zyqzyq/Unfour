import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const CRITICAL_THRESHOLD = 1200;
export const P0_THRESHOLD = 800;
export const P1_THRESHOLD = 500;

// Backward-compatible aliases for callers that still think in check severities.
export const ERROR_THRESHOLD = P0_THRESHOLD;
export const WARNING_THRESHOLD = P1_THRESHOLD;

export const DEFAULT_SCAN_ROOTS = ["apps", "packages", "crates", "docs"];
export const DEFAULT_TOP_LIMIT = 50;

const SOURCE_EXTENSIONS = new Set([
  ".cjs",
  ".css",
  ".cts",
  ".js",
  ".jsx",
  ".md",
  ".mdx",
  ".mjs",
  ".mts",
  ".rs",
  ".ts",
  ".tsx",
]);
const EXCLUDED_DIRS = new Set([
  ".astro",
  ".git",
  ".next",
  "build",
  "coverage",
  "dist",
  "ds-bundle",
  "generated",
  "node_modules",
  "target",
  "vendor",
  "vendored",
]);
const LOCK_FILE_NAMES = new Set([
  "Cargo.lock",
  "npm-shrinkwrap.json",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
]);
const GENERATED_PATH_PATTERNS = [
  /\.generated\.[^/\\]+$/i,
  /\.gen\.[^/\\]+$/i,
  /(^|[/\\])generated([/\\]|$)/i,
  /(^|[/\\])third[-_]party([/\\]|$)/i,
];
const GENERATED_HEADER_PATTERNS = [
  /@generated/i,
  /auto-generated/i,
  /automatically generated/i,
  /do not edit/i,
];
const BUNDLE_FILE_PATTERNS = [
  /(^|[.-])min\.(cjs|css|js|mjs)$/i,
  /(^|[.-])bundle\.(cjs|css|js|mjs)$/i,
];
const TEST_PATH_PATTERNS = [
  /(^|[/\\])(__tests__|tests?|testdata)([/\\]|$)/i,
  /([._-](test|spec)\.[cm]?[jt]sx?$|_tests?\.rs$)/i,
];
const TEST_CONTENT_PATTERNS = [
  /#\[(tokio::)?test\]/,
  /\bmod tests\s*\{/,
  /\bdescribe\s*\(/,
  /\bit\s*\(/,
  /(?<!\.)\btest\s*\(/,
];

function normalizeRelativePath(filePath, root) {
  return path.relative(root, filePath).split(path.sep).join("/");
}

function matchesAny(patterns, value) {
  return patterns.some((pattern) => pattern.test(value));
}

export function countLines(text) {
  if (text.length === 0) {
    return 0;
  }

  const normalized = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const withoutFinalNewline = normalized.endsWith("\n")
    ? normalized.slice(0, -1)
    : normalized;

  if (withoutFinalNewline.length === 0) {
    return 0;
  }

  return withoutFinalNewline.split("\n").length;
}

export function classifyLineCount(lineCount) {
  if (lineCount > CRITICAL_THRESHOLD) {
    return {
      category: "Critical",
      severity: "critical",
      threshold: CRITICAL_THRESHOLD,
    };
  }

  if (lineCount > P0_THRESHOLD) {
    return {
      category: "P0",
      severity: "p0",
      threshold: P0_THRESHOLD,
    };
  }

  if (lineCount > P1_THRESHOLD) {
    return {
      category: "P1",
      severity: "p1",
      threshold: P1_THRESHOLD,
    };
  }

  return null;
}

export function shouldSkipPath(filePath, root = process.cwd()) {
  const relativePath = normalizeRelativePath(filePath, root);
  const parts = relativePath.split("/");
  const fileName = parts.at(-1) ?? "";

  if (parts.some((part) => EXCLUDED_DIRS.has(part))) {
    return true;
  }

  if (LOCK_FILE_NAMES.has(fileName)) {
    return true;
  }

  if (matchesAny(BUNDLE_FILE_PATTERNS, fileName)) {
    return true;
  }

  return matchesAny(GENERATED_PATH_PATTERNS, relativePath);
}

function isSupportedSourceFile(filePath) {
  return SOURCE_EXTENSIONS.has(path.extname(filePath));
}

export function containsTestCode(relativePath, text) {
  return (
    matchesAny(TEST_PATH_PATTERNS, relativePath) ||
    matchesAny(TEST_CONTENT_PATTERNS, text)
  );
}

export function looksLikeGeneratedOrBuildArtifact(relativePath, text) {
  const header = text.slice(0, 2048);
  const fileName = relativePath.split("/").at(-1) ?? "";

  return (
    matchesAny(GENERATED_HEADER_PATTERNS, header) ||
    matchesAny(GENERATED_PATH_PATTERNS, relativePath) ||
    matchesAny(BUNDLE_FILE_PATTERNS, fileName) ||
    /(^|[/\\])(build|coverage|dist|target)([/\\]|$)/i.test(relativePath)
  );
}

async function walkSourceFiles(dir, root, files) {
  let entries;

  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") {
      return;
    }

    throw error;
  }

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);

    if (shouldSkipPath(fullPath, root)) {
      continue;
    }

    if (entry.isDirectory()) {
      await walkSourceFiles(fullPath, root, files);
      continue;
    }

    if (entry.isFile() && isSupportedSourceFile(fullPath)) {
      files.push(fullPath);
    }
  }
}

export async function scanLargeFiles(root = process.cwd(), options = {}) {
  const resolvedRoot = path.resolve(root);
  const rootDirectories = options.rootDirectories ?? DEFAULT_SCAN_ROOTS;
  const files = [];

  for (const rootDirectory of rootDirectories) {
    await walkSourceFiles(path.join(resolvedRoot, rootDirectory), resolvedRoot, files);
  }

  const issues = [];

  for (const filePath of files) {
    const text = await readFile(filePath, "utf8");
    const lineCount = countLines(text);
    const classification = classifyLineCount(lineCount);

    if (classification) {
      const relativePath = normalizeRelativePath(filePath, resolvedRoot);

      issues.push({
        path: relativePath,
        lineCount,
        containsTestCode: containsTestCode(relativePath, text),
        possibleGeneratedArtifact: looksLikeGeneratedOrBuildArtifact(relativePath, text),
        ...classification,
      });
    }
  }

  issues.sort((left, right) => {
    if (right.lineCount !== left.lineCount) {
      return right.lineCount - left.lineCount;
    }

    return left.path.localeCompare(right.path);
  });

  return issues;
}

async function loadBaseline(root, baselinePath) {
  if (baselinePath === null) {
    return new Map();
  }

  const resolvedPath =
    baselinePath ?? path.join(root, "scripts", "large-files-baseline.json");

  try {
    const raw = await readFile(resolvedPath, "utf8");
    const parsed = JSON.parse(raw);
    const entries = Array.isArray(parsed.files) ? parsed.files : [];

    return new Map(
      entries.map((entry) => [
        entry.path,
        {
          maxLines: entry.maxLines,
          reason: entry.reason ?? "Known oversized file",
        },
      ]),
    );
  } catch (error) {
    if (error?.code === "ENOENT") {
      return new Map();
    }

    throw error;
  }
}

function applyBaseline(issues, baseline) {
  return issues.map((issue) => {
    const baselineEntry = baseline.get(issue.path);
    const baselineAllowed =
      Boolean(baselineEntry) && issue.lineCount <= baselineEntry.maxLines;

    return {
      ...issue,
      baselineAllowed,
      baselineMaxLines: baselineEntry?.maxLines,
      baselineReason: baselineEntry?.reason,
    };
  });
}

function isBlockingIssue(issue) {
  return issue.category === "Critical" && !issue.baselineAllowed;
}

function formatBoolean(value) {
  return value ? "yes" : "no";
}

export function formatIssue(issue) {
  const baselineText = issue.baselineAllowed
    ? ` | grandfathered <= ${issue.baselineMaxLines}: ${issue.baselineReason}`
    : "";

  return [
    `[large-files] ${issue.category}`,
    issue.path,
    `${issue.lineCount} lines`,
    `threshold >${issue.threshold}`,
    `tests ${formatBoolean(issue.containsTestCode)}`,
    `generated/build artifact ${formatBoolean(issue.possibleGeneratedArtifact)}`,
    "split only along responsibility boundaries",
  ].join(" | ") + baselineText;
}

function summarizeIssues(issues) {
  return {
    total: issues.length,
    critical: issues.filter((issue) => issue.category === "Critical").length,
    p0: issues.filter((issue) => issue.category === "P0").length,
    p1: issues.filter((issue) => issue.category === "P1").length,
    containsTestCode: issues.filter((issue) => issue.containsTestCode).length,
    possibleGeneratedArtifacts: issues.filter((issue) => issue.possibleGeneratedArtifact).length,
    grandfathered: issues.filter((issue) => issue.baselineAllowed).length,
    blocking: issues.filter(isBlockingIssue).length,
  };
}

function parsePositiveInteger(value, optionName) {
  const parsed = Number.parseInt(value, 10);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${optionName} must be a positive integer.`);
  }

  return parsed;
}

function parseArgs(argv) {
  const options = {
    baselinePath: undefined,
    json: false,
    root: process.cwd(),
    rootDirectories: DEFAULT_SCAN_ROOTS,
    topLimit: DEFAULT_TOP_LIMIT,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--json") {
      options.json = true;
      continue;
    }

    if (arg === "--no-baseline") {
      options.baselinePath = null;
      continue;
    }

    if (arg === "--baseline") {
      options.baselinePath = argv[index + 1];
      index += 1;
      continue;
    }

    if (arg === "--top") {
      options.topLimit = parsePositiveInteger(argv[index + 1], "--top");
      index += 1;
      continue;
    }

    if (arg === "--roots") {
      options.rootDirectories = argv[index + 1]
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean);
      index += 1;
      continue;
    }

    options.root = arg;
  }

  return options;
}

export async function runLargeFileCheck(argv = process.argv.slice(2)) {
  const options = parseArgs(argv);
  const root = path.resolve(options.root);
  const issues = await scanLargeFiles(root, {
    rootDirectories: options.rootDirectories,
  });
  const baseline = await loadBaseline(root, options.baselinePath);
  const annotatedIssues = applyBaseline(issues, baseline);
  const displayedIssues = annotatedIssues.slice(0, options.topLimit);
  const summary = summarizeIssues(annotatedIssues);

  if (options.json) {
    console.log(
      JSON.stringify(
        {
          roots: options.rootDirectories,
          topLimit: options.topLimit,
          summary,
          files: displayedIssues,
        },
        null,
        2,
      ),
    );
    return summary.blocking > 0 ? 1 : 0;
  }

  if (annotatedIssues.length === 0) {
    console.log(
      `[large-files] OK: no supported files exceed ${P1_THRESHOLD} lines under ${options.rootDirectories.join(", ")}.`,
    );
    return 0;
  }

  console.log(
    `[large-files] Top ${displayedIssues.length} of ${annotatedIssues.length} files over ${P1_THRESHOLD} lines under ${options.rootDirectories.join(", ")}.`,
  );

  for (const issue of displayedIssues) {
    console.log(formatIssue(issue));
  }

  console.log(
    `[large-files] Summary: ${summary.critical} Critical, ${summary.p0} P0, ${summary.p1} P1, ${summary.containsTestCode} with test code, ${summary.possibleGeneratedArtifacts} possible generated/build artifacts, ${summary.grandfathered} grandfathered, ${summary.blocking} blocking.`,
  );

  if (summary.blocking > 0) {
    console.error(
      "[large-files] Review Critical files or add a baseline exception with a non-increasing maxLines value.",
    );
    return 1;
  }

  return 0;
}

const currentFile = fileURLToPath(import.meta.url);

if (process.argv[1] && path.resolve(process.argv[1]) === currentFile) {
  const exitCode = await runLargeFileCheck();
  process.exitCode = exitCode;
}
