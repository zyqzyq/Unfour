import { useState } from "react";
import type { RedisKeyInfo } from "@unfour/command-client";
import { Select, useI18n } from "@unfour/ui";
import { hexBytes, readableText } from "./redis-bytes";

export function RedisKeyDetails({ result, keyLabel, readAt }: { result: RedisKeyInfo; keyLabel: string; readAt: Date }) {
  const { t, locale } = useI18n();
  const [format, setFormat] = useState("auto");
  if (!result.exists) return <p role="status" className="p-3 text-xs text-[var(--u-color-text-muted)]">{t("redis.missing", { key: keyLabel })}</p>;
  const { preview, keyType, length, pttlMs } = result;
  const value = preview && readableText(preview.base64);
  const json = value && !preview?.truncated ? prettyJson(value) : null;
  const selectedFormat = value === null || format === "hex" ? "hex" : format === "json" && json ? "json" : "auto";
  const text = preview ? (selectedFormat === "hex" ? hexBytes(preview.base64) : selectedFormat === "json" ? json : value) : null;
  return <section className="min-w-0 p-3 text-xs">
    <h3 className="select-text break-all font-mono font-semibold">{keyLabel}</h3>
    <dl className="my-3 flex flex-wrap gap-x-5 gap-y-1 text-[var(--u-color-text-muted)]">
      <div><dt className="inline">{t("redis.type")} </dt><dd className="inline select-text text-[var(--u-color-text)]">{keyType}</dd></div>
      <div><dt className="inline">TTL </dt><dd className="inline select-text text-[var(--u-color-text)]">{pttlMs === -1 ? t("redis.persistent") : t("redis.milliseconds", { count: pttlMs })}</dd></div>
      {length !== null && <div><dt className="inline">{t(keyType === "string" ? "redis.bytes" : "redis.elements")} </dt><dd className="inline select-text text-[var(--u-color-text)]">{length}</dd></div>}
      <div><dt className="inline">{t("redis.readAt")} </dt><dd className="inline select-text text-[var(--u-color-text)]"><time dateTime={readAt.toISOString()}>{readAt.toLocaleTimeString(locale)}</time></dd></div>
    </dl>
    {result.fieldFound === false && <p role="status">{t("redis.missingField")}</p>}
    {preview && <>
      <div className="mb-2 flex items-center gap-2">
        <Select aria-label={t("redis.format")} className="w-32" value={selectedFormat} onChange={(event) => setFormat(event.target.value)}
          options={[...(value === null ? [] : [{ value: "auto", label: t("redis.text") }]), ...(json ? [{ value: "json", label: "JSON" }] : []), { value: "hex", label: "HEX" }]} />
        {preview.truncated && <span className="text-[var(--u-color-warning)]">{t("redis.truncated")}</span>}
      </div>
      {text === "" ? <p className="text-[var(--u-color-text-muted)]">{t("redis.emptyValue")}</p> :
        <pre className="select-text whitespace-pre-wrap break-all rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 font-mono">{text}</pre>}
    </>}
  </section>;
}

function prettyJson(text: string) {
  try {
    const parsed: unknown = JSON.parse(text);
    const compact = JSON.stringify(parsed);
    const sourceTokens = text.match(/"(?:\\[\s\S]|[^"\\])*"|[^\s]/g) ?? [];
    // Keep numeric precision, duplicate fields and ordering intact. Fall back
    // to exact text when the JavaScript value would silently change the data.
    if (sourceTokens.join("") !== compact) return null;
    let depth = 0;
    for (const token of sourceTokens) {
      if (token === "[" || token === "{") depth++;
      if (token === "]" || token === "}") depth--;
      if (depth > 32) return null;
    }
    const pretty = JSON.stringify(parsed, null, 2);
    return pretty.length <= 128_000 ? pretty : null;
  } catch { return null; }
}
