import { useEffect, useState } from "react";

import { getMessages, type Messages } from "./i18n/resources";
import { tauriInvoke } from "./lib/tauri";
import type { AppSnapshot } from "./lib/types";

type BackendState =
  | { status: "checking" }
  | { status: "ready"; snapshot: AppSnapshot }
  | { status: "unavailable" };

const navKeys: Array<keyof Messages["navigation"]["items"]> = [
  "dashboard", "codes", "accounts", "favorites", "categories",
  "import", "backup", "security", "activity", "settings",
];

function statusCopy(state: BackendState, messages: Messages) {
  if (state.status === "checking") {
    return { title: messages.status.checkingTitle, detail: messages.status.checkingDetail, tone: "text-sky-200 border-sky-400/25 bg-sky-400/8" };
  }
  if (state.status === "unavailable") {
    return { title: messages.status.unavailableTitle, detail: messages.status.unavailableDetail, tone: "text-amber-100 border-amber-300/25 bg-amber-300/8" };
  }
  return {
    title: state.snapshot.locked ? messages.status.lockedTitle : messages.status.readyTitle,
    detail: state.snapshot.vault_exists ? messages.status.lockedDetail : messages.status.noVaultDetail,
    tone: "text-violet-100 border-violet-300/25 bg-violet-300/8",
  };
}

export default function App() {
  const [backend, setBackend] = useState<BackendState>({ status: "checking" });
  const messages = getMessages(backend.status === "ready" ? backend.snapshot.locale : "zh-CN");
  const status = statusCopy(backend, messages);

  useEffect(() => {
    let active = true;
    tauriInvoke<AppSnapshot>("app_snapshot")
      .then((snapshot) => { if (active) setBackend({ status: "ready", snapshot }); })
      .catch(() => { if (active) setBackend({ status: "unavailable" }); });
    return () => { active = false; };
  }, []);

  return (
    <div className="min-h-screen bg-[#080b16] text-slate-100">
      <div className="mx-auto grid min-h-screen max-w-[1500px] grid-cols-1 lg:grid-cols-[17rem_1fr]">
        <aside className="border-b border-white/8 bg-[#0b1020] px-5 py-6 lg:border-r lg:border-b-0">
          <div className="flex items-center gap-3 border-b border-white/8 pb-6">
            <div aria-hidden="true" className="grid size-10 place-items-center rounded-xl border border-violet-300/30 bg-violet-400/10 text-sm font-semibold text-violet-100 shadow-[inset_0_0_24px_rgba(139,92,246,0.12)]">AD</div>
            <div><p className="font-semibold tracking-[0.08em] text-white">AUTHENTICATOR</p><p className="text-sm text-slate-500">DESK</p></div>
          </div>
          <nav aria-label={messages.navigation.label} className="mt-6">
            <ul className="grid grid-cols-2 gap-1 sm:grid-cols-5 lg:grid-cols-1">
              {navKeys.map((key, index) => (
                <li key={key}>
                  <span
                    aria-current={index === 0 ? "page" : undefined}
                    aria-disabled={index === 0 ? undefined : true}
                    className={`flex min-h-10 items-center rounded-lg px-3 text-sm ${index === 0 ? "bg-violet-400/12 font-medium text-violet-100 ring-1 ring-inset ring-violet-300/18" : "text-slate-500"}`}
                  >{messages.navigation.items[key]}</span>
                </li>
              ))}
            </ul>
          </nav>
        </aside>
        <main className="flex min-w-0 flex-col">
          <header className="flex flex-wrap items-center justify-between gap-4 border-b border-white/8 px-6 py-5 md:px-10">
            <div><p className="text-sm text-slate-500">{messages.shell.workspace}</p><h1 className="mt-1 text-xl font-semibold tracking-tight text-white">AUTHENTICATOR DESK</h1></div>
            <span className="rounded-full border border-white/10 bg-white/4 px-3 py-1.5 text-xs text-slate-400">{messages.shell.localOnly}</span>
          </header>
          <div className="grid flex-1 place-items-center px-6 py-12 md:px-10">
            <section className="w-full max-w-3xl overflow-hidden rounded-[1.75rem] border border-white/10 bg-[#0e1427] shadow-[0_28px_90px_rgba(0,0,0,0.38)]">
              <div className="grid gap-8 p-7 md:grid-cols-[13rem_1fr] md:p-10">
                <div className="grid place-items-center rounded-2xl border border-white/8 bg-[#090d1a] p-8">
                  <div aria-hidden="true" className="grid aspect-square w-32 place-items-center rounded-full border border-violet-300/25 bg-violet-400/6 shadow-[inset_0_0_42px_rgba(124,58,237,0.14),0_0_60px_rgba(76,29,149,0.12)]">
                    <div className="grid size-16 place-items-center rounded-2xl border border-violet-200/30 bg-[#11182d]"><div className="h-7 w-5 rounded-b-md rounded-t-full border-2 border-violet-200/70" /></div>
                  </div>
                </div>
                <div className="self-center">
                  <div role="status" aria-live="polite" className={`rounded-xl border px-4 py-3 ${status.tone}`}><h2 className="font-medium">{status.title}</h2><p className="mt-1 text-sm leading-6 opacity-80">{status.detail}</p></div>
                  <p className="mt-7 text-sm leading-6 text-slate-400">{messages.shell.lockedExplanation}</p>
                  <div className="mt-7 border-t border-white/8 pt-5 text-sm text-slate-500">{messages.shell.nextStep}</div>
                </div>
              </div>
            </section>
          </div>
        </main>
      </div>
    </div>
  );
}
