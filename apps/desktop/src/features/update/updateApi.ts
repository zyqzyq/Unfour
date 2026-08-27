import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  UpdateDownloadEvent,
  UpdateInfo,
  UpdateMeta,
  UpdateRecovery,
} from "./updateTypes";

export async function getUpdateInfo(): Promise<UpdateMeta> {
  return invoke<UpdateMeta>("get_update_info");
}

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("check_for_update");
}

export async function installUpdate(
  onEvent: (event: UpdateDownloadEvent) => void,
): Promise<void> {
  const eventChannel = new Channel<UpdateDownloadEvent>();
  eventChannel.onmessage = onEvent;
  return invoke<void>("install_update", { onEvent: eventChannel });
}

export function updaterError(error: unknown, fallback: UpdateRecovery) {
  if (typeof error === "object" && error !== null) {
    const value = error as { code?: unknown; message?: unknown };
    const recovery = value.code === "installer_start_failed"
      ? "installer"
      : value.code === "update_signature_verification_failed"
        ? "signature"
        : value.code === "update_download_failed" || value.code === "download_failed"
          ? "download"
          : fallback;
    return {
      message: typeof value.message === "string" ? value.message : String(error),
      recovery,
    };
  }
  return {
    message: error instanceof Error ? error.message : String(error),
    recovery: fallback,
  };
}
