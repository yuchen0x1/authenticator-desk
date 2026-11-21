import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceProps } from "../App";
import { getMessages } from "../i18n";
import type { AccountView, ImportPreview } from "../lib/types";
import { AccountsPage } from "./AccountsPage";
import { ImportPage } from "./ImportPage";
import { SettingsPage } from "./SettingsPage";

const api = vi.hoisted(() => ({
  move: vi.fn(), remove: vi.fn(), preview: vi.fn(), pickImportFile: vi.fn(),
  pickQrImage: vi.fn(), commit: vi.fn(), cancel: vi.fn(),
}));
vi.mock("../lib/tauri", () => ({
  accountCommands: { move: api.move, remove: api.remove },
  importCommands: { preview: api.preview, commit: api.commit, cancel: api.cancel },
  desktopCommands: { pickImportFile: api.pickImportFile, pickQrImage: api.pickQrImage },
}));
vi.mock("../components/backup/AutoBackupSettings", () => ({ AutoBackupSettings: () => null }));
vi.mock("../components/desktop/DesktopSettings", () => ({ DesktopSettings: () => null }));
vi.mock("../components/desktop/UpdateSettings", () => ({ UpdateSettings: () => null }));

const t = getMessages("en");
const alpha: AccountView = {
  id: "alpha", issuer: "Alpha", account_name: "test-alpha", category_id: "work", tags: ["work"],
  favorite: true, algorithm: "SHA1", digits: 6, kind: "totp", period_secs: 30, t0: "0",
  hotp_counter: "0", notes_present: false, sort_order: 0, created_at: "", updated_at: "",
};
const beta: AccountView = { ...alpha, id: "beta", issuer: "Beta", account_name: "test-beta", category_id: null, tags: [], favorite: false };

function workspace(): WorkspaceProps {
  return {
    locale: "en", accounts: [alpha, beta], categories: [{ id: "work", name: "Work", color: "#111111" }], codes: [],
    settings: {
      locale: "en", theme: "dark", idle_timeout_secs: 900, clipboard_ttl_secs: 30, hide_codes: false,
      view_mode: "cards", animations: false, compact: false, font_size: 14, default_algorithm: "SHA1",
      default_digits: 6, default_period_secs: 30, close_to_tray: false, minimize_to_tray: false,
      startup: false, backup_enabled: false, backup_interval_hours: 24, backup_retention: 7, show_next_code: false,
    },
    refresh: vi.fn(async () => {}), notify: vi.fn(), onError: vi.fn(), onAdd: vi.fn(), onEdit: vi.fn(),
    navigate: vi.fn(), saveSettings: vi.fn(async () => {}),
  };
}

beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.restoreAllMocks());

function selectBoth() {
  fireEvent.click(screen.getByRole("checkbox", { name: `${t.status.selected}: ${alpha.issuer} · ${alpha.account_name}` }));
  fireEvent.click(screen.getByRole("checkbox", { name: `${t.status.selected}: ${beta.issuer} · ${beta.account_name}` }));
  expect(screen.getByText(`2 ${t.status.selected}`)).toBeVisible();
}

