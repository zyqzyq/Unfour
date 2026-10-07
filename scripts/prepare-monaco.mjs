import { cpSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Keep Monaco's lazy AMD loader and workers local in both Vite and Tauri.
// The desktop dependency pins the editor version; no runtime CDN is needed.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(path.join(root, "apps/desktop/package.json"));
const source = path.join(path.dirname(require.resolve("monaco-editor/package.json")), "min/vs");
const destination = path.join(root, "apps/desktop/public/monaco/vs");
mkdirSync(destination, { recursive: true });
cpSync(source, destination, { recursive: true });
