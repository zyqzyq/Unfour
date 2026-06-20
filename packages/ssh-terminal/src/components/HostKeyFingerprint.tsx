import { useEffect, useState } from "react";
import { Shield, ShieldAlert, Trash2 } from "lucide-react";
import type { SshHostFingerprintInfo } from "@unfour/command-client";
import { getSshHostFingerprint, resetSshHostFingerprint } from "@unfour/command-client";
import { Button } from "@unfour/ui";

export function HostKeyFingerprint({
  host,
  port,
}: {
  host: string;
  port: number;
}) {
  const trimmedHost = host.trim();
  const validPort = port > 0;

  const [state, setState] = useState<{
    error: string | null;
    info: SshHostFingerprintInfo | null;
    requestedKey: string;
    resolvedKey: string;
  }>({ error: null, info: null, requestedKey: "", resolvedKey: "" });

  const currentKey = trimmedHost && validPort ? `${trimmedHost}:${port}` : "";
  const loading = state.requestedKey !== "" && state.resolvedKey !== state.requestedKey;

  // Render-time sync: mark a new fetch request when host/port changes.
  if (currentKey && state.requestedKey !== currentKey) {
    setState({ error: null, info: null, requestedKey: currentKey, resolvedKey: "" });
  }
  if (!currentKey && (state.info !== null || state.error !== null)) {
    setState({ error: null, info: null, requestedKey: "", resolvedKey: "" });
  }

  useEffect(() => {
    if (!trimmedHost || !validPort) {
      return;
    }
    let cancelled = false;
    getSshHostFingerprint({ host: trimmedHost, port })
      .then((info) => {
        if (!cancelled) setState((prev) => ({ ...prev, info, error: null, resolvedKey: currentKey }));
      })
      .catch((err) => {
        if (!cancelled)
          setState((prev) => ({
            ...prev,
            error: String(err?.message ?? err),
            info: null,
            resolvedKey: currentKey,
          }));
      });
    return () => {
      cancelled = true;
    };
  }, [trimmedHost, port, validPort, currentKey]);

  function handleReset() {
    if (!trimmedHost || !validPort) return;
    setState((prev) => ({ ...prev, error: null, info: null, resolvedKey: "" }));
    resetSshHostFingerprint({ host: trimmedHost, port })
      .then(() =>
        setState((prev) => ({ ...prev, info: null, error: null, resolvedKey: currentKey })),
      )
      .catch((err) =>
        setState((prev) => ({
          ...prev,
          error: String(err?.message ?? err),
          info: null,
          resolvedKey: currentKey,
        })),
      );
  }

  const { error, info } = state;

  if (!trimmedHost || !validPort) return null;

  if (loading && !info) {
    return (
      <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-text-soft)]">
        <Shield size={14} />
        <span>Loading host-key fingerprint...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-danger)]">
        <ShieldAlert size={14} />
        <span>{error}</span>
      </div>
    );
  }

  if (!info) {
    return (
      <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-text-soft)]">
        <Shield size={14} />
        <span>No trusted fingerprint recorded yet. It will be saved on first connection.</span>
      </div>
    );
  }

  return (
    <div className="space-y-1.5 rounded border border-[var(--u-color-border)] p-2">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          Trusted Host-Key Fingerprint
        </span>
        <Button
          disabled={loading}
          onClick={handleReset}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Trash2 size={12} />
          Reset
        </Button>
      </div>
      <code className="block break-all text-[12px] text-[var(--u-color-text)]">
        {info.fingerprint}
      </code>
      <span className="block text-[11px] text-[var(--u-color-text-soft)]">
        Trusted since {new Date(info.createdAt).toLocaleDateString()}
      </span>
    </div>
  );
}
