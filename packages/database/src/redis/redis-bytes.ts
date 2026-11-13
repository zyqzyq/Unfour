import type { RedisCollectionKind } from "@unfour/command-client";

export function isRedisCollectionKind(type: string): type is RedisCollectionKind {
  return type === "hash" || type === "list" || type === "set" || type === "zset";
}

function bytesFromBase64(value: string) {
  return Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
}

export function textKey(value: string) {
  const bytes = new TextEncoder().encode(value);
  return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
}

export function hexBytes(base64: string) {
  return Array.from(bytesFromBase64(base64), (byte) => byte.toString(16).padStart(2, "0")).join(" ");
}

/** A distinct caption for each byte sequence, including text that looks like a HEX label. */
export function redisName(encoded: string) {
  const text = readableText(encoded);
  if (text === null || /[\p{C}\p{Z}]/u.test(text.replace(/[ \r\n\t]/g, ""))) {
    return "HEX " + hexBytes(encoded);
  }
  if (/^[\p{L}\p{N}_.:/@+-]+$/u.test(text)) return text;
  // Quoting distinguishes literal labels. Escape space runs that HTML would collapse.
  return JSON.stringify(text).replace(/ {2,}/g, (spaces) => "\\u0020".repeat(spaces.length));
}

export function readableText(base64: string) {
  try {
    const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytesFromBase64(base64));
    for (const char of text) {
      if (char.charCodeAt(0) < 32 && !"\n\r\t".includes(char)) return null;
    }
    return text;
  } catch {
    return null;
  }
}
