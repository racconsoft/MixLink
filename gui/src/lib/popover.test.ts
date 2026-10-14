import { anchored } from "./popover";

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) } as DOMRect;
}

describe("anchored popovers", () => {
  it("are fixed below the trigger, aligned left or right and kept inside the viewport", () => {
    const trigger = document.createElement("div");
    trigger.getBoundingClientRect = () => rect(100, 50, 80, 20);
    const pop = document.createElement("div");
    Object.defineProperty(pop, "offsetWidth", { value: 160 });

    const left = anchored(pop, { trigger, align: "left" });
    expect(pop.style.position).toBe("fixed");
    expect(pop.style.top).toBe("74px");
    expect(pop.style.left).toBe("100px");

    left.update({ trigger, align: "right" });
    expect(pop.style.left).toBe("20px"); // right edges line up: 180 - 160

    trigger.getBoundingClientRect = () => rect(-50, 50, 20, 20);
    window.dispatchEvent(new Event("scroll"));
    expect(pop.style.left).toBe("8px"); // follows scrolling, clamped to the margin
    left.destroy();
  });
});
