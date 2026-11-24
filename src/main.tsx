import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/global.css";
import { applyTheme, initialTheme } from "./theme";
import App from "./App";

applyTheme(initialTheme());

// Le menu par défaut de la WebView (Reload, Inspect Element…) est en anglais et n'a rien à faire dans l'app :
// on le bloque, sauf dans les champs de saisie (couper / copier / coller). Les menus de Flux viendront plus tard.
window.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  if (!target?.closest("input, textarea")) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
