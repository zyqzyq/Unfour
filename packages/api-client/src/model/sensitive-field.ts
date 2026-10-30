export function isSensitiveField(key: string): boolean {
  return ["authorization", "cookie", "set-cookie", "proxy-authorization", "x-api-key", "x-auth-token", "password", "passwd", "token", "access_token", "refresh_token", "secret", "private_key", "api_key", "license_key"].includes(key.trim().toLowerCase());
}
