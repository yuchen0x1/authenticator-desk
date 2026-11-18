import App from "./App";
import { QuickPanel } from "./components/desktop/QuickPanel";

export function ApplicationRoot({ search = window.location.search }: { search?: string }) {
  return search.includes("quick-panel=1") ? <QuickPanel /> : <App />;
}
