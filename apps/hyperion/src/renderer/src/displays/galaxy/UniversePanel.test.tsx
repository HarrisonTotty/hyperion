import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  aCreatedUniverse,
  anOpenedUniverse,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { UniversePanel } from "./UniversePanel";

const RECONNECT_DELAY_MS = 2_000;

/** Height of a universe row at the 16px root font size jsdom reports. */
const ROW_PX = 56;

/** Height of the universe table's header at the same size. */
const HEADER_PX = 44;

const SURVEY_1 = aUniverse();
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });
const OLD_SURVEY = aUniverse({
  id: "00000000000000c3",
  name: "OLD SURVEY",
  seed: "0000000000000007",
  generator_version: 1,
  status: "generator_mismatch",
});

/** Holds the panel's fold, as the display does, starting whole. */
function FoldablePanel() {
  const [expanded, setExpanded] = useState(true);
  return (
    <UniversePanel
      expanded={expanded}
      onToggle={() => {
        setExpanded((shown) => !shown);
      }}
    />
  );
}

/** Renders the panel under a welcomed link, with the universe session. */
function renderPanel() {
  const user = userEvent.setup();
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <FoldablePanel />
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket };
}

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** The value the open-universe readout shows beside `label`. */
function reading(label: string): HTMLElement {
  const term = screen.getAllByText(label).find((element) => element.tagName === "DT");
  const value = term?.nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`no readout value beside ${label}`);
  }
  return value;
}

/** The elements showing `text` that are not hidden, such as a description shown on screen. */
function shownText(text: string): HTMLElement[] {
  return screen.queryAllByText(text).filter((element) => element.closest("[hidden]") === null);
}

function universeRows(): HTMLElement[] {
  const table = screen.getByRole("table", { name: "Universes" });
  // The first row is the header.
  return within(table).getAllByRole("row").slice(1);
}

/** Creates a universe, drops the link before the answer, and brings the link back. */
async function createAcrossALinkDrop() {
  const { user, socket } = renderPanel();
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([]));
  });
  await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3{Enter}");
  // Fake time only for the wait before reconnecting, which the operator's input never needs.
  vi.useFakeTimers();
  await server(() => {
    socket.close();
  });
  act(() => {
    vi.advanceTimersByTime(RECONNECT_DELAY_MS);
  });
  vi.useRealTimers();
  const reconnected = FakeWebSocket.latest();
  act(() => {
    reconnected.serverWelcomes();
  });
  return { user, reconnected };
}

function formStatus(): HTMLElement {
  const form = screen.getByRole("group", { name: "NEW UNIVERSE" });
  return within(form).getByRole("status");
}

