import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { announcements } from "./liveRegions";

/** Replaces the text of the element showing `text`, as a reading's update does. */
function retext(text: string, next: string): void {
  const element = screen.getByText(text);
  const node = element.firstChild;
  if (node === null) {
    throw new Error(`${text} has no text node`);
  }
  node.nodeValue = next;
}

/** Lets the observer take the changes made so far, as the end of a task does. */
async function nextUpdate(): Promise<void> {
  await Promise.resolve();
}

describe("announcements", () => {
  it("counts a region once however many of its nodes change in one update", async () => {
    render(
      <output>
        <span>A1</span> <span>B1</span>
      </output>,
    );

    const announced = await announcements(() => {
      retext("A1", "A2");
      retext("B1", "B2");
      return Promise.resolve();
    });

    expect(announced).toEqual([screen.getByRole("status")]);
  });

  it("counts two regions that change in one update as two announcements", async () => {
    render(
      <>
        <p aria-live="polite">A1</p>
        <p aria-live="polite">B1</p>
      </>,
    );

    const announced = await announcements(() => {
      retext("A1", "A2");
      retext("B1", "B2");
      return Promise.resolve();
    });

    expect(announced).toEqual([screen.getByText("A2"), screen.getByText("B2")]);
  });

  it("counts a region twice when it changes in two updates of one change", async () => {
    render(<p aria-live="polite">A1</p>);

    const announced = await announcements(async () => {
      retext("A1", "A2");
      await nextUpdate();
      retext("A2", "A3");
    });

    const region = screen.getByText("A3");
    expect(announced).toEqual([region, region]);
  });

  it("counts an output as a status without a role", async () => {
    render(<output>A1</output>);

    const announced = await announcements(() => {
      retext("A1", "A2");
      return Promise.resolve();
    });

    expect(announced).toEqual([screen.getByRole("status")]);
  });

  it("counts nothing from inside a region turned off, though a region holds it", async () => {
    render(
      <p aria-live="polite">
        <output aria-live="off">A1</output>
      </p>,
    );

    const announced = await announcements(() => {
      retext("A1", "A2");
      return Promise.resolve();
    });

    expect(announced).toEqual([]);
  });

  it.each([
    ["hidden", { hidden: true }],
    ["aria-hidden", { "aria-hidden": true }],
    ["inert", { inert: true }],
    ["not displayed, as Activity hides a display", { style: { display: "none" } }],
  ])("counts nothing from a region inside an element %s", async (_, hiding) => {
    render(
      <div {...hiding}>
        <p aria-live="polite">A1</p>
      </div>,
    );

    const announced = await announcements(() => {
      retext("A1", "A2");
      return Promise.resolve();
    });

    expect(announced).toEqual([]);
  });

  it("counts nothing for an element added hidden to a region", async () => {
    render(<output>A1</output>);

    const announced = await announcements(() => {
      const added = document.createElement("span");
      added.hidden = true;
      added.textContent = "B1";
      screen.getByRole("status").append(added);
      return Promise.resolve();
    });

    expect(announced).toEqual([]);
  });

  it("counts nothing for a removal alone, which a region does not announce by default", async () => {
    render(
      <output>
        <span>A1</span>
      </output>,
    );

    const announced = await announcements(() => {
      screen.getByText("A1").remove();
      return Promise.resolve();
    });

    expect(announced).toEqual([]);
  });

  it("counts a removal where the region asks for removals", async () => {
    render(
      <output aria-relevant="additions removals">
        <span>A1</span>
      </output>,
    );

    const announced = await announcements(() => {
      screen.getByText("A1").remove();
      return Promise.resolve();
    });

    expect(announced).toEqual([screen.getByRole("status")]);
  });

  it("counts a removal where the region asks for all changes", async () => {
    render(
      <output aria-relevant="all">
        <span>A1</span>
      </output>,
    );

    const announced = await announcements(() => {
      screen.getByText("A1").remove();
      return Promise.resolve();
    });

    expect(announced).toEqual([screen.getByRole("status")]);
  });

  it("counts nothing from a region removed before the update was delivered", async () => {
    render(
      <div>
        <p aria-live="polite">A1</p>
      </div>,
    );

    const announced = await announcements(() => {
      const region = screen.getByText("A1");
      retext("A1", "A2");
      region.remove();
      return Promise.resolve();
    });

    expect(announced).toEqual([]);
  });
});
