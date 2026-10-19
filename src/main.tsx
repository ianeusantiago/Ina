import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";
import "./index.css";
import { applyTheme, parseTheme, THEME_KEY } from "./lib/theme";

// Before the first paint, so the saved theme never flashes.
try {
  applyTheme(parseTheme(localStorage.getItem(THEME_KEY)));
} catch {
  applyTheme("light");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
