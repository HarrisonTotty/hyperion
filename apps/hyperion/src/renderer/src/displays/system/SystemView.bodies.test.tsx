/**
 * The `SYSTEM` display's planets over a fake socket, against the shared wire fixture (plan 14,
 * P14.T41.b, T42 and T43 for bodies).
 */
import type {
  BodyEnvelopeDto,
  BodySurfaceDto,
  GasShareDto,
  MassFractionsDto,
  ResponseFor,
  SectionDto,
} from "@hyperion/protocol";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  earthDetail,
  earthDetailOfAnOlderServer,
  earthMassAndOrbit,
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  jupiterDetail,
  populatedBodies,
  populatedBodiesWith,
  sliceBodies,
  sliceBodiesWith,
  tiltedPole,
  withBodyRotation,
} from "../../test/planetaryFixture";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  anUnsupportedBodiesError,
  aSingleStarSummary,
  aSummaryResponse,
  aSystemTarget,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import { orbitNormal } from "../../lib/system/hierarchy";
import { dot, type Vec3 } from "../../geometry/vec3";
import { SystemView } from "./SystemView";

const WIDTH_PX = 400;
const HEIGHT_PX = 300;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function renderView() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(HEIGHT_PX);
  render(
    <ServerLinkHarness>
      <SystemView target={aSystemTarget()} />
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket };
}

/** Answers the hosts, then the bodies with `bodies`, or with `unsupported` for `null`. */
async function answer(
  socket: FakeWebSocket,
  bodies: ResponseFor<"system_bodies"> | null = sliceBodies(),
): Promise<void> {
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
  });
  await server(() => {
    const [request] = socket.requestsOfKind("system_bodies").toReversed();
    if (request === undefined) {
      throw new Error("the display asks for its bodies");
    }
    if (bodies === null) {
      socket.serverRejects(request.id, anUnsupportedBodiesError());
    } else {
      socket.serverResponds(request.id, bodies);
    }
  });
}

function mapPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Orbit map/ });
}

function bodiesPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Bodies/ });
}

function readout(): HTMLElement {
  return screen.getByRole("status", { name: "Selected body" });
}

/** The terms of the readout that read `label`. */
function terms(label: string): HTMLElement[] {
  return within(readout())
    .queryAllByRole("term")
    .filter((term) => term.textContent === label);
}

/** The value a readout label reads, as its `dd` holds it. */
function reading(label: string): string {
  return terms(label)[0]?.nextElementSibling?.textContent ?? "";
}

/** The `dd` that holds the value a readout label reads. */
function readingOf(label: string): HTMLElement {
  const value = terms(label)[0]?.nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`the readout reads ${label}`);
  }
  return value;
}

/** The populated answer's planet `A b`, whose moon `A b I` is `/257`. */
const PLANET_ID = `${FIXTURE_SYSTEM}.0100`;

/**
 * The `INC` the populated answer's moon reads to the plane normal to `pole`: the angle between its
 * orbit normal (i 0.09, Ω 1.2) and `pole`, in degrees to one decimal.
 */
function inclinationTo(pole: Vec3): string {
  const cosine = dot(orbitNormal({ inclinationRad: 0.09, ascendingNodeRad: 1.2 }), pole);
  return `${((Math.acos(cosine) * 180) / Math.PI).toFixed(1)}°`;
}

function rowNames(): Array<string | null> {
  return within(screen.getByRole("tree", { name: "Bodies" }))
    .getAllByRole("treeitem")
    .map((item) => item.getAttribute("aria-label"));
}

async function selectRow(user: ReturnType<typeof userEvent.setup>, name: RegExp): Promise<void> {
  const row = within(screen.getByRole("tree", { name: "Bodies" })).getByRole("treeitem", { name });
  await user.click(row);
}

async function answerDetail(socket: FakeWebSocket, detail: ResponseFor<"body_detail">) {
  await server(() => {
    socket.serverAnswers("body_detail", () => detail);
  });
}

/** The fixture's Earth with a hooks section that is `ok` and carries `detailSeed`. */
function earthWithDetailSeed(detailSeed: SectionDto<string>): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  return {
    ...detail,
    record: { ...detail.record, hooks: { state: "ok", value: { detail_seed: detailSeed } } },
  };
}

/** The fixture's Earth with its `ok` surface changed by `change`. */
function earthWithSurface(
  change: (surface: BodySurfaceDto) => BodySurfaceDto,
): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  const { surface } = detail.record;
  if (surface.state !== "ok") {
    throw new Error("the fixture's Earth has a surface");
  }
  return {
    ...detail,
    record: { ...detail.record, surface: { state: "ok", value: change(surface.value) } },
  };
}

/** The fixture's Earth with its `ok` bulk section's layers `massFractions`. */
function earthWithMassFractions(massFractions: MassFractionsDto): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  const { bulk } = detail.record;
  if (bulk.state !== "ok") {
    throw new Error("the fixture's Earth has a bulk section");
  }
  return {
    ...detail,
    record: {
      ...detail.record,
      bulk: { state: "ok", value: { ...bulk.value, mass_fractions: massFractions } },
    },
  };
}

