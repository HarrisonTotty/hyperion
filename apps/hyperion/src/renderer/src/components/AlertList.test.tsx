import type { ClassCensusDto, ContactDto } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Acknowledgement } from "../lib/useAlerts";
import STYLES from "../styles.css?raw";
import {
  aBearingContact,
  aCensusLine,
  aResolvedContact,
  NOVA_DESIGNATION,
  NOVA_HOST,
} from "../test/alertFixtures";
import { stubMatchMedia } from "../test/stubMatchMedia";
import { stylesheetRule } from "../test/stylesheet";
import {
  ACKNOWLEDGE_KEY,
  AlertList,
  type AlertSort,
  alertText,
  censusLineWords,
  NO_CENSUS,
  sortAlerts,
} from "./AlertList";

/** Ten rows of 3 rem at the test root font size, the window the list is measured at. */
const VIEWPORT_PX = 10 * 48;

/**
 * Three contacts: 1, a bearing raised at +0.50 yr; 2, a resolved nova raised at +0.25 yr,
 * acknowledged; 3, a bearing raised at +0.75 yr.
 */
const CONTACTS: ReadonlyArray<ContactDto> = [
  aBearingContact(),
  aResolvedContact([0, 26_100, 0], { acknowledged: true }),
  aBearingContact({
    contact: 3,
    first_seen: { seconds: Math.round(0.75 * 31_557_600), nanos: 0 },
    bearing: { azimuth_deg: 300, elevation_deg: -5, frame: "galactic_x" },
  }),
];

interface Rendered {
  readonly onAcknowledge: ReturnType<typeof vi.fn<(contact: number) => void>>;
  readonly onActivate: ReturnType<typeof vi.fn<(contact: ContactDto) => void>>;
}

/** The list as a panel holds it, its order and active row its own state. */
function renderList({
  contacts = CONTACTS,
  census = [],
  acknowledgements = new Map(),
  heldBack = null,
  stale = false,
  active = null,
}: {
  contacts?: ReadonlyArray<ContactDto>;
  census?: ReadonlyArray<ClassCensusDto>;
  acknowledgements?: ReadonlyMap<number, Acknowledgement>;
  heldBack?: string | null;
  stale?: boolean;
  active?: number | null;
} = {}): Rendered & { readonly container: HTMLElement } {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(VIEWPORT_PX);
  const onAcknowledge = vi.fn<(contact: number) => void>();
  const onActivate = vi.fn<(contact: ContactDto) => void>();
  function Harness() {
    const [sort, setSort] = useState<AlertSort>("priority");
    const [activeContact, setActive] = useState<number | null>(active);
    return (
      <AlertList
        contacts={contacts}
        census={census}
        acknowledgements={acknowledgements}
        sort={sort}
        onSort={setSort}
        activeContact={activeContact}
        onActivate={(contact) => {
          setActive(contact.contact);
          onActivate(contact);
        }}
        onAcknowledge={onAcknowledge}
        heldBack={heldBack}
        stale={stale}
      />
    );
  }
  const { container } = render(<Harness />);
  return { onAcknowledge, onActivate, container };
}

/** The rows' alert numbers in the order shown, read from their accessible names' order. */
function shownOrder(): number[] {
  return screen.getAllByRole("option").map((row) => {
    const name = row.getAttribute("aria-label") ?? "";
    return CONTACTS.findIndex((contact) => name.startsWith(alertText(contact))) + 1;
  });
}

/** A rendering's markup, but for the IDs React gives each render. */
function markup(container: HTMLElement): string {
  return container.innerHTML.replaceAll(/_r_[0-9a-z]+_/gu, "");
}

