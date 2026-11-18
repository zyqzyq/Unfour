import type { TFunction } from "@unfour/ui";

const taskOutcomeUncertain = "SSH_TASK_OUTCOME_UNCERTAIN";
const temporaryTaskErrors = ["SSH_TASK_TEMPORARY_TARGET_CONFLICT", "SSH_TASK_TEMPORARY_BUSY", "SSH_TASK_CHANNEL_BUSY", "SSH_TASK_CHANNEL_REJECTED", "SSH_TASK_CHANNEL_TIMEOUT", "SSH_TASK_TRANSPORT_CLOSED", "SSH_TASK_SFTP_UNSUPPORTED", "SSH_TASK_SFTP_FAILED",
  "SSH_TASK_TERMINAL_BUSY", "SSH_TASK_TERMINAL_RECOVERY_REQUIRED", "SSH_TASK_TERMINAL_COMMAND_ONLY", "SSH_TASK_TERMINAL_NOT_READY", "SSH_TASK_TERMINAL_INPUT_UNCONFIRMED", "SSH_TASK_TERMINAL_OUTPUT_OVERFLOW", "SSH_TASK_EXECUTION_MODE_INVALID", "SSH_TASK_EXEC_QUEUE_TIMEOUT",
  "SSH_TASK_INTERACTIVE_LOGIN_PROMPT", "SSH_TASK_OUTPUT_INCOMPLETE"];

export function taskDisplayStatus(status: string, error?: string | null) {
  return error === taskOutcomeUncertain ? "outcomeUncertain" : status;
}

export function formatTaskRunError(error: string, t: TFunction) {
  if (error === taskOutcomeUncertain) return t("ssh.tasks.run.outcomeUncertainHint");
  if (error === "SSH_TASK_RUN_INTERRUPTED") return t("ssh.tasks.run.interrupted");
  const code = temporaryTaskErrors.find((candidate) => error.includes(candidate));
  if (code) return t(`ssh.tasks.run.errors.${code}`);
  return error;
}

export function formatTerminalError(error: unknown, t: TFunction) {
  const taskError = temporaryTaskErrors.find((code) => rawTerminalError(error).includes(code));
  if (taskError) return t(`ssh.tasks.run.errors.${taskError}`);
  if (error && typeof error === "object" && "code" in error && String(error.code).startsWith("SSH_TUNNEL_")) {
    return t(`network.errors.${error.code}`, t("api.route.failed"));
  }
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

export function formatTaskHistoryLog(log: string, t: TFunction) {
  return log.replace(/^(\[[^\]\r\n]+\] diagnostic )(SSH_TASK_[A-Z_]+)\r?$/gm,
    (line, prefix: string, code: string) => temporaryTaskErrors.includes(code) ? `${prefix}${formatTaskRunError(code, t)}` : line);
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
