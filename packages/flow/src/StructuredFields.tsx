import { useEffect, useRef, useState } from "react";
import type { FlowAction, FlowPredicate } from "@unfour/command-client";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { isInRightOperand } from "./model";
import { ValueEditor } from "./ValueEditor";
import type { Variable } from "./variables";

export function ArgumentFields({ action, variables, onChange, onValidity }: {
  action: FlowAction; variables: Variable[]; onChange: (action: FlowAction) => void; onValidity: (field: string, valid: boolean) => void;
}) {
  const { t } = useI18n();
  const fields = action.capability === "api" ? ["url", "body", "headers", "query"] : action.capability === "database" ? ["sql", "catalog", "schema", "limit"] : ["inputs"];
  return <div className="grid gap-3">{Object.entries(action.arguments).map(([key, value]) => <div key={key} className="grid gap-1">
    {(["headers", "query"].includes(key) && Array.isArray(value) && value.every((item) => item && typeof item === "object" && typeof item.key === "string")) ?
      <PairFields label={t(`flow.argument.${key}`)} value={value} variables={variables} onChange={(next) => onChange({ ...action, arguments: { ...action.arguments, [key]: next } })} onValidity={(field, valid) => onValidity(`${key}:${field}`, valid)} /> :
    (key === "inputs" && value && typeof value === "object" && !Array.isArray(value) && !("$ref" in value)) ?
      <ObjectFields label={t(`flow.argument.${key}`)} value={value as Record<string, unknown>} variables={variables} onChange={(next) => onChange({ ...action, arguments: { ...action.arguments, [key]: next } })} onValidity={(field, valid) => onValidity(`${key}:${field}`, valid)} /> :
      <ValueEditor label={fields.includes(key) ? t(`flow.argument.${key}`) : key} value={value} types={key === "limit" ? ["number", "variable"] : ["url", "body", "sql", "catalog", "schema"].includes(key) ? ["string", "variable"] : undefined} variables={variables} onValidity={(valid) => onValidity(key, valid)} onChange={(next) => onChange({ ...action, arguments: { ...action.arguments, [key]: next } })} />}
    <Button size="sm" variant="ghost" onClick={() => { const next = { ...action.arguments }; delete next[key]; onChange({ ...action, arguments: next }); onValidity(key, true); }}>{t("flow.remove")}</Button>
  </div>)}<Select aria-label={t("flow.addArgument")} value="" options={[{ value: "", label: t("flow.addArgument") }, ...fields.filter((key) => !(key in action.arguments)).map((key) => ({ value: key, label: t(`flow.argument.${key}`) }))]} onChange={(event) => {
    const key = event.target.value;
    if (key) onChange({ ...action, arguments: { ...action.arguments, [key]: ["headers", "query"].includes(key) ? [] : key === "inputs" ? {} : key === "limit" ? 100 : "" } });
  }} /></div>;
}

function PairFields({ label, value, variables, onChange, onValidity }: { label: string; value: Record<string, unknown>[]; variables: Variable[]; onChange: (value: Record<string, unknown>[]) => void; onValidity: (field: string, valid: boolean) => void }) {
  const { t } = useI18n();
  const [rowIds, setRowIds] = useState(() => value.map(() => crypto.randomUUID()));
  const ids = value.map((_, index) => rowIds[index] ?? `row-${index}`);
  return <fieldset className="grid gap-2"><legend>{label}</legend>{value.map((pair, index) => <div key={ids[index]} className="grid gap-1">
    <Input aria-label={`${label} ${index + 1} · ${t("flow.fieldName")}`} value={String(pair.key)} onChange={(event) => onChange(value.map((item, i) => i === index ? { ...item, key: event.target.value } : item))} />
    <ValueEditor label={`${label} ${index + 1}`} value={pair.value} types={["string", "variable"]} variables={variables} onChange={(next) => onChange(value.map((item, i) => i === index ? { ...item, value: next } : item))} onValidity={(valid) => onValidity(ids[index], valid)} />
    <label><input type="checkbox" checked={pair.enabled !== false} onChange={(event) => onChange(value.map((item, i) => i === index ? { ...item, enabled: event.target.checked } : item))} />{t("flow.enabled")}</label>
    <Button size="sm" variant="ghost" onClick={() => { setRowIds(ids.filter((_, i) => i !== index)); onChange(value.filter((_, i) => i !== index)); onValidity(ids[index], true); }}>{t("flow.remove")}</Button>
  </div>)}<Button size="sm" variant="secondary" onClick={() => { setRowIds([...ids, crypto.randomUUID()]); onChange([...value, { key: "", value: "", enabled: true }]); }}>{t("flow.addField")}</Button></fieldset>;
}