describe("UniversePanel", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("the open universe and the list", () => {
    it("reads NO UNIVERSE OPEN with every value missing before one is open", () => {
      renderPanel();

      expect(screen.getByRole("heading", { level: 2, name: "Universe" })).toBeInTheDocument();
      expect(screen.getByText("NO UNIVERSE OPEN")).toBeInTheDocument();
      for (const label of ["NAME", "SEED", "GEN VER", "ID"]) {
        expect(reading(label)).toHaveTextContent("—");
      }
    });

    it("shows PENDING while the list is requested, then a row per universe", async () => {
      const { socket } = renderPanel();
      expect(screen.getByRole("status")).toHaveTextContent("PENDING");

      await server(() => {
        socket.serverAnswers("list_universes", () =>
          aUniverseList([SURVEY_1, SURVEY_2, OLD_SURVEY]),
        );
      });

      const rows = universeRows();
      expect(rows).toHaveLength(3);
      expect(rows[1]).toHaveTextContent("SURVEY 2");
      expect(rows[1]).toHaveTextContent("000000000000BEEF");
      expect(within(rows[2] ?? document.body).getByText("1")).toBeInTheDocument();
      expect(screen.queryByText("PENDING")).not.toBeInTheDocument();
    });

    it("reads NO UNIVERSES for an empty list, and says what to do", async () => {
      const { socket } = renderPanel();

      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([]));
      });

      expect(screen.getByText("NO UNIVERSES: create one below")).toBeInTheDocument();
      expect(screen.queryByRole("table", { name: "Universes" })).not.toBeInTheDocument();
    });

    it("opens a universe with OPEN: PENDING, then its name and upper-case seed", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 2" }));
      expect(socket.requestsOfKind("open_universe").map(({ body }) => body)).toEqual([
        { kind: "open_universe", universe: SURVEY_2.id },
      ]);
      expect(screen.getByRole("status")).toHaveTextContent("PENDING");
      expect(screen.getByRole("button", { name: "Open universe SURVEY 2" })).toHaveTextContent(
        "PENDING",
      );
      expect(screen.getByRole("button", { name: "Open universe SURVEY 1" })).toHaveTextContent(
        "OPEN UNIVERSE",
      );
      expect(reading("NAME")).toHaveTextContent("—");

      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_2));
      });

      expect(reading("NAME")).toHaveTextContent("SURVEY 2");
      expect(reading("SEED")).toHaveTextContent("000000000000BEEF");
      expect(reading("GEN VER")).toHaveTextContent("2");
      expect(reading("ID")).toHaveTextContent("00000000000000B2");
      expect(screen.queryByText("NO UNIVERSE OPEN")).not.toBeInTheDocument();
      expect(screen.queryByRole("status")).not.toBeInTheDocument();
    });

    it("marks the open universe's row OPEN in place of its button", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
      });

      const [first] = universeRows();
      expect(within(first ?? document.body).queryByRole("button")).not.toBeInTheDocument();
      expect(within(first ?? document.body).getByText("OPEN")).toHaveAttribute(
        "aria-current",
        "true",
      );
      expect(screen.getByRole("button", { name: "Open universe SURVEY 2" })).toBeInTheDocument();
    });

    it("shows the server's reason when an open is rejected", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      const [open] = socket.requestsOfKind("open_universe");
      await server(() => {
        socket.serverRejects(open?.id ?? -1, {
          code: "unknown_universe",
          message: "no universe 00000000000000a1",
          field: "universe",
        });
      });

      expect(screen.getByRole("status")).toHaveTextContent(
        "REJECTED: no universe 00000000000000a1",
      );
      expect(reading("NAME")).toHaveTextContent("—");
    });

    it("inhibits OPEN for a universe of another generator version, and says why", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([OLD_SURVEY], 2));
      });

      const button = screen.getByRole("button", { name: "Open universe OLD SURVEY" });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(button).toHaveAccessibleDescription("GENERATOR VERSION 1: server runs version 2");
      await user.click(button);

      expect(socket.requestsOfKind("open_universe")).toEqual([]);
      expect(shownText("GENERATOR VERSION 1: server runs version 2")).toHaveLength(1);
    });

    it("shows why an OPEN is held back only while it is pointed at or focused", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, OLD_SURVEY], 2));
      });
      const reason = "GENERATOR VERSION 1: server runs version 2";
      const oldSurvey = screen.getByRole("button", { name: "Open universe OLD SURVEY" });
      expect(shownText(reason)).toEqual([]);

      await user.hover(oldSurvey);
      expect(shownText(reason)).toHaveLength(1);
      await user.unhover(oldSurvey);
      expect(shownText(reason)).toEqual([]);
      await user.hover(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      expect(shownText(reason)).toEqual([]);

      oldSurvey.focus();
      await user.keyboard("{Enter}");
      expect(shownText(reason)).toHaveLength(1);
      expect(socket.requestsOfKind("open_universe")).toEqual([]);
    });

    it("holds every command back while the link is down, and says NO CARRIER", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });

      await server(() => {
        socket.close();
      });

      const buttons = [
        screen.getByRole("button", { name: "Open universe SURVEY 1" }),
        screen.getByRole("button", { name: "Open universe SURVEY 2" }),
        screen.getByRole("button", { name: "CREATE" }),
      ];
      for (const button of buttons) {
        expect(button).toHaveAttribute("aria-disabled", "true");
        expect(button).toHaveAccessibleDescription("NO CARRIER");
      }
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      expect(socket.requestsOfKind("open_universe")).toEqual([]);
    });

    it("holds every command back while one is pending, and says PENDING", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      const others = [
        screen.getByRole("button", { name: "Open universe SURVEY 2" }),
        screen.getByRole("button", { name: "CREATE" }),
      ];
      for (const button of others) {
        expect(button).toHaveAttribute("aria-disabled", "true");
        expect(button).toHaveAccessibleDescription("PENDING");
      }
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 2" }));
      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3{Enter}");

      expect(socket.requestsOfKind("open_universe")).toHaveLength(1);
      expect(socket.requestsOfKind("create_universe")).toEqual([]);
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
      });
      expect(screen.getByRole("button", { name: "Open universe SURVEY 2" })).not.toHaveAttribute(
        "aria-disabled",
      );
      await user.click(screen.getByRole("button", { name: "NEW UNIVERSE" }));
      expect(screen.getByRole("button", { name: "CREATE" })).not.toHaveAttribute("aria-disabled");
    });

    it("offers RETRY when the list is refused, and requests it again", async () => {
      const { user, socket } = renderPanel();
      const [list] = socket.requestsOfKind("list_universes");
      await server(() => {
        socket.serverRejects(list?.id ?? -1, {
          code: "queue_full",
          message: "the interactive queue is full",
          field: null,
        });
      });
      expect(screen.getByRole("status")).toHaveTextContent(
        "REJECTED: the interactive queue is full",
      );

      await user.click(screen.getByRole("button", { name: "RETRY" }));

      expect(socket.requestsOfKind("list_universes")).toHaveLength(2);
    });

    it("hands the focus to its title control when the list's RETRY is pressed", async () => {
      const { user, socket } = renderPanel();
      const [list] = socket.requestsOfKind("list_universes");
      await server(() => {
        socket.serverRejects(list?.id ?? -1, {
          code: "queue_full",
          message: "the interactive queue is full",
          field: null,
        });
      });
      act(() => {
        screen.getByRole("button", { name: "RETRY" }).focus();
      });

      await user.keyboard("{Enter}");

      // RETRY goes as the list goes pending; the title control stays (the orchestrator's ruling
      // 18).
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Universe" }));
    });

    it("keeps the list through a link loss and requests it again on reconnect", async () => {
      vi.useFakeTimers();
      const { socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });

      await server(() => {
        socket.close();
      });
      expect(universeRows()).toHaveLength(1);
      act(() => {
        vi.advanceTimersByTime(RECONNECT_DELAY_MS);
      });
      const reconnected = FakeWebSocket.latest();
      act(() => {
        reconnected.serverWelcomes();
      });

      expect(reconnected.requestsOfKind("list_universes")).toHaveLength(1);
      expect(screen.getByRole("button", { name: "Open universe SURVEY 1" })).not.toHaveAttribute(
        "aria-disabled",
      );
    });

    it("shows which rows are in view and how many there are", async () => {
      vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(
        HEADER_PX + ROW_PX * 8,
      );
      const { socket } = renderPanel();
      const universes = Array.from({ length: 23 }, (_, index) =>
        aUniverse({
          id: (0xa0 + index).toString(16).padStart(16, "0"),
          name: `SURVEY ${index + 1}`,
        }),
      );

      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList(universes));
      });

      expect(screen.getByText("1-8 of 23")).toBeInTheDocument();
    });

    it("updates the position when the list's height changes", async () => {
      const height = vi
        .spyOn(HTMLElement.prototype, "clientHeight", "get")
        .mockReturnValue(HEADER_PX + ROW_PX);
      const { socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () =>
          aUniverseList([SURVEY_1, SURVEY_2, OLD_SURVEY]),
        );
      });
      expect(screen.getByText("1-1 of 3")).toBeInTheDocument();

      height.mockReturnValue(HEADER_PX + ROW_PX * 3);
      act(() => {
        FakeResizeObserver.resizeAll();
      });

      expect(screen.getByText("1-3 of 3")).toBeInTheDocument();
    });
  });

  describe("folded to one line", () => {
    it("names the open universe by name, seed and generator version beside its title", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
      });
      const toggle = screen.getByRole("button", { name: "Universe" });

      await user.click(toggle);

      expect(toggle).toHaveAttribute("aria-expanded", "false");
      const summary = shownText("SEED")[0]?.closest("dl");
      expect(summary).toHaveTextContent(
        /^NAME\s*SURVEY 1\s*SEED\s*00000000000004D2\s*GEN VER\s*2$/,
      );
      expect(screen.queryByRole("table", { name: "Universes" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "NEW UNIVERSE" })).not.toBeInTheDocument();
    });

    it("says NO UNIVERSE OPEN when none is", async () => {
      const { user } = renderPanel();

      await user.click(screen.getByRole("button", { name: "Universe" }));

      expect(shownText("NO UNIVERSE OPEN")).toHaveLength(1);
    });

    it("is shown and folded from the keyboard by its title, a heading", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });
      const toggle = within(screen.getByRole("heading", { level: 2 })).getByRole("button", {
        name: "Universe",
      });

      toggle.focus();
      await user.keyboard("{Enter}");
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      await user.keyboard(" ");

      expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("table", { name: "Universes" })).toBeInTheDocument();
    });
  });

  describe("the NEW UNIVERSE form", () => {
    it("is shown while no universe is open, and folded once one is", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });
      const toggle = screen.getByRole("button", { name: "NEW UNIVERSE" });
      expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("textbox", { name: "NAME" })).toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
      });

      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("textbox", { name: "NAME" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "CREATE" })).not.toBeInTheDocument();
    });

    it("is shown and folded from the keyboard, keeping what was typed", async () => {
      const { user } = renderPanel();
      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 5");
      const toggle = screen.getByRole("button", { name: "NEW UNIVERSE" });

      toggle.focus();
      await user.keyboard("{Enter}");
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("textbox", { name: "NAME" })).not.toBeInTheDocument();
      await user.keyboard(" ");

      expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("textbox", { name: "NAME" })).toHaveValue("SURVEY 5");
    });

    it("moves the focus to NEW UNIVERSE when a create folds the fields", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 2{Enter}");
      await server(() => {
        socket.serverAnswers("create_universe", () => aCreatedUniverse(SURVEY_2));
      });

      const toggle = screen.getByRole("button", { name: "NEW UNIVERSE" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(toggle).toHaveFocus();
    });

    it("stays shown for the operator who showed it until another universe opens", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
      });
      const toggle = screen.getByRole("button", { name: "NEW UNIVERSE" });

      await user.click(toggle);
      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 1{Enter}");
      const [create] = socket.requestsOfKind("create_universe");
      await server(() => {
        socket.serverRejects(create?.id ?? -1, {
          code: "name_taken",
          message: "a universe named SURVEY 1 exists",
          field: "name",
        });
      });
      expect(toggle).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByText("REJECTED: a universe named SURVEY 1 exists")).toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 2" }));
      await server(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_2));
      });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
    });

    it("creates with a seed drawn by the server by default, the seed field disabled", async () => {
      const { user, socket } = renderPanel();

      expect(screen.getByRole("radio", { name: "RANDOM" })).toBeChecked();
      const seedField = screen.getByRole("textbox", { name: "SEED VALUE" });
      expect(seedField).toBeDisabled();
      expect(seedField).toHaveAccessibleDescription("DRAWN BY SERVER");
      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3");
      await user.click(screen.getByRole("button", { name: "CREATE" }));

      expect(socket.requestsOfKind("create_universe").map(({ body }) => body)).toEqual([
        { kind: "create_universe", name: "SURVEY 3", seed: null },
      ]);
    });

    it("sends an entered seed padded and in lower case when Enter is pressed", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 2");
      await user.click(screen.getByRole("radio", { name: "ENTERED" }));
      const seed = screen.getByRole("textbox", { name: "SEED VALUE" });
      expect(seed).toBeEnabled();
      expect(seed).toHaveAccessibleDescription("1-16 HEXADECIMAL DIGITS");
      await user.type(seed, "BEEF{Enter}");

      expect(socket.requestsOfKind("create_universe").map(({ body }) => body)).toEqual([
        { kind: "create_universe", name: "SURVEY 2", seed: "000000000000beef" },
      ]);
    });

    it("trims the name before sending it", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "  SURVEY 4 {Enter}");

      expect(socket.requestsOfKind("create_universe")[0]?.body.name).toBe("SURVEY 4");
    });

    it("refuses an empty name, says what is valid, and sends nothing", async () => {
      const { user, socket } = renderPanel();

      await user.click(screen.getByRole("button", { name: "CREATE" }));

      const name = screen.getByRole("textbox", { name: "NAME" });
      expect(name).toHaveAttribute("aria-invalid", "true");
      expect(name).toHaveAccessibleDescription(
        "1-48 CHARACTERS NAME INVALID: enter 1 to 48 printable characters",
      );
      expect(socket.requestsOfKind("create_universe")).toEqual([]);
    });

    it("refuses a name of more than 48 characters", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), `${"X".repeat(49)}{Enter}`);

      expect(
        screen.getByText("NAME INVALID: enter 1 to 48 printable characters"),
      ).toBeInTheDocument();
      expect(socket.requestsOfKind("create_universe")).toEqual([]);
    });

    it("refuses an entered seed that is not 1 to 16 hex digits, and sends nothing", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 2");
      await user.click(screen.getByRole("radio", { name: "ENTERED" }));
      const seed = screen.getByRole("textbox", { name: "SEED VALUE" });
      await user.type(seed, "BEEG{Enter}");

      expect(seed).toHaveAttribute("aria-invalid", "true");
      expect(seed).toHaveAccessibleDescription(
        "1-16 HEXADECIMAL DIGITS SEED INVALID: enter 1 to 16 hexadecimal digits (0-9, A-F)",
      );
      expect(socket.requestsOfKind("create_universe")).toEqual([]);
    });

    it("clears a field's message once the field is edited", async () => {
      const { user } = renderPanel();
      await user.click(screen.getByRole("button", { name: "CREATE" }));

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "S");

      expect(
        screen.queryByText("NAME INVALID: enter 1 to 48 printable characters"),
      ).not.toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "NAME" })).not.toHaveAttribute("aria-invalid");
    });

    it("shows PENDING beside CREATE, then lists and opens the new universe", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
      });

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 2");
      await user.click(screen.getByRole("radio", { name: "ENTERED" }));
      await user.type(screen.getByRole("textbox", { name: "SEED VALUE" }), "beef{Enter}");
      const form = screen.getByRole("group", { name: "NEW UNIVERSE" });
      expect(within(form).getByRole("status")).toHaveTextContent("PENDING");

      await server(() => {
        socket.serverAnswers("create_universe", () => aCreatedUniverse(SURVEY_2));
      });
      expect(reading("NAME")).toHaveTextContent("SURVEY 2");
      expect(reading("SEED")).toHaveTextContent("000000000000BEEF");
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
      });

      expect(universeRows()).toHaveLength(2);
      expect(within(universeRows()[1] ?? document.body).getByText("OPEN")).toHaveAttribute(
        "aria-current",
        "true",
      );
    });

    it("shows the server's reason beside CREATE when a create is rejected", async () => {
      const { user, socket } = renderPanel();

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 1{Enter}");
      const [create] = socket.requestsOfKind("create_universe");
      await server(() => {
        socket.serverRejects(create?.id ?? -1, {
          code: "name_taken",
          message: "a universe named SURVEY 1 exists",
          field: "name",
        });
      });

      const form = screen.getByRole("group", { name: "NEW UNIVERSE" });
      expect(within(form).getByRole("status")).toHaveTextContent(
        "REJECTED: a universe named SURVEY 1 exists",
      );
      expect(screen.getByText("NO UNIVERSE OPEN")).toBeInTheDocument();
    });

    describe("a create cut off by a lost link", () => {
      it("reads CREATE UNCONFIRMED, its cause and the action, in caution once the link returns", async () => {
        await createAcrossALinkDrop();

        expect(formStatus()).toHaveTextContent(
          "CREATE UNCONFIRMED: link lost before reply; check the universe list",
        );
        expect(formStatus()).toHaveClass("request-status__text--fault");
      });

      it("clears CREATE UNCONFIRMED when a universe is opened from the list", async () => {
        const { user, reconnected } = await createAcrossALinkDrop();
        await server(() => {
          reconnected.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
        });

        await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
        expect(screen.getByText(/^CREATE UNCONFIRMED/)).toBeInTheDocument();
        await server(() => {
          reconnected.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_1));
        });

        expect(screen.queryByText(/CREATE UNCONFIRMED/)).not.toBeInTheDocument();
      });

      it("keeps the panel and its fields shown while the report stands, saying why", async () => {
        const { user } = await createAcrossALinkDrop();
        const fields = screen.getByRole("button", { name: "NEW UNIVERSE" });
        const panel = screen.getByRole("button", { name: "Universe" });

        await user.click(fields);
        await user.click(panel);

        for (const toggle of [fields, panel]) {
          expect(toggle).toHaveAttribute("aria-expanded", "true");
          expect(toggle).toHaveAttribute("aria-disabled", "true");
          expect(toggle).toHaveAccessibleDescription(
            "CREATE UNCONFIRMED: link lost before reply; check the universe list",
          );
        }
        expect(screen.getByText(/^CREATE UNCONFIRMED/)).toBeVisible();
      });

      it("gives the report an ID of its own beside an OPEN's status", async () => {
        const { user, reconnected } = await createAcrossALinkDrop();
        await server(() => {
          reconnected.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
        });

        await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));

        const statuses = screen.getAllByRole("status");
        expect(statuses.map((status) => status.textContent)).toEqual([
          "PENDING",
          "CREATE UNCONFIRMED: link lost before reply; check the universe list",
        ]);
        expect(new Set(statuses.map((status) => status.id)).size).toBe(2);
      });

      it("clears CREATE UNCONFIRMED when the operator dismisses it", async () => {
        const { user } = await createAcrossALinkDrop();

        await user.click(screen.getByRole("button", { name: "DISMISS" }));

        expect(screen.queryByText(/CREATE UNCONFIRMED/)).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: "CREATE" })).toHaveFocus();
      });

      it("clears CREATE UNCONFIRMED with the next create", async () => {
        const { user, reconnected } = await createAcrossALinkDrop();

        await user.click(screen.getByRole("button", { name: "CREATE" }));

        expect(reconnected.requestsOfKind("create_universe")).toHaveLength(1);
        expect(formStatus()).toHaveTextContent("PENDING");
        expect(screen.queryByText(/CREATE UNCONFIRMED/)).not.toBeInTheDocument();
      });
    });

    it("sends nothing from CREATE while the link is down", async () => {
      const { user, socket } = renderPanel();
      await server(() => {
        socket.close();
      });

      await user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3");
      await user.click(screen.getByRole("button", { name: "CREATE" }));

      expect(socket.requestsOfKind("create_universe")).toEqual([]);
      expect(screen.getByText("NO CARRIER")).toBeInTheDocument();
    });
  });
});
