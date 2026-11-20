import { useCallback, useEffect, useRef, useState } from "react";
import { getMessages, type Locale } from "../../i18n";
import { desktopCommands, type DesktopCapabilities, type DesktopConfig } from "../../lib/tauri";

type LoadState = "loading" | "ready" | "error";
const isVaultLocked = (error: unknown) => (error as { code?: string } | null)?.code === "vault_locked";

export function DesktopSettings({ locale, onError, notify }: {
  locale: Locale;
  onError: (error: unknown) => void;
  notify: (message: string) => void;
}) {
  const t = getMessages(locale);
  const localeRef = useRef(locale);
  localeRef.current = locale;
  const [config, setConfig] = useState<DesktopConfig | null>(null);
  const [savedNotifications, setSavedNotifications] = useState<boolean | null>(null);
  const [capabilities, setCapabilities] = useState<DesktopCapabilities | null>(null);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [showLoadError, setShowLoadError] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testingNotification, setTestingNotification] = useState(false);
  const [actionError, setActionError] = useState(false);
  const request = useRef(0);
  const loadRequest = useRef<number | null>(null);
  const savePending = useRef(false);
  const notificationPending = useRef(false);
  const mounted = useRef(true);

  const load = useCallback(async () => {
    if (loadRequest.current !== null) return;
    const currentRequest = ++request.current;
    loadRequest.current = currentRequest;
    setLoadState("loading");
    setShowLoadError(false);
    setActionError(false);
    try {
      const [currentConfig, currentCapabilities] = await Promise.all([
        desktopCommands.config(),
        desktopCommands.capabilities(),
      ]);
      if (!mounted.current || currentRequest !== request.current) return;
      setConfig(currentConfig);
      setSavedNotifications(currentConfig.notifications);
      setCapabilities(currentCapabilities);
      setLoadState("ready");
    } catch (error) {
      if (!mounted.current || currentRequest !== request.current) return;
      setLoadState("error");
      if (isVaultLocked(error)) onError(error);
      else setShowLoadError(true);
    } finally {
      if (loadRequest.current === currentRequest) loadRequest.current = null;
    }
  }, [onError]);

  useEffect(() => {
    mounted.current = true;
    void load();
    return () => {
      mounted.current = false;
      request.current++;
      loadRequest.current = null;
    };
  }, [load]);

  async function save() {
    if (!config || savePending.current || notificationPending.current || loadRequest.current !== null) return;
    savePending.current = true;
    const updated = { ...config };
    setSaving(true);
    setActionError(false);
    try {
      await desktopCommands.saveConfig(updated);
      if (!mounted.current) return;
      setConfig(updated);
      setSavedNotifications(updated.notifications);
      setCapabilities((current) => current ? { ...current, startup: updated.startup } : current);
      notify(getMessages(localeRef.current).status.saved);
    } catch (error) {
      if (!mounted.current) return;
      if (isVaultLocked(error)) onError(error);
      else setActionError(true);
    } finally {
      savePending.current = false;
      if (mounted.current) setSaving(false);
    }
  }

  async function testNotification() {
    if (!config?.notifications || savedNotifications !== true || !capabilities?.notifications ||
      savePending.current || notificationPending.current || loadRequest.current !== null) return;
    notificationPending.current = true;
    setTestingNotification(true);
    setActionError(false);
    try {
      await desktopCommands.testNotification();
      if (mounted.current) notify(getMessages(localeRef.current).desktop.notificationSent);
    } catch (error) {
      if (!mounted.current) return;
      if (isVaultLocked(error)) onError(error);
      else setActionError(true);
    } finally {
      notificationPending.current = false;
      if (mounted.current) setTestingNotification(false);
    }
  }

  function update<K extends keyof DesktopConfig>(key: K, value: DesktopConfig[K]) {
    setActionError(false);
    setConfig((current) => current ? { ...current, [key]: value } : current);
  }

  return <section className="settings-group" aria-labelledby="desktop-settings-title">
    <h3 id="desktop-settings-title">{t.desktop.title}</h3>
    {loadState === "loading" && <p role="status">{t.desktop.loading}</p>}
    {loadState === "error" && showLoadError && <div className="inline-error" role="alert">
      <p>{t.desktop.loadError}</p>
      <button type="button" className="button button-subtle" disabled={loadRequest.current !== null} onClick={() => void load()}>{t.desktop.retry}</button>
    </div>}
    {loadState === "ready" && config && capabilities && <>
      <fieldset className="settings-form-fieldset" disabled={saving || testingNotification} aria-label={t.desktop.title} aria-busy={saving || testingNotification}>
        <div className="desktop-settings-options">
          <label className="toggle-row"><span>{t.desktop.closeToTray}</span><input type="checkbox" checked={config.close_to_tray} onChange={(event) => update("close_to_tray", event.target.checked)} /></label>
          <label className="toggle-row"><span>{t.desktop.minimizeToTray}</span><input type="checkbox" checked={config.minimize_to_tray} onChange={(event) => update("minimize_to_tray", event.target.checked)} /></label>
          <label className="toggle-row"><span>{t.desktop.startup}</span><input type="checkbox" checked={config.startup} onChange={(event) => update("startup", event.target.checked)} /></label>
          <label className="toggle-row"><span>{t.desktop.notifications}</span><input type="checkbox" checked={config.notifications} onChange={(event) => update("notifications", event.target.checked)} /></label>
        </div>
        <div className="setting-grid">
          <label>{t.desktop.showShortcut}<input autoComplete="off" maxLength={100} value={config.show_shortcut} onChange={(event) => update("show_shortcut", event.target.value)} /></label>
          <label>{t.desktop.quickPanelShortcut}<input autoComplete="off" maxLength={100} value={config.quick_panel_shortcut} onChange={(event) => update("quick_panel_shortcut", event.target.value)} /></label>
          <label>{t.desktop.lockShortcut}<input autoComplete="off" maxLength={100} value={config.lock_shortcut} onChange={(event) => update("lock_shortcut", event.target.value)} /></label>
        </div>
        <p className="backup-hint">{t.desktop.shortcutHint}</p>
        <div className="desktop-capabilities" aria-label={t.desktop.capabilities}>
          <Capability label={t.desktop.tray} value={capabilities.tray ? t.desktop.available : t.desktop.unavailable} ready={capabilities.tray} />
          <Capability label={t.desktop.shortcuts} value={capabilities.shortcuts ? t.desktop.available : t.desktop.unavailable} ready={capabilities.shortcuts} />
          <Capability label={t.desktop.startupStatus} value={config.startup ? t.desktop.enabled : t.desktop.disabled} ready={config.startup} />
          <Capability label={t.desktop.notificationPermission} value={capabilities.notifications ? t.desktop.available : t.desktop.unavailable} ready={capabilities.notifications} />
          <Capability label={t.desktop.clipboard} value={capabilities.clipboard ? t.desktop.available : t.desktop.unavailable} ready={capabilities.clipboard} />
          <Capability label={t.desktop.qrImage} value={capabilities.qr_image ? t.desktop.available : t.desktop.unavailable} ready={capabilities.qr_image} />
          <Capability label={t.desktop.camera} value={capabilities.camera ? t.desktop.available : t.desktop.unavailable} ready={capabilities.camera} />
        </div>
        <div className="backup-actions">
          <button type="button" className="button button-subtle" disabled={!config.notifications || savedNotifications !== config.notifications || !capabilities.notifications || saving || testingNotification} onClick={() => void testNotification()}>
            {testingNotification ? t.pages.loading : t.desktop.testNotification}
          </button>
          <button type="button" className="button button-primary" disabled={saving} onClick={() => void save()}>
            {saving ? t.pages.loading : t.desktop.save}
          </button>
        </div>
      </fieldset>
      {actionError && <p className="inline-error" role="alert">{t.status.error}</p>}
    </>}
  </section>;
}

function Capability({ label, value, ready }: {
  label: string;
  value: string;
  ready: boolean;
}) {
  return <div className="capability-inline">
    <span>{label}</span>
    <span className="desktop-capability-state" data-ready={ready}>{value}</span>
  </div>;
}
