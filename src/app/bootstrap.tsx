import ReactDOM from "react-dom/client";

import "../services/tauriWailsBridge";
import { App } from "./App";
import "./styles.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Root element #root was not found");
}

ReactDOM.createRoot(rootElement).render(<App />);
