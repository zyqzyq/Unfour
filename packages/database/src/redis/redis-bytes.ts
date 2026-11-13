function bytesFromBase64(value: string) {
  return Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
}

export function textKey(value: string) {
  const bytes = new TextEncoder().encode(value);
  return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
}

export function hexBytes(base64: string) {
  return Array.from(bytesFromBase64(base64), (byte) => byte.toString(16).padStart(2, "0")).join(" ");
}

export function readableText(base64: string) {
  try {
    const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytesFromBase64(base64));
    for (const char of text) {
      if (char.charCodeAt(0) < 32 && !"\n\r\t".includes(char)) return null;
    }
    return text;
  } catch {
    return null;
  }
}