describe("visible account bulk selection", () => {
  it.each(["search", "category", "tag", "favorites", "refresh"])("excludes IDs hidden by %s from confirmation and delete IPC", async (filter) => {
    const props = workspace();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { rerender } = render(<AccountsPage {...props} mode="accounts" />);
    selectBoth();

    if (filter === "search") fireEvent.change(screen.getByRole("textbox", { name: t.fields.search }), { target: { value: "alpha" } });
    if (filter === "category") fireEvent.change(screen.getByRole("combobox", { name: t.fields.category }), { target: { value: "work" } });
    if (filter === "tag") fireEvent.change(screen.getByRole("combobox", { name: t.fields.tags }), { target: { value: "work" } });
    if (filter === "favorites") rerender(<AccountsPage {...props} mode="favorites" />);
    if (filter === "refresh") rerender(<AccountsPage {...props} mode="accounts" accounts={[alpha]} />);

    expect(screen.getByText(`1 ${t.status.selected}`)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: t.actions.deleteSelected }));
    expect(confirm).toHaveBeenCalledWith(`${t.actions.deleteSelected}: 1`);
    expect(api.remove).toHaveBeenCalledExactlyOnceWith([alpha.id]);
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });

  it("only moves visible IDs, including when moving to Uncategorized", async () => {
    const props = workspace();
    render(<AccountsPage {...props} mode="accounts" />);
    selectBoth();
    fireEvent.change(screen.getByRole("textbox", { name: t.fields.search }), { target: { value: "alpha" } });
    const category = screen.getByRole("combobox", { name: t.actions.move });
    const uncategorized = screen.getByRole<HTMLOptionElement>("option", { name: t.fields.uncategorized });
    fireEvent.change(category, { target: { value: uncategorized.value } });
    expect(api.move).toHaveBeenCalledExactlyOnceWith([alpha.id], null);
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });

  it("removes the bulk bar if all selections are hidden and doesn't restore them", () => {
    render(<AccountsPage {...workspace()} mode="accounts" />);
    selectBoth();
    const search = screen.getByRole("textbox", { name: t.fields.search });
    fireEvent.change(search, { target: { value: "no matching accounts" } });
    expect(screen.queryByRole("button", { name: t.actions.deleteSelected })).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: t.actions.move })).not.toBeInTheDocument();
    fireEvent.change(search, { target: { value: "" } });
    expect(screen.getByRole("checkbox", { name: `${t.status.selected}: ${alpha.issuer} · ${alpha.account_name}` })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: `${t.status.selected}: ${beta.issuer} · ${beta.account_name}` })).not.toBeChecked();
    expect(api.remove).not.toHaveBeenCalled();
    expect(api.move).not.toHaveBeenCalled();
  });
});

describe("security settings numeric draft", () => {
  it.each([
    [t.fields.idleTimeout, ""], [t.fields.idleTimeout, "0"], [t.fields.idleTimeout, "29"], [t.fields.idleTimeout, "86401"],
    [t.fields.clipboardTtl, ""], [t.fields.clipboardTtl, "0"], [t.fields.clipboardTtl, "invalid"], [t.fields.clipboardTtl, "0.5"], [t.fields.clipboardTtl, "301"],
    [t.settings.defaultPeriod, ""], [t.settings.defaultPeriod, "86401"],
  ])("rejects %s with input '%s' even if native form checks are bypassed", (name, value) => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    const input = screen.getByRole<HTMLInputElement>("spinbutton", { name });
    fireEvent.change(input, { target: { value } });
    fireEvent.submit(input.form!);
    expect(props.saveSettings).not.toHaveBeenCalled();
    expect(props.onError).toHaveBeenCalledWith({ code: "invalid_input" });
    expect(input.value).toBe(value === "invalid" ? "" : value);
  });

  it.each([0, 30, 86400])("saves supported idle value %i and only disables on explicit zero", (idle) => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    const idleInput = screen.getByRole("spinbutton", { name: t.fields.idleTimeout });
    const clipboardInput = screen.getByRole("spinbutton", { name: t.fields.clipboardTtl });
    const periodInput = screen.getByRole("spinbutton", { name: t.settings.defaultPeriod });
    expect(idleInput).toHaveAttribute("max", "86400");
    expect(clipboardInput).toHaveAttribute("max", "300");
    expect(periodInput).toHaveAttribute("max", "86400");
    if(idle===0){
      fireEvent.click(screen.getByRole("checkbox",{name:"Disable automatic locking"}));
      fireEvent.click(screen.getByRole("checkbox",{name:"Disable clipboard cleanup"}));
    }else{
      fireEvent.change(idleInput, { target: { value: String(idle) } });
      fireEvent.change(clipboardInput, { target: { value: "300" } });
    }
    fireEvent.change(periodInput, { target: { value: "86400" } });
    fireEvent.click(screen.getByRole("button", { name: t.actions.save }));
    expect(props.saveSettings).toHaveBeenCalledExactlyOnceWith({
      ...props.settings, idle_timeout_secs: idle, clipboard_ttl_secs: idle ? 300 : 0, default_period_secs: 86400,
    });
    expect(props.onError).not.toHaveBeenCalled();
  });
});

function importPreview(id = "preview-1"): ImportPreview {
  return {
    preview_id: id, format: "detected_format",
    items: [
      { index: 2, issuer: "New", account_name: "example", status: "new", error_code: null },
      { index: 3, issuer: "Duplicate", account_name: "example", status: "duplicate", error_code: null },
      { index: 4, issuer: null, account_name: null, status: "invalid", error_code: "invalid_otpauth" },
    ],
  };
}