/** The fixture's Earth made airless, as the server sends such a body: no air, its figures none. */
function airlessEarth(): ResponseFor<"body_detail"> {
  return earthWithSurface((surface) => ({
    ...surface,
    state: "airless",
    surface_pressure_pa: 0,
    gases: [],
    mean_molar_mass_g_per_mol: null,
    heat_capacity_over_r: null,
  }));
}

/**
 * The fixture's Earth sent as `close_binary`'s dwarf planet `e101` is (P14.T35.f): a magma ocean at
 * 1,684 K whose volatiles are gone, its volatile air empty and its mixture figures none, and no
 * `silicate_vapour` key; then changed by `change`.
 */
function e101Earth(
  change: (surface: BodySurfaceDto) => BodySurfaceDto = (surface) => surface,
): ResponseFor<"body_detail"> {
  return earthWithSurface((surface) => {
    if ("silicate_vapour" in surface) {
      throw new Error("the fixture's Earth sends no silicate vapour");
    }
    return change({
      ...surface,
      state: "magma_ocean",
      mean_surface_temperature_k: 1684.4,
      surface_pressure_pa: 0,
      gases: [],
      mean_molar_mass_g_per_mol: null,
      heat_capacity_over_r: null,
    });
  });
}

/**
 * A reading of several parts, each named in one word, as the readout sets it: each part's spaces
 * no-break, so that such a part never breaks across a line, and the parts joined by middle dots.
 */
function partsReading(...parts: readonly string[]): string {
  return parts.map((part) => part.replaceAll(" ", "\u00A0")).join(" · ");
}

/** The fixture's Jupiter with its envelope section `envelope`. */
function jupiterWithEnvelope(envelope: SectionDto<BodyEnvelopeDto>): ResponseFor<"body_detail"> {
  const detail = jupiterDetail();
  return { ...detail, record: { ...detail.record, envelope } };
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("SystemView's bodies request", () => {
  it("asks once for its system's bodies at the time it was opened at, at every level", () => {
    const { socket } = renderView();

    expect(socket.requestsOfKind("system_bodies").map((request) => request.body)).toEqual([
      {
        kind: "system_bodies",
        universe: aSystemTarget().universe,
        system: PIN_SYSTEM,
        time: A_CENTURY,
        detail: "full",
      },
    ]);
  });

  it("shows the hosts alone, with no fault, while the server does not serve the bodies", async () => {
    const { socket } = renderView();
    await answer(socket, null);

    expect(screen.getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(within(mapPanel()).queryByText(/REJECTED/)).not.toBeInTheDocument();
    expect(
      within(mapPanel()).getByText(
        "PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
      ),
    ).toBeInTheDocument();
    expect(rowNames()).toEqual(["H7K 4C0RFZ D-7 /0, DWARF"]);
  });

  it("says PENDING while the bodies are asked for, the hosts drawn", async () => {
    const { socket } = renderView();
    await server(() => {
      socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
    });

    expect(screen.getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(within(mapPanel()).getByText("PENDING")).toBeInTheDocument();
  });
});

describe("SystemView with the slice's bodies", () => {
  it("lists the planets under their star by semi-major axis", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(rowNames()).toEqual([
      "H7K 4C0RFZ D-7 /0, DWARF",
      "H7K 4C0RFZ D-7 /768, PLANET, SMA 1.00 AU",
      "H7K 4C0RFZ D-7 /1280, PLANET, SMA 5.20 AU",
    ]);
    expect(screen.getByText("1-3 of 3")).toBeInTheDocument();
  });

  it("composes the note from the tags, without the planets", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(
      within(mapPanel()).getByText("MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED"),
    ).toBeInTheDocument();
    expect(within(mapPanel()).queryByText(/^PLANETS/)).not.toBeInTheDocument();
  });

  it("names the belts and halo a contact withholds on a line of their own", async () => {
    // The slice as a `contact` reader holds it (ruling 113.2): the system's belts and halo are
    // withheld, and each body is a contact whose sections are all withheld.
    const withheld = { state: "not_resolved" } as const;
    const contacts = sliceBodiesWith((body) => ({
      ...body,
      kind: { type: "unresolved" },
      label: withheld,
      mass_kg: withheld,
      orbit: withheld,
      moons: withheld,
      rings: withheld,
      population: withheld,
      bulk: withheld,
    }));
    const contact: ResponseFor<"system_bodies"> = {
      ...contacts,
      granted: "contact",
      belts: withheld,
      halo: withheld,
    };
    const { socket } = renderView();
    await answer(socket, contact);
    const notes = within(mapPanel())
      .getAllByText(/: NOT (RESOLVED|YET MODELLED)$/)
      .map((note) => note.textContent);

    expect(notes).toEqual(["BELTS AND COMETARY HALO: NOT RESOLVED"]);
  });

  it("shows the granted level and the system's architecture class", async () => {
    const { socket } = renderView();
    await answer(socket);
    // The system's own readings come first in the panel, before the selected star's.
    const [detail] = within(bodiesPanel()).getAllByText("DETAIL");
    const [arch] = within(bodiesPanel()).getAllByText("ARCH");

    expect(detail?.nextElementSibling).toHaveTextContent("FULL");
    expect(arch?.nextElementSibling).toHaveTextContent("SOLAR-LIKE");
  });

  it("draws the map on the system plane, a giant planet apart from a smaller one", async () => {
    const { socket } = renderView();
    await answer(socket);
    const legend = screen.getByRole("group", { name: "Orbit map legend" });

    expect(within(legend).getByText("FILLED ABOVE SYSTEM PLANE")).toBeInTheDocument();
    expect(within(legend).getByText("PLANET")).toBeInTheDocument();
    expect(within(legend).getByText("GIANT PLANET")).toBeInTheDocument();
  });

  it("reads the zones in the star's readout", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(reading("ZONE")).toBe("H7K 4C0RFZ D-7 /0");
    expect(reading("ARCH")).toBe("SOLAR-LIKE");
    expect(reading("SNOW LINE")).toBe("2.26 AU");
    expect(reading("HABITABLE ZONE")).toBe("0.989 – 1.69 AU");
    expect(reading("OPTIMISTIC")).toBe("0.749 – 1.77 AU");
    expect(terms("STABLE ZONE")).toHaveLength(0);
  });

  it("switches the optimistic habitable zone's annulus off and on, apart from the conservative", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const toggle = within(mapPanel()).getByRole("button", { name: "OPTIMISTIC" });
    const label = (text: string) =>
      within(mapPanel()).queryByText(text, { selector: ".spatial-label" });

    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(label("OPTIMISTIC")).toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(label("OPTIMISTIC")).not.toBeInTheDocument();
    expect(label("HABITABLE ZONE")).toBeInTheDocument();
    // The readout reads the zone whatever the map draws.
    expect(reading("OPTIMISTIC")).toBe("0.749 – 1.77 AU");

    await user.click(toggle);

    expect(label("OPTIMISTIC")).toBeInTheDocument();
  });

  it("switches the habitable zone's annulus off and on", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const toggle = within(mapPanel()).getByRole("button", { name: "HABITABLE ZONE" });
    const annulusLabel = () =>
      within(mapPanel()).queryByText("HABITABLE ZONE", { selector: ".spatial-label" });

    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(annulusLabel()).toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(annulusLabel()).not.toBeInTheDocument();
    expect(
      within(mapPanel()).queryByText("SNOW LINE", { selector: ".spatial-label" }),
    ).toBeVisible();
  });
});

