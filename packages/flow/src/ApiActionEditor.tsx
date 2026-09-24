import { Button, Input, useI18n } from "@unfour/ui";
import { ActionTextField } from "./ActionTextField";
import { inheritedPairs, pairs, type Pair } from "./actionAuthoring";
import type { ActionEditorProps } from "./actionEditorTypes";
import { ApiBodyEditor } from "./ApiBodyEditor";
import { X } from "lucide-react";

export function ApiActionEditor(props: ActionEditorProps) {
  const { action, resources, variables, onChange, onValidity } = props;
  const { t } = useI18n();
  const saved = resources.api.find((r) => r.id === action.resourceId);
  const set = (key: string, value: unknown) => onChange({ ...action, arguments: { ...action.arguments, [key]: value } });
  const reset = (key: string) => { const args = { ...action.arguments }; delete args[key]; onChange({ ...action, arguments: args }); };
  return <div className="grid gap-2">
    {saved && <p className="break-all text-xs"><strong>{saved.method} · {saved.name}</strong><br />{saved.url}</p>}
    <strong className="text-xs">{t("flow.runtimeOverrides")}</strong>
    {["url", "body"].map((key) => <div key={key} className="grid gap-1">
      {key in action.arguments ? <><details><summary className="text-xs">{t("flow.inherited")}</summary><pre className="max-h-32 overflow-auto whitespace-pre-wrap text-xs">{String((key === "url" ? saved?.url : saved?.body) ?? "")}</pre></details>{key === "body" ? <ApiBodyEditor kind={saved?.bodyKind} value={action.arguments[key]} variables={variables} onChange={(v) => set(key, v)} onValidity={(valid) => onValidity(key, valid)} /> : <ActionTextField label={t(`flow.argument.${key}`)} value={action.arguments[key]} variables={variables} onChange={(v) => set(key, v)} onValidity={(valid) => onValidity(key, valid)} />}<Button size="sm" variant="ghost" onClick={() => reset(key)}>{t("flow.useInherited")}</Button></> : <>
        <div className="flex items-center justify-between gap-1"><span className="text-xs">{t(`flow.argument.${key}`)} · {t("flow.inherited")}{key === "body" ? ` · ${saved?.bodyKind ?? "none"}` : ""}</span><Button size="sm" variant="ghost" disabled={key === "body" && saved?.bodyKind === "multipart-form-data"} onClick={() => set(key, (key === "url" ? saved?.url : saved?.body) ?? "")}>{t("flow.override")}</Button></div>
        <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all text-xs">{String((key === "url" ? saved?.url : saved?.body) ?? "")}</pre>
      </>}
    </div>)}
    {saved?.bodyKind === "multipart-form-data" && <p role="alert">{t("flow.apiMultipartUnsupported")}</p>}
    {(["headers", "query"] as const).map((key) => {
      const legacy = key in action.arguments;
      const argument = legacy ? key : `${key}Patch`;
      const rows = pairs(action.arguments[argument] ?? []);
      const inherited = inheritedPairs(key === "headers" ? saved?.headersJson : saved?.queryJson);
      return <fieldset key={key} className="grid gap-1"><legend>{t(`flow.argument.${key}`)}</legend>
        {legacy && <p className="text-xs">{t("flow.legacyReplace")}</p>}
        <div className="text-xs text-[var(--u-color-text-muted)]">{inherited.map((pair, i) => <div key={i} className="flex items-center justify-between gap-1"><span className="min-w-0 break-all">{pair.key}: {String(pair.value)} · {t("flow.inherited")}{pair.enabled === false ? ` · ${t("flow.disabled")}` : ""}</span>{!legacy && rows && !rows.some((row) => key === "headers" ? row.key.toLowerCase() === pair.key.toLowerCase() : row.key === pair.key) && <Button size="sm" variant="ghost" onClick={() => set(argument, [...rows, { ...pair }])}>{t("flow.override")}</Button>}</div>)}</div>
        {rows ? <>{rows.map((pair, index) => {
          const update = (patch: Partial<Pair>) => set(argument, rows.map((row, i) => i === index ? { ...row, ...patch } : row));
          return <div key={index} className="grid grid-cols-[1fr_2fr_auto] items-start gap-1">
            <Input aria-label={`${t(`flow.argument.${key}`)} ${index + 1} · ${t("flow.fieldName")}`} value={pair.key} onChange={(e) => update({ key: e.target.value })} />
            <ActionTextField compact label={`${t(`flow.argument.${key}`)} ${index + 1}`} value={pair.value} variables={variables} onChange={(value) => update({ value })} onValidity={(valid) => onValidity(`${argument}:${index}`, valid)} />
            <div className="flex h-[var(--u-size-input)] items-center"><input aria-label={`${t(`flow.argument.${key}`)} ${index + 1} · ${t("flow.enabled")}`} type="checkbox" checked={pair.enabled !== false} onChange={(e) => update({ enabled: e.target.checked })} /><Button aria-label={`${t(`flow.argument.${key}`)} ${index + 1} · ${t("flow.remove")}`} size="sm" variant="ghost" onClick={() => set(argument, rows.filter((_, i) => i !== index))}><X size={14} /></Button></div>
          </div>;
        })}<Button size="sm" variant="secondary" onClick={() => set(argument, [...rows, { key: "", value: "", enabled: true }])}>{t("flow.addOverride")}</Button></> : <p>{t("flow.editAdvanced")}</p>}
        {argument in action.arguments && <Button size="sm" variant="ghost" onClick={() => reset(argument)}>{t("flow.useInherited")}</Button>}
      </fieldset>;
    })}
  </div>;
}
