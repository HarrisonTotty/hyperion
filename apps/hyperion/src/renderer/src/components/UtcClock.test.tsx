import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { UtcClock } from "./UtcClock";

describe("UtcClock", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2350-03-04T14:08:33.500Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows 24-hour UTC time labelled with its time system", () => {
    render(<UtcClock />);

    expect(screen.getByText("UTC")).toBeInTheDocument();
    expect(screen.getByText("14:08:33")).toBeInTheDocument();
  });

  it("advances with the clock", () => {
    render(<UtcClock />);

    act(() => {
      vi.advanceTimersByTime(27_000);
    });

    expect(screen.getByText("14:09:00")).toBeInTheDocument();
  });
});
