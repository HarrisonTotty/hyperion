import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { RenderStyle } from "../../view/camera/state";
import { StyleControl } from "./StyleControl";

const BOTH = { wireframe: true, photorealistic: true } as const;
const SOFTWARE = { wireframe: true, photorealistic: false } as const;

describe("StyleControl", () => {
  it("shows the view's style as the pressed button", () => {
    render(<StyleControl renderStyle="wireframe" availability={BOTH} onStyle={() => undefined} />);
    expect(screen.getByRole("button", { name: "WIREFRAME" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "PHOTOREALISTIC" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("asks for the photorealistic style when it is offered", async () => {
    const user = userEvent.setup();
    const onStyle = vi.fn<(style: RenderStyle) => void>();
    render(<StyleControl renderStyle="wireframe" availability={BOTH} onStyle={onStyle} />);
    await user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    expect(onStyle).toHaveBeenCalledWith("photorealistic");
  });

  it("holds the photorealistic style back on a software adapter and says why", async () => {
    const user = userEvent.setup();
    const onStyle = vi.fn<(style: RenderStyle) => void>();
    render(<StyleControl renderStyle="wireframe" availability={SOFTWARE} onStyle={onStyle} />);
    const photorealistic = screen.getByRole("button", { name: "PHOTOREALISTIC" });
    expect(photorealistic).toHaveAttribute("aria-disabled", "true");
    expect(photorealistic).toHaveAccessibleDescription(
      "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
    );
    await user.click(photorealistic);
    expect(onStyle).not.toHaveBeenCalled();
  });

  it("shows the key that toggles the style on the group", () => {
    render(<StyleControl renderStyle="wireframe" availability={BOTH} onStyle={() => undefined} />);
    expect(screen.getByRole("group", { name: "4 STYLE" })).toHaveAttribute(
      "aria-keyshortcuts",
      "4",
    );
  });
});
