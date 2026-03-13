import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installDiagnosticsHandlers } from "./diagnostics";
import "./index.css";

installDiagnosticsHandlers();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
