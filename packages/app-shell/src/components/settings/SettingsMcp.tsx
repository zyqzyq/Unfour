import {
  Bot,
  Check,
  Copy,
  LoaderCircle,
  MousePointer2,
  type LucideIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  Button,
  ConnectionStatus,
  StatusBadge,
  extractErrorDetail,
  useFeedbackErrorHandler,
  useI18n,
  type TFunction,
} from "@unfour/ui";
import {
  configureMcpClient,
  getMcpBinaryPath,
  getMcpClientStatus,
  type McpBinaryPathResult,
  type McpClient,
  type McpClientStatusResult,
} from "@unfour/command-client";
import { SettingsSectionHeading } from "./SettingsPrimitives";

type ClientMap<T> = Record<McpClient, T>;
type ClientMessage = { tone: "success" | "error"; text: string } | null;

const CLIENTS: Array<{
  client: McpClient;
  icon: LucideIcon;
  name: string;
}> = [
  { client: "codex", icon: Bot, name: "Codex" },
  { client: "cursor", icon: MousePointer2, name: "Cursor" },
];

const EMPTY_STATUSES: ClientMap<McpClientStatusResult | null> = {
  codex: null,
  cursor: null,
};

const EMPTY_ERRORS: ClientMap<boolean> = {
  codex: false,
  cursor: false,
};

const EMPTY_MESSAGES: ClientMap<ClientMessage> = {
  codex: null,
  cursor: null,
};

