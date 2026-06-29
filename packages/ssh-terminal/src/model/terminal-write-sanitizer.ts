const ESC = String.fromCharCode(0x1b);
const XTERM_REQUEST_MODE_SEQUENCE_PATTERN = new RegExp(`${ESC}\\[[0-?]*\\$p`, "g");

export type SanitizedTerminalWriteChunk = {
  value: string;
  removedSequences: string[];
};

export function sanitizeTerminalWriteChunk(chunk: string): SanitizedTerminalWriteChunk {
  const removedSequences: string[] = [];
  const value = chunk.replace(XTERM_REQUEST_MODE_SEQUENCE_PATTERN, (sequence) => {
    removedSequences.push(escapeTerminalText(sequence));
    return "";
  });
  return { value, removedSequences };
}

function escapeTerminalText(value: string) {
  let escaped = "";
  for (const char of value) {
    const code = char.charCodeAt(0);
    if (char === "\\") {
      escaped += "\\\\";
    } else if (char === ESC) {
      escaped += "\\x1b";
    } else if (char === "\r") {
      escaped += "\\r";
    } else if (char === "\n") {
      escaped += "\\n";
    } else if (char === "\t") {
      escaped += "\\t";
    } else if (code <= 0x1f || (code >= 0x7f && code <= 0x9f)) {
      escaped += `\\x${code.toString(16).padStart(2, "0")}`;
    } else {
      escaped += char;
    }
  }
  return escaped;
}
