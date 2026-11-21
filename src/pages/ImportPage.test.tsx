import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceProps } from "../App";
import { getMessages } from "../i18n";
import type { ImportPreview } from "../lib/types";
import { ImportPage } from "./ImportPage";

const api = vi.hoisted(() => ({
  preview: vi.fn(), pickImportFile: vi.fn(), pickQrImage: vi.fn(), commit: vi.fn(), cancel: vi.fn(),
}));
vi.mock("../lib/tauri", () => ({
  importCommands: { preview: api.preview, commit: api.commit, cancel: api.cancel },
  desktopCommands: { pickImportFile: api.pickImportFile, pickQrImage: api.pickQrImage },
}));

const t = getMessages("en");
const settings: WorkspaceProps["settings"] = {
  locale: "en", theme: "dark", idle_timeout_secs: 900, clipboard_ttl_secs: 30, hide_codes: false,
  view_mode: "cards", animations: false, compact: false, font_size: 14, default_algorithm: "SHA1",
  default_digits: 6, default_period_secs: 30, close_to_tray: false, minimize_to_tray: false,
  startup: false, backup_enabled: false, backup_interval_hours: 24, backup_retention: 7, show_next_code: false,
};

function workspace(): WorkspaceProps {
  return {
    locale: "en", accounts: [], categories: [], codes: [], settings,
    refresh: vi.fn(async () => {}), notify: vi.fn(), onError: vi.fn(), onAdd: vi.fn(), onEdit: vi.fn(),
    navigate: vi.fn(), saveSettings: vi.fn(async () => {}),
  };
}

function importPreview(id = "preview-1"): ImportPreview {
  return {
    preview_id: id, format: "detected_format",
    items: [
      { index: 2, issuer: id, account_name: "new-account", status: "new", error_code: null },
      { index: 3, issuer: "Duplicate", account_name: "duplicate-account", status: "duplicate", error_code: null },
      { index: 4, issuer: null, account_name: null, status: "invalid", error_code: "invalid_otpauth" },
    ],
  };
}

beforeEach(() => vi.resetAllMocks());

