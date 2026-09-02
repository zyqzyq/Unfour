import { describe, expect, it } from "vitest";
import {
  apiHistoryPath,
  apiHistoryPrimaryLabel,
  apiHistoryTooltip,
} from "./api-history-display";

describe("API history display", () => {
  it.each([
    [
      "http://192.168.20.50:30211/dataset/2082290710210600962/graph/schema?full=true#top",
      "/dataset/2082290710210600962/graph/schema",
    ],
    [
      "https://api.example.com/users/42",
      "/users/42",
    ],
    [
      "http://localhost:3000/health",
      "/health",
    ],
    [
      "https://127.0.0.1:8443/api/v1/status",
      "/api/v1/status",
    ],
  ])("extracts a pathname from %s", (url, expected) => {
    expect(apiHistoryPath(url)).toBe(expected);
  });

  it("falls back to a safe path-like label for malformed or unusual URLs", () => {
    expect(() => apiHistoryPath("http://[invalid/api?value=1")).not.toThrow();
    expect(apiHistoryPath("http://[invalid/api?value=1")).toBe("/api");
    expect(apiHistoryPath("localhost:3000/api/v1?debug=true")).toBe("/api/v1");
    expect(apiHistoryPath("not a URL")).toBe("not a URL");
    expect(apiHistoryPath("")).toBe("/");
  });

  it("prefers a meaningful request name and keeps the full URL in the tooltip", () => {
    const named = {
      method: "get",
      name: "Query schema",
      url: "https://api.example.com/dataset/schema",
    };
    const unnamed = { ...named, name: "  " };

    expect(apiHistoryPrimaryLabel(named)).toBe("Query schema");
    expect(apiHistoryPrimaryLabel(unnamed)).toBe("/dataset/schema");
    expect(apiHistoryTooltip(named)).toBe(
      "GET\nhttps://api.example.com/dataset/schema",
    );
  });
});
