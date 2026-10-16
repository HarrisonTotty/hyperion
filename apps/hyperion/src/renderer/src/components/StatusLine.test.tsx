import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatusLine } from "./StatusLine";

describe("StatusLine", () => {
  it("sets a state and its sentence as two parts inside its one output", () => {
    render(<StatusLine text="REJECTED: the interactive queue is full" standing="fault" />);

    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("REJECTED: the interactive queue is full");
    const state = screen.getByText("REJECTED:");
    const sentence = screen.getByText("the interactive queue is full");
    expect(state).toHaveClass("status-text__state");
    expect(sentence).toHaveClass("status-text__sentence");
    expect(state.parentElement).toBe(status);
    expect(sentence.parentElement).toBe(status);
    // The standing's colour is the line's, across both parts.
    expect(status).toHaveClass("request-status__text--fault");
  });
});
