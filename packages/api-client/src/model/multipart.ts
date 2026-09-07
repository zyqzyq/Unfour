import type { MultipartPart, RequestDraft } from "./types";

export function multipartDefinition(parts: MultipartPart[]) {
  return parts.map((part) => part.type === "text"
    ? { id: part.id, enabled: part.enabled, key: part.key, type: part.type, value: part.value }
    : { id: part.id, enabled: part.enabled, key: part.key, type: part.type, fileName: part.fileName });
}

export function parseMultipartDefinition(body: string): MultipartPart[] {
  const data: unknown = JSON.parse(body || "[]");
  if (!Array.isArray(data)) throw new Error("Invalid multipart definition");
  const ids = new Set<string>();
  return data.map((item: unknown) => {
    if (!item || typeof item !== "object") throw new Error("Invalid multipart definition");
    const row = item as Record<string, unknown>;
    if (typeof row.id !== "string" || !row.id.trim() || ids.has(row.id) ||
        typeof row.enabled !== "boolean" || typeof row.key !== "string") throw new Error("Invalid multipart definition");
    ids.add(row.id);
    const common = { id: row.id, enabled: row.enabled, key: row.key };
    if (row.type === "text" && typeof row.value === "string") return { ...common, type: "text", value: row.value };
    if (row.type === "file" && (row.fileName == null || typeof row.fileName === "string")) {
      const fileName = typeof row.fileName === "string" ? row.fileName.split(/[\\/]/).pop() ?? null : null;
      return { ...common, type: "file", fileName, filePath: null };
    }
    throw new Error("Invalid multipart definition");
  });
}

export function attachMultipartBindings(saved: MultipartPart[], current: MultipartPart[]) {
  return saved.map((part) => {
    if (part.type !== "file") return part;
    const previous = current.find((item) => item.id === part.id);
    return previous?.type === "file" && previous.fileName === part.fileName
      ? { ...part, filePath: previous.filePath } : part;
  });
}

export function multipartValidation(parts: MultipartPart[]): "key" | "file" | null {
  for (const part of parts) {
    if (!part.enabled) continue;
    if (!part.key.trim()) return "key";
    if (part.type === "file" && !part.filePath) return "file";
  }
  return null;
}

/** Only Send carries bindings; neither save nor GET/HEAD serializes paths. */
export function multipartRuntimeInput(draft: RequestDraft, purpose: "save" | "send") {
  if (purpose !== "send" || draft.bodyMode !== "multipart" || ["GET", "HEAD"].includes(draft.method)) {
    return {};
  }
  return {
    multipartParts: draft.multipartParts.flatMap((part) =>
      part.type === "file" && part.enabled && part.filePath
        ? [{ id: part.id, filePath: part.filePath }]
        : [],
    ),
  };
}
