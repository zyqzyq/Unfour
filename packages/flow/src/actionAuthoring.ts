// Authoring feedback only. The database engine performs dialect-aware validation
// before saving/running, and again after Flow interpolation.
export function sqlProblem(value: unknown): string | null {
  if (value && typeof value === "object") return null;
  if (typeof value !== "string" || !value.trim()) return "flow.sqlRequired";
  const tokens = value.replace(/\$\{[^}]*\}|\$([a-zA-Z_]\w*)?\$[\s\S]*?\$\1\$|'(?:''|\\.|[^'])*'|"(?:""|[^"])*"|`(?:``|[^`])*`|\[(?:\]\]|[^\]])*\]|--[^\n]*|\/\*[\s\S]*?\*\//g, (token) => token.startsWith("--") || token.startsWith("/*") ? " " : "x");
  // Compound CREATE TRIGGER bodies are dialect-specific. Let the owning Rust
  // parser validate them on Save; semicolons within a trigger are not a batch.
  if (/^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?TRIGGER\b/i.test(tokens)) return null;
  const statements = tokens.split(";").filter((part) => part.trim());
  return statements.length === 0 ? "flow.sqlRequired" : statements.length > 1 ? "flow.sqlSingleStatement" : null;
}

export type Pair = { key: string; value: unknown; enabled: boolean; occurrence?: number };
export function pairs(value: unknown): Pair[] | null {
  if (!Array.isArray(value)) return null;
  return value.every((item) => item && typeof item.key === "string" && "value" in item) ? value : null;
}
export function inheritedPairs(value?: string): Pair[] {
  try { return pairs(JSON.parse(value ?? "[]")) ?? []; } catch { return []; }
}

// Freeze legacy sequential targets before editing/removing individual overrides.
export function queryOccurrences(rows: Pair[]): Pair[] {
  const consumed = new Map<string, Set<number>>();
  return rows.map((row) => {
    const used = consumed.get(row.key) ?? new Set<number>();
    let occurrence = row.occurrence ?? 0;
    if (row.occurrence === undefined) while (used.has(occurrence)) occurrence++;
    used.add(occurrence);
    consumed.set(row.key, used);
    return { ...row, occurrence };
  });
}
