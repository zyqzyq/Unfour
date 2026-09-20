import { useState } from "react";
import type { FlowInputDefinition } from "@unfour/command-client";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { JsonField } from "./StepEditor";
import { sensitiveKey } from "./model";

export function InputEditor({ definitions, onChange, onValidity }: {
  definitions: FlowInputDefinition[];
  onChange: (definitions: FlowInputDefinition[]) => void;
  onValidity: (key: string, valid: boolean) => void;
}) {
  const { t } = useI18n();
  const [rowIds, setRowIds] = useState(() => definitions.map(() => crypto.randomUUID()));
  function edit(index: number, value: FlowInputDefinition) {
    if (value.secret || sensitiveKey(value.name)) {
      delete value.default;
      onValidity(`default:${rowIds[index]}`, true);
    }
    onChange(definitions.map((field, i) => i === index ? value : field));
  }
  return <section className="space-y-2 border-b border-[var(--u-color-border)] pb-3">
    <strong>{t("flow.inputDefinitions")}</strong>
    {definitions.map((field, index) => <div key={rowIds[index]} className="grid gap-2 border-b border-[var(--u-color-border)] pb-2">
      <div className="grid grid-cols-2 gap-2">
        <label>{t("flow.inputName")}<Input value={field.name} onChange={(e) => edit(index, { ...field, name: e.target.value })} /></label>
        <label>{t("flow.inputType")}<Select value={field.type} options={["string", "number", "boolean", "json"].map((value) => ({ value, label: t(`flow.inputTypes.${value}`) }))} onChange={(e) => { onValidity(`default:${rowIds[index]}`, true); edit(index, { ...field, type: e.target.value as FlowInputDefinition["type"], default: undefined }); }} /></label>
        <label>{t("flow.inputRequired")}<Select value={String(field.required)} options={[{ value: "true", label: t("flow.yes") }, { value: "false", label: t("flow.no") }]} onChange={(e) => edit(index, { ...field, required: e.target.value === "true" })} /></label>
        <label>{t("flow.inputSecret")}<Select value={String(field.secret)} options={[{ value: "true", label: t("flow.yes") }, { value: "false", label: t("flow.no") }]} onChange={(e) => { onValidity(`default:${rowIds[index]}`, true); edit(index, { ...field, secret: e.target.value === "true" }); }} /></label>
      </div>
      <label>{t("flow.inputDescription")}<Input value={field.description ?? ""} onChange={(e) => edit(index, { ...field, description: e.target.value })} /></label>
      {!field.secret && !sensitiveKey(field.name) && <ValueField key={field.type} field={field} label={t("flow.inputDefault")} value={field.default} onChange={(value) => edit(index, { ...field, default: value })} onValidity={(valid) => onValidity(`default:${rowIds[index]}`, valid)} />}
      {(field.secret || sensitiveKey(field.name)) && <p className="text-xs text-[var(--u-color-text-muted)]">{t("flow.secretDefaultHelp")}</p>}
      <Button size="sm" variant="ghost" onClick={() => { onValidity(`default:${rowIds[index]}`, true); setRowIds(rowIds.filter((_, i) => i !== index)); onChange(definitions.filter((_, i) => i !== index)); }}>{t("flow.removeInput")}</Button>
    </div>)}
    <Button size="sm" variant="secondary" onClick={() => { setRowIds([...rowIds, crypto.randomUUID()]); onChange([...definitions, { name: "", type: "string", required: true, secret: false }]); }}>{t("flow.addInput")}</Button>
  </section>;
}

export function RunInputs({ definitions, values, onChange, onValidity }: {
  definitions: FlowInputDefinition[];
  values: Record<string, unknown>;
  onChange: (values: Record<string, unknown>) => void;
  onValidity: (key: string, valid: boolean) => void;
}) {
  const { t } = useI18n();
  return <section className="space-y-2">
    <strong>{t("flow.inputValues")}</strong>
    {definitions.map((field, index) => <div key={`${index}:${field.name}:${field.type}`}>
      <ValueField field={field} label={field.name} value={values[field.name]} onChange={(value) => {
        const next = { ...values };
        if (value === undefined) delete next[field.name]; else next[field.name] = value;
        onChange(next);
      }} onValidity={(valid) => onValidity(field.name, valid)} />
      {field.description && <p className="text-xs text-[var(--u-color-text-muted)]">{field.description}</p>}
      {field.required && <span className="text-xs text-[var(--u-color-text-muted)]">{t("flow.inputRequired")}</span>}
    </div>)}
  </section>;
}

function ValueField({ field, label, value, onChange, onValidity }: {
  field: FlowInputDefinition; label: string; value: unknown;
  onChange: (value: unknown) => void; onValidity: (valid: boolean) => void;
}) {
  const { t } = useI18n();
  const secret = field.secret || sensitiveKey(field.name);
  if (secret && field.type !== "string") return <JsonField label={label} value={value} optional secret onValidity={onValidity} onChange={(next) => {
    if (next !== undefined && field.type !== "json" && typeof next !== field.type) return false;
    onChange(next);
  }} />;
  if (field.type === "json" && !secret) return <JsonField label={label} value={value} optional onChange={onChange} onValidity={onValidity} />;
  if (field.type === "boolean" && !secret) return <label className="grid gap-1">{label}<Select aria-label={label} value={value === undefined ? "" : String(value)} options={[{ value: "", label: t("flow.unset") }, { value: "true", label: t("flow.true") }, { value: "false", label: t("flow.false") }]} onChange={(e) => onChange(e.target.value === "" ? undefined : e.target.value === "true")} /></label>;
  return <label className="grid gap-1">{label}<Input aria-label={label} type={secret ? "password" : field.type === "number" ? "number" : "text"} autoComplete={secret ? "new-password" : "off"} value={value === undefined ? "" : field.type === "json" ? JSON.stringify(value) : String(value)} onChange={(e) => {
    const text = e.target.value;
    if (!text) { onChange(undefined); onValidity(true); return; }
    if (field.type === "string") onChange(text);
    else if (field.type === "number") onChange(Number(text));
    else {
      try { onChange(JSON.parse(text)); onValidity(true); }
      catch { onValidity(false); }
    }
  }} /></label>;
}