describe("SystemView's body readout", () => {
  it("asks for the record of the planet selected, at the time the system was asked for", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await selectRow(user, /\/768/);

    expect(socket.requestsOfKind("body_detail").map((request) => request.body)).toEqual([
      {
        kind: "body_detail",
        universe: aSystemTarget().universe,
        body: FIXTURE_EARTH,
        time: A_CENTURY,
        detail: "full",
      },
    ]);
    expect(within(bodiesPanel()).getByText("PENDING")).toBeInTheDocument();
    // The list's entry reads until the record comes.
    expect(reading("MASS")).toBe("1.00 M");
  });

  it("reads the whole record with a unit on every value", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(reading("KIND")).toBe("PLANET");
    expect(reading("STATE")).toBe("PRESENT");
    expect(reading("PARENT")).toBe("H7K 4C0RFZ D-7 /0");
    expect(reading("DETAIL")).toBe("FULL");
    expect(reading("MASS")).toBe("1.00 M");
    expect(within(readout()).getByRole("img", { name: "Earth masses" })).toBeInTheDocument();
    expect(reading("SMA")).toBe("1.00 AU");
    expect(reading("PERIOD")).toBe("365 d");
    expect(reading("ECC")).toBe("0.0167");
    expect(reading("INC")).toBe("0.0°");
    expect(reading("DIST")).toMatch(/^0\.9\d{2} AU$|^1\.0\d AU$/);
    expect(reading("RADIUS")).toBe("6370 km");
    expect(reading("DENSITY")).toBe("5510 kg/m³");
    expect(reading("GRAVITY")).toBe("9.82 m/s²");
    expect(reading("T EQ")).toBe("255 K");
    // Ruling 112.7: a body with no luminosity of its own has no effective temperature row.
    expect(terms("T EFF")).toHaveLength(0);
    expect(reading("CLASS")).toBe("ROCKY");
    // The layers' quantity named once (signoff-5, item 8).
    expect(reading("MASS FRACTION")).toBe(
      partsReading("IRON 32.3 %", "ROCK 67.7 %", "WATER 0.0 %", "ENVELOPE 0.0 %"),
    );
    expect(within(bodiesPanel()).queryByText("PENDING")).not.toBeInTheDocument();
  });

  it("reads NOT YET MODELLED once for each section not modelled", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    for (const section of ["NAME", "MOONS", "RINGS", "GENERATOR INPUTS"]) {
      expect(terms(section)).toHaveLength(1);
      expect(reading(section)).toBe("NOT YET MODELLED");
    }
    expect(within(readout()).getAllByText("NOT YET MODELLED")).toHaveLength(4);
  });

  it("reads an ok surface's state, mean temperature, pressure and gases", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(terms("SURFACE")).toHaveLength(1);
    expect(reading("SURFACE")).toBe("TEMPERATE");
    expect(reading("T SURF")).toBe("288 K");
    expect(terms("MEAN SURFACE TEMPERATURE")).toHaveLength(0);
    expect(reading("SURFACE PRESSURE")).toBe("101 kPa");
    // Largest first, each by its formula, the quantity named once in the label.
    expect(reading("MOLE FRACTION")).toBe(
      partsReading("N2 98.1 %", "H2O 1.02 %", "Ar 0.900 %", "CO2 0.0300 %"),
    );
  });

  it("leaves out a rocky planet's ATMOSPHERE section, whose air is its surface's", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(terms("ATMOSPHERE")).toHaveLength(0);
  });

  it("shows no EXOSPHERE row for a world with air", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(terms("EXOSPHERE")).toHaveLength(0);
  });

  it("reads an airless world's air as NONE, and its exosphere as NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, airlessEarth());

    expect(reading("SURFACE")).toBe("AIRLESS");
    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(reading("EXOSPHERE")).toBe("NOT YET MODELLED");
  });

  it("reads e101's empty air over its melt as the em dash, and its silicate vapour as NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // As this server and T35.e's send it: no `silicate_vapour` key.
    await answerDetail(socket, e101Earth());

    expect(reading("SURFACE")).toBe("MAGMA OCEAN");
    expect(reading("T SURF")).toBe("1684 K");
    for (const label of ["SURFACE PRESSURE", "MOLE FRACTION"]) {
      expect(reading(label)).toBe("—");
      expect(within(readingOf(label)).getByText("—")).toHaveClass("readout__missing");
    }
    expect(within(readout()).queryByText("NONE")).not.toBeInTheDocument();
    expect(reading("SILICATE VAPOUR")).toBe("NOT YET MODELLED");
    expect(terms("EXOSPHERE")).toHaveLength(0);
  });

  it("reads a magma ocean's steam air by its figures, beside its silicate vapour NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      e101Earth((surface) => ({
        ...surface,
        surface_pressure_pa: 2.5e7,
        gases: [
          { species: "H2O", mole_fraction: 0.75 },
          { species: "CO2", mole_fraction: 0.25 },
        ],
      })),
    );

    expect(reading("SURFACE PRESSURE")).toBe("25.0 MPa");
    expect(reading("MOLE FRACTION")).toBe(partsReading("H2O 75.0 %", "CO2 25.0 %"));
    expect(reading("SILICATE VAPOUR")).toBe("NOT YET MODELLED");
  });

  it("reads a melt's empty air as NONE where its silicate vapour is computed as none", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      e101Earth((surface) => ({ ...surface, silicate_vapour: null })),
    );

    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("reads a melt's empty air as the em dash beside a newer server's silicate vapour, with no row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      e101Earth((surface): BodySurfaceDto => ({
        ...surface,
        // @ts-expect-error -- SilicateVapourDto has no members until P14.T55.b, and a newer server's value must still read.
        silicate_vapour: { vapour_pressure_pa: 10 },
      })),
    );

    expect(reading("SURFACE PRESSURE")).toBe("—");
    expect(reading("MOLE FRACTION")).toBe("—");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("shows no SILICATE VAPOUR row for a temperate world", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("shows no SILICATE VAPOUR row for an airless world, whose air still reads NONE", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, airlessEarth());

    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("names a gas the console does not know as UNLISTED MATERIAL, never by its key", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface((surface) => ({
        ...surface,
        gases: [
          { species: "N2", mole_fraction: 0.99 },
          { species: "novel_gas", mole_fraction: 0.01 },
        ],
      })),
    );

    // A name of two words may break before its figure; the figure keeps its `%`.
    expect(reading("MOLE FRACTION")).toBe(
      `${partsReading("N2 99.0 %")} · UNLISTED MATERIAL 1.00\u00A0%`,
    );
    expect(within(readout()).queryByText(/novel_gas/)).not.toBeInTheDocument();
  });

  // signoff-11, item 21: no part claims the whole beside another, as the survey's coverage reads.
  it.each<readonly [string, ReadonlyArray<GasShareDto>, string]>([
    [
      "reads a gas below the whole whose figures round to 100 as ABOVE 99.9 %",
      [
        { species: "N2", mole_fraction: 0.9996 },
        { species: "CH4", mole_fraction: 0.0004 },
      ],
      partsReading("N2 ABOVE 99.9 %", "CH4 0.0400 %"),
    ],
    [
      "reads a gas just below the edge of the whole at its three figures",
      [
        { species: "N2", mole_fraction: 0.9994 },
        { species: "CH4", mole_fraction: 0.0006 },
      ],
      partsReading("N2 99.9 %", "CH4 0.0600 %"),
    ],
    [
      "reads a single gas, the whole, as 100 %",
      [{ species: "N2", mole_fraction: 1 }],
      partsReading("N2 100 %"),
    ],
  ])("%s", async (_name, gases, moleFraction) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface((surface) => ({ ...surface, gases: [...gases] })),
    );

    expect(reading("MOLE FRACTION")).toBe(moleFraction);
  });

  // signoff-12, item 43: no layer claims the whole beside another, as MOLE FRACTION's gases read.
  it.each<readonly [string, MassFractionsDto, string]>([
    [
      "reads a layer below the whole whose figure rounds to 100.0 as ABOVE 99.9 %",
      { iron: 0.00004, rock: 0.99996, water: 0, envelope: 0 },
      partsReading("IRON 0.0 %", "ROCK ABOVE 99.9 %", "WATER 0.0 %", "ENVELOPE 0.0 %"),
    ],
    [
      "reads a body of one layer alone, the whole, as 100.0 %",
      { iron: 0, rock: 1, water: 0, envelope: 0 },
      partsReading("IRON 0.0 %", "ROCK 100.0 %", "WATER 0.0 %", "ENVELOPE 0.0 %"),
    ],
    [
      "reads a layer just below the edge of the whole at its one decimal",
      { iron: 0.0006, rock: 0.9994, water: 0, envelope: 0 },
      partsReading("IRON 0.1 %", "ROCK 99.9 %", "WATER 0.0 %", "ENVELOPE 0.0 %"),
    ],
  ])("%s", async (_name, massFractions, massFraction) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithMassFractions(massFractions));

    expect(reading("MASS FRACTION")).toBe(massFraction);
  });

  it("reads an older server's surface and atmosphere as NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // A server before P14.T35.e: its surface not modelled, and no envelope at all.
    await answerDetail(socket, earthDetailOfAnOlderServer());

    for (const section of ["SURFACE", "ATMOSPHERE"]) {
      expect(terms(section)).toHaveLength(1);
      expect(reading(section)).toBe("NOT YET MODELLED");
    }
    expect(terms("MOLE FRACTION")).toHaveLength(0);
  });

  it("reads NOT RESOLVED for what a mass-and-orbit record withholds", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthMassAndOrbit());

    expect(reading("DETAIL")).toBe("MASS AND ORBIT ONLY");
    expect(reading("BULK")).toBe("NOT RESOLVED");
    expect(reading("SURFACE")).toBe("NOT RESOLVED");
    // A rocky body's envelope is withheld with its surface, never inferred not to apply.
    expect(reading("ATMOSPHERE")).toBe("NOT RESOLVED");
    expect(reading("GENERATOR INPUTS")).toBe("NOT RESOLVED");
    expect(terms("RADIUS")).toHaveLength(0);
    expect(reading("SMA")).toBe("1.00 AU");
  });

  it.each(["not_modelled", "not_resolved"] as const)(
    "reads a %s DETAIL SEED as the em dash",
    async (state) => {
      const { user, socket } = renderView();
      await answer(socket);
      await selectRow(user, /\/768/);
      await answerDetail(socket, earthWithDetailSeed({ state }));

      expect(terms("GENERATOR INPUTS")).toHaveLength(0);
      expect(reading("DETAIL SEED")).toBe("—");
    },
  );

  it("leaves the DETAIL SEED row out where it does not apply", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithDetailSeed({ state: "not_applicable" }));

    expect(terms("DETAIL SEED")).toHaveLength(0);
  });

  it("reads the DETAIL SEED in upper case once the server sends it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithDetailSeed({ state: "ok", value: "0123456789abcdef" }));

    expect(reading("DETAIL SEED")).toBe("0123456789ABCDEF");
  });

  it("shows no surface row at all for a gas giant, whose surface does not apply", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/1280/);
    await answerDetail(socket, jupiterDetail());

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1280");
    expect(reading("CLASS")).toBe("GAS GIANT");
    // Ruling 112.7: the light's equilibrium temperature, and the giant's own effective one.
    expect(reading("T EQ")).toBe("112 K");
    expect(reading("T EFF")).toBe("129 K");
    expect(terms("SURFACE")).toHaveLength(0);
    expect(reading("GENERATOR INPUTS")).toBe("NOT YET MODELLED");
    expect(reading("MASS")).toBe("318 M");
  });

  it("reads a gas giant's atmosphere from its envelope, in its surface's place", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/1280/);
    await answerDetail(socket, jupiterDetail());

    expect(reading("ATMOSPHERE")).toBe("GAS ENVELOPE");
    expect(reading("MOLE FRACTION")).toBe(partsReading("H2 86.3 %", "He 13.6 %", "CH4 0.180 %"));
  });

  it("reads a gas giant's atmosphere as NOT YET MODELLED while the server does not compute it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/1280/);
    // As every server sends a giant's envelope until P14.T24.d.
    await answerDetail(socket, jupiterWithEnvelope({ state: "not_modelled" }));

    expect(terms("ATMOSPHERE")).toHaveLength(1);
    expect(reading("ATMOSPHERE")).toBe("NOT YET MODELLED");
    expect(terms("MOLE FRACTION")).toHaveLength(0);
  });

  it("selects a planet from the map's list by the keyboard, and the map with it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const tree = screen.getByRole("tree", { name: "Bodies" });

    act(() => {
      tree.focus();
    });
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1280");
    expect(socket.requestsOfKind("body_detail").at(-1)?.body.body).toBe(FIXTURE_JUPITER);
  });
});