describe("import preview sources", () => {
  it.each(["paste", "file", "qr"] as const)("previews %s input and selects only new entries", async (source) => {
    const load = source === "paste" ? api.preview : source === "file" ? api.pickImportFile : api.pickQrImage;
    load.mockResolvedValue(importPreview());
    render(<ImportPage {...workspace()} />);
    const text = screen.getByRole("textbox", { name: t.fields.secret });
    const sample = "otpauth-migration://offline?data=sample";

    if (source === "paste") {
      fireEvent.change(text, { target: { value: sample } });
      fireEvent.click(screen.getByRole("button", { name: t.actions.preview }));
      expect(load).toHaveBeenCalledExactlyOnceWith(sample);
    } else {
      const name = source === "file" ? t.actions.chooseFile : t.actions.chooseQr;
      fireEvent.click(screen.getByRole("button", { name }));
      expect(load).toHaveBeenCalledExactlyOnceWith();
    }

    await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    const [newItem, duplicate, invalid] = screen.getAllByRole("checkbox");
    expect(newItem).toBeChecked();
    expect(duplicate).toBeDisabled();
    expect(invalid).toBeDisabled();
    if (source === "paste") expect(text).toHaveValue("");
  });

  it.each([
    ["en", "Steam tokens are not supported", "Unsupported number of code digits"],
    ["zh-CN", "不支持 Steam 令牌", "不支持的验证码位数"],
    ["zh-TW", "不支援 Steam 權杖", "不支援的驗證碼位數"],
    ["ja", "Steam トークンには対応していません", "未対応のコード桁数"],
  ] as const)("shows named invalid rows' translated reasons in %s without changing rows or selections", async (locale, reason, digitsReason) => {
    const props = workspace();
    const preview = importPreview("aegis-preview");
    preview.format = "aegis";
    preview.items.push(
      { index: 5, issuer: "Steam", account_name: "named-steam", status: "invalid", error_code: "unsupported_steam" },
      { index: 6, issuer: "Google", account_name: "named-digits", status: "invalid", error_code: "unsupported_digits" },
      { index: 8, issuer: "Other", account_name: "second-new-account", status: "new", error_code: null },
    );
    api.pickImportFile.mockResolvedValue(preview);
    api.commit.mockResolvedValue(1);
    const view = render(<ImportPage {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    await screen.findByRole("button", { name: `${t.actions.confirmImport} (2)` });
    fireEvent.click(screen.getAllByRole("checkbox")[0]);

    view.rerender(<ImportPage {...props} locale={locale} />);
    const messages = getMessages(locale);
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(preview.items.length);
    const invalidRow = rows.find((row) => row.textContent?.includes("named-steam"))!;
    expect(within(invalidRow).getByText("named-steam")).toBeVisible();
    expect(within(invalidRow).getByText(reason)).toBeVisible();
    expect(within(invalidRow).getByRole("checkbox")).toBeDisabled();
    expect(within(invalidRow).getByRole("checkbox")).not.toBeChecked();
    const digitsRow = rows.find((row) => row.textContent?.includes("named-digits"))!;
    expect(within(digitsRow).getByText("named-digits")).toBeVisible();
    expect(within(digitsRow).getByText(digitsReason)).toBeVisible();
    expect(within(digitsRow).getByRole("checkbox")).toBeDisabled();
    expect(within(digitsRow).getByRole("checkbox")).not.toBeChecked();
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes.map((checkbox) => (checkbox as HTMLInputElement).checked)).toEqual([false, false, false, false, false, true]);
    expect(checkboxes[1]).toBeDisabled();
    expect(checkboxes[2]).toBeDisabled();
    expect(api.pickImportFile).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: `${messages.actions.confirmImport} (1)` }));
    await waitFor(() => expect(api.commit).toHaveBeenCalledExactlyOnceWith("aegis-preview", [8]));
  });

  it("leaves a preview intact when the native picker is cancelled, then cancels it on dismiss", async () => {
    api.pickImportFile.mockResolvedValueOnce(importPreview("active-file")).mockResolvedValueOnce(null);
    const props = workspace();
    render(<ImportPage {...props} />);
    const fileButton = screen.getByRole("button", { name: t.actions.chooseFile });

    fireEvent.click(fileButton);
    await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    fireEvent.click(fileButton);
    await waitFor(() => expect(fileButton).toBeEnabled());

    expect(api.cancel).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox", { name: /active-file/ })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: t.actions.cancel }));
    expect(api.cancel).toHaveBeenCalledExactlyOnceWith("active-file");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("cancels a replaced preview as well as the dismissed current preview", async () => {
    api.pickImportFile.mockResolvedValue(importPreview("file-preview"));
    api.pickQrImage.mockResolvedValue(importPreview("qr-preview"));
    render(<ImportPage {...workspace()} />);

    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    await screen.findByText("file-preview");
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseQr }));
    await screen.findByText("qr-preview");
    await waitFor(() => expect(api.cancel).toHaveBeenCalledExactlyOnceWith("file-preview"));

    fireEvent.click(screen.getByRole("button", { name: t.actions.cancel }));
    expect(api.cancel).toHaveBeenLastCalledWith("qr-preview");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("drops a review whose commit failed because the backend consumed its id", async () => {
    const props = workspace();
    const commitError = { code: "conflict" };
    api.pickImportFile.mockResolvedValue(importPreview("stale-preview"));
    api.commit.mockRejectedValueOnce(commitError);
    render(<ImportPage {...props} />);

    const source = screen.getByRole("button", { name: t.actions.chooseFile });
    fireEvent.click(source);
    const commit = await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    fireEvent.click(commit);
    await screen.findByRole("status");

    // commit_import consumes the preview id before validating, so a failed
    // request is shown inline and does not also raise a duplicate global alert.
    expect(props.onError).not.toHaveBeenCalled();

    // commit_import takes the pending slot before validating, so the same id can
    // never succeed again; the review must not stay enabled as a dead action.
    expect(screen.queryByRole("button", { name: new RegExp(t.actions.confirmImport) })).not.toBeInTheDocument();
    expect(screen.queryByText("stale-preview")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("The import review expired");
    expect(api.commit).toHaveBeenCalledTimes(1);
    expect(props.notify).not.toHaveBeenCalled();
  });

  it("keeps a successful import successful when the follow-up refresh fails", async () => {
    const props = workspace();
    props.refresh = vi.fn().mockRejectedValueOnce(new Error("refresh failed"));
    api.pickImportFile.mockResolvedValue(importPreview("committed-preview"));
    api.commit.mockResolvedValueOnce(1);
    render(<ImportPage {...props} />);

    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    fireEvent.click(await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` }));
    await waitFor(() => expect(props.notify).toHaveBeenCalledExactlyOnceWith("Imported: 1"));
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(expect.any(Error));
    expect(screen.queryByText("The import review expired")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: new RegExp(t.actions.confirmImport) })).not.toBeInTheDocument();
  });

  it("refreshes the shared account list when a commit succeeds after navigation", async () => {
    const props = workspace();
    let finish!: (count: number) => void;
    api.commit.mockImplementationOnce(() => new Promise<number>((resolve) => { finish = resolve; }));
    api.pickImportFile.mockResolvedValue(importPreview());
    const view = render(<ImportPage {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    fireEvent.click(await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` }));
    view.unmount();
    await act(async () => finish(1));
    expect(props.refresh).toHaveBeenCalledOnce();
    expect(props.notify).not.toHaveBeenCalled();
    expect(props.onError).not.toHaveBeenCalled();
  });

  it("cancels the active preview and a late native result on unmount", async () => {
    api.pickImportFile.mockResolvedValue(importPreview("active-file"));
    let resolveQr!: (value: ImportPreview) => void;
    api.pickQrImage.mockImplementation(
      () => new Promise<ImportPreview>((resolve) => { resolveQr = resolve; }),
    );
    const props = workspace();
    const view = render(<ImportPage {...props} />);

    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseQr }));
    await waitFor(() => expect(resolveQr).toBeDefined());

    view.unmount();
    expect(api.cancel).toHaveBeenCalledExactlyOnceWith("active-file");
    await act(async () => resolveQr(importPreview("late-qr-result")));

    expect(api.cancel).toHaveBeenLastCalledWith("late-qr-result");
    expect(props.onError).not.toHaveBeenCalled();
  });

  it("announces and focuses a preview, truncates long account names, and restores focus on dismiss", async () => {
    const longName = "account-" + "x".repeat(180);
    api.pickImportFile.mockResolvedValue({
      ...importPreview("accessible-preview"),
      items: [{ index: 0, issuer: "Issuer", account_name: longName, status: "new", error_code: null }],
    });
    render(<ImportPage {...workspace()} />);

    const source = screen.getByRole("button", { name: t.actions.chooseFile });
    fireEvent.click(source);
    const region = await screen.findByRole("region", { name: t.actions.preview });
    expect(document.activeElement).toBe(region);
    expect(screen.getByRole("status")).toHaveAttribute("aria-live", "polite");
    expect(screen.getByText(longName)).toHaveClass("truncate");

    fireEvent.click(screen.getByRole("button", { name: t.actions.cancel }));
    expect(document.activeElement).toBe(source);
  });
});
