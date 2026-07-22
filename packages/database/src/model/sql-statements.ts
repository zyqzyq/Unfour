/** One SQL statement extracted from an editor buffer, with source offsets. */
export type SqlStatementRange = {
  /** Inclusive start offset in the original source. */
  start: number;
  /** Exclusive end offset in the original source (after optional trailing `;`). */
  end: number;
  /** Trimmed statement text without a trailing semicolon. */
  sql: string;
};

/**
 * Split a SQL buffer into executable statements using `;` as the delimiter.
 * Respects single/double/backtick quotes and line/block comments so that
 * semicolons inside those regions do not end a statement.
 */
export function splitSqlStatements(source: string): SqlStatementRange[] {
  const statements: SqlStatementRange[] = [];
  let start = 0;
  let i = 0;
  let inSingle = false;
  let inDouble = false;
  let inBacktick = false;
  let inLineComment = false;
  let inBlockComment = false;

  const pushStatement = (end: number) => {
    const raw = source.slice(start, end);
    const sql = trimStatement(raw);
    if (sql) {
      statements.push({ start, end, sql });
    }
  };

  while (i < source.length) {
    const ch = source[i];
    const next = source[i + 1];

    if (inLineComment) {
      if (ch === "\n") {
        inLineComment = false;
      }
      i += 1;
      continue;
    }

    if (inBlockComment) {
      if (ch === "*" && next === "/") {
        inBlockComment = false;
        i += 2;
        continue;
      }
      i += 1;
      continue;
    }

    if (inSingle) {
      if (ch === "'" && next === "'") {
        i += 2;
        continue;
      }
      if (ch === "'") {
        inSingle = false;
      }
      i += 1;
      continue;
    }

    if (inDouble) {
      if (ch === '"' && next === '"') {
        i += 2;
        continue;
      }
      if (ch === '"') {
        inDouble = false;
      }
      i += 1;
      continue;
    }

    if (inBacktick) {
      if (ch === "`") {
        inBacktick = false;
      }
      i += 1;
      continue;
    }

    if (ch === "-" && next === "-") {
      inLineComment = true;
      i += 2;
      continue;
    }

    if (ch === "/" && next === "*") {
      inBlockComment = true;
      i += 2;
      continue;
    }

    if (ch === "'") {
      inSingle = true;
      i += 1;
      continue;
    }

    if (ch === '"') {
      inDouble = true;
      i += 1;
      continue;
    }

    if (ch === "`") {
      inBacktick = true;
      i += 1;
      continue;
    }

    if (ch === ";") {
      pushStatement(i + 1);
      start = i + 1;
      i += 1;
      continue;
    }

    i += 1;
  }

  pushStatement(source.length);
  return statements;
}

/** Return the statement under `offset`, or the nearest following/previous one. */
export function statementAtOffset(source: string, offset: number): SqlStatementRange | null {
  const statements = splitSqlStatements(source);
  if (!statements.length) {
    return null;
  }

  const clamped = Math.max(0, Math.min(offset, source.length));
  for (const statement of statements) {
    if (clamped >= statement.start && clamped < statement.end) {
      return statement;
    }
  }

  for (const statement of statements) {
    if (statement.start >= clamped) {
      return statement;
    }
  }

  return statements[statements.length - 1] ?? null;
}

/** Statements to run for the current editor action. */
export function resolveExecutableStatements(
  source: string,
  options: { mode: "current" | "all"; sql?: string; cursorOffset?: number },
): string[] {
  if (options.sql !== undefined) {
    const text = options.sql;
    if (options.mode === "all") {
      return splitSqlStatements(text).map((item) => item.sql);
    }
    const parts = splitSqlStatements(text);
    if (parts.length > 1) {
      return parts.map((item) => item.sql);
    }
    const trimmed = text.trim().replace(/;+\s*$/, "").trim();
    return trimmed ? [trimmed] : [];
  }

  if (options.mode === "all") {
    return splitSqlStatements(source).map((item) => item.sql);
  }

  const atCursor = statementAtOffset(source, options.cursorOffset ?? 0);
  return atCursor ? [atCursor.sql] : [];
}

function trimStatement(raw: string): string {
  return raw.trim().replace(/;+\s*$/, "").trim();
}
