import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("MSIX package binds Store identity to the compiled Store distribution", async () => {
  const [build, validate, manifest] = await Promise.all([
    read("scripts/msix/build-msix.ps1"),
    read("scripts/msix/validate-msix.ps1"),
    read("scripts/msix/AppxManifest.template.xml"),
  ]);
  assert.match(build, /--distribution microsoft-store[\s\S]*--package-channel \$PackageChannel/);
  assert.match(build, /UNFOUR_DISTRIBUTION = \$buildProfile\.distribution/);
  assert.match(build, /--write-build-metadata/);
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