describe("SystemView with every kind of body", () => {
  it("lists a destroyed planet with its state in words, and does not draw it", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /512, PLANET, DESTROYED");
    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /768, PLANET, NOT YET FORMED");
    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /1024, PLANET, UNBOUND");
    expect(
      within(mapPanel()).queryByText("H7K 4C0RFZ D-7 /512", { selector: ".spatial-label" }),
    ).not.toBeInTheDocument();

    await selectRow(user, /\/512,/);

    expect(reading("STATE")).toBe("DESTROYED");
    expect(reading("CAUSE")).toBe("ENGULFED");
    expect(reading("SINCE")).toBe("UT -500 yr 000/00:00:00");
    expect(terms("ORBIT")).toHaveLength(0);
  });

  it("reads an unbinding billions of years past in years (P14.T35.d)", async () => {
    const { user, socket } = renderView();
    // About -6.3 Gyr, past the 2^53 s that a JavaScript number holds exactly.
    const longAgo = { seconds: -199_097_968_544_446_944, nanos: 0 };
    await answer(
      socket,
      populatedBodiesWith((body) =>
        body.state.type === "unbound" ? { ...body, state: { type: "unbound", at: longAgo } } : body,
      ),
    );

    await selectRow(user, /\/1024,/);

    expect(reading("STATE")).toBe("UNBOUND");
    expect(reading("SINCE")).toBe("UT -6,309,033,910.83 yr");
  });

  it("names only the moons and rings the tags leave unmodelled", async () => {
    const { socket } = renderView();
    await answer(socket, populatedBodies());

    expect(within(mapPanel()).getByText("MOONS AND RINGS: NOT YET MODELLED")).toBeInTheDocument();
    expect(within(bodiesPanel()).getAllByText("DETAIL")[0]?.nextElementSibling).toHaveTextContent(
      "TO BULK",
    );
  });
});

