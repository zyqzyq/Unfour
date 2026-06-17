import type { DatabaseQueryResult } from "@unfour/command-client";

export type DatabaseErrorCategory =
  | "confirmation"
  | "connection"
  | "database"
  | "network"
  | "permission"
  | "syntax"
  | "validation"
  | "unknown";

export type DatabaseErrorDescription = {
  category: DatabaseErrorCategory;
  code?: string;
  message: string;
  technicalDetail?: string;
  title: string;
};

export function serializeDatabaseResult(
  result: DatabaseQueryResult,
  delimiter: "," | "\t",
) {
  const header = result.columns
    .map((column) => serializeDatabaseCell(column.name, delimiter))
    .join(delimiter);
  const rows = result.rows.map((row) => serializeDatabaseRow(result, row, delimiter));
  return [header, ...rows].join("\r\n");
}

export function serializeDatabaseRow(
  result: DatabaseQueryResult,
  row: Array<string | null>,
  delimiter: "," | "\t",
) {
  return result.columns
    .map((_, index) => serializeDatabaseCell(row[index], delimiter))
    .join(delimiter);
}

export function serializeDatabaseCell(value: string | null | undefined, delimiter: "," | "\t") {
  const cell = value ?? "";
  const needsQuotes =
    cell.includes(delimiter) ||
    cell.includes("\"") ||
    cell.includes("\n") ||
    cell.includes("\r");
  if (!needsQuotes) {
    return cell;
  }
  return `"${cell.replace(/"/g, "\"\"")}"`;
}

export function isConfirmationRequired(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code: unknown }).code === "CONFIRMATION_REQUIRED"
  );
}

export function confirmationMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "details" in error) {
    const details = (error as { details?: { classification?: unknown } }).details;
    if (details?.classification) {
      return `Confirmation required for ${String(details.classification)} SQL. Review the statement, then click Confirm run.`;
    }
  }
  return "Confirmation required. Review the SQL statement, then click Confirm run.";
}

export function describeDatabaseError(error: unknown): DatabaseErrorDescription {
  const parsed = parseDatabaseError(error);
  const message = parsed.message || "Unknown database error";
  const category = categorizeDatabaseError(parsed.code, message.toLowerCase());

  return {
    category,
    code: parsed.code,
    message,
    technicalDetail: parsed.technicalDetail,
    title: databaseErrorTitle(category),
  };
}

export function formatDatabaseError(error: unknown) {
  return describeDatabaseError(error).message;
}

function parseDatabaseError(error: unknown) {
  if (error instanceof Error) {
    return {
      message: error.message,
      technicalDetail: error.stack,
    };
  }

  if (typeof error === "string") {
    return { message: error };
  }

  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    const message =
      typeof record.message === "string" && record.message.trim()
        ? record.message
        : "Unknown database error";
    const code = typeof record.code === "string" ? record.code : undefined;
    return {
      code,
      message,
      technicalDetail: JSON.stringify(record, null, 2),
    };
  }

  return { message: "Unknown database error" };
}

function categorizeDatabaseError(
  code: string | undefined,
  lowerMessage: string,
): DatabaseErrorCategory {
  if (code === "CONFIRMATION_REQUIRED") {
    return "confirmation";
  }
  if (code === "VALIDATION_ERROR") {
    return "validation";
  }
  if (
    lowerMessage.includes("permission") ||
    lowerMessage.includes("denied") ||
    lowerMessage.includes("access denied")
  ) {
    return "permission";
  }
  if (
    lowerMessage.includes("syntax") ||
    lowerMessage.includes("parse error") ||
    lowerMessage.includes("parser error")
  ) {
    return "syntax";
  }
  if (
    lowerMessage.includes("network") ||
    lowerMessage.includes("timeout") ||
    lowerMessage.includes("timed out") ||
    lowerMessage.includes("unreachable")
  ) {
    return "network";
  }
  if (
    lowerMessage.includes("connection") ||
    lowerMessage.includes("connect") ||
    lowerMessage.includes("refused") ||
    lowerMessage.includes("closed") ||
    lowerMessage.includes("details redacted")
  ) {
    return "connection";
  }
  if (code === "DATABASE_ERROR") {
    return "database";
  }
  return "unknown";
}

function databaseErrorTitle(category: DatabaseErrorCategory) {
  switch (category) {
    case "confirmation":
      return "Confirmation required";
    case "connection":
      return "Connection error";
    case "database":
      return "Database error";
    case "network":
      return "Network error";
    case "permission":
      return "Permission error";
    case "syntax":
      return "SQL syntax error";
    case "validation":
      return "Validation error";
    default:
      return "Database error";
  }
}