describe("AlertList (plan 12, P12.T12)", () => {
  it("writes a detection as the guide's sensor alert, its direction and whether it is resolved", () => {
    expect(alertText(aBearingContact())).toBe("SENSORS TRANSIENT 047° +12°: unresolved");
    expect(alertText(aResolvedContact([0, 1, 0]))).toBe("SENSORS TRANSIENT 090° +00°: resolved");
    expect(alertText(aBearingContact({ bearing: null }))).toBe("SENSORS TRANSIENT —: unresolved");
    expect(
      alertText(
        aBearingContact({ bearing: { azimuth_deg: 300, elevation_deg: -5, frame: "galactic_x" } }),
      ),
    ).toBe("SENSORS TRANSIENT 300° -05° FROM +X: unresolved");
  });

  it("sorts by priority, the unacknowledged first and then the newest", () => {
    renderList();

    expect(shownOrder()).toEqual([3, 1, 2]);
  });

  it("sorts by time, the newest raised first, and by system, the newest first within one", async () => {
    const user = userEvent.setup();
    renderList();

    await user.click(screen.getByRole("button", { name: "TIME" }));
    expect(shownOrder()).toEqual([3, 1, 2]);
    expect(sortAlerts(CONTACTS, "time").map((contact) => contact.contact)).toEqual([3, 1, 2]);

    await user.click(screen.getByRole("button", { name: "SYSTEM" }));
    expect(screen.getByRole("button", { name: "SYSTEM" })).toHaveAttribute("aria-pressed", "true");
    expect(sortAlerts(CONTACTS, "system").map((contact) => contact.contact)).toEqual([3, 1, 2]);
  });

  it("puts an acknowledged alert after an older unacknowledged one by priority, not by time", () => {
    const contacts = [
      aBearingContact({ contact: 5, first_seen: { seconds: 100, nanos: 0 } }),
      aBearingContact({ contact: 6, first_seen: { seconds: 200, nanos: 0 }, acknowledged: true }),
    ];

    expect(sortAlerts(contacts, "priority").map((contact) => contact.contact)).toEqual([5, 6]);
    expect(sortAlerts(contacts, "time").map((contact) => contact.contact)).toEqual([6, 5]);
  });

  it("moves the order's choice with the arrows, one stop in the tab order", async () => {
    const user = userEvent.setup();
    renderList();

    await user.click(screen.getByRole("button", { name: "PRIORITY" }));
    await user.keyboard("{ArrowRight}");

    expect(screen.getByRole("button", { name: "TIME" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "TIME" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "PRIORITY" })).toHaveAttribute("tabindex", "-1");
  });

  it("shows the rows in view and the total", () => {
    renderList();

    expect(screen.getByText("1-3 of 3")).toBeInTheDocument();
  });

  it("acknowledges the active alert from the keyboard with A", async () => {
    const user = userEvent.setup();
    const { onAcknowledge, onActivate } = renderList();

    await user.click(screen.getByRole("listbox", { name: "Alerts" }));
    await user.keyboard("{Home}{ArrowDown}");
    await user.keyboard(ACKNOWLEDGE_KEY.toLowerCase());

    expect(onActivate).toHaveBeenLastCalledWith(CONTACTS[0]);
    expect(onAcknowledge).toHaveBeenCalledExactlyOnceWith(1);
  });

  it("acknowledges the active alert with ACKNOWLEDGE, which shows its key", async () => {
    const user = userEvent.setup();
    const { onAcknowledge } = renderList({ active: 3 });
    const command = screen.getByRole("button", { name: `${ACKNOWLEDGE_KEY} ACKNOWLEDGE` });

    expect(command).toHaveAttribute("aria-keyshortcuts", ACKNOWLEDGE_KEY);
    expect(command).toHaveClass("command");
    await user.click(command);

    expect(onAcknowledge).toHaveBeenCalledExactlyOnceWith(3);
  });

  it.each([
    ["with no alert active", null, new Map<number, Acknowledgement>(), null, "NO ALERT SELECTED"],
    ["for an acknowledged alert", 2, new Map<number, Acknowledgement>(), null, "ACKNOWLEDGED"],
    [
      "while its acknowledgement is pending",
      1,
      new Map<number, Acknowledgement>([[1, { kind: "pending" }]]),
      null,
      "ACKNOWLEDGE PENDING",
    ],
    ["with the link down", 1, new Map<number, Acknowledgement>(), "NO CARRIER", "NO CARRIER"],
  ])("holds ACKNOWLEDGE back %s and says why", async (_case, active, acks, heldBack, reason) => {
    const user = userEvent.setup();
    const { onAcknowledge } = renderList({ active, acknowledgements: acks, heldBack });
    const command = screen.getByRole("button", { name: `${ACKNOWLEDGE_KEY} ACKNOWLEDGE` });

    expect(command).toHaveAttribute("aria-disabled", "true");
    expect(command).toHaveAccessibleDescription(reason);
    await user.click(command);
    expect(onAcknowledge).not.toHaveBeenCalled();
  });

  it("reads a pending acknowledgement in its row, and a refusal's reason beside the command", () => {
    renderList({
      active: 1,
      acknowledgements: new Map<number, Acknowledgement>([
        [1, { kind: "rejected", reason: "no such contact" }],
        [3, { kind: "pending" }],
      ]),
    });
    const rows = screen.getAllByRole("option");

    expect(within(rows[0] ?? document.body).getByText("PENDING")).toBeInTheDocument();
    expect(within(rows[1] ?? document.body).getByText("REJECTED")).toBeInTheDocument();
    expect(screen.getByText("ACKNOWLEDGE REJECTED:").closest("output")).toHaveTextContent(
      "ACKNOWLEDGE REJECTED: no such contact",
    );
  });

  it("never hides an acknowledged alert", () => {
    renderList();
    const resolved = screen.getAllByRole("option")[2];

    expect(within(resolved ?? document.body).getByText("ACKNOWLEDGED")).toBeInTheDocument();
  });

  it("renders no host text for a contact at bearing level", () => {
    renderList({ contacts: [aBearingContact()] });
    const [row] = screen.getAllByRole("option");

    expect(row).toHaveTextContent("UNCLASSIFIED");
    expect(row?.textContent).not.toContain(NOVA_DESIGNATION);
    expect(row?.textContent).not.toContain(NOVA_HOST);
    expect(row?.getAttribute("aria-label")).not.toMatch(/host|HYP/iu);
  });

  it("names a resolved contact's kind and its host", () => {
    renderList({ contacts: [aResolvedContact([0, 1, 0])] });

    expect(screen.getByRole("option")).toHaveTextContent(`NOVA · ${NOVA_DESIGNATION}`);
  });

  it("presents each alert as an Advisory, its text in --status-advisory", () => {
    renderList();

    expect(screen.getAllByText(/^SENSORS TRANSIENT/u)[0]).toHaveClass("alert-list__text");
    expect(stylesheetRule(".alert-list__text")).toContain("color: var(--status-advisory)");
  });

  it("reads each row's time and flux with their units", () => {
    renderList({ contacts: [aBearingContact()] });
    const row = screen.getByRole("option");

    expect(row).toHaveTextContent("UT +0.50 yr");
    expect(row).toHaveTextContent("FLUX V 3.20E-12 W/m²");
  });

  it("trails each stale row with one S and names it stale", () => {
    renderList({ stale: true });

    for (const row of screen.getAllByRole("option")) {
      expect(row.getAttribute("aria-label")).toMatch(/, stale$/u);
      expect(within(row).getByText("S")).toBeInTheDocument();
    }
  });

  it("sets a census line per class in words", () => {
    const census = [
      aCensusLine("complete", "core_collapse"),
      aCensusLine("pending", "accreting_white_dwarf"),
      aCensusLine("over_limit", "stellar_merger"),
    ];
    renderList({ census });

    const lines = within(screen.getByRole("list", { name: "Census by class" })).getAllByRole(
      "listitem",
    );
    expect(lines.map((line) => line.textContent)).toEqual([
      "CORE-COLLAPSE SUPERNOVA COMPLETE",
      "ACCRETING WD PENDING",
      "STELLAR MERGER OVER LIMIT",
    ]);
    expect(censusLineWords(aCensusLine("pending", "accreting_white_dwarf_slow"))).toBe(
      "SLOW ACCRETING WD PENDING",
    );
  });

  it("says the server has not scanned before any census, so that no alerts is not read as none", () => {
    renderList({ contacts: [] });

    expect(screen.getByText("NO ALERTS")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Census by class" })).toHaveTextContent(NO_CENSUS);
  });

  it("has nothing to reduce under reduced motion: no animation, transition or flash", () => {
    const rules = [...STYLES.matchAll(/(?:^|\n)(\.alert-list[^{]*)\{([^}]*)\}/gu)];
    expect(rules.length).toBeGreaterThan(5);
    for (const [, selector, body] of rules) {
      expect(`${selector ?? ""}${body ?? ""}`).not.toMatch(/animation|transition|blink/u);
    }

    // The same list either way.
    stubMatchMedia(false);
    const moving = markup(renderList().container);
    stubMatchMedia(true);
    const reduced = markup(renderList().container);
    expect(reduced).toBe(moving);
  });
});