describe("SystemView with moons, rings, belts and the cometary halo", () => {
  function scaleBar(): string {
    return (
      within(mapPanel())
        .getByRole("img", { name: /^Scale bar/ })
        .getAttribute("aria-label") ?? ""
    );
  }

  function mapLabel(text: string): HTMLElement | null {
    return within(mapPanel()).queryByText(text, { selector: ".spatial-label" });
  }

  it("draws the belt as a labelled annulus and offers BELTS, which fits it", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());
    const belts = within(mapPanel()).getByRole("button", { name: "B BELTS" });

    expect(belts).toHaveAttribute("aria-keyshortcuts", "B");
    expect(belts).toHaveAttribute("aria-pressed", "false");

    await user.keyboard("b");

    expect(belts).toHaveAttribute("aria-pressed", "true");
    expect(scaleBar()).toMatch(/ AU$/);

    // The test's small view has room for the belt's label once the optimistic zone's is gone.
    await user.click(within(mapPanel()).getByRole("button", { name: "OPTIMISTIC" }));

    expect(mapLabel("ASTEROID BELT")).toBeInTheDocument();
  });

  it("offers no BELTS in a system with no belt", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(within(mapPanel()).queryByRole("button", { name: "B BELTS" })).not.toBeInTheDocument();
  });

  it("does not draw the cometary halo, far outside the view, but lists it", async () => {
    const { socket } = renderView();
    await answer(socket, populatedBodies());

    expect(mapLabel("COMETARY HALO")).not.toBeInTheDocument();
    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /57600, COMETARY HALO");
  });

  it("lists moons and rings under their planet and a belt's members under the belt", async () => {
    const { socket } = renderView();
    await answer(socket, populatedBodies());
    const tree = screen.getByRole("tree", { name: "Bodies" });
    const item = (name: RegExp) => within(tree).getByRole("treeitem", { name });

    expect(item(/\/384, RING/)).toHaveAttribute("aria-level", "3");
    expect(item(/\/257, MOON/)).toHaveAttribute("aria-level", "3");
    expect(item(/\/57344, ASTEROID BELT/)).toHaveAttribute("aria-level", "2");
    expect(item(/\/57345, DWARF PLANET/)).toHaveAttribute("aria-level", "3");
    expect(item(/\/57345, DWARF PLANET/)).toHaveAttribute("aria-setsize", "1");
  });

  it("focuses the selected planet with C, in its own frame, and returns with C", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());
    const focus = within(mapPanel()).getByRole("button", { name: "C FOCUS BODY" });

    expect(focus).toHaveAttribute("aria-disabled", "true");
    expect(focus).toHaveAccessibleDescription("NO PLANET SELECTED");
    await user.keyboard("c");
    expect(within(mapPanel()).getByText("SYSTEM BARYCENTRIC")).toBeInTheDocument();
    expect(within(mapPanel()).getByText("SYSTEM BARYCENTRIC")).toBeInTheDocument();

    await selectRow(user, /\/256,/);

    expect(focus).not.toHaveAttribute("aria-disabled");

    await user.keyboard("c");

    expect(focus).toHaveAttribute("aria-pressed", "true");
    expect(within(mapPanel()).getByText("BODY H7K 4C0RFZ D-7 /256")).toBeInTheDocument();
    expect(within(mapPanel()).queryByText("SYSTEM BARYCENTRIC")).not.toBeInTheDocument();
    expect(scaleBar()).toMatch(/ (Mm|km)$/);
    expect(mapLabel("MASSIVE RING")).toBeInTheDocument();
    const legend = screen.getByRole("group", { name: "Orbit map legend" });
    expect(within(legend).getByText("MOON")).toBeInTheDocument();
    expect(within(legend).getByText("FILLED ABOVE EQUATORIAL PLANE")).toBeInTheDocument();
    expect(within(legend).getByText("BODIES NOT TO SCALE")).toBeInTheDocument();
    expect(within(mapPanel()).queryByRole("button", { name: "SNOW LINE" })).not.toBeInTheDocument();

    await user.keyboard("c");

    expect(focus).toHaveAttribute("aria-pressed", "false");
    expect(within(mapPanel()).getByText("SYSTEM BARYCENTRIC")).toBeInTheDocument();
    expect(scaleBar()).toMatch(/ AU$/);
  });

  it("focuses a moon's planet from the moon, and a zoom preset returns to the system", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    await selectRow(user, /\/257,/);
    await user.click(within(mapPanel()).getByRole("button", { name: "C FOCUS BODY" }));

    expect(within(mapPanel()).getByText("BODY H7K 4C0RFZ D-7 /256")).toBeInTheDocument();

    await user.keyboard("a");

    expect(within(mapPanel()).getByText("SYSTEM BARYCENTRIC")).toBeInTheDocument();
    expect(within(mapPanel()).getByRole("button", { name: "A ALL" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("reads a moon's origin and its inclination to its planet's equator", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    await selectRow(user, /\/257,/);

    expect(reading("KIND")).toBe("MOON");
    expect(reading("ORIGIN")).toBe("GIANT IMPACT");
    expect(reading("PARENT")).toBe("H7K 4C0RFZ D-7 /256");
    expect(reading("NAME")).toBe("A b I");
    // The angle between the moon's orbit normal (i 0.09, Ω 1.2) and its planet's (i 1.0, Ω 2.5),
    // which stands for the planet's pole, since the answer sends no rotation section.
    const cosine = dot(
      orbitNormal({ inclinationRad: 0.09, ascendingNodeRad: 1.2 }),
      orbitNormal({ inclinationRad: 1, ascendingNodeRad: 2.5 }),
    );
    expect(reading("INC")).toBe(`${((Math.acos(cosine) * 180) / Math.PI).toFixed(1)}°`);
  });

  it("reads a moon's inclination to the equator its planet's pole gives, where the rotation is sent", async () => {
    const tilted = tiltedPole(populatedBodies(), PLANET_ID, 0.4);
    const { user, socket } = renderView();
    await answer(socket, populatedBodiesWith(withBodyRotation(PLANET_ID, tilted.rotation)));

    await selectRow(user, /\/257,/);

    expect(reading("INC")).toBe(inclinationTo(tilted.pole));
  });

  it("reads a moon's inclination to its planet's pole where the planet's orbit is withheld", async () => {
    const tilted = tiltedPole(populatedBodies(), PLANET_ID, 0.4);
    const rotate = withBodyRotation(PLANET_ID, tilted.rotation);
    const { user, socket } = renderView();
    await answer(
      socket,
      populatedBodiesWith((body) =>
        body.id === PLANET_ID ? { ...rotate(body), orbit: { state: "not_resolved" } } : body,
      ),
    );

    await selectRow(user, /\/257,/);

    expect(reading("INC")).toBe(inclinationTo(tilted.pole));
  });

  it("reads no inclination for a moon whose planet's orbit is withheld and rotation not modelled", async () => {
    const { user, socket } = renderView();
    await answer(
      socket,
      populatedBodiesWith((body) =>
        body.id === PLANET_ID ? { ...body, orbit: { state: "not_resolved" } } : body,
      ),
    );

    await selectRow(user, /\/257,/);

    expect(reading("INC")).toBe("—");
  });

  it("reads no inclination for a moon whose planet's rotation is withheld", async () => {
    const { user, socket } = renderView();
    await answer(
      socket,
      populatedBodiesWith((body) =>
        body.id === PLANET_ID ? { ...body, rotation: { state: "not_resolved" } } : body,
      ),
    );

    await selectRow(user, /\/257,/);

    expect(reading("INC")).toBe("—");
  });

  it("reads a ring's kind, material, edges, optical depth and gaps", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    await selectRow(user, /\/384,/);

    expect(reading("KIND")).toBe("RING");
    expect(reading("NAME")).toBe("NOT YET MODELLED");
    expect(reading("CLASS")).toBe("MASSIVE");
    expect(reading("COMPOSITION")).toBe("POROUS ICE");
    expect(reading("EDGES")).toBe("66.0 – 137 Mm");
    expect(reading("OPTICAL DEPTH")).toBe("0.600");
    expect(reading("GAP 2:1")).toBe("117 Mm");
    expect(terms("ORBIT")).toHaveLength(0);
    expect(terms("SMALL BODIES")).toHaveLength(0);
  });

  it("reads a belt's site, edges, statistics and members", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    await selectRow(user, /\/57344,/);

    expect(reading("SITE")).toBe("INSIDE GIANT ORBIT");
    expect(reading("COMPOSITION")).toBe("ROCKY");
    expect(reading("EDGES")).toBe("2.06 – 3.28 AU");
    expect(reading("SIZE INDEX")).toBe("3.00");
    expect(reading("LARGEST DIAMETER")).toBe("940 km");
    expect(reading("MEAN ECC")).toBe("0.1250");
    expect(reading("MEAN INC")).toBe("10.0°");
    expect(reading("DUST LUMINOSITY")).toBe("1.00E-9 × HOST");
    expect(reading("GAP 3:1")).toBe("2.50 AU");
    expect(reading("MEMBERS")).toBe("1");
    expect(terms("BELT PROPER")).toHaveLength(0);
  });

  it("reads a scattered belt's two components each as one span in one unit", async () => {
    // A Kuiper-like belt: its proper 40 to 48 AU, its scattered disc on to 1,000 AU (ruling 113.3).
    const au = 149_597_870_700;
    const scattered = populatedBodiesWith((body) =>
      body.population.state === "ok" && body.population.value.type === "belt"
        ? {
            ...body,
            population: {
              state: "ok",
              value: {
                ...body.population.value,
                inner_edge_m: 40 * au,
                outer_edge_m: 1000 * au,
                main: { inner_edge_m: 40 * au, outer_edge_m: 48 * au },
                scattered: { inner_edge_m: 48 * au, outer_edge_m: 1000 * au },
                gaps: [],
              },
            },
          }
        : body,
    );
    const { user, socket } = renderView();
    await answer(socket, scattered);

    await selectRow(user, /\/57344,/);

    expect(reading("EDGES")).toBe("40.0 – 1000 AU");
    expect(reading("BELT PROPER")).toBe("40.0 – 48.0 AU");
    expect(reading("SCATTERED DISC")).toBe("48.0 – 1000 AU");
  });

  it("reads NOT RESOLVED for a belt's members withheld below the bulk level", async () => {
    const withheld = populatedBodiesWith((body) =>
      body.population.state === "ok" && body.population.value.type === "belt"
        ? {
            ...body,
            population: {
              state: "ok",
              value: { ...body.population.value, members: { state: "not_resolved" } },
            },
          }
        : body,
    );
    const { user, socket } = renderView();
    await answer(socket, withheld);

    await selectRow(user, /\/57344,/);

    expect(reading("MEMBERS")).toBe("NOT RESOLVED");
  });

  it("reads the cometary halo's edges, comets and comet rate", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    await selectRow(user, /\/57600,/);

    expect(reading("PARENT")).toBe("BARYCENTRE");
    expect(reading("EDGES")).toBe("2000 – 100,000 AU");
    expect(reading("COMETS OVER 1 km")).toBe("7.50E11");
    expect(reading("COMET RATE")).toBe("10.9 /yr");
  });
});

