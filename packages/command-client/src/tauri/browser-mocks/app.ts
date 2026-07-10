import { UNHANDLED, type MockResult } from "./types";

export function handleAppMock<T>(command: string): MockResult<T> {
  if (command === "get_app_info") {
    return {
      version: "0.1.0",
      edition: "community",
    } as T;
  }

  return UNHANDLED;
}
