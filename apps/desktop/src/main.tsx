import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nProvider, ThemeProvider, initializeTheme } from "@unfour/ui";
import App from "./App";
import { DesktopErrorBoundary } from "./DesktopErrorBoundary";
import "@unfour/ui/styles.css";
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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider defaultTheme={initialTheme}>
        <I18nProvider>
          <DesktopErrorBoundary>
            <App />
          </DesktopErrorBoundary>
        </I18nProvider>
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