/**
 * The bodies panel's two scrolling regions as the built client laid them out in Electron, with a
 * planet selected in a system of twelve bodies (round 8, measured over the DevTools protocol): at
 * 1920 × 1080 the list's rows had 366 px and the readout 395 px, at 1280 × 720 170 px and 214 px.
 * The readout's 26 readings then ran to 431 px at 1280 px, about 16.6 px apiece as they stand two
 * to a line with some a line of their own; jsdom lays nothing out, so the readings are placed so.
 */
const WINDOWS = [
  { name: "1920 × 1080", listPx: 366, readoutPx: 395 },
  { name: "1280 × 720", listPx: 170, readoutPx: 214 },
] as const;
const READING_PX = 431 / 26;
const ROW_PX = 32;

/** Lays the panel out: each region's visible height, and each reading a line of its own. */
function layOut(listPx: number, readoutPx: number): void {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function height(
    this: HTMLElement,
  ) {
    if (this.getAttribute("role") === "tree") {
      return listPx;
    }
    return this.classList.contains("body-readout__scroll") ? readoutPx : HEIGHT_PX;
  });
  vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function top(
    this: HTMLElement,
  ) {
    const readings = this.closest("dl")?.querySelectorAll("dt");
    const index = readings === undefined ? -1 : [...readings].indexOf(this);
    return index < 0 ? 0 : index * READING_PX;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function tall(
    this: HTMLElement,
  ) {
    return this.tagName === "DT" ? READING_PX - 1 : 0;
  });
  FakeResizeObserver.resizeAll();
}

