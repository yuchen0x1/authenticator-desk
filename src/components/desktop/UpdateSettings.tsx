import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { tauriInvoke } from "../../lib/tauri";

type Version = { version: string; available: boolean };
type Release = { version: string; date: string; notes: string; bytes: number };
type Progress = { received: number; total: number };
type Status = "current" | "failed" | "integrity" | "incompatible";

const copy = {
  en: {
    title: "Application updates", check: "Check for updates", checking: "Checking…",
    current: "You have the latest version.", offer: "Update available",
    install: "Install and restart", later: "Later", notes: "Release notes",
    downloading: "Downloading", verifying: "Verifying and starting installer…",
    manual: "Use GitHub Releases to update this build manually.",
    failed: "The update could not start. Check the connection or check for updates again.",
    integrity: "The release metadata or package could not be verified.",
    incompatible: "This update does not support your installation or Windows version.",
  },
  "zh-CN": {
    title: "应用更新", check: "检查更新", checking: "正在检查…", current: "当前已是最新版本。",
    offer: "发现新版本", install: "安装并重启", later: "稍后", notes: "更新说明",
    downloading: "正在下载", verifying: "正在验证并启动安装程序…", manual: "请从 GitHub Releases 手动更新此版本。",
    failed: "更新未能启动。请检查网络连接或重新检查更新。",
    integrity: "无法验证更新信息或安装包。", incompatible: "此更新不支持当前安装方式或 Windows 版本。",
  },
  "zh-TW": {
    title: "應用程式更新", check: "檢查更新", checking: "正在檢查…", current: "目前已是最新版本。",
    offer: "發現新版本", install: "安裝並重新啟動", later: "稍後", notes: "更新說明",
    downloading: "正在下載", verifying: "正在驗證並啟動安裝程式…", manual: "請從 GitHub Releases 手動更新此版本。",
    failed: "更新無法啟動。請檢查網路連線或重新檢查更新。",
    integrity: "無法驗證更新資訊或安裝檔。", incompatible: "此更新不支援目前的安裝方式或 Windows 版本。",
  },
  ja: {
    title: "アプリの更新", check: "更新を確認", checking: "確認中…", current: "最新バージョンです。",
    offer: "更新があります", install: "インストールして再起動", later: "後で", notes: "リリースノート",
    downloading: "ダウンロード中", verifying: "検証してインストーラーを起動中…",
    manual: "このビルドは GitHub Releases から手動で更新してください。",
    failed: "更新を開始できませんでした。接続を確認してもう一度お試しください。",
    integrity: "更新情報またはパッケージを検証できません。", incompatible: "現在のインストール形式または Windows バージョンには対応していません。",
  },
};
const size = (bytes: number) => (bytes / (1024 * 1024)).toFixed(1) + " MiB";

export function UpdateSettings({ locale }: { locale: string }) {
  const t = copy[locale as keyof typeof copy] ?? copy.en;
  const [version, setVersion] = useState<Version>();
  const [release, setRelease] = useState<Release>();
  const [status, setStatus] = useState<Status>();
  const [phase, setPhase] = useState<"idle" | "checking" | "installing">("idle");
  const [progress, setProgress] = useState<Progress>();
  const started = useRef(0);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    let active = true;
    void tauriInvoke<Version>("update_version").then((value) => {
      if (active) setVersion(value);
    }).catch(() => {});
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (release && !dialog.current?.open) dialog.current?.showModal();
  }, [release]);

  function report(reason: unknown) {
    if (reason === "integrity" || reason === "invalid_release") setStatus("integrity");
    else if (reason === "incompatible") setStatus("incompatible");
    else setStatus("failed");
  }

  async function check() {
    setPhase("checking");
    setStatus(undefined);
    try {
      const candidate = await tauriInvoke<Release | null>("check_update");
      if (candidate) setRelease(candidate);
      else setStatus("current");
    } catch (reason) {
      report(reason);
    } finally {
      setPhase("idle");
    }
  }

  async function install() {
    if (!release) return;
    setPhase("installing");
    setStatus(undefined);
    setProgress(undefined);
    started.current = performance.now();
    const channel = new Channel<Progress>();
    channel.onmessage = setProgress;
    try {
      await tauriInvoke("install_update", { version: release.version, progress: channel });
    } catch (reason) {
      report(reason);
      setPhase("idle");
    }
  }

  if (!version) return null;
  const installing = phase === "installing";
  const speed = progress ? size(progress.received / Math.max(1, (performance.now() - started.current) / 1000)) + "/s" : "";

  return <section className="settings-group mt-5" aria-labelledby="updates-title">
    <h3 id="updates-title">{t.title} · {version.version}</h3>
    {version.available
      ? <button type="button" className="button button-subtle mt-3" disabled={phase !== "idle"}
          onClick={() => void check()}>{phase === "checking" ? t.checking : t.check}</button>
      : <p className="mt-2 text-sm">{t.manual}</p>}
    {status && !release && <p role="status" className="mt-3 text-sm">{t[status]}</p>}
    <dialog ref={dialog} aria-labelledby="update-dialog-title" className="dialog-panel dialog-small m-auto backdrop:bg-black/65"
      onClose={() => setRelease(undefined)}
      onCancel={(event) => { if (installing) event.preventDefault(); }}>
      {release && <>
        <h2 id="update-dialog-title">AUTHENTICATOR DESK · {t.offer}</h2>
        <p className="mt-3">{version.version} → {release.version} · {release.date} · {size(release.bytes)}</p>
        <details className="mt-3">
          <summary>{t.notes}</summary>
          <div className="mt-2 max-h-44 overflow-auto whitespace-pre-wrap text-sm">{release.notes}</div>
        </details>
        {installing && <div className="mt-4" role="status">
          <p>{progress && progress.received === progress.total ? t.verifying : t.downloading}</p>
          <progress aria-label={t.downloading} className="w-full" value={progress?.received} max={release.bytes} />
          {progress && <p className="text-sm">{size(progress.received)} / {size(release.bytes)} · {speed}</p>}
        </div>}
        {status && <p role="alert" className="mt-3 text-sm">{t[status]}</p>}
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button type="button" className="button button-subtle" autoFocus
            onClick={() => dialog.current?.close()} disabled={installing}>{t.later}</button>
          <button type="button" className="button button-primary" onClick={() => void install()} disabled={installing}>{t.install}</button>
        </div>
      </>}
    </dialog>
  </section>;
}
