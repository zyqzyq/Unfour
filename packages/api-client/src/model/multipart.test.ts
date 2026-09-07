import { describe, expect, it } from "vitest";
import { bodyFieldsFromInput, bodyFieldsToInput, historyDetailToInput } from "../request-utils";
import { tabToInput, validateBeforeSend } from "../hooks/useApiRequestTabs";
import { createNewRequestTab, emptyApiTabsState } from "./request-tabs";
import type { MultipartPart } from "./types";
import type { ApiHistoryDetail } from "@unfour/command-client";

function tab(parts: MultipartPart[]) {
  const tab = createNewRequestTab(emptyApiTabsState("ws"), "new").tabs[0];
  return { ...tab, draft: { ...tab.draft, method: "POST", bodyMode: "multipart" as const, multipartParts: parts } };
}
const file: MultipartPart = { id: "f", enabled: true, key: "avatar", type: "file", fileName: "avatar.png", filePath: "C:/private/avatar.png" };
const text: MultipartPart = { id: "t", enabled: true, key: "avatar", type: "text", value: "" };
describe("multipart definition and execution boundary", () => {
  it("round trips ordered duplicate keys and disabled rows without local paths", () => {
    const draft = tab([text, file, { ...file, id: "disabled", enabled: false }]).draft;
    const saved = bodyFieldsToInput(draft, "save");
    expect(saved.body).not.toContain("filePath");
    expect(saved.body).not.toContain("C:/private");
    const restored = bodyFieldsFromInput(saved.bodyKind, saved.body);
    expect(restored.bodyMode).toBe("multipart");
    expect(restored.multipartParts).toEqual([text, { ...file, filePath: null }, { ...file, id: "disabled", enabled: false, filePath: null }]);
    expect(tabToInput(tab([file]), "ws", { purpose: "save" }).multipartParts).toBeUndefined();
    expect(tabToInput(tab([file]), "ws").multipartParts).toEqual([{ id: "f", filePath: "C:/private/avatar.png" }]);
  });
  it("fails closed for enabled unbound files and empty keys", () => {
    expect(validateBeforeSend(tab([{ ...file, filePath: null }]))).toMatch(/local file/);
    expect(validateBeforeSend(tab([{ ...file, enabled: false, filePath: null }, text]))).toBeNull();
    expect(validateBeforeSend(tab([{ ...text, key: " " }]))).toMatch(/key/);
  });
  it("does not generate content type and omits runtime bindings for GET and HEAD", () => {
    expect(tabToInput(tab([file]), "ws").headers.some((header) => header.key.toLowerCase() === "content-type")).toBe(false);
    for (const method of ["GET", "HEAD"]) {
      const item = tab([{ ...file, filePath: null }]); item.draft.method = method;
      expect(validateBeforeSend(item)).toBeNull();
      expect(tabToInput(item, "ws").multipartParts).toBeUndefined();
      expect(tabToInput(item, "ws").body).toBeUndefined();
    }
  });
  it("restores history body kind and filename without binding", () => {
    const history = { workspaceId: "ws", name: null, method: "POST", url: "https://example.test", requestHeadersJson: "[]", requestQueryJson: "[]", requestBodyKind: "multipart-form-data", requestBody: bodyFieldsToInput(tab([file]).draft, "save").body } as ApiHistoryDetail;
    const input = historyDetailToInput(history);
    expect(bodyFieldsFromInput(input.bodyKind, input.body).multipartParts[0]).toMatchObject({ fileName: "avatar.png", filePath: null });
  });
});