export function SettingsMcp() {
  const { t } = useI18n();
  const reportError = useFeedbackErrorHandler();
  const [mcp, setMcp] = useState<McpBinaryPathResult | null>(null);
  const [mcpError, setMcpError] = useState(false);
  const [clientStatuses, setClientStatuses] = useState(EMPTY_STATUSES);
  const [clientLoadErrors, setClientLoadErrors] = useState(EMPTY_ERRORS);
  const [clientMessages, setClientMessages] = useState(EMPTY_MESSAGES);
  const [configuring, setConfiguring] = useState<McpClient | null>(null);
  const [copyState, setCopyState] = useState<"copied" | "failed" | null>(null);
  const [promptCopyState, setPromptCopyState] = useState<"copied" | "failed" | null>(null);

  useEffect(() => {
    let active = true;

    void getMcpBinaryPath()
      .then((result) => {
        if (active) setMcp(result);
      })
      .catch(() => {
        if (active) setMcpError(true);
      });

    for (const { client } of CLIENTS) {
      void getMcpClientStatus(client)
        .then((result) => {
          if (active) {
            setClientStatuses((current) => ({ ...current, [client]: result }));
          }
        })
        .catch(() => {
          if (active) {
            setClientLoadErrors((current) => ({ ...current, [client]: true }));
          }
        });
    }

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!copyState) return undefined;
    const timeout = window.setTimeout(() => setCopyState(null), 1600);
    return () => window.clearTimeout(timeout);
  }, [copyState]);

  useEffect(() => {
    if (!promptCopyState) return undefined;
    const timeout = window.setTimeout(() => setPromptCopyState(null), 1600);
    return () => window.clearTimeout(timeout);
  }, [promptCopyState]);

  const mcpChecking = !mcp && !mcpError;
  const status = mcpChecking ? "connecting" : mcp?.found ? "connected" : "error";
  const statusText = mcpChecking
    ? t("app.settings.mcp.statusChecking")
    : mcp?.found
      ? t("app.settings.mcp.statusValue")
      : t("app.settings.mcp.statusUnavailable");

  async function configureClient(client: McpClient, clientName: string) {
    if (!mcp?.found || configuring) return;
    setConfiguring(client);
    setClientMessages((current) => ({ ...current, [client]: null }));
    try {
      const result = await configureMcpClient(client);
      setClientStatuses((current) => ({ ...current, [client]: result }));
      setClientMessages((current) => ({
        ...current,
        [client]: {
          tone: "success",
          text: t("app.settings.mcp.configureSuccess", { client: clientName }),
        },
      }));
    } catch (error) {
      const title = t("app.settings.mcp.configureFailed", { client: clientName });
      const detail = extractErrorDetail(error);
      setClientMessages((current) => ({
        ...current,
        [client]: { tone: "error", text: detail ? `${title} ${detail}` : title },
      }));
      reportError(error, { message: title });
    } finally {
      setConfiguring(null);
    }
  }

  async function copyCommand() {
    if (!mcp?.path) return;
    try {
      await navigator.clipboard.writeText(mcp.path);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
  }

  async function copyExamplePrompt() {
    try {
      await navigator.clipboard.writeText(t("app.settings.mcp.examplePrompt"));
      setPromptCopyState("copied");
    } catch {
      setPromptCopyState("failed");
    }
  }

  return (
    <div className="space-y-4">
      <SettingsSectionHeading
        description={t("app.settings.mcp.description")}
        title={t("app.settings.mcp.title")}
      />

      <InfoBlock label={t("app.settings.mcp.statusLabel")}>
        <ConnectionStatus
          label={statusText}
          pulse={mcpChecking}
          status={status}
          variant="dot"
        />
      </InfoBlock>

      {mcp && !mcp.found ? (
        <div
          className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-warning)] bg-[var(--u-color-warning-soft)] p-3 text-[12px] text-[var(--u-color-warning)]"
          role="alert"
        >
          <p className="font-semibold">{t("app.settings.mcp.notFoundTitle")}</p>
          <p className="mt-1 leading-5">
            {mcp.buildKind === "dev"
              ? t("app.settings.mcp.notFoundDev")
              : t("app.settings.mcp.notFoundRelease")}
          </p>
        </div>
      ) : null}

      <section className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
        <div>
          <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
            {t("app.settings.mcp.clientsLabel")}
          </h3>
          <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
            {t("app.settings.mcp.clientsDescription")}
          </p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2">
          {CLIENTS.map(({ client, icon, name }) => (
            <ClientCard
              client={client}
              clientName={name}
              configuring={configuring === client}
              disabled={!mcp?.found || Boolean(configuring)}
              icon={icon}
              key={client}
              loadError={clientLoadErrors[client]}
              message={clientMessages[client]}
              onConfigure={() => void configureClient(client, name)}
              status={clientStatuses[client]}
              t={t}
            />
          ))}
        </div>
      </section>

      <section className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.mcp.examplePromptLabel")}
        </h3>
        <p className="text-[12px] leading-5 text-[var(--u-color-text-muted)]">
          {t("app.settings.mcp.examplePromptDescription")}
        </p>
        <p className="whitespace-pre-line text-[12px] leading-5 text-[var(--u-color-text)]">
          {t("app.settings.mcp.examplePrompt")}
        </p>
        <Button
          aria-live="polite"
          onClick={() => void copyExamplePrompt()}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Copy aria-hidden="true" size={13} />
          {promptCopyState === "copied"
            ? t("app.settings.copy.copied")
            : promptCopyState === "failed"
              ? t("app.settings.copy.failed")
              : t("app.settings.mcp.copyExamplePrompt")}
        </Button>
      </section>

      <section className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text-muted)]">
          {t("app.settings.mcp.advancedLabel")}
        </h3>
        <div className="flex items-center justify-between gap-2">
          <span className="text-[12px] font-medium text-[var(--u-color-text-muted)]">
            {t("app.settings.mcp.commandLabel")}
          </span>
          <Button
            disabled={!mcp?.path}
            onClick={() => void copyCommand()}
            size="sm"
            type="button"
            variant="ghost"
          >
            <Copy aria-hidden="true" size={13} />
            {copyState === "copied"
              ? t("app.settings.copy.copied")
              : copyState === "failed"
                ? t("app.settings.copy.failed")
                : t("app.settings.mcp.copyCommand")}
          </Button>
        </div>
        <code className="block overflow-x-auto rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] px-2 py-1.5 font-mono text-[12px] text-[var(--u-color-text-muted)]">
          {mcp?.path ?? t("app.settings.mcp.commandUnavailable")}
        </code>
      </section>
    </div>
  );
}

