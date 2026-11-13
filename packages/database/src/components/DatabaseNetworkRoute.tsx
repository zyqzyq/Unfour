import { listSshConnections, listTemporaryRouteHandles, temporaryRouteOptions } from "@unfour/command-client";
import { Button, Select, useI18n, useLiveOptions } from "@unfour/ui";

export function DatabaseNetworkRoute({ workspaceId, sshId, revision, temporaryHandle, onChange }: {
  workspaceId: string;
  sshId?: string | null;
  revision?: number | null;
  temporaryHandle?: string | null;
  onChange: (route: { sshConnectionId?: string | null; expectedSshRevision?: number; temporaryHandle: string | null }) => void;
}) {
  const { t } = useI18n();
  const saved = useLiveOptions(workspaceId, listSshConnections, true);
  const live = useLiveOptions(workspaceId, listTemporaryRouteHandles, true);
  const hosts = saved?.items.filter((host) => host.workspaceId === workspaceId && !host.deletedAt) ?? [];
  const chosen = hosts.find((host) => host.id === sshId);
  const sessions = temporaryRouteOptions(live?.items ?? [], temporaryHandle, (id) => t("network.temporarySession", { id }), t("network.temporaryUnavailable"));
  const selected = temporaryHandle ? "live:" + temporaryHandle : sshId ? "saved:" + sshId : "";
  return <div className="grid gap-1">
    <label className="grid gap-1 text-xs">{t("redis.route.label")}
      <Select value={selected} options={[
        { value: "", label: t("redis.route.direct") },
        ...hosts.map((host) => ({ value: "saved:" + host.id, label: t("redis.route.via", { name: `${host.name} · ${host.username}@${host.host}:${host.port}` }) })),
        ...(sshId && !hosts.some((h) => h.id === sshId) ? [{ value: "saved:" + sshId, label: t("redis.route.missing") }] : []),
        ...sessions.map((row) => ({ ...row, value: "live:" + row.value })),
      ]} onChange={(event) => {
        const value = event.target.value;
        if (value.startsWith("live:")) onChange({ temporaryHandle: value.slice(5) });
        else if (!value) onChange({ sshConnectionId: null, expectedSshRevision: undefined, temporaryHandle: null });
        else {
          const host = hosts.find((item) => "saved:" + item.id === value);
          onChange({ sshConnectionId: value.slice(6), expectedSshRevision: host?.revision, temporaryHandle: null });
        }
      }} />
    </label>
    {temporaryHandle && <p className="text-xs text-[var(--u-color-text-muted)]">{t("network.temporaryHelp")}</p>}
    {!temporaryHandle && chosen && revision != null && chosen.revision !== revision && <div className="flex items-center gap-1 text-xs" role="status">
      {t("network.errors.SSH_TUNNEL_ROUTE_CHANGED")}
      <Button type="button" variant="outline" size="sm" onClick={() => onChange({ sshConnectionId: chosen.id,
        expectedSshRevision: chosen.revision, temporaryHandle: null })}>{t("common.reloadLatest")}</Button>
    </div>}
    {(saved?.failed || live?.failed) && <p role="alert" className="text-xs">{t("redis.route.listFailed")}</p>}
  </div>;
}
