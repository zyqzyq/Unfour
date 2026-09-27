import { useEffect, useRef, useState } from "react";
import type { FlowPredicate } from "@unfour/command-client";
import { Button, Select, useI18n } from "@unfour/ui";
import { isInRightOperand } from "./model";
import { ValueEditor } from "./ValueEditor";
import type { Variable } from "./variables";
import { decodePredicate, encodePredicate, type PredicateOperator } from "./predicateAdapter";

export function PredicateFields({ label, value, variables, onChange, onValidity }: { label: string; value: FlowPredicate | null; variables: Variable[]; onChange: (value: FlowPredicate | null) => void; onValidity: (field: string, valid: boolean) => void }) {
  const { t } = useI18n();
  const validity = useRef(onValidity);
  useEffect(() => { validity.current = onValidity; }, [onValidity]);
  const incomplete = Boolean(value && [value.left, value.right].some((operand) => operand && typeof operand === "object" && "$ref" in operand && !operand.$ref));
  const invalidArray = Boolean(value?.op === "in" && !isInRightOperand(value.right));
  const [selection, setSelection] = useState<{ wire: string; operator: PredicateOperator } | null>(null);
  const decoded = value ? decodePredicate(value) : null;
  const model = decoded && selection?.wire === JSON.stringify(value) ? { ...decoded, operator: selection.operator } : decoded;
  const edit = (patch: Partial<NonNullable<typeof model>>) => { if (model && value) { const next = { ...value, ...encodePredicate({ ...model, ...patch }) }; setSelection({ wire: JSON.stringify(next), operator: model.operator }); onChange(next); } };
  useEffect(() => { validity.current("configuration", !incomplete && !invalidArray); }, [incomplete, invalidArray]);
  return <fieldset className="grid gap-2"><legend>{label}</legend>{model ? <>
    <ValueEditor label={`${label} · ${t("flow.left")}`} value={model.left} variables={variables} types={model.operator === "contains" ? ["variable", "json"] : ["variable", "string", "number", "boolean", "null", "json"]} onChange={(left) => edit({ left })} onValidity={(valid) => onValidity("left", valid)} />
    <Select aria-label={`${label} · ${t("flow.operator")}`} value={model.operator} options={[
      ...["eq", "ne", "contains", "gt", "ge", "lt", "le", "true", "false"].map((op) => ({ value: op, label: t(`flow.operatorLabels.${op}`) })),
      ...["exists", "notExists"].map((op) => ({ value: op, label: t(`flow.operatorLabels.${op}`), disabled: true })),
    ]} onChange={(event) => {
      const operator = event.target.value as PredicateOperator;
      const next = { ...value, ...encodePredicate({ ...model, operator }) };
      setSelection({ wire: JSON.stringify(next), operator });
      onChange(next);
    }} />
    {model.operator !== "true" && model.operator !== "false" && <ValueEditor label={`${label} · ${t("flow.right")}`} value={model.right} variables={variables} onChange={(right) => edit({ right })} onValidity={(valid) => onValidity("right", valid)} />}
    <details className="text-xs text-[var(--u-color-text-muted)]"><summary>{t("flow.operatorHelp")}</summary><p>{t("flow.predicateCompatibility")}</p></details>
    {incomplete && <p role="alert">{t("flow.chooseVariable")}</p>}
    {invalidArray && !incomplete && <p role="alert">{t("flow.arrayRequired")}</p>}
  </> : <Button variant="secondary" onClick={() => onChange({ left: { $ref: "" }, op: "eq", right: false })}>{t("flow.enablePredicate")}</Button>}</fieldset>;
}
