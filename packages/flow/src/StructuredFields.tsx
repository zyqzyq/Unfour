import { useEffect, useRef } from "react";
import type { FlowPredicate } from "@unfour/command-client";
import { Button, Select, useI18n } from "@unfour/ui";
import { isInRightOperand } from "./model";
import { ValueEditor } from "./ValueEditor";
import type { Variable } from "./variables";

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
