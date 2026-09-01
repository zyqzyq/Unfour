import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  I18nProvider,
  ThemeProvider,
  initializeTheme,
  type I18nResources,
} from "@unfour/ui";
import App from "./App";
import { DesktopErrorBoundary } from "./DesktopErrorBoundary";
import { accountI18nResources } from "./features/account";
import { cloudSyncI18nResources } from "./features/cloud-sync";
import { updateI18nResources } from "./features/update";
import { telemetryI18nResources } from "./features/telemetry";
import "@unfour/ui/styles.css";
import "@unfour/app-shell/styles.css";
import "./styles.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
const initialTheme = initializeTheme();
const desktopFeatureI18nResources: I18nResources = {
  en: {
    ...accountI18nResources.en,
    ...cloudSyncI18nResources.en,
    ...updateI18nResources.en,
    ...telemetryI18nResources.en,
  },
  "zh-CN": {
    ...accountI18nResources["zh-CN"],
    ...cloudSyncI18nResources["zh-CN"],
    ...updateI18nResources["zh-CN"],
    ...telemetryI18nResources["zh-CN"],
  },
};

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider defaultThemeMode={initialTheme}>
        <I18nProvider resources={desktopFeatureI18nResources}>
          <DesktopErrorBoundary>
            <App />
          </DesktopErrorBoundary>
        </I18nProvider>
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
