import { useState } from "react";
import { Route } from "lucide-react";
import type { ApiRoute, ApiRouteEdit } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Select, useI18n } from "@unfour/ui";
import type { ApiRouteControls } from "../hooks/useApiRoutes";
import { errorCode } from "../model/api-request-state";

function choice(route: ApiRoute) { return route.mode === "ssh" ? "ssh:" + route.sshConnectionId : route.mode; }
function fromChoice(value: string): ApiRoute {
  if (value === "direct" || value === "inherit") return { mode: value };
  return { mode: "ssh", sshConnectionId: value.slice(4) };
}

type Props = {
  routing: ApiRouteControls;
  environmentName?: string;
  draft?: ApiRouteEdit;
  onChange: (route: ApiRouteEdit | undefined) => void;
  disabled: boolean;
};

export function ApiRoutePicker(props: Props) {
  const { t } = useI18n();
  const { routing, draft, disabled } = props;
  const [open, setOpen] = useState(false);
  const effective = routing.effective;
  const name = routing.selectedHost?.name;
  const label = effective?.mode === "ssh"
    ? t("api.route.via", { name: name || t("api.route.missing") })
    : routing.ready ? t("api.route.direct") : t("api.route.unavailableShort");
  const inherited = (draft?.route ?? routing.routes?.request)?.mode === "inherit";
  const source = routing.routes?.environmentId && inherited ? t("api.route.fromEnvironment", { route: label }) : label;
  return <>
    <Button type="button" size="sm" variant="ghost" disabled={disabled} onClick={() => setOpen(true)}
      aria-label={t("api.route.edit", { route: source })} title={t("api.route.edit", { route: source })}
      className="max-w-[44%] shrink-0"><Route size={13} /><span className="truncate">{source}</span></Button>
    {open && <RouteDialog {...props} key={routing.routes?.environmentId ?? ""} onClose={() => setOpen(false)} />}
  </>;
}

function RouteDialog({ routing, draft, environmentName, onChange, onClose }: Props & { onClose: () => void }) {
  const { t } = useI18n();
  const { routes, ssh, query, environmentMutation } = routing;
  const [request, setRequest] = useState<ApiRouteEdit | null>(null);
  const [environment, setEnvironment] = useState<ApiRouteEdit | null>(null);
  const [error, setError] = useState("");
  const stored = routes?.request ?? { mode: "inherit" };
  const selected = choice(request?.route ?? draft?.route ?? stored);
  const defaultValue = choice(environment?.route ?? routes?.environment ?? { mode: "direct" });
  const requestChanged = draft && choice(draft.expected) !== choice(stored);
  const options = [{ value: "direct", label: t("api.route.direct") }, ...(ssh.data ?? []).map((host) => ({
    value: "ssh:" + host.id, label: t("api.route.via", { name: host.name }),
  }))];
  const withMissing = (value: string) => value.startsWith("ssh:") && !options.some((item) => item.value === value)
    ? [{ value, label: t("api.route.missing") }, ...options] : options;
  const ready = Boolean(routes && !query.isError);
  async function apply() {
    if (!routes || environmentMutation.isPending) return;
    setError("");
    try {
      if (environment && routes.environmentId && choice(environment.route) !== choice(routes.environment)) {
        await environmentMutation.mutateAsync({ id: routes.environmentId, edit: environment });
      }
      if (request) onChange(choice(request.route) === choice(stored) ? undefined : request);
      else if (requestChanged) onChange(choice(draft.route) === choice(stored) ? undefined : { expected: stored, route: draft.route });
      onClose();
    } catch (cause) {
      setError(t(`network.errors.${errorCode(cause)}`, t("api.route.failed")));
      if (errorCode(cause) === "SSH_TUNNEL_ROUTE_CHANGED") { setEnvironment(null); void query.refetch(); }
    }
  }
  return <Dialog open onOpenChange={(next) => { if (!next && !environmentMutation.isPending) onClose(); }}>
    <DialogContent title={t("api.route.title")}>
      <DialogHeader><DialogTitle>{t("api.route.title")}</DialogTitle>
        <DialogDescription>{t("api.route.localHelp")}</DialogDescription></DialogHeader>
      <DialogBody className="space-y-3">
        <label className="grid gap-1 text-xs">{t("api.route.request")}
          <Select value={selected} disabled={!ready || environmentMutation.isPending}
            onChange={(e) => setRequest({ expected: stored, route: fromChoice(e.target.value) })} options={[{ value: "inherit", label: t("api.route.inherit") }, ...withMissing(selected)]} />
        </label>
        {routes?.environmentId && <label className="grid gap-1 text-xs">{t("api.route.environment", { name: environmentName ?? routes.environmentId })}
          <Select value={defaultValue} disabled={!ready || environmentMutation.isPending}
            onChange={(e) => setEnvironment({ expected: routes.environment, route: fromChoice(e.target.value) })} options={withMissing(defaultValue)} />
        </label>}
        {requestChanged && <p role="status" className="text-xs">{t("network.errors.SSH_TUNNEL_ROUTE_CHANGED")}</p>}
        {(query.isError || ssh.isError) && <div role="alert" className="flex items-center gap-2 text-xs">
          {t("api.route.loadFailed")}
          <Button type="button" variant="ghost" size="sm" onClick={() => { void query.refetch(); void ssh.refetch(); }}>{t("api.route.retry")}</Button>
        </div>}
        {ssh.isSuccess && !ssh.data.length && <p className="text-xs text-[var(--u-color-text-muted)]">{t("api.route.noHosts")}</p>}
        {error && <p role="alert" className="text-xs text-[var(--u-color-danger)]">{error}</p>}
      </DialogBody>
      <DialogFooter>
        <Button type="button" variant="outline" disabled={environmentMutation.isPending} onClick={onClose}>{t("common.confirm.cancel")}</Button>
        <Button type="button" disabled={!ready || environmentMutation.isPending} onClick={() => void apply()}>{t("api.route.apply")}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>;
}
