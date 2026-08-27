import { UNHANDLED, type MockResult } from "./types";

export function handleAppMock<T>(command: string): MockResult<T> {
  if (command === "get_app_info") {
    return {
      name: "Unfour",
      version: "0.1.0",
      distribution: "standard",
      channel: "test",
      commit: null,
    } as T;
  }

  return UNHANDLED;
}
