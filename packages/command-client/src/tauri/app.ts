import { call } from "./invoke";
import type { AppInfo } from "../types";

export function getAppInfo() {
  return call<AppInfo>("get_app_info");
}
