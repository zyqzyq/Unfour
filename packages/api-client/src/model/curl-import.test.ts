import { describe, expect, it } from "vitest";
import { parseCurl } from "./curl-import";

describe("cURL import", () => {
  it("preserves encoded Unicode query values", () => {
    expect(parseCurl("curl 'https://example.test/?q=%E4%B8%AD%E6%96%87'").query[0].value).toBe("中文");
  });
  it("reads Chrome/Edge caret-quoted cmd arguments", () => {
    const draft = parseCurl(String.raw`curl ^"https://example.test/path?a=1^&b=2^" -H ^"Content-Type: application/json^" --data-raw ^"^{^\^"name^\^":^\^"Ada^\^"^}^"`);
    expect(draft.body).toBe('{"name":"Ada"}');
    expect(draft.query.map(x => x.key)).toEqual(["a", "b"]);
  });
  it("does not interpret caret syntax within Bash single quotes", () => {
    expect(parseCurl(`curl 'https://example.test' --data-raw 'a^"b'`).body).toBe('a^"b');
  });
  it.each(["https://example.test --data-raw '{''{token}}'", "--url 'https://example.test/?q=%7B%7Btoken%7D%7D'"])("rejects interpolation after decoding: %s", options => {
    expect(() => parseCurl(`curl ${options}`)).toThrow();
  });
  it("rejects unescaped cmd operators within caret quote delimiters", () => {
    expect(() => parseCurl('curl ^"https://example.test/?a=1&b=2^"')).toThrow();
  });
  it("reads DevTools inline cookies and long equals/short attached options", () => {
    const draft = parseCurl(`curl --url=https://example.test -XPUT -b 'sid=fixture; theme=dark' -H'Content-Type: text/plain' --data-binary='hello'`);
    expect(draft.method).toBe("PUT");
    expect(draft.headers).toContainEqual({ key: "Cookie", value: "sid=fixture; theme=dark", enabled: true });
    expect(draft.body).toBe("hello");
  });
  it.each(["https://example.test/?q=%FF", "https://example.test/?q=%oops", "https://example.test/?q={{token}}"])("rejects lossy query decoding or runtime interpolation: %s", url => {
    expect(() => parseCurl(`curl '${url}'`)).toThrow();
  });
  it.each(["-b cookies.txt", "-b ''", "-b 'a=1' -H 'Cookie: b=2'", "-G -d 'q=a#b'", "--data-raw '{{token}}'"])("rejects ambiguous values: %s", options => {
    expect(() => parseCurl(`curl https://example.test ${options}`)).toThrow();
  });
  it("reads DevTools headers, duplicate query and JSON without generating auth", () => {
    const draft = parseCurl(`curl 'https://example.test/a?q=one&q=two' \\\n -H 'Authorization: Bearer fixture' -H 'Cookie: sid=fixture' -H 'X-Test: a' -H 'X-Test: b' -H 'Content-Type: application/json' --data-raw '{"a":1}'`);
    expect(draft.method).toBe("POST");
    expect(draft.query.map(x => x.value)).toEqual(["one", "two"]);
    expect(draft.headers.filter(x => x.key === "X-Test")).toHaveLength(2);
    expect(draft.auth).toEqual({ type: "none" });
    expect(draft.body).toBe('{"a":1}');
    expect(draft.preRequestScript).toBe("");
  });
  it("supports CRLF, cmd continuation and escaped JSON quotes", () => {
    const draft = parseCurl('curl "https://example.test" ^\r\n --data-raw "{^\\"a^\\":1}"');
    expect(draft.body).toBe('{"a":1}');
  });
  it("handles shell single quote concatenation and literal raw @", () => {
    expect(parseCurl(`curl https://example.test --data-raw 'it'\\''s'`).body).toBe("it's");
    expect(parseCurl(`curl https://example.test --data-raw '@literal'`).body).toBe("@literal");
  });
  it("joins data and moves it into query with get", () => {
    const draft = parseCurl(`curl -G 'https://example.test?a=1' -d 'b=two+words' --data 'b=3'`);
    expect(draft.method).toBe("GET");
    expect(draft.query.map(x => [x.key, x.value])).toEqual([["a", "1"], ["b", "two words"], ["b", "3"]]);
    expect(draft.bodyMode).toBe("none");
  });
  it.each(["| whoami", "> out", "; whoami", "&& whoami", "\nwhoami", "--compressed", "-F 'file=@a'", "--data @file", "--data-binary @file", "-H @file", "--unknown secret", "--url https://other.test", "-X GET -d a=1", "-H 'Accept:'", "-H 'Content-Type: multipart/form-data' -d text"])("rejects unsupported semantics: %s", suffix => {
    expect(() => parseCurl(`curl https://example.test ${suffix}`)).toThrow();
  });
  it.each(["$(whoami)", "`whoami`", "$HOME", "%TOKEN%", "!TOKEN!"])("rejects expansion without echoing input: %s", value => {
    expect(() => parseCurl(`curl 'https://example.test' -H "X: ${value}"`)).toThrow(/^CURL_/);
  });
});
