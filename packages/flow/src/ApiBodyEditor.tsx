import { Button, useI18n } from "@unfour/ui";
import { ActionTextField } from "./ActionTextField";
import type { Variable } from "./variables";

// Body stays a string in FlowAction. These are text editors, not a second
// request builder: formatting is explicit and never rewrites a loaded Flow.
export function ApiBodyEditor({ kind, value, variables, onChange, onValidity }: {
  kind?: string; value: unknown; variables: Variable[]; onChange: (value: unknown) => void; onValidity: (valid: boolean) => void;
}) {
  const { t } = useI18n();
  let formatted: string | null = null;
  if (kind === "json" && typeof value === "string") {
    try { formatted = JSON.stringify(JSON.parse(value), null, 2); } catch { /* Templates need not be valid JSON before interpolation. */ }
  }
  return <div className="grid gap-1">
    <span className="text-xs text-[var(--u-color-text-muted)]">{kind ?? "none"}</span>
    <ActionTextField label={t("flow.argument.body")} value={value} variables={variables} onChange={onChange} onValidity={onValidity} multiline placeholder={kind === "json" ? '{\n  "value": "${/inputs/value}"\n}' : kind === "form-urlencoded" ? "key=value&other=value" : undefined} />
    {kind === "json" && <Button size="sm" variant="ghost" disabled={formatted === null} onClick={() => { if (formatted !== null) onChange(formatted); }}>{t("flow.formatJson")}</Button>}
    {kind === "form-urlencoded" && <p className="text-xs">{t("flow.encodedBodyHelp")}</p>}
  </div>;
}
