import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const WARNING_THRESHOLD = 600;
export const ERROR_THRESHOLD = 1000;
export const CRITICAL_THRESHOLD = 1500;

const SOURCE_EXTENSIONS = new Set([".ts", ".tsx", ".js", ".jsx", ".rs", ".css", ".mdx"]);
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

function normalizeRelativePath(filePath, root) {
  return path.relative(root, filePath).split(path.sep).join("/");
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
      severity: "critical",
      threshold: CRITICAL_THRESHOLD,
    };
  }

  if (lineCount > ERROR_THRESHOLD) {
    return {
      severity: "error",
      threshold: ERROR_THRESHOLD,
    };
  }

  if (lineCount > WARNING_THRESHOLD) {
    return {
      severity: "warning",
      threshold: WARNING_THRESHOLD,
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

  return GENERATED_PATH_PATTERNS.some((pattern) => pattern.test(relativePath));
}

function isSupportedSourceFile(filePath) {
  return SOURCE_EXTENSIONS.has(path.extname(filePath));
}

function looksGenerated(text) {
  const header = text.slice(0, 2048);
  return GENERATED_HEADER_PATTERNS.some((pattern) => pattern.test(header));
}

async function walkSourceFiles(dir, root, files) {
  const entries = await readdir(dir, { withFileTypes: true });

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

export async function scanLargeFiles(root = process.cwd()) {
  const resolvedRoot = path.resolve(root);
  const files = [];
  await walkSourceFiles(resolvedRoot, resolvedRoot, files);

  const issues = [];

  for (const filePath of files) {
    const text = await readFile(filePath, "utf8");

    if (looksGenerated(text)) {
      continue;
    }

    const lineCount = countLines(text);
    const classification = classifyLineCount(lineCount);

    if (classification) {
      issues.push({
        path: normalizeRelativePath(filePath, resolvedRoot),
        lineCount,
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
  const BLOCKING_SEVERITIES = new Set(["error", "critical"]);

  return issues.map((issue) => {
    const baselineEntry = baseline.get(issue.path);
    const baselineAllowed =
      BLOCKING_SEVERITIES.has(issue.severity) &&
      baselineEntry &&
      issue.lineCount <= baselineEntry.maxLines;

    return {
      ...issue,
      baselineAllowed: Boolean(baselineAllowed),
      baselineMaxLines: baselineEntry?.maxLines,
      baselineReason: baselineEntry?.reason,
    };
  });
}

export function formatIssue(issue) {
  const prefix =
    issue.severity === "critical"
      ? "CRITICAL"
      : issue.severity === "error"
        ? "ERROR"
        : "WARN";
  const baselineText = issue.baselineAllowed
    ? ` (grandfathered large file <= ${issue.baselineMaxLines}: ${issue.baselineReason})`
    : "";

  return [
    `[large-files] ${prefix} ${issue.path}`,
    `${issue.lineCount} lines`,
    `threshold ${issue.threshold}${baselineText}`,
    "Extract types, constants, mock data, pure utils, adapters, hooks, services, or child components before adding more logic.",
  ].join(" | ");
}

function parseArgs(argv) {
  const options = {
    baselinePath: undefined,
    root: process.cwd(),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--no-baseline") {
      options.baselinePath = null;
      continue;
    }

    if (arg === "--baseline") {
      options.baselinePath = argv[index + 1];
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
  const issues = await scanLargeFiles(root);
  const baseline = await loadBaseline(root, options.baselinePath);
  const annotatedIssues = applyBaseline(issues, baseline);
  const blockingErrors = annotatedIssues.filter(
    (issue) =>
      (issue.severity === "error" || issue.severity === "critical") &&
      !issue.baselineAllowed,
  );

  if (annotatedIssues.length === 0) {
    console.log("[large-files] OK: no supported source files exceed 600 lines.");
    return 0;
  }

  for (const issue of annotatedIssues) {
    const isBlocking =
      (issue.severity === "error" || issue.severity === "critical") &&
      !issue.baselineAllowed;
    const writer = isBlocking ? console.error : console.warn;
    writer(formatIssue(issue));
  }

  const warningCount = annotatedIssues.filter((issue) => issue.severity === "warning").length;
  const errorCount = annotatedIssues.filter((issue) => issue.severity === "error").length;
  const criticalCount = annotatedIssues.filter((issue) => issue.severity === "critical").length;
  const grandfatheredCount = annotatedIssues.filter((issue) => issue.baselineAllowed).length;

  console.log(
    `[large-files] Summary: ${warningCount} warning(s), ${errorCount} error(s), ${criticalCount} critical(s), ${grandfatheredCount} grandfathered, ${blockingErrors.length} blocking.`,
  );

  if (blockingErrors.length > 0) {
    console.error(
      "[large-files] Split blocking files or add a reviewed baseline exception with a non-increasing maxLines value.",
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
