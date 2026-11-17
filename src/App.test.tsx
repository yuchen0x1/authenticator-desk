import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import App from "./App";

describe("locked application shell", () => {
  it("identifies the unavailable desktop backend without inventing vault data", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "AUTHENTICATOR DESK" })).toBeVisible();
    expect(screen.getByRole("navigation", { name: "主导航" })).toBeVisible();
    expect(await screen.findByText("桌面安全后端不可用")).toBeVisible();
    expect(screen.getByText("请在 AUTHENTICATOR DESK 桌面应用中打开此界面。")).toBeVisible();
    expect(screen.queryAllByRole("textbox")).toHaveLength(0);
  });
});
