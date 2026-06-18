// Compiles apps/desktop/src/styles.css → dist/styles.css using @tailwindcss/node
// Scans packages/ for Tailwind utility class candidates so the output
// includes all utilities used by @unfour/ui components.
import { compile } from '../node_modules/@tailwindcss/node/dist/index.mjs';
import { readFileSync, writeFileSync, readdirSync, statSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const srcFile = path.join(root, 'apps/desktop/src/styles.css');
const outFile = path.join(root, 'packages/ui/dist/styles.css');
const input = readFileSync(srcFile, 'utf-8');

// Scan packages/**/*.{ts,tsx,js,jsx} for Tailwind class candidates
function* scanDir(dir, exts) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) yield* scanDir(full, exts);
    else if (exts.some(e => entry.name.endsWith(e))) yield full;
  }
}

const CLASS_RE = /['"`]([^'"`\n]+)['"`]/g;
const candidates = new Set();
for (const file of scanDir(path.join(root, 'packages'), ['.ts', '.tsx', '.js', '.jsx'])) {
  const content = readFileSync(file, 'utf-8');
  for (const [, str] of content.matchAll(CLASS_RE)) {
    for (const cls of str.split(/\s+/)) {
      if (cls) candidates.add(cls);
    }
  }
}

process.stderr.write(`  build-css: scanning ${candidates.size} class candidates\n`);

const result = await compile(input, {
  base: path.dirname(srcFile),
  onDependency: () => {},
});
const css = result.build([...candidates]);
mkdirSync(path.dirname(outFile), { recursive: true });
writeFileSync(outFile, css);
process.stderr.write(`  build-css: wrote dist/styles.css (${css.length} bytes)\n`);
