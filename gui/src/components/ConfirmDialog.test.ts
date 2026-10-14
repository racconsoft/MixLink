import { fireEvent, render, screen } from "@testing-library/svelte";
import ConfirmDialog from "./ConfirmDialog.svelte";

describe("ConfirmDialog", () => {
  it("confirm and cancel call their handlers", async () => {
    const onconfirm = vi.fn();
    const oncancel = vi.fn();
    render(ConfirmDialog, { message: "Really?", confirmLabel: "Delete", onconfirm, oncancel });
    expect(screen.getByText("Really?")).toBeInTheDocument();
    await fireEvent.click(screen.getByText("Delete"));
    expect(onconfirm).toHaveBeenCalled();
    await fireEvent.click(screen.getByText("Cancel"));
    expect(oncancel).toHaveBeenCalled();
  });

  it("Escape calls oncancel", async () => {
    const oncancel = vi.fn();
    render(ConfirmDialog, { message: "Really?", confirmLabel: "Delete", onconfirm: vi.fn(), oncancel });
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(oncancel).toHaveBeenCalled();
  });
});
