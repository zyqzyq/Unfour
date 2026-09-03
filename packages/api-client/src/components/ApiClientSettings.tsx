import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getApiClientPreferences,
  updateApiClientPreferences,
} from "@unfour/command-client";
import { Input, useI18n } from "@unfour/ui";
import { formatError } from "../model/api-request-state";

const API_CLIENT_PREFERENCES_QUERY_KEY = ["api-client-preferences"] as const;

export function ApiClientSettings() {
  const { t } = useI18n();
  const preferencesQuery = useQuery({
    queryKey: API_CLIENT_PREFERENCES_QUERY_KEY,
    queryFn: getApiClientPreferences,
  });
  return (
    <ApiClientSettingsForm
      initialValue={preferencesQuery.data?.requestTimeoutMs ?? 0}
      key={preferencesQuery.data?.requestTimeoutMs ?? "loading"}
      loading={preferencesQuery.isLoading}
      queryError={preferencesQuery.error}
      t={t}
    />
  );
}

function ApiClientSettingsForm({
  initialValue,
  loading,
  queryError,
  t,
}: {
  initialValue: number;
  loading: boolean;
  queryError: Error | null;
  t: (key: string) => string;
}) {
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState(String(initialValue));
  const updateMutation = useMutation({
    mutationFn: (requestTimeoutMs: number) =>
      updateApiClientPreferences({ requestTimeoutMs }),
    onSuccess: (preferences) => {
      setDraft(String(preferences.requestTimeoutMs));
      queryClient.setQueryData(API_CLIENT_PREFERENCES_QUERY_KEY, preferences);
    },
  });

  function commit() {
    const parsed = Number(draft);
    const next = Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : 0;
    setDraft(String(next));
    if (next !== initialValue) {
      updateMutation.mutate(next);
    }
  }

  const error = queryError ?? updateMutation.error;

  return (
    <section className="mt-6 space-y-3 border-t border-[var(--u-color-border)] pt-4">
      <div>
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("api.settings.title")}
        </h3>
        <p className="mt-1 text-[12px] leading-5 text-[var(--u-color-text-muted)]">
          {t("api.settings.description")}
        </p>
      </div>
      <div className="grid grid-cols-[150px_minmax(0,1fr)] gap-3 border-t border-[var(--u-color-border)] pt-3">
        <div>
          <label
            className="text-[12px] font-semibold text-[var(--u-color-text)]"
            htmlFor="api-default-request-timeout"
          >
            {t("api.settings.defaultTimeout")}
          </label>
          <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
            {t("api.settings.defaultTimeoutDescription")}
          </p>
        </div>
        <div className="max-w-[260px]">
          <Input
            disabled={loading || updateMutation.isPending}
            id="api-default-request-timeout"
            max={Number.MAX_SAFE_INTEGER}
            min={0}
            onBlur={commit}
            onChange={(event) => setDraft(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              }
            }}
            step={1}
            type="number"
            value={draft}
          />
          <p className="mt-1 text-[11px] text-[var(--u-color-text-soft)]">
            {updateMutation.isPending
              ? t("api.settings.saving")
              : t("api.settings.zeroUnlimited")}
          </p>
          {error && (
            <p className="mt-1 text-[11px] text-[var(--u-color-danger)]">
              {formatError(error)}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
