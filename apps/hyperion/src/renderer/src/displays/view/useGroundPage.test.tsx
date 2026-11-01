/**
 * An instrument's `GROUND` page's state (plan R10, R10.T12.d): which page stands, why `GROUND` is
 * held back, and which slot the loop asks a readout for, over runs framed on `TERRAIN TEST`'s moon
 * and star and on `viewFixtures`' `TEST PLANET` as a server serves it.
 */
import type { BodyIdHex } from "@hyperion/protocol";
import { render, renderHook, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { type Vec3, vec3 } from "../../geometry/vec3";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { statusLine } from "../../test/statusLine";
import { aViewScene } from "../../test/viewFixtures";
import type { CameraState } from "../../view/camera/state";
import { terrainScene, terrainStar } from "../../view/scenes/terrainScene";
import { TERRAIN_NOT_AVAILABLE } from "../../view/terrain/annunciation";
import type { FieldStatus } from "../../view/terrain/coarseFieldStore";
import { CoverageView } from "../../view/terrain/coverage";
import { NO_SIGHT_FAR } from "../../view/terrain/readouts";
import type { TerrainSurvey } from "../../view/terrain/session";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import { type GroundSnapshot, groundBodyOf } from "./groundReadout";
import { GroundPage } from "./GroundPage";
import type { PlacePage } from "./PlaceTabs";
import { type GroundPageInstrument, useGroundPage } from "./useGroundPage";
import type { InstrumentShown } from "./useInstruments";
import { NO_ATMOSPHERE_SHOWN } from "./viewAtmosphere";
import { lowerYieldLine } from "./viewRoom";
import { startRun, startServerRun, type ViewRun } from "./viewRun";

/** `run` with its camera's pose put in `body`'s frame, `positionM` from its centre. */
function framedOn(run: ViewRun, body: BodyIdHex, positionM: Vec3 = vec3(0, 0, 1e10)): ViewRun {
  const camera: CameraState = {
    ...run.camera,
    pose: { ...run.camera.pose, frame: { kind: "body", body }, positionM },
  };
  return { ...run, camera };
}

/** `INSTRUMENT 1` having drawn `run`, its `GROUND` snapshot not yet published. */
function instrumentOn(run: ViewRun): GroundPageInstrument {
  const shown: InstrumentShown = {
    run,
    anchors: [],
    drawnStyle: "wireframe",
    sceneStars: false,
    photoreal: "idle",
    terrain: null,
    surveyEdges: [],
    contourIntervalM: null,
    atmosphere: NO_ATMOSPHERE_SHOWN,
    ground: null,
  };
  return { slot: 1, name: "INSTRUMENT 1", shown, faults: [] };
}

/** The page's state for `instrument` as `CONTROLS`, `chosen` pressed, folded where given. */
function pageFor(
  instrument: GroundPageInstrument | null,
  chosen: PlacePage = "ground",
  folded = false,
) {
  return renderHook(() => useGroundPage({ instrument, chosen, folded })).result.current;
}

/** `TERRAIN TEST` at its start: the seat on its ship, held in `TEST MOON`'s frame. */
const ON_MOON = instrumentOn(startRun(terrainScene()));

describe("an instrument's GROUND page's state (R10.T12.d; guide draft D2)", () => {
  it("stands CAMERA, asking nothing, while CONTROLS names PRIMARY", () => {
    const page = pageFor(null);
    expect([page.page, page.groundReason, page.pagedSlot]).toEqual(["camera", null, null]);
  });

  it("stands CAMERA while CAMERA is pressed", () => {
    expect(pageFor(ON_MOON, "camera").page).toBe("camera");
  });

  it("stands GROUND once pressed, the instrument framing TEST MOON, and asks its readout", () => {
    const page = pageFor(ON_MOON);
    expect([page.page, page.groundReason, page.pagedSlot]).toEqual(["ground", null, 1]);
  });

  it("offers GROUND for another body than the primary's, a server's TEST PLANET", () => {
    expect(pageFor(instrumentOn(startServerRun(aViewScene()))).page).toBe("ground");
  });

  it("holds GROUND back while the instrument's FRAME is a star, CAMERA standing", () => {
    const page = pageFor(instrumentOn(framedOn(startRun(terrainScene()), terrainStar())));
    expect([page.page, page.groundReason, page.pagedSlot]).toEqual([
      "camera",
      "NOT AVAILABLE: INSTRUMENT 1's FRAME is no body with a surface",
      null,
    ]);
  });

  it("asks no readout while the compact layout folds the place, GROUND standing", () => {
    const page = pageFor(ON_MOON, "ground", true);
    expect([page.page, page.pagedSlot]).toEqual(["ground", null]);
  });

  it("states no parts and no note before the instrument's first readout is published", () => {
    const page = pageFor(ON_MOON);
    expect([page.parts, page.groundNote]).toEqual([[], false]);
  });
});

/** A coverage of level 5 with its first five cells surveyed at about 2 m (cell code 62). */
function coverageOfFive(): CoverageView {
  const merged = CoverageView.empty(5).merged(
    { ranges: [{ start: 0, end: 5, resolution: 62 }] },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** A field's standing: wanted as the local body, answered, nothing failed. */
const STANDING: FieldStatus = {
  reason: "local-body",
  pending: false,
  notModelled: null,
  failure: null,
};

/** A held, live survey of `body`, its first cells surveyed at about 2 m. */
function surveyOf(body: BodyIdHex, stale = false): TerrainSurvey {
  return { status: STANDING, coverage: coverageOfFive(), header: moonLikeHeader(body), stale };
}

/** A readout where the line of sight meets surveyed ground 41.2 km away. */
const ON_GROUND: SurfaceGroundReadout = {
  kind: "ground",
  sight: {
    rangeM: 41_234,
    sigmaM: 290,
    resolved: false,
    hitM: [1_738_000, 0, 0],
    at: { face: 0, u: 0, v: 0 },
  },
  reading: {
    elevationM: 2_143.7,
    elevationSigmaM: 180,
    slopeRmsRad: 0.21,
    slopeSigmaRad: 0.07,
    slopeBaselineM: 2,
    resolutionM: 410,
    resolved: false,
  },
  material: { classCode: 1, entry: 0 },
  properties: {
    bearingCapacityPa: 3_432_000,
    capacitySlopeRad: 0.21,
    padFriction: 0.2,
    frictionOnTemperature: false,
    temperatureK: 250,
  },
};

/** The `FRAME` body `run` states the terrain of. */
function frameBodyOf(run: ViewRun): BodyIdHex {
  const body = groundBodyOf(run);
  if (body === null) {
    throw new Error("the run's FRAME is no body with a surface");
  }
  return body.id;
}

/** The runs the page is shown for: the primary's own body, and another as a server serves it. */
const RUNS: ReadonlyArray<readonly [string, ViewRun]> = [
  ["TERRAIN TEST's TEST MOON, the primary's body", startRun(terrainScene())],
  ["a server's TEST PLANET, another body", startServerRun(aViewScene())],
];

/** A snapshot of `run`'s `FRAME` body as its loop publishes it: its survey held, no answer yet. */
function snapshotFor(run: ViewRun, overrides: Partial<GroundSnapshot> = {}): GroundSnapshot {
  const body = frameBodyOf(run);
  return {
    body,
    surfaceUnset: false,
    survey: surveyOf(body),
    readout: null,
    sessionFault: null,
    slantFromCamera: false,
    ...overrides,
  };
}

/** `INSTRUMENT 1` having drawn `run`, its loop's last publish `ground`. */
function instrumentWith(run: ViewRun, ground: GroundSnapshot): GroundPageInstrument {
  const instrument = instrumentOn(run);
  return { ...instrument, shown: { ...instrument.shown, ground } };
}

/** Props of {@link PageHost}. */
interface PageHostProps {
  readonly instrument: GroundPageInstrument;
}

/**
 * The side column as the display lays it out with `GROUND` pressed: the instrument's `Targets`
 * panel, then its `GROUND` page from {@link useGroundPage}'s state, and the note the list carries.
 */
function PageHost({ instrument }: PageHostProps) {
  const { groundNote, parts, readout, survey, plateRef, sizerRef, targetsRef } = useGroundPage({
    instrument,
    chosen: "ground",
    folded: false,
  });
  return (
    <div className="view__column">
      <section className="panel view-targets" aria-label="Targets" ref={targetsRef}>
        <p>{groundNote ? "NOTE" : "NO NOTE"}</p>
      </section>
      <section aria-label={`Ground ${instrument.name}`}>
        <GroundPage
          parts={parts}
          readout={readout}
          survey={survey}
          plateRef={plateRef}
          sizerRef={sizerRef}
        />
      </section>
    </div>
  );
}

/** The page's live plate, its forms' copies apart. */
function livePlate(): HTMLElement {
  const plate = screen
    .getByRole("region", { name: "Ground INSTRUMENT 1" })
    .querySelector(":scope > .view-label");
  if (!(plate instanceof HTMLElement)) {
    throw new Error("no live plate stands");
  }
  return plate;
}

/** An element's text as read, its block elements apart and its white space collapsed. */
function readAs(element: Element): string {
  const blocks = [...element.querySelectorAll("h3, p, dt, dd")];
  return (blocks.length === 0 ? [element] : blocks)
    .map((block) => block.textContent.replace(/\s+/gu, " ").trim())
    .join(" ");
}

/** What the page reads, entry by entry. */
function pageReads(): string[] {
  return [...livePlate().children].map(readAs);
}

/** Whether the instrument's list carries the readout's note. */
function listNotes(): boolean {
  return within(screen.getByRole("region", { name: "Targets" })).queryByText("NOTE") !== null;
}

describe.each(RUNS)(
  "an instrument's GROUND page for %s, by its readout's state (R10.T12.d)",
  (_, run) => {
    /** The page shown for `run`, its loop's last publish `overrides` of a held survey. */
    function showPage(overrides: Partial<GroundSnapshot> = {}): void {
      render(<PageHost instrument={instrumentWith(run, snapshotFor(run, overrides))} />);
    }

    it("states SURVEY COVERAGE: PENDING alone, and no note, before the survey arrives", () => {
      showPage({ survey: null });
      expect([pageReads(), listNotes()]).toEqual([["SURVEY COVERAGE: PENDING"], false]);
    });

    it("states the survey line, DATUM and the readout's NOT SURVEYED, and notes the readout", () => {
      showPage({ readout: { kind: "not-surveyed" } });
      expect([pageReads(), listNotes()]).toEqual([
        [
          expect.stringMatching(/^SURVEY COVERAGE /u),
          expect.stringMatching(/^DATUM /u),
          "GROUND AT LINE OF SIGHT NOT SURVEYED",
        ],
        true,
      ]);
    });

    it("states the readout's NO GROUND, with no note, since it gives no point", () => {
      showPage({ readout: { kind: "no-ground" } });
      expect([pageReads().at(-1), listNotes()]).toEqual([
        "GROUND AT LINE OF SIGHT NO GROUND",
        false,
      ]);
    });

    it("states why no line of sight forms in the readout's place, DATUM standing, with no note", () => {
      showPage({ sightGap: "far" });
      expect([pageReads().slice(1), listNotes()]).toEqual([
        [
          expect.stringMatching(/^DATUM /u),
          `GROUND AT LINE OF SIGHT ${NO_SIGHT_FAR.replace(/\s+/gu, " ")}`,
        ],
        false,
      ]);
    });

    it("states the survey line alone under TERRAIN NOT AVAILABLE, no DATUM, readout or note", () => {
      showPage({ readout: ON_GROUND, sessionFault: TERRAIN_NOT_AVAILABLE });
      expect([pageReads(), listNotes()]).toEqual([
        [expect.stringMatching(/^SURVEY COVERAGE /u)],
        false,
      ]);
    });

    it("states the readout's readings over ground, and notes the readout", () => {
      showPage({ readout: ON_GROUND });
      expect([pageReads().at(-1), listNotes()]).toEqual([
        expect.stringMatching(/^GROUND AT LINE OF SIGHT ELEVATION ~2140 m/u),
        true,
      ]);
    });

    it("marks the readout's values stale with their S on loss of the link, its note kept", () => {
      showPage({ readout: ON_GROUND, survey: surveyOf(frameBodyOf(run), true) });
      expect([within(livePlate()).getAllByText("stale").length > 0, listNotes()]).toEqual([
        true,
        true,
      ]);
    });

    it("marks nothing stale while the link holds", () => {
      showPage({ readout: ON_GROUND });
      expect(within(livePlate()).queryAllByText("stale")).toEqual([]);
    });
  },
);

/** The heights of the page's forms' copies, CSS px: whole, its readout given way, and its survey. */
const FORM_PX: ReadonlyMap<string, number> = new Map([
  ["whole", 220],
  ["readout", 120],
  ["survey", 60],
]);

/**
 * Lays the side column out with the page's forms at {@link FORM_PX} and its live plate, the
 * place's room, at `roomPx()`; the `Targets` panel and everything else at nothing, so that the
 * list has nothing to give.
 */
function layOutPlace(roomPx: () => number): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function laidOut(
    this: HTMLElement,
  ) {
    const form = this.parentElement?.getAttribute("data-form") ?? null;
    let heightPx = 0;
    if (this.classList.contains("view-label")) {
      heightPx =
        form === null
          ? this.closest(".view-ground__sizer") === null
            ? roomPx()
            : 0
          : (FORM_PX.get(form) ?? 0);
    }
    return DOMRect.fromRect({ x: 0, y: 0, width: 0, height: heightPx });
  });
}

/** The readout's yield line, and the yield line of the survey's values, `DATUM` and the readout. */
const READOUT_YIELDS = lowerYieldLine(["readout"]);
const ALL_YIELD = lowerYieldLine(["survey", "datum", "readout"]);

/** What stands on the page: each part by its first words, a yield line whole. */
function standing(): string[] {
  return pageReads().map((text) => {
    if (text.startsWith("SURVEY COVERAGE")) {
      return "SURVEY COVERAGE";
    }
    if (text.startsWith("DATUM")) {
      return "DATUM";
    }
    return text.startsWith("GROUND AT LINE OF SIGHT") ? "GROUND AT LINE OF SIGHT" : text;
  });
}

describe("an instrument's GROUND page giving way and returning (R10.T12.d)", () => {
  /** TERRAIN TEST's moon, its readout over ground. */
  const PAGED = instrumentWith(
    startRun(terrainScene()),
    snapshotFor(startRun(terrainScene()), { readout: ON_GROUND }),
  );

  it("stands whole where the place holds its whole form", () => {
    layOutPlace(() => 300);
    render(<PageHost instrument={PAGED} />);
    expect(standing()).toEqual(["SURVEY COVERAGE", "DATUM", "GROUND AT LINE OF SIGHT"]);
  });

  it("gives its readout way first where the place cannot hold the whole form, and its note", () => {
    layOutPlace(() => 200);
    render(<PageHost instrument={PAGED} />);
    expect([standing(), listNotes()]).toEqual([
      ["SURVEY COVERAGE", "DATUM", READOUT_YIELDS],
      false,
    ]);
  });

  it("gives the survey's values and DATUM way too where the place cannot hold even that", () => {
    layOutPlace(() => 100);
    render(<PageHost instrument={PAGED} />);
    expect(within(livePlate()).getByText(statusLine(ALL_YIELD))).toBeInTheDocument();
  });

  it("returns the survey only with a line to spare over its form", () => {
    let roomPx = 100;
    layOutPlace(() => roomPx);
    const { rerender } = render(<PageHost instrument={PAGED} />);
    // 120 px would hold it, but a return asks a line, 17.5 px, more: 137.5 px.
    roomPx = 130;
    rerender(<PageHost instrument={PAGED} />);
    const held = standing();
    roomPx = 140;
    rerender(<PageHost instrument={PAGED} />);
    expect([held, standing()]).toEqual([[ALL_YIELD], ["SURVEY COVERAGE", "DATUM", READOUT_YIELDS]]);
  });

  it("returns the readout only with a line to spare over the whole form", () => {
    let roomPx = 200;
    layOutPlace(() => roomPx);
    const { rerender } = render(<PageHost instrument={PAGED} />);
    roomPx = 230;
    rerender(<PageHost instrument={PAGED} />);
    const held = standing();
    roomPx = 240;
    rerender(<PageHost instrument={PAGED} />);
    expect([held, standing()]).toEqual([
      ["SURVEY COVERAGE", "DATUM", READOUT_YIELDS],
      ["SURVEY COVERAGE", "DATUM", "GROUND AT LINE OF SIGHT"],
    ]);
  });
});
