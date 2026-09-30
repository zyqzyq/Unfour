// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getDatabaseSchema, listDatabaseCatalogs, type DatabaseConnection, type DatabaseSchema } from "@unfour/command-client";
import type { DatabaseConnectionSessionState } from "../model/types";
import { useDatabaseSchemaTreeActions } from "./useDatabaseSchemaTreeActions";
import { useDatabaseTreeController, useDatabaseTreeRootLoading, useDatabaseTreeSynchronization } from "./useDatabaseTreeController";
import { connection, deferred, schema, table } from "./database-controller.test-fixtures";

vi.mock("@unfour/command-client", () => ({ getDatabaseSchema: vi.fn(), listDatabaseCatalogs: vi.fn() }));

const postgres = connection("pg", "postgres");
const mysql = connection("mysql", "mysql");
const sqlite = connection("sqlite", "sqlite");
const connections = [postgres, mysql, sqlite];
const connected: Record<string, DatabaseConnectionSessionState> = {
  pg: { status: "connected" }, mysql: { status: "connected" }, sqlite: { status: "connected" },
};
const schemaMock = vi.mocked(getDatabaseSchema);
const catalogsMock = vi.mocked(listDatabaseCatalogs);

type Props = { active: boolean; selected: DatabaseConnection | null; states: typeof connected };

function setup(initialProps: Props = { active: true, selected: postgres, states: connected }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const setConnectionState = vi.fn();
  const rootLoads = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const hook = renderHook(({ active, selected, states }: Props) => {
    const tree = useDatabaseTreeController({
      schemaEnabled: active && Boolean(selected), selectedConnection: selected,
      selectedConnectionId: selected?.id ?? null, workspaceId: "ws",
    });
    useDatabaseTreeSynchronization(tree, {
      schemaEnabled: active && Boolean(selected), selectedConnectionId: selected?.id ?? null, setConnectionState,
    });
    const actions = useDatabaseSchemaTreeActions({ ...tree, queryClient: client, workspaceId: "ws" });
    useDatabaseTreeRootLoading({
      active, connections, connectionStates: states,
      loadConnectionRoot: (item) => { rootLoads(item); actions.loadConnectionRoot(item); },
    });
    return { ...tree, ...actions };
  }, { initialProps, wrapper });
  return { ...hook, client, rootLoads, setConnectionState };
}

beforeEach(() => {
  vi.resetAllMocks();
  catalogsMock.mockImplementation(async (_workspace, id) => id === "pg" ? ["app", "analytics"] : ["app", "archive"]);
  schemaMock.mockImplementation(async (_workspace, id, catalog) => {
    if (id === "sqlite") return schema(id, table(null));
    if (id === "mysql" && !catalog) return schema(id, table("app"), table("archive", "events"));
    return schema(id, table(catalog ?? "app", "users", id === "pg" ? "public" : null));
  });
});
afterEach(cleanup);

