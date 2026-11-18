import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ApplicationRoot } from "./ApplicationRoot";

vi.mock("./App", () => ({ default: () => <main>Main workspace</main> }));
vi.mock("./components/desktop/QuickPanel", () => ({ QuickPanel: () => <main>Favorite quick panel</main> }));

describe("application window routing", () => {
  it("switches between the main app and the independent quick panel", () => {
    const { rerender } = render(<ApplicationRoot search="" />);

    expect(screen.getByText("Main workspace")).toBeVisible();
    rerender(<ApplicationRoot search="?quick-panel=1" />);
    expect(screen.getByText("Favorite quick panel")).toBeVisible();
  });
});
