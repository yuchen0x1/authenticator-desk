import { invoke, isTauri, type InvokeArgs } from "@tauri-apps/api/core";

export class TauriUnavailableError extends Error {
  readonly code = "backend_unavailable";
  constructor() {
    super("The AUTHENTICATOR DESK desktop backend is unavailable.");
    this.name = "TauriUnavailableError";
  }
}

export function tauriInvoke<T>(command: string, payload?: unknown): Promise<T> {
  if (!isTauri()) return Promise.reject(new TauriUnavailableError());
  return invoke<T>(command, payload as InvokeArgs | undefined);
}
