import { CheckCircle2, KeyRound, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import {
  createCredential,
  deleteCredential,
  inspectCredential,
  rotateCredential,
  type CredentialMetadata,
} from "@unfour/command-client";
import { ErrorState, IconButton, Input, StatusBadge } from "@unfour/ui";
import { formatTerminalError } from "../model/errors";

export function CredentialReferenceControl({
  kind,
  label,
  onChange,
  value,
  workspaceId,
}: {
  kind: string;
  label: string;
  onChange: (credentialRef: string | null) => void;
  value?: string | null;
  workspaceId: string;
}) {
  const [credentialLabel, setCredentialLabel] = useState(label);
  const [lastSyncedLabel, setLastSyncedLabel] = useState(label);
  if (label !== lastSyncedLabel) {
    setLastSyncedLabel(label);
    setCredentialLabel(label);
  }
  const [secret, setSecret] = useState("");
  const [metadata, setMetadata] = useState<CredentialMetadata | null>(null);
  const [status, setStatus] = useState("");
  const credentialRef = value?.trim() ?? "";

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- clearing derived metadata/status when credentialRef prop changes
    setMetadata(null);
    setStatus("");
  }, [credentialRef]);

  const createMutation = useMutation({
    mutationFn: () =>
      createCredential({
        workspaceId,
        kind,
        label: credentialLabel.trim() || label,
        secret,
      }),
    onSuccess: (created) => {
      setMetadata(created);
      setSecret("");
      setStatus("Credential reference created");
      onChange(created.credentialRef);
    },
  });
  const inspectMutation = useMutation({
    mutationFn: () => inspectCredential({ workspaceId, credentialRef }),
    onSuccess: (inspected) => {
      setMetadata(inspected);
      setStatus("Credential reference verified");
    },
  });
  const rotateMutation = useMutation({
    mutationFn: () => rotateCredential({ workspaceId, credentialRef, secret }),
    onSuccess: (rotated) => {
      setMetadata(rotated);
      setSecret("");
      setStatus("Credential rotated");
    },
  });
  const deleteMutation = useMutation({
    mutationFn: () => deleteCredential({ workspaceId, credentialRef }),
    onSuccess: () => {
      setMetadata(null);
      setSecret("");
      setStatus("Credential deleted");
      onChange(null);
    },
  });
  const error =
    createMutation.error ??
    inspectMutation.error ??
    rotateMutation.error ??
    deleteMutation.error;
  const isPending =
    createMutation.isPending ||
    inspectMutation.isPending ||
    rotateMutation.isPending ||
    deleteMutation.isPending;

  return (
    <div className="space-y-2 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2">
      <div className="flex items-center gap-2 text-[12px] font-semibold uppercase text-[var(--u-color-text-soft)]">
        <KeyRound size={13} />
        Credential
      </div>
      <Input
        onChange={(event) => onChange(event.target.value.trim() || null)}
        placeholder="Create or paste a credential reference"
        value={credentialRef}
      />
      <div className="grid grid-cols-2 gap-2">
        <Input
          onChange={(event) => setCredentialLabel(event.target.value)}
          placeholder={label}
          value={credentialLabel}
        />
        <Input
          onChange={(event) => setSecret(event.target.value)}
          placeholder="Secret value"
          type="password"
          value={secret}
        />
      </div>
      <div className="grid grid-cols-4 gap-1.5">
        <IconButton
          label="Create credential"
          disabled={!secret || isPending}
          onClick={() => createMutation.mutate()}
          type="button"
        >
          <Plus size={13} />
        </IconButton>
        <IconButton
          label="Check credential"
          disabled={!credentialRef || isPending}
          onClick={() => inspectMutation.mutate()}
          type="button"
        >
          <CheckCircle2 size={13} />
        </IconButton>
        <IconButton
          label="Rotate credential"
          disabled={!credentialRef || !secret || isPending}
          onClick={() => rotateMutation.mutate()}
          type="button"
        >
          <RefreshCw size={13} />
        </IconButton>
        <IconButton
          label="Delete credential"
          disabled={!credentialRef || isPending}
          onClick={() => deleteMutation.mutate()}
          type="button"
        >
          <Trash2 size={13} />
        </IconButton>
      </div>
      {metadata && <StatusBadge tone="success">{metadata.kind}</StatusBadge>}
      {status && !error && <StatusBadge>{status}</StatusBadge>}
      {error && (
        <ErrorState className="min-h-0 justify-start py-2 text-left">
          {formatTerminalError(error)}
        </ErrorState>
      )}
    </div>
  );
}