function ClientCard({
  client,
  clientName,
  configuring,
  disabled,
  icon: Icon,
  loadError,
  message,
  onConfigure,
  status,
  t,
}: {
  client: McpClient;
  clientName: string;
  configuring: boolean;
  disabled: boolean;
  icon: LucideIcon;
  loadError: boolean;
  message: ClientMessage;
  onConfigure: () => void;
  status: McpClientStatusResult | null;
  t: TFunction;
}) {
  const resolvedStatus = loadError ? "error" : status?.status;
  const isConfigured = resolvedStatus === "configured";
  const statusLabel = clientStatusLabel(resolvedStatus, t);
  const actionLabel = clientActionLabel(clientName, resolvedStatus, configuring, t);
  const actionDisabled = disabled || isConfigured || resolvedStatus === "error" || !resolvedStatus;

  return (
    <article className="flex min-w-0 flex-col rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-[var(--u-radius-sm)] bg-[var(--u-color-surface-muted)] text-[var(--u-color-text-muted)]">
          <Icon aria-hidden="true" size={15} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h4 className="text-[13px] font-semibold text-[var(--u-color-text)]">
              {clientName}
            </h4>
            <StatusBadge tone={clientStatusTone(resolvedStatus)}>{statusLabel}</StatusBadge>
          </div>
          <p className="mt-1 text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {t(`app.settings.mcp.${client}Description`)}
          </p>
          {status?.configPath ? (
            <code
              className="mt-1 block truncate font-mono text-[11px] text-[var(--u-color-text-soft)]"
              title={status.configPath}
            >
              {status.configPath}
            </code>
          ) : null}
        </div>
      </div>
      <Button
        className="mt-3 w-full"
        disabled={actionDisabled}
        onClick={onConfigure}
        size="sm"
        type="button"
        variant={isConfigured ? "secondary" : "default"}
      >
        {configuring ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" size={13} />
        ) : isConfigured ? (
          <Check aria-hidden="true" size={13} />
        ) : null}
        {actionLabel}
      </Button>
      {message ? (
        <p
          className={`mt-2 text-[11px] leading-4 ${
            message.tone === "success"
              ? "text-[var(--u-color-success)]"
              : "text-[var(--u-color-danger)]"
          }`}
          role={message.tone === "error" ? "alert" : "status"}
        >
          {message.text}
        </p>
      ) : null}
    </article>
  );
}

function clientStatusLabel(
  status: McpClientStatusResult["status"] | undefined,
  t: TFunction,
) {
  switch (status) {
    case "configured":
      return t("app.settings.mcp.clientStatusConfigured");
    case "outdated":
      return t("app.settings.mcp.clientStatusOutdated");
    case "notConfigured":
      return t("app.settings.mcp.clientStatusNotConfigured");
    case "error":
      return t("app.settings.mcp.clientStatusError");
    default:
      return t("app.settings.mcp.statusChecking");
  }
}

function clientStatusTone(status: McpClientStatusResult["status"] | undefined) {
  switch (status) {
    case "configured":
      return "success" as const;
    case "outdated":
      return "warning" as const;
    case "error":
      return "danger" as const;
    default:
      return "neutral" as const;
  }
}

function clientActionLabel(
  clientName: string,
  status: McpClientStatusResult["status"] | undefined,
  configuring: boolean,
  t: TFunction,
) {
  if (configuring) return t("app.settings.mcp.configuringClient", { client: clientName });
  if (status === "configured") {
    return t("app.settings.mcp.clientConfigured", { client: clientName });
  }
  if (status === "outdated") {
    return t("app.settings.mcp.updateClient", { client: clientName });
  }
  return t("app.settings.mcp.configureClient", { client: clientName });
}

function InfoBlock({ children, label }: { children: ReactNode; label: string }) {
  return (
    <div className="space-y-2 border-t border-[var(--u-color-border)] pt-3">
      <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">{label}</h3>
      {children}
    </div>
  );
}
