import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ErrorBoundary } from "@sentry/react";
import { App } from "./App";
import { ErrorFallback } from "./app/ErrorFallback";
import { bootErrorReporting } from "./lib/errorReporting";
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "./ui/tokens.css";

const root = document.getElementById("root");
if (!root) throw new Error("Missing #root element in index.html");

// Optional error reporting: starts only when the backend says it is on.
void bootErrorReporting();

createRoot(root).render(
  <StrictMode>
    <ErrorBoundary fallback={<ErrorFallback />}>
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