function readoutRegion(): HTMLElement {
  return screen.getByRole("region", { name: "Readings" });
}

/** The position readout under the readings: `1-13 of 26`. */
function readingsPosition(): string {
  return readoutRegion().nextElementSibling?.textContent ?? "";
}

describe.each(WINDOWS)("SystemView's bodies panel at $name", ({ listPx, readoutPx }) => {
  it("scrolls the planet's readings, with those in view and their total under them", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const total = within(readout()).getAllByRole("term").length;
    const inView = Math.min(total, Math.ceil(readoutPx / READING_PX));

    expect(readingsPosition()).toBe(`1-${inView} of ${total}`);

    const region = readoutRegion();
    act(() => {
      region.scrollTop = total * READING_PX - readoutPx;
      fireEvent.scroll(region);
    });

    expect(readingsPosition()).toMatch(new RegExp(`^\\d+-${total} of ${total}$`));
  });

  it("keeps the focus on the list and the selected row in view as the keys select", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const tree = screen.getByRole("tree", { name: "Bodies" });
    const rows = within(tree).getAllByRole("treeitem");
    act(() => {
      tree.focus();
    });

    await user.keyboard("{End}{Enter}");

    expect(tree).toHaveFocus();
    expect(rows.at(-1)).toHaveAttribute("aria-selected", "true");
    // The last row's bottom edge meets the list's, or the list holds every row.
    expect(tree.scrollTop).toBe(Math.max(0, rows.length * ROW_PX - listPx));
  });

  it("returns the readings to their top when another body is selected", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const region = readoutRegion();
    act(() => {
      region.scrollTop = 120;
      fireEvent.scroll(region);
    });

    await selectRow(user, /\/1280/);

    expect(region.scrollTop).toBe(0);
    expect(readingsPosition()).toMatch(/^1-/);
  });
});
