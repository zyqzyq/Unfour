import { copyFile, mkdir, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const tauriDir = join(repoRoot, "apps", "desktop", "src-tauri");
const binaryBaseName = "unfour-mcp";

function normalizeTarget(value) {
  const trimmed = value?.trim();
  return trimmed && trimmed !== "default" ? trimmed : null;
}

function inferTargetFromTauriEnv() {
  const platform = process.env.TAURI_ENV_PLATFORM?.toLowerCase();
  const arch = process.env.TAURI_ENV_ARCH?.toLowerCase();

  if (!platform || !arch) {
    return null;
  }

  if (platform === "windows" || platform === "win32") {
    if (arch === "aarch64" || arch === "arm64") return "aarch64-pc-windows-msvc";
    if (arch === "x86" || arch === "i686" || arch === "ia32") return "i686-pc-windows-msvc";
    return "x86_64-pc-windows-msvc";
  }

  if (platform === "macos" || platform === "darwin") {
    return arch === "aarch64" || arch === "arm64"
      ? "aarch64-apple-darwin"
      : "x86_64-apple-darwin";
  }

  if (platform === "linux") {
    if (arch === "aarch64" || arch === "arm64") return "aarch64-unknown-linux-gnu";
    if (arch === "arm" || arch === "armv7") return "armv7-unknown-linux-gnueabihf";
    return "x86_64-unknown-linux-gnu";
  }

  return null;
}

function inferHostTarget() {
  if (process.platform === "win32") {
    if (process.arch === "arm64") return "aarch64-pc-windows-msvc";
    if (process.arch === "ia32") return "i686-pc-windows-msvc";
    return "x86_64-pc-windows-msvc";
  }

  if (process.platform === "darwin") {
    return process.arch === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin";
  }

  if (process.platform === "linux") {
    if (process.arch === "arm64") return "aarch64-unknown-linux-gnu";
    if (process.arch === "arm") return "armv7-unknown-linux-gnueabihf";
    return "x86_64-unknown-linux-gnu";
  }

  throw new Error(`Unsupported platform for Tauri sidecar target inference: ${process.platform}`);
}

function resolveTarget() {
  return (
    normalizeTarget(process.env.UNFOUR_TAURI_TARGET) ??
    normalizeTarget(process.env.CARGO_BUILD_TARGET) ??
    normalizeTarget(process.env.TAURI_ENV_TARGET_TRIPLE) ??
    inferTargetFromTauriEnv() ??
    inferHostTarget()
  );
}

async function main() {
  const target = resolveTarget();
  const args = process.argv.slice(2);
  const isDebug = args.includes("--debug") || process.env.UNFOUR_SIDECAR_PROFILE === "debug";
  const profile = isDebug ? "debug" : "release";
  const cargoArgs = ["build", "-p", binaryBaseName, "--message-format=json-render-diagnostics"];
  if (profile === "release") {
    cargoArgs.push("--release");
  }
  cargoArgs.push("--target", target);

  console.log(`[prepare-tauri-sidecars] Building ${binaryBaseName} (${profile}) for ${target}`);
  const build = spawnSync("cargo", cargoArgs, {
    cwd: repoRoot,
    stdio: ["inherit", "pipe", "inherit"],
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });

  if (build.error) throw build.error;
  if (build.status !== 0) {
    throw new Error(`cargo ${cargoArgs.join(" ")} failed with exit code ${build.status}`);
  }

  // Cargo's artifact path honors target-dir and per-user Cargo configuration.
  const source = build.stdout.trim().split(/\r?\n/).map((line) => JSON.parse(line))
    .find((message) => message.reason === "compiler-artifact"
      && message.target.name === binaryBaseName && message.target.kind.includes("bin")
      && message.executable)?.executable;
  if (!source) throw new Error(`Cargo reported no ${binaryBaseName} executable for ${target}`);
  const destination = join(
    tauriDir,
    "binaries",
    `${binaryBaseName}-${target}${target.includes("windows") ? ".exe" : ""}`,
  );

  await stat(source);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
  console.log(`[prepare-tauri-sidecars] Copied ${source} -> ${destination}`);
}

main().catch((error) => {
  console.error(`[prepare-tauri-sidecars] ${error.message}`);
  process.exit(1);
});
