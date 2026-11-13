import type { ApiRoute, ApiRouteEdit, ApiRoutes, ApiRoutingInput } from "../../types";
import { mockStore } from "./state";
import { UNHANDLED, type MockCommandHandler } from "./types";

const requestRoutes = new Map<string, ApiRoute>();
const environmentRoutes = new Map<string, ApiRoute>();
const direct: ApiRoute = { mode: "direct" };
const inherit: ApiRoute = { mode: "inherit" };
const key = (workspace: string, id: string) => JSON.stringify([workspace, id]);
const missing = () => ({ code: "SSH_TUNNEL_ROUTE_MISSING" });

function validate(workspace: string, route: ApiRoute) {
  if (route.mode === "ssh" && !mockStore.sshConnections.some((c) => c.workspaceId === workspace && c.id === route.sshConnectionId)) throw missing();
}

export function writeMockRequestRoute(workspace: string, id: string, route?: ApiRoute) {
  if (route === undefined) return;
  validate(workspace, route);
  if (route.mode === "inherit") requestRoutes.delete(key(workspace, id));
  else requestRoutes.set(key(workspace, id), route);
}

function checkEdit(current: ApiRoute, edit: ApiRouteEdit) {
  if (JSON.stringify(current) !== JSON.stringify(edit.expected)) throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
}

export function updateMockRequestRoute(workspace: string, id: string, edit?: ApiRouteEdit) {
  if (!edit) return;
  checkEdit(requestRoutes.get(key(workspace, id)) ?? inherit, edit);
  writeMockRequestRoute(workspace, id, edit.route);
}

export function mockApiRoutes(workspace: string, requestId?: string | null, environmentId?: string | null): ApiRoutes {
  const env = mockStore.workspaceEnvironments.find(e => e.workspaceId === workspace && !e.deletedAt && (environmentId ? e.id === environmentId : e.isActive));
  if (environmentId && !env) throw missing();
  if (requestId && !mockStore.savedRequests.some(r => r.workspaceId === workspace && r.id === requestId && !r.deletedAt)) throw missing();
  const environment = env ? environmentRoutes.get(key(workspace, env.id)) ?? direct : direct;
  const request = requestId ? requestRoutes.get(key(workspace, requestId)) ?? inherit : inherit;
  return { environment, request, environmentId: env?.id ?? null, effective: request.mode === "inherit" ? environment : request };
}

export const handleApiRouteMock: MockCommandHandler = <T>(command: string, args: Record<string, unknown> = {}) => {
  if (command !== "api_routes_get" && command !== "api_environment_route_set") return UNHANDLED;
  const workspace = String(args.workspaceId);
  if (command === "api_routes_get") return mockApiRoutes(workspace, args.requestId as string | undefined, args.environmentId as string | undefined) as T;
  const env = String(args.environmentId);
  const { environment } = mockApiRoutes(workspace, undefined, env);
  const edit = args.edit as ApiRouteEdit;
  checkEdit(environment, edit);
  const route = edit.route;
  if (route.mode === "inherit") throw { code: "SSH_TUNNEL_INVALID_ROUTE" };
  validate(workspace, route);
  environmentRoutes.set(key(workspace, env), route);
  return undefined as T;
};

export function requireDirectBrowserRoute(workspace: string, preview?: ApiRoutingInput) {
  const saved = mockApiRoutes(workspace, preview?.requestId);
  const policy = preview?.route ?? saved.request;
  const effective = policy.mode === "inherit" ? saved.environment : policy;
  if (preview && (preview.expectedEnvironmentId !== saved.environmentId || JSON.stringify(effective) !== JSON.stringify(preview.expectedEffective))) {
    throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
  }
  if (effective.mode === "ssh") {
    const host = mockStore.sshConnections.find(c => c.workspaceId === workspace && c.id === effective.sshConnectionId);
    if (!host) throw missing();
    if (preview?.expectedSshRevision !== undefined && preview.expectedSshRevision !== host.revision) throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
    throw { code: "SSH_TUNNEL_UNAVAILABLE" };
  }
}
