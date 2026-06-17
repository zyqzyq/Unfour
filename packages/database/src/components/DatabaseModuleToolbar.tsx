import { Eraser, Play, Plug, RefreshCw, Square, Unplug } from "lucide-react";
import type { DatabaseConnection } from "@unfour/command-client";
import { Button, ConnectionStatus, IconButton, Select, Toolbar, ToolbarGroup } from "@unfour/ui";
import type { DatabaseConnectionStatus } from "../model/types";

export function DatabaseModuleToolbar({
  connectionStatus,
  connections,
  executePending,
  onClearSql,
  onConnect,
  onDisconnect,
  onNewQuery,
  onRefresh,
  onRun,
  onSelectConnection,
  onStop,
  pendingConfirmation,
  selectedConnectionId,
  sqlDirty,
}: {
  connectionStatus: DatabaseConnectionStatus;
  connections: DatabaseConnection[];
  executePending: boolean;
  onClearSql: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onNewQuery: () => void;
  onRefresh: () => void;
  onRun: () => void;
  onSelectConnection: (connectionId: string) => void;
  onStop: () => void;
  pendingConfirmation: boolean;
  selectedConnectionId: string | null;
  sqlDirty: boolean;
}) {
  const selected = connections.find((connection) => connection.id === selectedConnectionId);
  const connected = connectionStatus === "connected" || connectionStatus === "connecting";

  return (
    <Toolbar>
      <ToolbarGroup>
        <Button onClick={onNewQuery} size="sm" type="button" variant="outline">
          New Query
        </Button>
        <Button disabled={!selectedConnectionId || executePending} onClick={onRun} size="sm" type="button">
          <Play size={14} />
          {pendingConfirmation ? "Confirm run" : "Run"}
        </Button>
        <IconButton disabled={!executePending} label="Stop SQL execution" onClick={onStop}>
          <Square size={14} />
        </IconButton>
        <IconButton disabled={!sqlDirty || executePending} label="Clear SQL" onClick={onClearSql}>
          <Eraser size={14} />
        </IconButton>
        <IconButton label="Refresh database module" onClick={onRefresh}>
          <RefreshCw size={14} />
        </IconButton>
      </ToolbarGroup>
      <ToolbarGroup className="max-w-[560px]">
        <ConnectionStatus label={connectionStatus} status={connectionStatus === "failed" ? "error" : connectionStatus} />
        <Select
          aria-label="Database connection"
          className="w-[220px]"
          onChange={(event) => onSelectConnection(event.target.value)}
          options={connections.map((connection) => ({
            label: connection.name,
            value: connection.id,
          }))}
          value={selectedConnectionId ?? ""}
        >
          {!selectedConnectionId && <option value="">Select connection</option>}
          {!connections.length && <option value="">No connections</option>}
        </Select>
        {connected ? (
          <Button disabled={!selected || executePending} onClick={onDisconnect} size="sm" type="button" variant="outline">
            <Unplug size={13} />
            Disconnect
          </Button>
        ) : (
          <Button disabled={!selected || executePending} onClick={onConnect} size="sm" type="button" variant="outline">
            <Plug size={13} />
            Connect
          </Button>
        )}
      </ToolbarGroup>
    </Toolbar>
  );
}
