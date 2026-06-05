import { useState } from "react";
import type { RequestParamsTab, ResponsePanelTab, ResponseTab } from "../model/types";

export function useApiLayout() {
  const [requestTab, setRequestTab] = useState<RequestParamsTab>("query");
  const [resultTab, setResultTab] = useState<ResponsePanelTab>("response");
  const [responseTab, setResponseTab] = useState<ResponseTab>("body");

  return {
    requestTab,
    responseTab,
    resultTab,
    setRequestTab,
    setResponseTab,
    setResultTab,
  };
}