function ObjectFields({ label, value, variables, onChange, onValidity }: { label: string; value: Record<string, unknown>; variables: Variable[]; onChange: (value: Record<string, unknown>) => void; onValidity: (field: string, valid: boolean) => void }) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  return <fieldset className="grid gap-2"><legend>{label}</legend>{Object.entries(value).map(([key, child]) => <div key={key}>
    <ValueEditor label={key} value={child} variables={variables} onChange={(next) => onChange({ ...value, [key]: next })} onValidity={(valid) => onValidity(key, valid)} />
    <Button size="sm" variant="ghost" onClick={() => { const next = { ...value }; delete next[key]; onChange(next); onValidity(key, true); }}>{t("flow.remove")}</Button>
  </div>)}<div className="flex gap-1"><Input aria-label={`${label} · ${t("flow.fieldName")}`} value={name} onChange={(event) => setName(event.target.value)} /><Button size="sm" disabled={!name.trim() || Object.prototype.hasOwnProperty.call(value, name)} onClick={() => { onChange({ ...value, [name]: "" }); setName(""); }}>{t("flow.addField")}</Button></div></fieldset>;
}

export function PredicateFields({ label, value, variables, onChange, onValidity }: { label: string; value: FlowPredicate | null; variables: Variable[]; onChange: (value: FlowPredicate | null) => void; onValidity: (field: string, valid: boolean) => void }) {
  const { t } = useI18n();
  const validity = useRef(onValidity);
  useEffect(() => { validity.current = onValidity; }, [onValidity]);
  const incomplete = Boolean(value && [value.left, value.right].some((operand) => operand && typeof operand === "object" && "$ref" in operand && !operand.$ref));
  const invalidArray = Boolean(value?.op === "in" && !isInRightOperand(value.right));
  useEffect(() => { validity.current("configuration", !incomplete && !invalidArray); }, [incomplete, invalidArray]);
  return <fieldset className="grid gap-2"><legend>{label}</legend>{value ? <>
    <ValueEditor label={`${label} · ${t("flow.left")}`} value={value.left} variables={variables} onChange={(left) => onChange({ ...value, left })} onValidity={(valid) => onValidity("left", valid)} />
    <Select aria-label={`${label} · ${t("flow.operator")}`} value={value.op} options={["eq", "ne", "gt", "ge", "lt", "le", "in"].map((op) => ({ value: op, label: t(`flow.operatorLabels.${op}`) }))} onChange={(event) => onChange({ ...value, op: event.target.value as FlowPredicate["op"] })} />
    <ValueEditor label={`${label} · ${t("flow.right")}`} value={value.right} types={value.op === "in" ? ["json", "variable"] : undefined} variables={variables} onChange={(right) => onChange({ ...value, right })} onValidity={(valid) => onValidity("right", valid)} />
    {incomplete && <p role="alert">{t("flow.chooseVariable")}</p>}
    {invalidArray && !incomplete && <p role="alert">{t("flow.arrayRequired")}</p>}
  </> : <Button variant="secondary" onClick={() => onChange({ left: { $ref: "" }, op: "eq", right: false })}>{t("flow.enablePredicate")}</Button>}</fieldset>;
}
