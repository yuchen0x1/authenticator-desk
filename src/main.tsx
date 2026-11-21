import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ApplicationRoot } from "./ApplicationRoot";
import { readCachedTheme } from "./lib/uiPrefs";
import "./styles.css";

// Apply the remembered appearance before the first paint so a light-theme user
// never sees a dark lock screen while the vault is still encrypted.
document.documentElement.dataset.theme = readCachedTheme();

const root = document.getElementById("root");
if (!root) throw new Error("Application root element is missing.");
createRoot(root).render(<StrictMode><ApplicationRoot /></StrictMode>);
