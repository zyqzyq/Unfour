import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { resolveBuildProfile } from "./release-channel.mjs";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("MSIX package binds Store identity to the compiled Store distribution", async () => {
  const [build, validate, manifest] = await Promise.all([
    read("scripts/msix/build-msix.ps1"),
    read("scripts/msix/validate-msix.ps1"),
    read("scripts/msix/AppxManifest.template.xml"),
  ]);
  assert.match(build, /--distribution microsoft-store[\s\S]*--package-channel \$PackageChannel/);
  assert.match(build, /UNFOUR_DISTRIBUTION = \$buildProfile\.distribution/);
  assert.match(build, /Copy-RequiredFile \$appExecutable \$stagedDesktop[\s\S]+Start-Process -FilePath \$stagedDesktop -ArgumentList @\("--write-build-metadata"/);
  assert.match(build, /Refusing to wrap a stale or cross-channel binary/);
  assert.match(validate, /distribution = "microsoft-store"/);
  assert.match(validate, /updaterEnabled = \$false/);
  assert.match(validate, /updaterEndpoint = \$null/);
  assert.match(validate, /Store package may contain only a plain X\.Y\.Z Stable/);
  assert.match(manifest, /Category="windows\.protocol"/);
  assert.match(manifest, /Name="unfour"[\s\S]*Parameters="&quot;%1&quot;"/);
  assert.match(manifest, /Category="windows\.appExecutionAlias"[\s\S]*Executable="unfour-mcp\.exe"/);
  assert.match(manifest, /<desktop:ExecutionAlias Alias="unfour-mcp\.exe" \/>/);
});

test("Store builds cannot register or invoke the internal updater", async () => {
  const [lib, updater, provider, dialog, workflow] = await Promise.all([
    read("apps/desktop/src-tauri/src/lib.rs"),
    read("apps/desktop/src-tauri/src/update.rs"),
    read("apps/desktop/src/features/update/UpdateProvider.tsx"),
    read("apps/desktop/src/features/update/UpdateDialog.tsx"),
    read(".github/workflows/release.yml"),
  ]);
  assert.match(lib, /if update::internal_updater_enabled\(\)[\s\S]*tauri_plugin_updater::Builder/);
  assert.match(updater, /ensure_internal_updater\(compiled_distribution\(\)\)\?;/);
  assert.match(updater, /MicrosoftStore => Err\(UpdateCommandError::managed_by_store\(\)\)/);
  assert.match(provider, /distribution !== "standard"/);
  assert.match(dialog, /distribution === "microsoft-store"[\s\S]*return null/);
  assert.doesNotMatch(workflow, /msstore|Partner Center|build-msix\.ps1/i);
});

test("MSIX staging rejects replacement of either executable after metadata export", async (t) => {
  if (process.platform !== "win32" && spawnSync("pwsh", ["--version"]).error?.code === "ENOENT") {
    t.skip("The MSIX staging validator requires PowerShell.");
    return;
  }
  const dir = await mkdtemp(join(tmpdir(), "unfour-msix-sidecar-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const profile = resolveBuildProfile("1.2.3", "test", "microsoft-store");
  const template = await read("scripts/msix/AppxManifest.template.xml");
  const manifest = template.replace(/\{\{(\w+)\}\}/g, (_, key) => ({
    IDENTITY_NAME: "Unfour.LocalTest", PUBLISHER: "CN=Test", PUBLISHER_DISPLAY_NAME: "Test",
    DISPLAY_NAME: "Unfour Test", DESCRIPTION: "Test", VERSION: "1.2.3.0", ARCHITECTURE: "x64",
  })[key]);
  const desktop = Buffer.from("expected desktop build");
  const mcp = Buffer.from("expected MCP build");
  const metadata = {
    ...profile, channel: profile.releaseChannel, profile: profile.kind, commit: "fixture",
    desktopSha256: createHash("sha256").update(desktop).digest("hex"),
    mcpSha256: createHash("sha256").update(mcp).digest("hex"),
  };
  await mkdir(join(dir, "Assets"));
  for (const name of ["StoreLogo", "Square44x44Logo", "Square71x71Logo", "Square150x150Logo", "Square310x310Logo"]) {
    await writeFile(join(dir, "Assets", `${name}.png`), "icon fixture");
  }
  await writeFile(join(dir, "AppxManifest.xml"), manifest);
  await writeFile(join(dir, "unfour.exe"), desktop);
  await writeFile(join(dir, "unfour-mcp.exe"), mcp);
  await writeFile(join(dir, "unfour-build-metadata.json"), JSON.stringify(metadata));
  const validator = fileURLToPath(new URL("./msix/validate-msix.ps1", import.meta.url));
  const validate = () => spawnSync("pwsh", ["-NoProfile", "-File", validator, "-StagingDirectory", dir], { encoding: "utf8" });
  const matched = validate();
  assert.equal(matched.status, 0, matched.error?.message ?? matched.stderr);
  await writeFile(join(dir, "unfour.exe"), "different desktop build");
  const wrongDesktop = validate();
  assert.notEqual(wrongDesktop.status, 0);
  assert.match(wrongDesktop.stderr, /Packaged desktop executable differs/);
  await writeFile(join(dir, "unfour.exe"), desktop);
  await writeFile(join(dir, "unfour-mcp.exe"), "stale MCP build");
  const mismatched = validate();
  assert.notEqual(mismatched.status, 0);
  assert.match(mismatched.stderr, /Packaged MCP sidecar differs/);
});
