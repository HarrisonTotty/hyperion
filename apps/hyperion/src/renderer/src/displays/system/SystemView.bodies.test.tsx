/**
 * The `SYSTEM` display's planets over a fake socket, against the shared wire fixture (plan 14,
 * P14.T41.b, T42 and T43 for bodies).
 */
import type {
  BodyEnvelopeDto,
  BodySurfaceDto,
  CrustCompositionDto,
  GasShareDto,
  MassFractionsDto,
  ResponseFor,
  SectionDto,
  SubstanceAreaDto,
  SubsurfaceOceanDto,
  SurfaceCondensateDto,
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
  earthUnderASupercriticalEnvelope,
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  jupiterDetail,
  lavaWorldDetail,
  populatedBodies,
  populatedBodiesWith,
  sliceBodies,
  sliceBodiesWith,
  tiltedPole,
  titanDetail,
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

const WIDTH_PX = 1214;
const HEIGHT_PX = 625;

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

/**
 * The readout's rows in order, each its label and its reading, from the one labelled `first` up
 * to, and not including, the one labelled `next`.
 */
function rowsBetween(first: string, next: string): Array<readonly [string, string]> {
  const rows = within(readout())
    .queryAllByRole("term")
    .map((term) => [term.textContent, term.nextElementSibling?.textContent ?? ""] as const);
  const start = rows.findIndex(([label]) => label === first);
  const end = rows.findIndex(([label]) => label === next);
  if (start < 0 || end < start) {
    throw new Error(`the readout reads ${first}, then ${next}`);
  }
  return rows.slice(start, end);
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

/** The `ok` surface section of the fixture's record `detail`. */
function surfaceOf(detail: ResponseFor<"body_detail">): BodySurfaceDto {
  const { surface } = detail.record;
  if (surface.state !== "ok") {
    throw new Error(`the fixture's ${detail.record.id} has a surface`);
  }
  return surface.value;
}

/**
 * The fixture's Earth with its `ok` surface changed by `change`, which may give it another body's
 * surface section of the fixture, as the readout reads that body's surface rows.
 */
function earthWithSurface(
  change: (surface: BodySurfaceDto) => BodySurfaceDto,
): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  return {
    ...detail,
    record: { ...detail.record, surface: { state: "ok", value: change(surfaceOf(detail)) } },
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

/**
 * The fixture's Earth made airless, as the server sends such a body: no air, its figures none, and
 * its crust's melt area 0.
 */
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

/** The crust the fixture's `ok` surface `surface` sends. */
function crustOf(surface: BodySurfaceDto): CrustCompositionDto {
  const { crust } = surface;
  if (crust === undefined || crust === null) {
    throw new Error("the fixture's surface sends a crust");
  }
  return crust;
}

/** The fixture's Earth with its surface's crust changed by `change`. */
function earthWithCrust(
  change: (crust: CrustCompositionDto) => CrustCompositionDto | null,
): ResponseFor<"body_detail"> {
  return earthWithSurface((surface) => ({ ...surface, crust: change(crustOf(surface)) }));
}

/**
 * The fixture's Earth sent as `close_binary`'s dwarf planet `e101` is (P14.T35.f): a magma ocean at
 * 1,684 K whose volatiles are gone, its volatile air empty and its mixture figures none, and no
 * `silicate_vapour` key; then changed by `change`.
 *
 * @remarks
 * It keeps the Earth's crust, with its melt area set to 1, the whole, as the server sends every
 * magma ocean's (the sim's `melt_area` returns 1 for `MagmaOcean`), so that it reads
 * `MELT COVER 100 %`. Its molten surface holds no condensate, so its cover is empty, as no water
 * lies liquid or frozen at 1,684 K.
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
      crust: { ...crustOf(surface), melt_area_fraction: 1 },
      condensates: [],
      surface_liquids: [],
      ocean_fraction: 0,
      surface_ices: [],
      ice_fraction: 0,
    });
  });
}

/**
 * The fixture's Titan's `ok` surface section with no `subsurface_ocean` key, as a planet's wire
 * sends it, so that a Snowball's `SUBSURFACE OCEAN` row stands by its cover alone (P14.T43.h).
 */
function titanWithNoOceanKey(): BodySurfaceDto {
  const { subsurface_ocean: _ocean, ...titan } = surfaceOf(titanDetail());
  return titan;
}

/** The ocean under the fixture's Titan's ice, as its `ok` surface section sends it. */
function titanOcean(): SubsurfaceOceanDto {
  const { subsurface_ocean: ocean } = surfaceOf(titanDetail());
  if (ocean === undefined || ocean === null) {
    throw new Error("the fixture's Titan sends its subsurface ocean");
  }
  return ocean;
}

/** LOOK72's `020e56ef60080003.0700`'s water ice, over its whole surface. */
const ICE_0700: SurfaceCondensateDto = {
  substance: "H2O",
  phase: "solid",
  reservoir_kg_m2: 117575983.2,
  reservoir_gel_m: 126151.05,
  area_fraction: 1,
  seasonal: false,
};

/** LOOK72's `020e56ef60080003.0700`'s methane–ethane sea, on its ice. */
const METHANE_SEA_0700: SurfaceCondensateDto = {
  substance: "ch4_c2h6_n2",
  phase: "liquid",
  reservoir_kg_m2: 17237308.17,
  reservoir_gel_m: 33218.94,
  area_fraction: 0.9727697189147213,
  seasonal: false,
};

/** LOOK72's `020e56ef60080003.0700`'s nitrogen sea, on its ice. */
const NITROGEN_SEA_0700: SurfaceCondensateDto = {
  substance: "N2",
  phase: "liquid",
  reservoir_kg_m2: 291426.2,
  reservoir_gel_m: 929.88,
  area_fraction: 0.02723028108527855,
  seasonal: true,
};

/**
 * LOOK72's `020e56ef60080003.0700`, a Snowball at 126 K whose whole surface lies under water ice,
 * its seas of methane–ethane and nitrogen on the ice, on the fixture's Titan, whose air and crust
 * it keeps (P14.T43.h). Its condensates list no `H2O` liquid, so the wire appends the ocean under
 * its ice to its liquids as an `H2O` item at 0.5, and divides the list by its total, 2, halving
 * the seas' shares.
 */
function snowball0700(): BodySurfaceDto {
  return {
    ...titanWithNoOceanKey(),
    state: "snowball",
    mean_surface_temperature_k: 126.33534432935379,
    condensates: [ICE_0700, METHANE_SEA_0700, NITROGEN_SEA_0700],
    surface_liquids: [
      { substance: "ch4_c2h6_n2", area_fraction: 0.48638485945736065 },
      { substance: "N2", area_fraction: 0.013615140542639275 },
      { substance: "H2O", area_fraction: 0.5 },
    ],
    ocean_fraction: 1,
    surface_ices: [{ substance: "H2O", area_fraction: 1 }],
    ice_fraction: 1,
  };
}

/**
 * {@link snowball0700} with a sea of liquid water over half its surface listed among its
 * condensates, its reservoir a stand-in, and its cover lists as {@link snowball0700}'s.
 */
function snowball0700WithWaterSea(): BodySurfaceDto {
  return {
    ...snowball0700(),
    condensates: [
      ICE_0700,
      METHANE_SEA_0700,
      NITROGEN_SEA_0700,
      {
        substance: "H2O",
        phase: "liquid",
        reservoir_kg_m2: 1000,
        area_fraction: 0.5,
        seasonal: false,
      },
    ],
  };
}

/**
 * {@link snowball0700} as a server whose generator models no condensates sends it: no
 * `condensates` key.
 */
function snowball0700OfAnOlderServer(): BodySurfaceDto {
  const { condensates: _condensates, ...older } = snowball0700();
  return older;
}

/**
 * The fixture's Earth with its liquid water's entry left out of its condensates and its `H2O` item
 * kept, a list no server sends, so that only its `temperate` state tells it from a Snowball whose
 * ocean lies under its ice.
 */
function earthWithNoLiquidWaterEntry(): ResponseFor<"body_detail"> {
  return earthWithSurface((surface) => {
    const { condensates } = surface;
    if (condensates === undefined || condensates === null) {
      throw new Error("the fixture's Earth sends its condensates");
    }
    return {
      ...surface,
      condensates: condensates.filter(
        (entry) => !(entry.substance === "H2O" && entry.phase === "liquid"),
      ),
    };
  });
}

/**
 * LOOK72's `620e377802000074.0703`, a Snowball at 83 K under water and CO2 ice whose only liquid is
 * the ocean under its ice, on the fixture's Titan, as {@link snowball0700} is.
 *
 * @remarks
 * Its areas are the census's; its reservoirs, which the readout does not read, stand in.
 */
function snowball0703(): BodySurfaceDto {
  return {
    ...titanWithNoOceanKey(),
    state: "snowball",
    mean_surface_temperature_k: 82.88430268358076,
    condensates: [
      ICE_0700,
      {
        substance: "CO2",
        phase: "solid",
        reservoir_kg_m2: 1000,
        area_fraction: 0.022461915318924903,
        seasonal: false,
      },
    ],
    surface_liquids: [{ substance: "H2O", area_fraction: 1 }],
    ocean_fraction: 1,
    surface_ices: [
      { substance: "H2O", area_fraction: 0.9780315384051066 },
      { substance: "CO2", area_fraction: 0.021968461594893355 },
    ],
    ice_fraction: 1,
  };
}

/**
 * A Europa-like airless icy moon, its whole surface under water ice and an ocean of NaCl brine
 * under the ice, which its own `subsurface_ocean` states, on {@link titanWithNoOceanKey}'s surface,
 * its air and its figures set as {@link airlessEarth} sets them (P14.T43.i).
 *
 * @remarks
 * Its temperature and its ocean's figures are stand-ins: the readout reads the ocean's liquid
 * alone. It keeps the fixture's Titan's crust, which reads, and its other members, its craters,
 * climate regime and wet epoch among them, which the readout does not read. Its ice,
 * {@link ICE_0700}, carries no trapped share, so its `COLD TRAPS` row stands.
 */
function airlessBrineMoon(): BodySurfaceDto {
  return {
    ...titanWithNoOceanKey(),
    state: "airless",
    mean_surface_temperature_k: 102,
    surface_pressure_pa: 0,
    gases: [],
    mean_molar_mass_g_per_mol: null,
    heat_capacity_over_r: null,
    condensates: [ICE_0700],
    surface_liquids: [],
    ocean_fraction: 0,
    surface_ices: [{ substance: "H2O", area_fraction: 1 }],
    ice_fraction: 1,
    subsurface_ocean: {
      shell_thickness_m: 20000,
      thickness_m: 100000,
      floor: "rock",
      liquid: "brine_nacl",
      solute_mass_fraction: 0.05,
      base_temperature_k: 270,
    },
  };
}

/**
 * {@link airlessBrineMoon} with no `subsurface_ocean` key, its ocean not modelled, as the wire
 * sends a body whose ocean the generator does not compute (P14.T51.g.b).
 */
function airlessIcyMoonWithNoOceanKey(): BodySurfaceDto {
  const { subsurface_ocean: _ocean, ...moon } = airlessBrineMoon();
  return moon;
}

/**
 * `detail`, a record with an `ok` surface section, with its subsurface ocean computed as none, as
 * a version-24 server sends a body with no water or with a surface at or above water's triple
 * point (P14.T51.g.b).
 */
function oceanComputedAsNone(detail: ResponseFor<"body_detail">): ResponseFor<"body_detail"> {
  return {
    ...detail,
    record: {
      ...detail.record,
      surface: { state: "ok", value: { ...surfaceOf(detail), subsurface_ocean: null } },
    },
  };
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

  it("reads NOT YET MODELLED once for each section not modelled, and once for the Earth's subsurface ocean", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    for (const section of ["NAME", "MOONS", "RINGS", "GENERATOR INPUTS"]) {
      expect(terms(section)).toHaveLength(1);
      expect(reading(section)).toBe("NOT YET MODELLED");
    }
    // The four sections, and a row inside the shown surface section: the fixture's Earth carries no
    // subsurface ocean, as a version-23 server sends it, so its ocean is not modelled (P14.T51.g.b).
    expect(within(readout()).getAllByText("NOT YET MODELLED")).toHaveLength(5);
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
    // None of the air's rows reads NONE; the ground's after them may (P14.T43.g).
    expect(rowsBetween("SURFACE", "CRUST").map(([, read]) => read)).not.toContain("NONE");
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

  it("shows no SILICATE VAPOUR row for an airless world whose crust holds no melt, its air still NONE", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, airlessEarth());

    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("shows no SILICATE VAPOUR row for an older server's airless world, which sends no crust", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => {
        const { crust: _crust, ...older } = surfaceOf(airlessEarth());
        return older;
      }),
    );

    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(reading("EXOSPHERE")).toBe("NOT YET MODELLED");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("reads a locked lava world's empty air over its dayside pool as the em dash, beside its exosphere and its silicate vapour NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The surface section of T51.c's locked lava world as the server sends it (P14.T54.a):
    // `airless`, its crust's melt area, its dayside pool, above 0, and no `silicate_vapour` key.
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(lavaWorldDetail())),
    );

    expect(reading("SURFACE")).toBe("AIRLESS");
    for (const label of ["SURFACE PRESSURE", "MOLE FRACTION"]) {
      expect(reading(label)).toBe("—");
      expect(within(readingOf(label)).getByText("—")).toHaveClass("readout__missing");
    }
    // None of the air's rows reads NONE; its empty cover, after them, does (P14.T43.g).
    expect(rowsBetween("SURFACE", "CRUST").map(([, read]) => read)).not.toContain("NONE");
    // Each on its own line (rendering plan R08, Design note 12).
    expect(terms("EXOSPHERE")).toHaveLength(1);
    expect(reading("EXOSPHERE")).toBe("NOT YET MODELLED");
    expect(terms("SILICATE VAPOUR")).toHaveLength(1);
    expect(reading("SILICATE VAPOUR")).toBe("NOT YET MODELLED");
  });

  it("reads a lava world's empty air as NONE where its silicate vapour is computed as none", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => ({ ...surfaceOf(lavaWorldDetail()), silicate_vapour: null })),
    );

    expect(reading("SURFACE PRESSURE")).toBe("NONE");
    expect(reading("MOLE FRACTION")).toBe("NONE");
    expect(reading("EXOSPHERE")).toBe("NOT YET MODELLED");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("shows no SILICATE VAPOUR row over a Titan's ice lid, whose melt area is 0", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The fixture's Titan's surface section: an `H2O` lid whose melt area is 0, since its lakes
    // are a condensate, not a melt of its crust.
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(titanDetail())),
    );

    expect(reading("SURFACE")).toBe("SNOWBALL");
    expect(terms("SILICATE VAPOUR")).toHaveLength(0);
  });

  it("reads a Titan's N2 and CH4 air at its measured surface, with no EXOSPHERE row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The fixture's Titan's surface section: Huygens's 93.65 K and 146.7 kPa, and CH4 5.65 % by
    // mole near the surface (`packages/protocol/src/planetary.test.ts` cites each figure).
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(titanDetail())),
    );

    expect(reading("T SURF")).toBe("94 K");
    expect(reading("SURFACE PRESSURE")).toBe("147 kPa");
    expect(reading("MOLE FRACTION")).toBe(partsReading("N2 94.4 %", "CH4 5.65 %"));
    expect(terms("EXOSPHERE")).toHaveLength(0);
  });

  // P14.T43.g (decision-ux-23-surface-rows): the ground's rows after the air's.
  it("reads a locked lava world's thirteen surface rows in order, its crust, melt and cover after its air", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(lavaWorldDetail())),
    );

    // The fixture Earth's record reads its hooks next, as `GENERATOR INPUTS NOT YET MODELLED`, its
    // envelope not applying.
    expect(rowsBetween("SURFACE", "GENERATOR INPUTS")).toEqual([
      ["SURFACE", "AIRLESS"],
      ["T SURF", "1239 K"],
      ["SURFACE PRESSURE", "—"],
      ["MOLE FRACTION", "—"],
      ["EXOSPHERE", "NOT YET MODELLED"],
      ["SILICATE VAPOUR", "NOT YET MODELLED"],
      ["CRUST", "SECONDARY · REDUCED BASALT"],
      ["MANTLE REDOX", "REDUCED"],
      // Its melt area, 0.24995…, to three figures; the `%` in the unit slot.
      ["MELT COVER", "25.0 %"],
      ["LIQUID COVER", "NONE"],
      ["ICE COVER", "NONE"],
      ["COLD TRAPS", "NOT YET MODELLED"],
      // It carries no subsurface ocean, as a version-23 server sends it (P14.T51.g.b).
      ["SUBSURFACE OCEAN", "NOT YET MODELLED"],
    ]);
  });

  it("reads a Titan's ice crust, its lakes and its ice, and MELT COVER NONE", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(titanDetail())),
    );

    expect(reading("CRUST")).toBe("PRIMARY · H2O");
    expect(reading("MANTLE REDOX")).toBe("OXIDISED");
    expect(reading("MELT COVER")).toBe("NONE");
    // A name of several words may break before its figure; the figure keeps its `%`.
    expect(reading("LIQUID COVER")).toBe("CH4–C2H6–N2 MIXTURE 1.00\u00A0%");
    expect(reading("ICE COVER")).toBe(partsReading("H2O 99.0 %"));
  });

  it("reads the Earth's crust, its mantle's redox class and its cover", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(reading("CRUST")).toBe("SECONDARY · BASALT");
    expect(reading("MANTLE REDOX")).toBe("OXIDISED");
    expect(reading("MELT COVER")).toBe("NONE");
    expect(reading("LIQUID COVER")).toBe(partsReading("H2O 71.0 %"));
    expect(reading("ICE COVER")).toBe(partsReading("H2O 3.00 %"));
  });

  it.each<readonly [string, () => ResponseFor<"body_detail">]>([
    ["the Earth", earthDetail],
    ["a Titan", () => earthWithSurface(() => surfaceOf(titanDetail()))],
  ])("shows no COLD TRAPS row for %s, a world with air", async (_name, detail) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, detail());

    expect(terms("COLD TRAPS")).toHaveLength(0);
  });

  it("reads e101's melt over the whole surface as MELT COVER 100 %", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, e101Earth());

    expect(reading("MELT COVER")).toBe("100 %");
  });

  // `ABOVE` keeps its figure across a no-break space; the `%` is in the unit slot.
  it.each<readonly [string, number, string]>([
    [
      "reads a melt below the whole whose figures round to 100 as ABOVE 99.9 %",
      0.99996,
      "ABOVE\u00A099.9 %",
    ],
    ["reads a melt just below the edge of the whole at its three figures", 0.9994, "99.9 %"],
    ["reads a melt below 0.001 % in E notation", 1e-6, "1.00E-4 %"],
  ])("%s", async (_name, meltAreaFraction, meltCover) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithCrust((crust) => ({ ...crust, melt_area_fraction: meltAreaFraction })),
    );

    expect(reading("MELT COVER")).toBe(meltCover);
  });

  it("reads a crust not modelled as CRUST NOT YET MODELLED once, with no MANTLE REDOX or MELT COVER row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // As every server before P14.T54.a sends it: no `crust` key.
    await answerDetail(
      socket,
      earthWithSurface((surface) => {
        const { crust: _crust, ...older } = surface;
        return older;
      }),
    );

    expect(terms("CRUST")).toHaveLength(1);
    expect(reading("CRUST")).toBe("NOT YET MODELLED");
    expect(terms("MANTLE REDOX")).toHaveLength(0);
    expect(terms("MELT COVER")).toHaveLength(0);
    // Its condensates are modelled, so its cover reads beside it.
    expect(reading("LIQUID COVER")).toBe(partsReading("H2O 71.0 %"));
  });

  it("reads a crust computed as none as CRUST NONE, with no MANTLE REDOX or MELT COVER row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithCrust(() => null),
    );

    expect(terms("CRUST")).toHaveLength(1);
    expect(reading("CRUST")).toBe("NONE");
    expect(terms("MANTLE REDOX")).toHaveLength(0);
    expect(terms("MELT COVER")).toHaveLength(0);
  });

  it("reads an undifferentiated body's crust by its own material, never as BULK", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithCrust((crust) => ({
        ...crust,
        primary: null,
        secondary: null,
        tertiary: null,
        bulk: "chondrite_ci",
        lid: "bulk",
        provinces: "chondrite_ci",
        redox: "intermediate",
      })),
    );

    expect(reading("CRUST")).toBe("UNDIFFERENTIATED · CI CHONDRITE");
    expect(reading("MANTLE REDOX")).toBe("INTERMEDIATE");
  });

  it.each<readonly [string, ReadonlyArray<SubstanceAreaDto>, string]>([
    [
      "reads a cover sent out of order largest first, and leaves out a part at 0",
      [
        { substance: "H2O", area_fraction: 0.1 },
        { substance: "ch4_c2h6_n2", area_fraction: 0 },
        { substance: "CO2", area_fraction: 0.3 },
      ],
      partsReading("CO2 30.0 %", "H2O 10.0 %"),
    ],
    [
      "reads a cover's equal shares in the wire's order",
      [
        { substance: "N2", area_fraction: 0.05 },
        { substance: "CH4", area_fraction: 0.2 },
        { substance: "CO", area_fraction: 0.05 },
      ],
      partsReading("CH4 20.0 %", "N2 5.00 %", "CO 5.00 %"),
    ],
    [
      "reads a cover whose every part is 0 as NONE",
      [{ substance: "H2O", area_fraction: 0 }],
      "NONE",
    ],
  ])("%s", async (_name, cover, read) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // Each list's total is the section's own, which the decoder checks it against.
    const total = cover.reduce((sum, area) => sum + area.area_fraction, 0);
    await answerDetail(
      socket,
      earthWithSurface((surface) => ({
        ...surface,
        surface_liquids: [...cover],
        ocean_fraction: total,
        surface_ices: [...cover],
        ice_fraction: total,
      })),
    );

    expect(reading("LIQUID COVER")).toBe(read);
    expect(reading("ICE COVER")).toBe(read);
  });

  it("reads a cover as NOT YET MODELLED where the condensates are not modelled, its lists water alone", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The fixture's Titan as a server before P14.T54.a sends it: no `condensates` or `crust` key,
    // and its cover lists its water alone, the lakes of methane not computed.
    await answerDetail(
      socket,
      earthWithSurface(() => {
        const { condensates: _condensates, crust: _crust, ...older } = surfaceOf(titanDetail());
        return {
          ...older,
          surface_liquids: [{ substance: "H2O", area_fraction: 0 }],
          ocean_fraction: 0,
          surface_ices: [{ substance: "H2O", area_fraction: 0.99 }],
          ice_fraction: 0.99,
        };
      }),
    );

    expect(reading("LIQUID COVER")).toBe("NOT YET MODELLED");
    expect(reading("ICE COVER")).toBe("NOT YET MODELLED");
  });

  // P14.T43.h (decision-absurd-states-24): the ocean under a Snowball's ice covers none of it.
  it("reads a Snowball's seas on its ice at their own extents, and the ocean under the ice last, as SUBSURFACE OCEAN", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(snowball0700));

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS")).toEqual([
      ["SURFACE", "SNOWBALL"],
      ["T SURF", "126 K"],
      ["SURFACE PRESSURE", "147 kPa"],
      ["MOLE FRACTION", partsReading("N2 94.4 %", "CH4 5.65 %")],
      ["CRUST", "PRIMARY · H2O"],
      ["MANTLE REDOX", "OXIDISED"],
      ["MELT COVER", "NONE"],
      // The seas' entries, 0.97277 and 0.02723, not the wire's halved shares; no `H2O` part.
      ["LIQUID COVER", "CH4–C2H6–N2 MIXTURE 97.3\u00A0% · N2\u00A02.72\u00A0%"],
      ["ICE COVER", partsReading("H2O 100 %")],
      ["SUBSURFACE OCEAN", "H2O"],
    ]);
  });

  it("reads LIQUID COVER NONE on a Snowball whose only liquid is the ocean under its ice, and the ocean in its own row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(snowball0703));

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS")).toEqual([
      ["SURFACE", "SNOWBALL"],
      ["T SURF", "83 K"],
      ["SURFACE PRESSURE", "147 kPa"],
      ["MOLE FRACTION", partsReading("N2 94.4 %", "CH4 5.65 %")],
      ["CRUST", "PRIMARY · H2O"],
      ["MANTLE REDOX", "OXIDISED"],
      ["MELT COVER", "NONE"],
      ["LIQUID COVER", "NONE"],
      ["ICE COVER", partsReading("H2O 97.8 %", "CO2 2.20 %")],
      ["SUBSURFACE OCEAN", "H2O"],
    ]);
  });

  it("reads SUBSURFACE OCEAN's liquid alone, with no unit", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(snowball0700));

    expect(readingOf("SUBSURFACE OCEAN").querySelector(".body-readout__unit")).toBeNull();
  });

  it("divides a Snowball's seas on its ice by their total where it exceeds the whole", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The seas' entries, 0.8 and 0.4, sum to 1.2; the wire's list, 0.4, 0.2 and the ocean's 0.4,
    // to 1.
    await answerDetail(
      socket,
      earthWithSurface(() => ({
        ...snowball0700(),
        condensates: [
          ICE_0700,
          { ...METHANE_SEA_0700, area_fraction: 0.8 },
          { ...NITROGEN_SEA_0700, area_fraction: 0.4 },
        ],
        surface_liquids: [
          { substance: "ch4_c2h6_n2", area_fraction: 0.4 },
          { substance: "N2", area_fraction: 0.2 },
          { substance: "H2O", area_fraction: 0.4 },
        ],
        ocean_fraction: 1,
      })),
    );

    expect(reading("LIQUID COVER")).toBe("CH4–C2H6–N2 MIXTURE 66.7\u00A0% · N2\u00A033.3\u00A0%");
    expect(reading("SUBSURFACE OCEAN")).toBe("H2O");
  });

  it("keeps a Snowball's sea at its listed share where its liquid entry is not listed", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // A list the contract never sends, with the nitrogen sea's item and no entry: the rule's
    // fallback reads the share as listed.
    await answerDetail(
      socket,
      earthWithSurface(() => ({
        ...snowball0700(),
        condensates: [ICE_0700, METHANE_SEA_0700],
      })),
    );

    expect(reading("LIQUID COVER")).toBe("CH4–C2H6–N2 MIXTURE 97.3\u00A0% · N2\u00A01.36\u00A0%");
  });

  // decision-ux-ocean-row §1: an ocean computed as none is a value, read as the section's other
  // rows read a quantity computed and empty.
  it.each<readonly [string, () => ResponseFor<"body_detail">]>([
    [
      "the fixture's Titan, its ocean computed as none",
      () => earthWithSurface(() => ({ ...surfaceOf(titanDetail()), subsurface_ocean: null })),
    ],
    ["the fixture's Earth, its ocean computed as none", () => oceanComputedAsNone(earthDetail())],
    [
      "the fixture's lava world, its ocean computed as none",
      () => oceanComputedAsNone(earthWithSurface(() => surfaceOf(lavaWorldDetail()))),
    ],
    [
      "a Snowball whose condensates list liquid water, its ocean computed as none",
      () => oceanComputedAsNone(earthWithSurface(snowball0700WithWaterSea)),
    ],
    [
      "a temperate surface whose condensates list no liquid water, its ocean computed as none",
      () => oceanComputedAsNone(earthWithNoLiquidWaterEntry()),
    ],
  ])("reads SUBSURFACE OCEAN NONE, the last surface row, for %s", async (_name, detail) => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, detail());

    expect(terms("SUBSURFACE OCEAN")).toHaveLength(1);
    expect(reading("SUBSURFACE OCEAN")).toBe("NONE");
    expect(rowsBetween("SURFACE", "GENERATOR INPUTS").at(-1)).toEqual(["SUBSURFACE OCEAN", "NONE"]);
  });

  // P14.T51.g.b (decision-p14-t51gb-ice-ii §3): an ocean not modelled is never read as none.
  it.each<readonly [string, () => ResponseFor<"body_detail">]>([
    [
      "a Snowball whose condensates list liquid water",
      () => earthWithSurface(snowball0700WithWaterSea),
    ],
    ["a temperate Earth, its water a sea", earthDetail],
    ["a temperate surface whose condensates list no liquid water", earthWithNoLiquidWaterEntry],
    [
      "a Snowball whose condensates are not modelled",
      () => earthWithSurface(snowball0700OfAnOlderServer),
    ],
    ["the fixture's lava world", () => earthWithSurface(() => surfaceOf(lavaWorldDetail()))],
  ])(
    "reads SUBSURFACE OCEAN NOT YET MODELLED, the last surface row, for %s, its ocean not modelled",
    async (_name, detail) => {
      const { user, socket } = renderView();
      await answer(socket);
      await selectRow(user, /\/768/);
      await answerDetail(socket, detail());

      expect(terms("SUBSURFACE OCEAN")).toHaveLength(1);
      expect(reading("SUBSURFACE OCEAN")).toBe("NOT YET MODELLED");
      expect(rowsBetween("SURFACE", "GENERATOR INPUTS").at(-1)).toEqual([
        "SUBSURFACE OCEAN",
        "NOT YET MODELLED",
      ]);
    },
  );

  it("reads a Snowball's listed sea of liquid water in LIQUID COVER, as the wire's list gives it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(snowball0700WithWaterSea));

    expect(reading("LIQUID COVER")).toBe(
      "H2O\u00A050.0\u00A0% · CH4–C2H6–N2 MIXTURE 48.6\u00A0% · N2\u00A01.36\u00A0%",
    );
  });

  it("reads a Snowball's cover as NOT YET MODELLED where its condensates are not modelled", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(snowball0700OfAnOlderServer));

    expect(reading("LIQUID COVER")).toBe("NOT YET MODELLED");
    expect(reading("ICE COVER")).toBe("NOT YET MODELLED");
  });

  // P14.T43.i (decision-p14-t43h-review §4): the ocean the wire states, under any body's ice.
  it("reads the ocean under the fixture's Titan's ice, which its wire states, last, as SUBSURFACE OCEAN", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => surfaceOf(titanDetail())),
    );

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS")).toEqual([
      ["SURFACE", "SNOWBALL"],
      ["T SURF", "94 K"],
      ["SURFACE PRESSURE", "147 kPa"],
      ["MOLE FRACTION", partsReading("N2 94.4 %", "CH4 5.65 %")],
      ["CRUST", "PRIMARY · H2O"],
      ["MANTLE REDOX", "OXIDISED"],
      ["MELT COVER", "NONE"],
      ["LIQUID COVER", "CH4–C2H6–N2 MIXTURE 1.00\u00A0%"],
      ["ICE COVER", partsReading("H2O 99.0 %")],
      ["SUBSURFACE OCEAN", "H2O"],
    ]);
  });

  it("reads an airless icy moon's brine ocean under its ice after its COLD TRAPS row", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(airlessBrineMoon));

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS")).toEqual([
      ["SURFACE", "AIRLESS"],
      ["T SURF", "102 K"],
      ["SURFACE PRESSURE", "NONE"],
      ["MOLE FRACTION", "NONE"],
      ["EXOSPHERE", "NOT YET MODELLED"],
      ["CRUST", "PRIMARY · H2O"],
      ["MANTLE REDOX", "OXIDISED"],
      ["MELT COVER", "NONE"],
      ["LIQUID COVER", "NONE"],
      ["ICE COVER", partsReading("H2O 100 %")],
      ["COLD TRAPS", "NOT YET MODELLED"],
      ["SUBSURFACE OCEAN", "NaCl BRINE"],
    ]);
  });

  it("reads one SUBSURFACE OCEAN row, the wire's ocean's liquid, where a Snowball's cover holds an ocean under its ice too", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => ({
        ...snowball0700(),
        subsurface_ocean: { ...titanOcean(), liquid: "brine_nacl" },
      })),
    );

    expect(terms("SUBSURFACE OCEAN")).toHaveLength(1);
    expect(reading("SUBSURFACE OCEAN")).toBe("NaCl BRINE");
    expect(reading("LIQUID COVER")).toBe("CH4–C2H6–N2 MIXTURE 97.3\u00A0% · N2\u00A02.72\u00A0%");
  });

  it("reads the ocean under a Snowball's ice from its cover where its wire computes its subsurface ocean as none", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // To generator version 23 the cover's ocean stands before the member's `NONE` (rule 5;
    // decision-ux-ocean-row §4): one row, the cover's.
    await answerDetail(
      socket,
      earthWithSurface(() => ({ ...snowball0700(), subsurface_ocean: null })),
    );

    expect(terms("SUBSURFACE OCEAN")).toHaveLength(1);
    expect(reading("SUBSURFACE OCEAN")).toBe("H2O");
  });

  it("names a subsurface ocean's liquid the console does not know as UNLISTED MATERIAL, never by its key", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface(() => ({
        ...surfaceOf(titanDetail()),
        subsurface_ocean: { ...titanOcean(), liquid: "novel_liquid" },
      })),
    );

    expect(reading("SUBSURFACE OCEAN")).toBe("UNLISTED MATERIAL");
    expect(within(readout()).queryByText(/novel_/)).not.toBeInTheDocument();
  });

  // P14.T51.g.b (decision-p14-t51gb-ice-ii §3): a Triton-like ocean the generator does not model.
  it("reads SUBSURFACE OCEAN NOT YET MODELLED last on a Triton-like icy surface, its ocean not modelled and none under its ice in its cover", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // A Snowball with no `H2O` liquid item, so no ocean under its ice from its cover, and no
    // `subsurface_ocean` key, as a version-24 server sends Triton's, whose ocean's floor would lie
    // in ice II's field.
    await answerDetail(socket, earthWithSurface(titanWithNoOceanKey));

    expect(terms("SUBSURFACE OCEAN")).toHaveLength(1);
    expect(rowsBetween("SURFACE", "GENERATOR INPUTS").at(-1)).toEqual([
      "SUBSURFACE OCEAN",
      "NOT YET MODELLED",
    ]);
  });

  it("reads an airless icy moon's ocean not modelled after its COLD TRAPS row, naming no liquid", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(airlessIcyMoonWithNoOceanKey));

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS").slice(-2)).toEqual([
      ["COLD TRAPS", "NOT YET MODELLED"],
      ["SUBSURFACE OCEAN", "NOT YET MODELLED"],
    ]);
    expect(within(readout()).queryByText(/NaCl BRINE/)).not.toBeInTheDocument();
  });

  // decision-ux-ocean-row §1: the member decides, as a version-23 server sends a regular moon's.
  it("reads SUBSURFACE OCEAN NONE after its COLD TRAPS row on an airless icy moon whose ocean is computed as none", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, oceanComputedAsNone(earthWithSurface(airlessBrineMoon)));

    expect(rowsBetween("SURFACE", "GENERATOR INPUTS").slice(-2)).toEqual([
      ["COLD TRAPS", "NOT YET MODELLED"],
      ["SUBSURFACE OCEAN", "NONE"],
    ]);
    expect(within(readout()).queryByText(/NaCl BRINE/)).not.toBeInTheDocument();
  });

  it("reads SUBSURFACE OCEAN NOT YET MODELLED with no unit", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthWithSurface(titanWithNoOceanKey));

    expect(readingOf("SUBSURFACE OCEAN").querySelector(".body-readout__unit")).toBeNull();
  });

  it("reads SUBSURFACE OCEAN NONE with no unit", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, oceanComputedAsNone(earthDetail()));

    expect(readingOf("SUBSURFACE OCEAN").querySelector(".body-readout__unit")).toBeNull();
  });

  it("names a crust and a cover the console does not know as UNLISTED MATERIAL, never by its key", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(
      socket,
      earthWithSurface((surface) => ({
        ...surface,
        crust: { ...crustOf(surface), secondary: "novel_rock" },
        surface_liquids: [{ substance: "novel_liquid", area_fraction: 0.71 }],
      })),
    );

    expect(reading("CRUST")).toBe("SECONDARY · UNLISTED MATERIAL");
    expect(reading("LIQUID COVER")).toBe("UNLISTED MATERIAL 71.0\u00A0%");
    expect(within(readout()).queryByText(/novel_/)).not.toBeInTheDocument();
  });

  it("shows no COLD TRAPS row for an airless world whose condensate carries its trapped share", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // The Earth's ice condensate sends `trapped_area_fraction: 0.0`, a value.
    await answerDetail(socket, airlessEarth());

    expect(terms("COLD TRAPS")).toHaveLength(0);
  });

  it.each<readonly [string, (surface: BodySurfaceDto) => BodySurfaceDto]>([
    [
      "are not modelled",
      (surface) => {
        const { condensates: _condensates, ...older } = surface;
        return older;
      },
    ],
    ["are computed as none", (surface) => ({ ...surface, condensates: null })],
    [
      "carry no trapped share",
      (surface) => ({
        ...surface,
        condensates: (surface.condensates ?? []).map(
          ({ trapped_area_fraction: _trapped, ...condensate }): SurfaceCondensateDto => condensate,
        ),
      }),
    ],
  ])(
    "keeps the COLD TRAPS row for an airless world whose condensates %s",
    async (_name, change) => {
      const { user, socket } = renderView();
      await answer(socket);
      await selectRow(user, /\/768/);
      await answerDetail(
        socket,
        earthWithSurface(() => change(surfaceOf(airlessEarth()))),
      );

      expect(terms("COLD TRAPS")).toHaveLength(1);
      expect(reading("COLD TRAPS")).toBe("NOT YET MODELLED");
    },
  );

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
    // The generator's composition at 1 bar (P14.T54.b.i), every gas of at least 10⁻¹².
    expect(reading("MOLE FRACTION")).toBe(
      partsReading(
        "H2 84.8 %",
        "He 15.0 %",
        "CH4 0.150 %",
        "NH3 0.0237 %",
        "Ne 0.0122 %",
        "Ar 0.00161 %",
        "N2 0.00124 %",
        "PH3 1.46E-4 %",
        "HCl 2.49E-5 %",
        "Kr 9.67E-7 %",
        "H2O 6.81E-7 %",
        "CO 1.19E-7 %",
        "Xe 9.45E-8 %",
        "H2S 4.22E-8 %",
        "HCN 7.61E-10 %",
        "CO2 7.40E-10 %",
      ),
    );
  });

  it("reads a gas giant's atmosphere as NOT YET MODELLED while the server does not compute it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/1280/);
    // As every server sends a giant's envelope below generator version 24 (P14.T54.b.i).
    await answerDetail(socket, jupiterWithEnvelope({ state: "not_modelled" }));

    expect(terms("ATMOSPHERE")).toHaveLength(1);
    expect(reading("ATMOSPHERE")).toBe("NOT YET MODELLED");
    expect(terms("MOLE FRACTION")).toHaveLength(0);
  });

  it("reads a rocky planet under a supercritical envelope with no SURFACE and its ATMOSPHERE NOT YET MODELLED", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    // Decision-absurd-states-24 R2.3: from generator version 24 such a planet's surface does not
    // apply, and its envelope is not modelled until P14.T53.b part 3.
    await answerDetail(socket, earthUnderASupercriticalEnvelope());

    expect(reading("CLASS")).toBe("ROCKY");
    expect(terms("SURFACE")).toHaveLength(0);
    expect(terms("SURFACE PRESSURE")).toHaveLength(0);
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