describe("import preview sources", () => {
  it.each(["paste", "file", "qr"] as const)("previews and commits only new items from %s", async (source) => {
    const props = workspace();
    const load = source === "paste" ? api.preview : source === "file" ? api.pickImportFile : api.pickQrImage;
    load.mockResolvedValue(importPreview());
    api.commit.mockResolvedValue(1);
    render(<ImportPage {...props} />);
    const text = screen.getByRole("textbox", { name: t.fields.secret });
    const sample = "otpauth-migration://offline?data=sample";
    if (source === "paste") {
      fireEvent.change(text, { target: { value: sample } });
      fireEvent.click(screen.getByRole("button", { name: t.actions.preview }));
      expect(load).toHaveBeenCalledExactlyOnceWith(sample);
    } else {
      fireEvent.click(screen.getByRole("button", { name: source === "file" ? t.actions.chooseFile : t.actions.chooseQr }));
      expect(load).toHaveBeenCalledExactlyOnceWith();
    }
    const button = await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    const [newItem, duplicate, invalid] = screen.getAllByRole("checkbox");
    expect(newItem).toBeChecked();
    expect(duplicate).toBeDisabled();
    expect(invalid).toBeDisabled();
    expect(text).toHaveValue("");
    fireEvent.click(button);
    expect(api.commit).toHaveBeenCalledExactlyOnceWith("preview-1", [2]);
    await waitFor(() => expect(props.notify).toHaveBeenCalledWith(`${t.status.imported}: 1`));
    expect(props.refresh).toHaveBeenCalledOnce();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(api.cancel).not.toHaveBeenCalled();
  });

  it("delegates size rejection to the bounded native picker without opening a browser file reader", async () => {
    const props = workspace();
    const sizeError = { code: "invalid_input" };
    api.pickImportFile.mockRejectedValue(sizeError);
    const read = vi.spyOn(FileReader.prototype, "readAsDataURL");
    const { container } = render(<ImportPage {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    await waitFor(() => expect(props.onError).toHaveBeenCalledExactlyOnceWith(sizeError));
    expect(container.querySelector('input[type="file"]')).toBeNull();
    expect(read).not.toHaveBeenCalled();
    expect(api.preview).not.toHaveBeenCalled();
    expect(api.commit).not.toHaveBeenCalled();
  });

  it("retains the preview on picker cancellation, then cancels replaced and dismissed IDs", async () => {
    api.pickImportFile.mockResolvedValueOnce(importPreview("file")).mockResolvedValueOnce(null);
    api.pickQrImage.mockResolvedValue(importPreview("qr"));
    const props = workspace();
    render(<ImportPage {...props} />);
    const file = screen.getByRole("button", { name: t.actions.chooseFile });
    fireEvent.click(file);
    expect(await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` })).toBeEnabled();
    fireEvent.click(file);
    await waitFor(() => expect(file).toBeEnabled());
    expect(api.cancel).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: `${t.actions.confirmImport} (1)` })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseQr }));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledExactlyOnceWith("file"));
    fireEvent.click(screen.getByRole("button", { name: t.actions.cancel }));
    expect(api.cancel).toHaveBeenLastCalledWith("qr");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(props.onError).not.toHaveBeenCalled();
  });

  it("cancels both an active preview and a late native result on unmount", async () => {
    api.pickImportFile.mockResolvedValue(importPreview("file"));
    let finish!: (value: ImportPreview) => void;
    api.pickQrImage.mockReturnValue(new Promise<ImportPreview>((resolve) => { finish = resolve; }));
    const props = workspace();
    const { unmount } = render(<ImportPage {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseFile }));
    await screen.findByRole("button", { name: `${t.actions.confirmImport} (1)` });
    fireEvent.click(screen.getByRole("button", { name: t.actions.chooseQr }));
    unmount();
    expect(api.cancel).toHaveBeenCalledExactlyOnceWith("file");
    await act(async () => finish(importPreview("qr")));
    expect(api.cancel).toHaveBeenLastCalledWith("qr");
    expect(props.onError).not.toHaveBeenCalled();
  });
});