describe("database tree orchestration", () => {
  it("loads every connected root, keeps other catalogs lazy and merges simultaneous completions", async () => {
    const pgCatalog = deferred<DatabaseSchema>();
    const mysqlCatalog = deferred<DatabaseSchema>();
    const { result, client, rootLoads } = setup();
    await waitFor(() => expect(result.current.treeSchemaCache["sqlite::"]).toBeDefined());
    await waitFor(() => expect(result.current.catalogNamesByConn).toEqual({ pg: ["app", "analytics"], mysql: ["app", "archive"] }));
    expect(schemaMock.mock.calls).toEqual(expect.arrayContaining([["ws", "pg", null], ["ws", "sqlite", null]]));
    expect(schemaMock).not.toHaveBeenCalledWith("ws", "pg", "analytics");
    expect(schemaMock).not.toHaveBeenCalledWith("ws", "mysql", null);
    expect(catalogsMock).not.toHaveBeenCalledWith("ws", "sqlite");
    const initialRoots = rootLoads.mock.calls.length;
    schemaMock.mockImplementation((_workspace, id) => id === "pg" ? pgCatalog.promise : mysqlCatalog.promise);
    act(() => {
      result.current.loadCatalogSchema("pg", "analytics");
      result.current.loadCatalogSchema("mysql", "archive");
    });
    expect(result.current.treeLoadingKeys).toEqual(expect.arrayContaining(["pg::analytics", "mysql::archive"]));
    act(() => result.current.loadCatalogSchema("pg", "analytics"));
    expect(schemaMock.mock.calls.filter(([, id, catalog]) => id === "pg" && catalog === "analytics")).toHaveLength(1);
    await act(async () => mysqlCatalog.resolve(schema("mysql", table("archive", "events"))));
    await act(async () => pgCatalog.resolve(schema("pg", table("analytics", "audit", "public"))));
    await waitFor(() => expect(result.current.treeLoadingKeys).toEqual([]));
    expect(Object.keys(result.current.treeSchemaCache).sort()).toEqual(["mysql::archive", "pg::analytics", "pg::app", "sqlite::"]);
    expect(rootLoads).toHaveBeenCalledTimes(initialRoots);
    act(() => result.current.loadCatalogSchema("pg", "analytics"));
    expect(schemaMock.mock.calls.filter(([, id, catalog]) => id === "pg" && catalog === "analytics")).toHaveLength(1);
    expect(client.getQueryData(["database-schema", "ws", "pg", "analytics"])).toEqual(result.current.treeSchemaCache["pg::analytics"]);
    expect(client.getQueryData(["database-catalogs", "ws", "mysql"])).toEqual(["app", "archive"]);
  });

  it("keeps per-connection schemas across selection and inactivity, including MySQL grouping and empty SQLite", async () => {
    const { result, rerender, client, setConnectionState } = setup();
    await waitFor(() => expect(result.current.treeSchemaCache["pg::app"]).toBeDefined());
    act(() => result.current.loadCatalogSchema("pg", "analytics"));
    await waitFor(() => expect(result.current.treeSchemaCache["pg::analytics"]).toBeDefined());
    const pgCache = result.current.treeSchemaCache["pg::analytics"];
    rerender({ active: true, selected: mysql, states: connected });
    await waitFor(() => expect(result.current.treeSchemaCache["mysql::app"]).toBeDefined());
    expect(result.current.treeSchemaCache["mysql::archive"].tables[0].name).toBe("events");
    expect(result.current.treeSchemaCache["pg::analytics"]).toBe(pgCache);
    expect(result.current.visibleSchema?.connectionId).toBe("mysql");
    const callsBeforeInactive = schemaMock.mock.calls.length;
    rerender({ active: false, selected: mysql, states: connected });
    expect(result.current.visibleSchema).toBeUndefined();
    expect(result.current.treeModel).toBeNull();
    expect(result.current.treeSchemaCache["pg::analytics"]).toBe(pgCache);
    expect(schemaMock).toHaveBeenCalledTimes(callsBeforeInactive);
    rerender({ active: true, selected: postgres, states: connected });
    await waitFor(() => expect(result.current.visibleSchema?.connectionId).toBe("pg"));
    expect(result.current.treeSchemaCache["mysql::archive"].tables[0].name).toBe("events");
    expect(setConnectionState).toHaveBeenCalledWith("pg", expect.objectContaining({ status: "connected" }));
    act(() => { client.setQueryData(["database-schema", "ws", "sqlite", null], schema("sqlite")); });
    rerender({ active: true, selected: sqlite, states: connected });
    await waitFor(() => expect(result.current.treeSchemaCache["sqlite::"].tables).toEqual([]));
  });

  it("isolates lazy-load errors and retries without losing other connection caches", async () => {
    const { result } = setup();
    await waitFor(() => expect(result.current.treeSchemaCache["sqlite::"]).toBeDefined());
    schemaMock.mockRejectedValueOnce(new Error("catalog unavailable"));
    act(() => result.current.loadCatalogSchema("pg", "analytics"));
    await waitFor(() => expect(result.current.treeErrors["pg::analytics"]).toBe("catalog unavailable"));
    expect(result.current.treeLoadingKeys).toEqual([]);
    expect(result.current.treeSchemaCache["sqlite::"]).toBeDefined();
    act(() => result.current.loadCatalogSchema("pg", "analytics"));
    await waitFor(() => expect(result.current.treeSchemaCache["pg::analytics"]).toBeDefined());
    expect(result.current.treeErrors).toEqual({});
  });

  it("syncs selected schema errors/success into the session only while schema loading is enabled", async () => {
    schemaMock.mockImplementation(async (_workspace, id) => {
      if (id === "pg") throw new Error("schema unavailable");
      return schema(id, table(null));
    });
    const { result, rerender, client, setConnectionState } = setup();
    await waitFor(() => expect(setConnectionState).toHaveBeenCalledWith("pg", {
      message: "schema unavailable", status: "failed",
    }));
    rerender({ active: false, selected: postgres, states: connected });
    setConnectionState.mockClear();
    act(() => { client.setQueryData(["database-schema", "ws", "pg", null], schema("pg", table("app"))); });
    await waitFor(() => expect(result.current.treeSchemaCache["pg::app"]).toBeDefined());
    expect(result.current.visibleSchema).toBeUndefined();
    expect(setConnectionState).not.toHaveBeenCalled();
    rerender({ active: true, selected: postgres, states: connected });
    await waitFor(() => expect(setConnectionState).toHaveBeenCalledWith("pg", expect.objectContaining({ status: "connected" })));
  });

  it("only re-runs root loading for activation or connection/state changes, using the latest loader", () => {
    const first = vi.fn();
    const second = vi.fn();
    const states: typeof connected = { pg: { status: "connecting" }, mysql: { status: "disconnected" }, sqlite: { status: "failed" } };
    const { rerender } = renderHook((props) => useDatabaseTreeRootLoading({ connections, ...props }), {
      initialProps: { active: false, connectionStates: states, loadConnectionRoot: first },
    });
    expect(first).not.toHaveBeenCalled();
    rerender({ active: true, connectionStates: states, loadConnectionRoot: first });
    expect(first).toHaveBeenCalledExactlyOnceWith(postgres);
    rerender({ active: true, connectionStates: states, loadConnectionRoot: second });
    expect(second).not.toHaveBeenCalled();
    rerender({ active: true, connectionStates: connected, loadConnectionRoot: second });
    expect(second.mock.calls.map(([item]) => item.id)).toEqual(["pg", "mysql", "sqlite"]);
  });
});
