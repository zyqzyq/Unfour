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

export type Pair = { key: string; value: unknown; enabled: boolean };
export function pairs(value: unknown): Pair[] | null {
  if (!Array.isArray(value)) return null;
  return value.every((item) => item && typeof item.key === "string" && "value" in item) ? value : null;
}
export function inheritedPairs(value?: string): Pair[] {
  try { return pairs(JSON.parse(value ?? "[]")) ?? []; } catch { return []; }
}
