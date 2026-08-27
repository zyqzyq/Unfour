import { ExternalLink } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { Button, useI18n } from "@unfour/ui";
import {
  APP_GITHUB_URL,
  APP_NAME,
  APP_WEBSITE_URL,
  createVersionInfo,
  formatShortCommit,
} from "../../settings/settings-config";
import { getAppInfo } from "@unfour/command-client";
import type { AppInfo } from "@unfour/command-client";

const FALLBACK_APP_INFO: AppInfo = {
  name: APP_NAME,
  version: "",
  distribution: "standard",
  channel: "test",
  commit: null,
};

export function SettingsAbout() {
  const { t } = useI18n();
  const [copyState, setCopyState] = useState<"copied" | "failed" | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    if (!copyState) {
      return undefined;
    }
    const timeout = window.setTimeout(() => setCopyState(null), 1600);
    return () => window.clearTimeout(timeout);
  }, [copyState]);

  useEffect(() => {
    let cancelled = false;
    void getAppInfo()
      .then((info) => {
        if (!cancelled) {
          setAppInfo(info);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAppInfo(FALLBACK_APP_INFO);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const distributionLabel =
    appInfo?.distribution === "microsoft-store"
      ? t("app.settings.about.distributionMicrosoftStore")
      : t("app.settings.about.distributionStandard");

  const shortCommit = formatShortCommit(appInfo?.commit);

  async function copyVersionInfo() {
    const info = appInfo ?? FALLBACK_APP_INFO;
    try {
      await navigator.clipboard.writeText(
        createVersionInfo(undefined, {
          name: info.name || APP_NAME,
          version: info.version,
          distribution: info.distribution,
          channel: info.channel,
          commit: info.commit,
        }),
      );
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">
          {t("app.settings.about.title")}
        </h2>
        <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {t("app.settings.about.description")}
        </p>
      </div>

      <dl className="divide-y divide-[var(--u-color-border)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
        <InfoRow label={t("app.settings.about.appName")} value={APP_NAME} />
        <InfoRow label={t("app.settings.about.version")} value={appInfo?.version || ""} />
        <InfoRow label={t("app.settings.about.distribution")} value={distributionLabel} />
        {shortCommit ? (
          <InfoRow
            label={t("app.settings.about.commit")}
            value={<span className="font-mono">{shortCommit}</span>}
          />
        ) : null}
        <InfoRow
          label={t("app.settings.about.website")}
          value={<ExternalLinkValue href={APP_WEBSITE_URL} label={APP_WEBSITE_URL} />}
        />
        <InfoRow
          label={t("app.settings.about.github")}
          value={<ExternalLinkValue href={APP_GITHUB_URL} label={APP_GITHUB_URL} />}
        />
      </dl>

      <Button onClick={() => void copyVersionInfo()} size="sm" type="button" variant="secondary">
        {copyState === "copied"
          ? t("app.settings.copy.copied")
          : copyState === "failed"
            ? t("app.settings.copy.failed")
            : t("app.settings.about.copyVersionInfo")}
      </Button>
    </div>
  );
}

function InfoRow({
  label,
  value,
}: {
  label: string;
  value: ReactNode;
}) {
  return (
    <div className="grid grid-cols-[140px_minmax(0,1fr)] gap-3 px-3 py-2">
      <dt className="text-[12px] font-semibold text-[var(--u-color-text-muted)]">{label}</dt>
      <dd className="min-w-0 text-[12px] text-[var(--u-color-text)]">{value}</dd>
    </div>
  );
}

function ExternalLinkValue({ href, label }: { href: string; label: string }) {
  return (
    <a
      className="inline-flex max-w-full items-center gap-1 text-[var(--u-color-primary)] hover:underline"
      href={href}
      rel="noreferrer"
      target="_blank"
    >
      <span className="truncate">{label}</span>
      <ExternalLink className="shrink-0" size={12} />
    </a>
  );
}
