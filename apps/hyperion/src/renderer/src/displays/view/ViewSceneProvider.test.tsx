/**
 * The `VIEW` display's scene host's choice with the body inspection (the owner's request,
 * 2026-11-01): each opening shown as it comes, and its closing back to the scene shown before.
 */
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { use } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { INSPECTION_SCENE_NAME } from "../../view/scenes/inspectionScene";
import { precisionScene } from "../../view/scenes/precision";
import { type InspectionOpening, ViewSceneContext, ViewSceneProvider } from "./ViewSceneProvider";

/** An opening of the body inspection, its scene a stand-in. */
function opening(sequence: number): InspectionOpening {
  return {
    option: { name: INSPECTION_SCENE_NAME, make: precisionScene },
    sequence,
    survey: Promise.resolve({ kind: "refused", body: "0200080020000000.0300", reason: "none" }),
  };
}

/** The host's choice, its inspection's count and its stand-in, and its choices as buttons. */
function Choice() {
  const host = use(ViewSceneContext);
  if (host === null) {
    throw new Error("the choice stands in the provider");
  }
  return (
    <>
      <output aria-label="Shown">{host.sceneName}</output>
      <output aria-label="Offered">{String(host.inspection?.sequence ?? "none")}</output>
      <output aria-label="Stand-in">{host.keptName}</output>
      {["SERVER", "PRECISION TEST", "FRAME CHANGE TEST", INSPECTION_SCENE_NAME].map((name) => (
        <button
          key={name}
          type="button"
          onClick={() => {
            host.choose(name);
          }}
        >
          {name}
        </button>
      ))}
      <button type="button" onClick={host.closeInspection}>
        CLOSE
      </button>
    </>
  );
}

/** The provider over the opening `latest`, in the providers it reads. */
function tree(latest: InspectionOpening | null) {
  return (
    <ServerLinkHarness>
      <UniverseProvider>
        <ViewSceneProvider active={false} knownSystem={null} inspection={latest}>
          {() => <Choice />}
        </ViewSceneProvider>
      </UniverseProvider>
    </ServerLinkHarness>
  );
}

/** The provider over `inspection`, rendered again with each later opening. */
function renderHost(inspection: InspectionOpening | null) {
  const view = render(tree(inspection));
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  return {
    user: userEvent.setup(),
    open: (next: InspectionOpening) => {
      view.rerender(tree(next));
    },
  };
}

function shown(): string | null {
  return screen.getByRole("status", { name: "Shown" }).textContent;
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("the scene host's body inspection (the owner's request)", () => {
  it("shows an opening at once, and closes back to the scene chosen before it", async () => {
    const { user, open } = renderHost(null);
    await user.click(screen.getByRole("button", { name: "PRECISION TEST" }));

    open(opening(1));
    expect(shown()).toBe(INSPECTION_SCENE_NAME);
    await user.click(screen.getByRole("button", { name: "CLOSE" }));

    expect(shown()).toBe("PRECISION TEST");
    expect(screen.getByRole("status", { name: "Offered" })).toHaveTextContent("none");
  });

  it("keeps the scene before when a new opening takes the shown one's place", async () => {
    const { user, open } = renderHost(opening(1));

    open(opening(2));
    expect([shown(), screen.getByRole("status", { name: "Offered" }).textContent]).toEqual([
      INSPECTION_SCENE_NAME,
      "2",
    ]);
    await user.click(screen.getByRole("button", { name: "CLOSE" }));

    expect(shown()).toBe("SERVER");
  });

  it("stays offered when another scene is chosen over it, which a close then leaves shown", async () => {
    const { user } = renderHost(opening(1));
    await user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    expect([shown(), screen.getByRole("status", { name: "Offered" }).textContent]).toEqual([
      "FRAME CHANGE TEST",
      "1",
    ]);
    await user.click(screen.getByRole("button", { name: INSPECTION_SCENE_NAME }));
    expect(shown()).toBe(INSPECTION_SCENE_NAME);
    await user.click(screen.getByRole("button", { name: "PRECISION TEST" }));

    await user.click(screen.getByRole("button", { name: "CLOSE" }));

    expect(shown()).toBe("PRECISION TEST");
  });

  it("is never the kept scene that stands in for the server's", async () => {
    const { user } = renderHost(opening(1));
    await user.click(screen.getByRole("button", { name: "FRAME CHANGE TEST" }));
    await user.click(screen.getByRole("button", { name: INSPECTION_SCENE_NAME }));

    expect(screen.getByRole("status", { name: "Stand-in" })).toHaveTextContent("FRAME CHANGE TEST");
  });
});
