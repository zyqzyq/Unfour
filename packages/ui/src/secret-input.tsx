import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState, type ComponentProps } from "react";
import { Button } from "./button";
import { Input } from "./input";
import { useI18n } from "./i18n";
import { cn } from "./utils";

/** Visibility is presentation state and never invokes onChange. */
export function SecretInput({ className, ...props }: Omit<ComponentProps<typeof Input>, "type">) {
  const { t } = useI18n();
  const [revealed, setRevealed] = useState(false);
  return <div className="flex min-w-0 items-center">
    <Input {...props} autoComplete="off" className={cn("min-w-0 flex-1 pr-8", className)} type={revealed ? "text" : "password"} />
    <Button aria-label={t(revealed ? "variables.hideSecret" : "variables.showSecret")} title={t(revealed ? "variables.hideSecret" : "variables.showSecret")} className="-ml-8 h-7 w-7" disabled={props.disabled} onClick={() => setRevealed(!revealed)} size="icon" type="button" variant="ghost">
      {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
    </Button>
  </div>;
}

/** Generic saved-value editor. Loading a view never updates the edit value. */
export function SavedSecretInput({ value, saved, cleared, disabled, onChange, onKeep, onClear, onReveal }: {
  value: string;
  saved: boolean;
  cleared: boolean;
  disabled?: boolean;
  onChange: (value: string) => void;
  onKeep: () => void;
  onClear: () => void;
  onReveal: () => Promise<string>;
}) {
  const { t } = useI18n();
  const [view, setView] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const revision = useRef(0);
  useEffect(() => () => { revision.current++; }, []);
  function reset() {
    revision.current++;
    setView(null);
    setPending(false);
    setFailed(false);
  }
  async function reveal() {
    if (view !== null || pending) { reset(); return; }
    const current = ++revision.current;
    setPending(true);
    setFailed(false);
    try {
      const secret = await onReveal();
      if (revision.current === current) setView(secret);
    } catch {
      if (revision.current === current) setFailed(true);
    } finally {
      if (revision.current === current) setPending(false);
    }
  }
  const retaining = saved && !cleared && !value;
  return <div className="space-y-1">
    {retaining ? <div className="flex min-w-0 items-center">
      <Input autoComplete="off" className="min-w-0 flex-1 pr-8" disabled={disabled} onChange={(event) => { reset(); onChange(event.target.value); }} placeholder={t("common.secret.saved")} type={view === null ? "password" : "text"} value={view ?? ""} />
      <Button aria-label={t(view !== null || pending ? "variables.hideSecret" : "variables.showSecret")} title={t(view !== null || pending ? "variables.hideSecret" : "variables.showSecret")} className="-ml-8 h-7 w-7" disabled={disabled} onClick={() => void reveal()} size="icon" type="button" variant="ghost">
        {view !== null || pending ? <EyeOff size={13} /> : <Eye size={13} />}
      </Button>
    </div> : <SecretInput disabled={disabled} onChange={(event) => { reset(); onChange(event.target.value); }} value={value} />}
    <div className="flex items-center gap-2 text-[11px] text-[var(--u-color-text-muted)]">
      <span>{t(cleared ? "common.secret.cleared" : value ? "common.secret.replacing" : saved ? "common.secret.retaining" : "common.secret.empty")}</span>
      {saved && <Button disabled={disabled || retaining} onClick={() => { reset(); onKeep(); }} size="sm" type="button" variant="ghost">{t("common.secret.keep")}</Button>}
      <Button disabled={disabled || cleared} onClick={() => { reset(); onClear(); }} size="sm" type="button" variant="ghost">{t("common.secret.clear")}</Button>
    </div>
    {failed && <span role="alert" className="text-[11px] text-[var(--u-color-danger)]">{t("common.secret.revealFailed")}</span>}
  </div>;
}
