import { ExternalLink } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import { Button, StatusBadge, useI18n } from "@unfour/ui";
import {
  MCP_DOCS_PATH,
  MCP_DOCS_URL,
  formatMcpClientConfig,
  getMcpCommand,
} from "../../settings/settings-config";

type CopyTarget = "command" | "config";

export function SettingsMcp() {
  const { t } = useI18n();
  const command = useMemo(() => getMcpCommand(), []);
  const config = useMemo(() => formatMcpClientConfig(command), [command]);
  const [copied, setCopied] = useState<CopyTarget | null>(null);
  const [copyFailed, setCopyFailed] = useState<CopyTarget | null>(null);

  useEffect(() => {
    if (!copied && !copyFailed) {
      return undefined;
    }
    const timeout = window.setTimeout(() => {
      setCopied(null);
      setCopyFailed(null);
    }, 1600);
    return () => window.clearTimeout(timeout);
  }, [copied, copyFailed]);

  async function copyText(target: CopyTarget, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(target);
      setCopyFailed(null);
    } catch {
      setCopyFailed(target);
      setCopied(null);
    }
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.mcp.title")}
        </h2>
        <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {t("app.settings.mcp.description")}
        </p>
      </div>

      <InfoBlock label={t("app.settings.mcp.statusLabel")}>
        <StatusBadge tone="success">{t("app.settings.mcp.statusValue")}</StatusBadge>
      </InfoBlock>

      <InfoBlock
        action={
          <Button
            onClick={() => void copyText("command", command)}
            size="sm"
            type="button"
            variant="secondary"
          >
            {buttonCopyText(copied, copyFailed, "command", t("app.settings.mcp.copyCommand"), t)}
          </Button>
        }
        label={t("app.settings.mcp.commandLabel")}
      >
        <code className="block overflow-x-auto rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-2 py-1.5 font-mono text-[12px] text-[var(--u-color-text)]">
          {command}
        </code>
      </InfoBlock>

      <InfoBlock
        action={
          <Button
            onClick={() => void copyText("config", config)}
            size="sm"
            type="button"
            variant="secondary"
          >
            {buttonCopyText(copied, copyFailed, "config", t("app.settings.mcp.copyConfig"), t)}
          </Button>
        }
        label={t("app.settings.mcp.configLabel")}
      >
        <pre className="max-h-44 overflow-auto rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] p-2 font-mono text-[12px] leading-5 text-[var(--u-color-text)]">
          {config}
        </pre>
      </InfoBlock>

      <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 text-[12px] text-[var(--u-color-text-muted)]">
        <p>{t("app.settings.mcp.docsDescription", { path: MCP_DOCS_PATH })}</p>
        <Button asChild className="mt-2" size="sm" variant="outline">
          <a href={MCP_DOCS_URL} rel="noreferrer" target="_blank">
            <ExternalLink size={13} />
            {t("app.settings.mcp.openDocs")}
          </a>
        </Button>
      </div>
    </div>
  );
}

function InfoBlock({
  action,
  children,
  label,
}: {
  action?: ReactNode;
  children: ReactNode;
  label: string;
}) {
  return (
    <div className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">{label}</h3>
        {action}
      </div>
      {children}
    </div>
  );
}

function buttonCopyText(
  copied: CopyTarget | null,
  copyFailed: CopyTarget | null,
  target: CopyTarget,
  label: string,
  t: (key: string) => string,
) {
  if (copied === target) return t("app.settings.copy.copied");
  if (copyFailed === target) return t("app.settings.copy.failed");
  return label;
}
