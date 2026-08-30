import type { TFunction } from "@unfour/ui";

export function formatTerminalError(error: unknown, t: TFunction) {
  const rawMessage = rawTerminalError(error);
  const normalized = rawMessage.toLowerCase();

  if (["password ssh session requires a stored password", "password auth requires a credential reference", "password ssh auth requires a password"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.credentialMissing");
  }

  if (normalized.includes("host key verification failed")) {
    return t("ssh.errors.hostKeyMismatch");
  }
  if (["fingerprint does not match", "host key"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.hostKeyFailed");
  }
  if (["authentication failed", "invalid credentials", "permission denied", "key rejected"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.authenticationFailed");
  }
  if (["timed out", "timeout"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.timeout");
  }
  if (["connection refused", "actively refused"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.connectionRefused");
  }
  if (["could not resolve", "dns", "nodename", "name or service not known"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.hostNotResolved");
  }
  if (["network unreachable", "host unreachable", "no route to host"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.hostUnreachable");
  }
  if (normalized.includes("private key file not found")) {
    return t("ssh.errors.keyFileMissing");
  }
  if (["failed to decrypt ssh private key", "failed to read ssh private key", "passphrase may be incorrect"].some((message) => normalized.includes(message))) {
    return t("ssh.errors.keyUnreadable");
  }
  if (normalized.includes("session is not connected")) {
    return t("ssh.errors.sessionDisconnected");
  }
  if (normalized.includes("pty size")) {
    return t("ssh.errors.ptySize");
  }

  return redactTerminalError(rawMessage);
}

function rawTerminalError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

function redactTerminalError(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => {
      if (
        /(^|\b)(authorization|cookie|proxy-authorization|x-api-key|x-auth-token|password|passphrase|private[-_ ]?key)(\b|:|=)/i.test(
          line,
        )
      ) {
        return "<redacted>";
      }
      return line;
    })
    .join("\n")
    .trim();
}
