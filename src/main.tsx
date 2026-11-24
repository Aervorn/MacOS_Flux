import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import "./styles/tokens.css";
import "./styles/global.css";
import { applyTheme, initialTheme } from "./theme";
import App from "./App";

applyTheme(initialTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
