import { render, screen } from "@testing-library/svelte";
import AppIcon from "./AppIcon.svelte";
import { initBackend } from "../lib/stores";
import { createMock } from "../lib/mock";

describe("AppIcon", () => {
  it("falls back to a letter avatar when no icon is found", async () => {
    initBackend(createMock());
    render(AppIcon, { entry: { app: "pw-play", name: "pw-play", iconName: null, running: true } });
    expect(await screen.findByText("P")).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("renders the icon image when one is found", async () => {
    const mock = createMock();
    mock.appIcon = async () => "data:image/svg+xml;base64,PHN2Zy8+";
    initBackend(mock);
    render(AppIcon, { entry: { app: "firefox", name: "Firefox", iconName: null, running: false } });
    const img = await screen.findByRole("img", { name: "Firefox" });
    expect(img).toHaveAttribute("src", "data:image/svg+xml;base64,PHN2Zy8+");
    expect(img.closest(".app")).toHaveClass("dim");
  });
});
