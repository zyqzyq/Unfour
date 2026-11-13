import type { RedisKeyInfo, RedisMutation } from "@unfour/command-client";
import { hexBytes, readableText, textKey } from "./redis-bytes";

export type WriteKind = RedisMutation["kind"];
export type WriteTarget = {
  kind: WriteKind;
  keyBase64: string;
  info?: RedisKeyInfo;
  fieldBase64?: string;
};

export type WriteForm = { kind: WriteKind; key: string; field: string; value: string; hex: boolean; seconds: string };
export type PreparedWrite = { keyBase64: string; mutation: RedisMutation };

const keyLimit = 64 * 1024;
const valueLimit = 16 * 1024;

export function initialWriteForm(target: WriteTarget): WriteForm {
  const preview = target.info?.preview;
  const editValue = target.kind === "updateString" || target.kind === "updateHashField";
  const encoded = editValue && preview ? preview.base64 : "";
  const text = readableText(encoded);
  // Textarea line endings are normalized by browsers. Use HEX to round-trip CR bytes.
  const hex = text === null || text.includes("\r");
  return { kind: target.kind, key: readableText(target.keyBase64) ?? "", field: "",
    value: hex ? hexBytes(encoded) : text, hex, seconds: "" };
}

export function keyCaption(encoded: string, emptyLabel: string): string {
  return (readableText(encoded) ?? `HEX ${hexBytes(encoded)}`) || emptyLabel;
}

export function createKind(kind: WriteKind) {
  return kind === "createString" || kind === "createHash";
}

export function writesValue(kind: WriteKind) {
  return createKind(kind) || kind === "addHashField" || kind === "updateHashField" || kind === "updateString";
}

export function parseWriteValue(value: string, hex: boolean): string | null {
  if (!hex) {
    if (value.length > valueLimit || new TextEncoder().encode(value).length > valueLimit) return null;
    return textKey(value);
  }
  if (value.length > valueLimit * 3) return null;
  const pairs = value.replace(/\s/g, "");
  if (pairs.length > valueLimit * 2 || pairs.length % 2 || /[^\da-f]/i.test(pairs)) return null;
  return btoa((pairs.match(/../g) ?? []).map((pair) => String.fromCharCode(parseInt(pair, 16))).join(""));
}

type Preparation = { ready: PreparedWrite; issue?: undefined } | { ready?: undefined; issue: string };

function encodeName(text: string) {
  return text.length <= keyLimit && new TextEncoder().encode(text).length <= keyLimit ? textKey(text) : null;
}

function milliseconds(input: string): number | null {
  const [seconds, fraction = ""] = input.split(".");
  const ms = Number(seconds) * 1000 + Number(fraction.padEnd(3, "0"));
  return /^\d+(?:\.\d{1,3})?$/.test(input) && Number.isSafeInteger(ms) && ms > 0 ? ms : null;
}

function prepareValue(target: WriteTarget, form: WriteForm, keyBase64: string): Preparation {
  const { kind } = form;
  const valueBase64 = parseWriteValue(form.value, form.hex);
  if (valueBase64 === null) return { issue: form.hex ? "redis.write.invalidHex" : "redis.write.valueTooLong" };
  const ready = (mutation: RedisMutation): Preparation => ({ ready: { keyBase64, mutation } });
  if (kind === "createString") {
    if (!form.seconds) return ready({ kind, valueBase64 });
    const ttlMs = milliseconds(form.seconds);
    return ttlMs === null ? { issue: "redis.write.invalidTtl" } : ready({ kind, valueBase64, ttlMs });
  }
  if (kind === "createHash" || kind === "addHashField") {
    const fieldBase64 = encodeName(form.field);
    return fieldBase64 === null ? { issue: "redis.write.fieldTooLong" } : ready({ kind, fieldBase64, valueBase64 });
  }
  const preview = target.info?.preview;
  if (!preview || preview.truncated) return { issue: "redis.write.fullValueRequired" };
  const expectedBase64 = preview.base64;
  if (kind === "updateString") return ready({ kind, expectedBase64, valueBase64 });
  const fieldBase64 = target.fieldBase64;
  if (fieldBase64 === undefined) return { issue: "redis.write.fullValueRequired" };
  if (kind === "deleteHashField") return ready({ kind, expectedBase64, fieldBase64 });
  return ready({ kind: "updateHashField", expectedBase64, fieldBase64, valueBase64 });
}

export function prepareWrite(target: WriteTarget, form: WriteForm): Preparation {
  const kind = form.kind;
  const keyBase64 = createKind(kind) ? encodeName(form.key) : target.keyBase64;
  if (keyBase64 === null) return { issue: "redis.write.keyTooLong" };
  const expectedType = target.info?.keyType ?? "";
  if (kind === "deleteKey" || kind === "persist") return { ready: { keyBase64, mutation: { kind, expectedType } } };
  if (kind === "setTtl") {
    const ttlMs = milliseconds(form.seconds);
    return ttlMs === null ? { issue: "redis.write.invalidTtl" } : { ready: { keyBase64, mutation: { kind, expectedType, ttlMs } } };
  }
  return prepareValue(target, form, keyBase64);
}
