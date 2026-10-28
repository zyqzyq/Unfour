import { useState } from "react";
import Editor from "@monaco-editor/react";
import { configureEditorThemes, cn, useI18n, useTheme } from "@unfour/ui";

type ScriptKind = "pre" | "post";

export function RequestScriptEditors({
  onPostResponseChange,
  onPreRequestChange,
  postResponseScript,
  preRequestScript,
}: {
  onPostResponseChange: (value: string) => void;
  onPreRequestChange: (value: string) => void;
  postResponseScript: string;
  preRequestScript: string;
}) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const [active, setActive] = useState<ScriptKind>("pre");
  const value = active === "pre" ? preRequestScript : postResponseScript;
  const onChange = active === "pre" ? onPreRequestChange : onPostResponseChange;

  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--u-color-bg)]">
      <div className="flex h-8 shrink-0 items-center gap-1 border-b border-[var(--u-color-border)] px-3">
        {(["pre", "post"] as const).map((kind) => (
          <button
            aria-pressed={active === kind}
            className={cn(
              "h-6 rounded-[var(--u-radius-sm)] px-2 text-[12px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--u-color-focus)]",
              active === kind
                ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
            )}
            key={kind}
            onClick={() => setActive(kind)}
            type="button"
          >
            {kind === "pre"
              ? t("api.scripts.preRequest")
              : t("api.scripts.postResponse")}
          </button>
        ))}
        <span className="ml-auto truncate text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)]">
          {active === "pre"
            ? t("api.scripts.preHint")
            : t("api.scripts.postHint")}
        </span>
      </div>
      {!value && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-1 font-mono text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)]">
          {active === "pre"
            ? t("api.scripts.preExample")
            : t("api.scripts.postExample")}
        </div>
      )}
      <div className="min-h-0 flex-1 bg-[var(--u-color-canvas)]">
        <Editor
          beforeMount={(monaco) => configureEditorThemes(monaco, theme)}
          language="javascript"
          onChange={(next) => onChange(next ?? "")}
          options={{
            automaticLayout: true,
            fontSize: 12,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            tabSize: 2,
            wordWrap: "on",
          }}
          theme={theme === "dark" ? "unfour-dark" : "unfour-light"}
          value={value}
        />
      </div>
    </div>
  );
}
