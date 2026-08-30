import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { readStoredTheme } from "./theme";
import "./styles.css";

document.documentElement.setAttribute("data-theme", readStoredTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
