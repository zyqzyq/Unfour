/** A single CSV or TSV field, including escaping for quoted and multiline values. */
export function serializeDelimitedCell(value: string | null | undefined, delimiter: "," | "\t") {
  const text = value ?? "";
  if (!text.includes(delimiter) && !/["\r\n]/.test(text)) return text;
  return `"${text.replace(/"/g, '""')}"`;
}
