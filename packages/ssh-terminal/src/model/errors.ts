export function formatTerminalError(error: unknown) {
  const rawMessage = rawTerminalError(error);
  const normalized = rawMessage.toLowerCase();

  if (normalized.includes("host key verification failed")) {
    return "Host key verification failed. The saved host fingerprint does not match the server. Review the fingerprint before reconnecting.";
  }
  if (
    normalized.includes("fingerprint does not match") ||
    normalized.includes("host key")
  ) {
    return "SSH host key check failed. The server identity changed or is not trusted yet.";
  }
  if (
    normalized.includes("authentication failed") ||
    normalized.includes("invalid credentials") ||
    normalized.includes("permission denied") ||
    normalized.includes("key rejected")
  ) {
    return "SSH authentication failed. Check the username, credential reference, private key, or passphrase.";
  }
  if (normalized.includes("timed out") || normalized.includes("timeout")) {
    return "SSH connection timed out. Check the host, port, VPN, firewall, or network route.";
  }
  if (
    normalized.includes("connection refused") ||
    normalized.includes("actively refused")
  ) {
    return "SSH port refused the connection. Confirm the server is running SSH on this port.";
  }
  if (
    normalized.includes("could not resolve") ||
    normalized.includes("dns") ||
    normalized.includes("nodename") ||
    normalized.includes("name or service not known")
  ) {
    return "SSH host could not be resolved. Check the hostname or DNS settings.";
  }
  if (
    normalized.includes("network unreachable") ||
    normalized.includes("host unreachable") ||
    normalized.includes("no route to host")
  ) {
    return "SSH host is unreachable. Check the network, VPN, firewall, and server address.";
  }
  if (normalized.includes("private key file not found")) {
    return "SSH private key file was not found. Check the key path saved on this connection.";
  }
  if (
    normalized.includes("failed to decrypt ssh private key") ||
    normalized.includes("failed to read ssh private key") ||
    normalized.includes("passphrase may be incorrect")
  ) {
    return "SSH private key could not be read or decrypted. Check the key format and passphrase credential.";
  }
  if (normalized.includes("session is not connected")) {
    return "SSH session is not connected. Reconnect before sending input.";
  }
  if (normalized.includes("pty size")) {
    return "Terminal size is outside the supported PTY range.";
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
