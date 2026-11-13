/**
 * The `PRIMARY` view's plates as laid out (plan R10, R10.T12.b): the upper plate's heights whole,
 * scrolling its lines before `STARS` and scrolling its body, the lower plate's forms, the room's
 * measures at both widths, and when two measures count as the same.
 *
 * @remarks
 * jsdom lays nothing out, so each element of the plates built here carries the height the test
 * lays it out at (`data-px`), its width where it matters (`data-width-px`), a scroll its box's
 * and its content's (`data-client-px`, `data-scroll-px`), and a lower plate's group its box's and
 * its content's widths (`data-client-width-px`, `data-scroll-width-px`), which the stubs read back.
 */
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  measureLowerForms,
  measurePage,
  measureRoom,
  measureUpperPlate,
  NOT_MEASURED,
  PAGE_NOT_MEASURED,
  type PageMeasures,
  type RoomMeasures,
  samePageMeasures,
  sameRoomMeasures,
} from "./plateMeasures";
import type { PlateMeasures, StageRoom } from "./viewRoom";

/** A height an element carries in the attribute `name`, CSS px, or 0 without it. */
function px(element: Element, name: string): number {
  return Number(element.getAttribute(name) ?? 0);
}

/** Lays each element out at the heights it carries, CSS px, 300 px wide unless it carries a width. */
function layOut(): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function laidOut(
    this: HTMLElement,
  ) {
    const widthPx = this.hasAttribute("data-width-px") ? px(this, "data-width-px") : 300;
    return DOMRect.fromRect({ x: 0, y: 0, width: widthPx, height: px(this, "data-px") });
  });
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function box(
    this: HTMLElement,
  ) {
    return px(this, "data-client-px");
  });
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function content(
    this: HTMLElement,
  ) {
    return px(this, "data-scroll-px");
  });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(function boxWidth(
    this: HTMLElement,
  ) {
    return px(this, "data-client-width-px");
  });
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function contentWidth(
    this: HTMLElement,
  ) {
    return px(this, "data-scroll-width-px");
  });
}

/** A position line 15 px tall with 3 px of margins: 18 px. */
function PositionLine() {
  return (
    <p
      className="list-position view-label__position"
      data-px="15"
      style={{ marginTop: "2px", marginBottom: "1px" }}
    >
      1-5 of 11
    </p>
  );
}

/** The labels of the lines before `STARS` the plates here hold, in order. */
const MIDDLE_LABELS = ["FRAME", "TIME", "STYLE", "QUALITY", "CAMERA"];

/** Lines before `STARS`, the first of {@link MIDDLE_LABELS}, each `heightsPx` tall. */
function MiddleLines({ heightsPx }: { readonly heightsPx: ReadonlyArray<number> }) {
  const lines = MIDDLE_LABELS.slice(0, heightsPx.length).map((label, index) => ({
    label,
    heightPx: heightsPx[index] ?? 0,
  }));
  return (
    <dl className="view-label__lines">
      {lines.map(({ label, heightPx }) => (
        <div className="field view-label__line" data-px={heightPx} key={label} />
      ))}
    </dl>
  );
}

/** The `STARS` line on, 20 px. */
function RestLines() {
  return (
    <dl className="view-label__lines">
      <div className="field view-label__line" data-px="20" />
    </dl>
  );
}

/**
 * The room's sizers: the position line's sample, 15 px, and, `beside`, the plates' copies beside
 * the slots, the upper 500 px and the lower 160 px whole.
 */
function Sizers({ beside }: { readonly beside: boolean }) {
  return (
    <div className="view-room-sizers">
      <div className="view-label view-room-sizers__sample">
        <p className="list-position view-label__position" data-sizer="position" data-px="15" />
      </div>
      {beside ? (
        <div data-sizer="beside">
          <div data-form="upper">
            <div className="view-label" data-px="500">
              <p className="view-label__class" data-px="20" />
              <div className="view-label__body" data-px="480" />
            </div>
          </div>
          <div data-form="whole">
            <div className="view-label" data-px="160" />
          </div>
        </div>
      ) : null}
    </div>
  );
}

/** Renders `plate` and returns it laid out. */
function laidOutPlate(plate: ReactElement): HTMLElement {
  layOut();
  const { container } = render(plate);
  const element = container.firstElementChild;
  if (!(element instanceof HTMLElement)) {
    throw new Error("no plate is rendered");
  }
  return element;
}

describe("the upper plate's measures (R10.T12.b)", () => {
  it("measures a plate standing whole as it stands, each line before STARS as laid out", () => {
    const plate = laidOutPlate(
      <div className="view-label" data-px="300">
        <p className="view-label__class" data-px="20" />
        <div className="view-label__body" data-px="280">
          <div className="view-label__middle">
            <MiddleLines heightsPx={[20, 20, 40, 20]} />
          </div>
          <RestLines />
        </div>
      </div>,
    );
    expect(measureUpperPlate(plate, 6, 18)).toEqual({
      wholePx: 300,
      middleLinesPx: [20, 20, 40, 20],
      pinnedLines: 4,
      bodyPx: 280,
      positionPx: 18,
      widthPx: 300,
    });
  });

  it("counts the lines' scrolled content in full and leaves their position line out", () => {
    // Laid out 250 px: 20 for VIEW, and a 230 px body holding a 60 px box of 100 px of lines
    // and the 18 px position line.
    const plate = laidOutPlate(
      <div className="view-label" data-px="250">
        <p className="view-label__class" data-px="20" />
        <div className="view-label__body" data-px="230">
          <div
            className="view-label__middle view-label__scroll"
            data-client-px="60"
            data-scroll-px="100"
          >
            <MiddleLines heightsPx={[20, 20, 20, 20, 20]} />
          </div>
          <PositionLine />
          <RestLines />
        </div>
      </div>,
    );
    const measured = measureUpperPlate(plate, 6, 18);
    expect([measured.wholePx, measured.bodyPx, measured.middleLinesPx]).toEqual([
      250 + 40 - 18,
      250 + 40 - 18 - 20,
      [20, 20, 20, 20, 20],
    ]);
  });

  it("counts the body's scrolled content in full and leaves its position line, under it, out", () => {
    // Laid out 188 px: 20 for VIEW, a 150 px box of 400 px of content and the 18 px position line.
    const plate = laidOutPlate(
      <div className="view-label" data-px="188">
        <p className="view-label__class" data-px="20" />
        <div
          className="view-label__body view-label__scroll"
          data-px="150"
          data-client-px="150"
          data-scroll-px="400"
        >
          <div className="view-label__middle">
            <MiddleLines heightsPx={[20, 20]} />
          </div>
          <RestLines />
        </div>
        <PositionLine />
      </div>,
    );
    const measured = measureUpperPlate(plate, 6, 18);
    expect([measured.wholePx, measured.bodyPx]).toEqual([420, 400]);
  });
});

describe("the lower plate's forms (R10.T12.b)", () => {
  it("measures each form's plate, its height and its width", () => {
    const root = laidOutPlate(
      <div>
        <div data-form="whole">
          <div className="view-label" data-px="120" data-width-px="384" />
        </div>
        <div data-form="readout">
          <div className="view-label" data-px="60" data-width-px="384" />
        </div>
        <div data-form="survey">
          <div className="view-label" data-px="40" data-width-px="360" />
        </div>
      </div>,
    );
    expect(measureLowerForms(root)).toEqual({
      wholePx: 120,
      readoutYieldedPx: 60,
      surveyYieldedPx: 40,
      passing: [],
      wholeWidthPx: 384,
      readoutYieldedWidthPx: 384,
      surveyYieldedWidthPx: 360,
    });
  });

  /**
   * The whole form's plate, its survey line's group `surveyContentPx` wide and its readout's
   * `readoutContentPx`, each in a 214 px box, with a state line whose content is wider still.
   */
  function wholeForm(surveyContentPx: number, readoutContentPx: number): HTMLElement {
    return laidOutPlate(
      <div>
        <div data-form="whole">
          <div className="view-label" data-px="120">
            <div
              className="view-label__group"
              data-part="state"
              data-client-width-px="214"
              data-scroll-width-px="400"
            />
            <div
              className="view-label__group"
              data-part="survey"
              data-client-width-px="214"
              data-scroll-width-px={surveyContentPx}
            />
            <div
              className="view-label__group"
              data-part="datum"
              data-client-width-px="214"
              data-scroll-width-px="214"
            />
            <div
              className="view-label__group"
              data-part="readout"
              data-client-width-px="214"
              data-scroll-width-px={readoutContentPx}
            />
          </div>
        </div>
      </div>,
    );
  }

  it.each<readonly [string, number, number, ReadonlyArray<string>]>([
    ["no run past the plate", 200, 214, []],
    ["the readout's run past it by two pixels", 214, 216, ["readout"]],
    ["the survey line's and the readout's past it", 264, 270, ["survey", "readout"]],
    ["a run past it by a pixel, as the two widths' rounding gives", 214, 215, []],
  ])("finds the parts whose widest run passes the plate, with %s (R10.T12.c)", (...args) => {
    const [, surveyPx, readoutPx, passing] = args;
    expect(measureLowerForms(wholeForm(surveyPx, readoutPx))?.passing).toEqual(passing);
  });

  it("states no form it has no part for", () => {
    const root = laidOutPlate(
      <div>
        <div data-form="whole">
          <div className="view-label" data-px="30" />
        </div>
      </div>,
    );
    expect(measureLowerForms(root)).toEqual({
      wholePx: 30,
      readoutYieldedPx: null,
      surveyYieldedPx: null,
      passing: [],
      wholeWidthPx: 300,
      readoutYieldedWidthPx: null,
      surveyYieldedWidthPx: null,
    });
  });

  it("measures no plate without its whole form", () => {
    const root = laidOutPlate(
      <div>
        <div data-form="readout">
          <div className="view-label" data-px="60" />
        </div>
      </div>,
    );
    expect(measureLowerForms(root)).toBeNull();
  });
});

describe("the room's measures (R10.T12.b)", () => {
  /** A live upper plate 300 px whole. */
  const LIVE = (
    <div className="view-label" data-px="300">
      <p className="view-label__class" data-px="20" />
      <div className="view-label__body" data-px="280">
        <div className="view-label__middle">
          <MiddleLines heightsPx={[20, 20]} />
        </div>
      </div>
    </div>
  );

  afterEach(() => {
    document.documentElement.style.fontSize = "";
  });

  /**
   * The room measured from the live plate and the sizers, both laid out in an 814 × 541 px
   * overlay: the stage of a 1280 × 720 window at 100%.
   */
  function measured(liveBeside: boolean, beside: boolean): RoomMeasures {
    layOut();
    const { container } = render(
      <div className="view__overlay" data-px="541" data-width-px="814">
        {LIVE}
        <Sizers beside={beside} />
      </div>,
    );
    const [upper, root] = [...(container.firstElementChild?.children ?? [])];
    if (!(upper instanceof HTMLElement) || !(root instanceof HTMLElement)) {
      throw new Error("the plates are not rendered");
    }
    return measureRoom({ upper, sizers: root, pinnedLines: 6, liveBeside });
  }

  it("measures nothing while the upper plate is not mounted", () => {
    expect(measureRoom({ upper: null, sizers: null, pinnedLines: 6, liveBeside: false })).toBe(
      NOT_MEASURED,
    );
  });

  it("measures the stage at the overlay the live plate stands in, at the interface's rem", () => {
    expect(measured(false, false).stage).toEqual({ widthPx: 814, heightPx: 541, remPx: 16 });
  });

  it("measures the stage's rem at the interface scale", () => {
    document.documentElement.style.fontSize = "24px";
    expect(measured(false, false).stage?.remPx).toBe(24);
  });

  it("measures no stage for a plate standing in nothing", () => {
    layOut();
    const plate = document.createElement("div");
    plate.className = "view-label";
    expect(
      measureRoom({ upper: plate, sizers: null, pinnedLines: 6, liveBeside: false }).stage,
    ).toBeNull();
  });

  it("takes the live plates for both widths while they stand beside the slots", () => {
    const room = measured(true, false);
    expect(room.beside).toBe(room.full);
  });

  it("measures the copies beside the slots while the slots are held for the plates", () => {
    const room = measured(false, true);
    expect([room.beside?.upper.wholePx, room.beside?.lower?.wholePx]).toEqual([500, 160]);
  });

  it("measures nothing beside the slots without the copies", () => {
    expect(measured(false, false).beside).toBeNull();
  });

  it("reads a position line's height from the sizers' sample", () => {
    expect(measured(false, false).full?.upper.positionPx).toBe(15);
  });
});

describe("two measures of the room alike (R10.T12.b)", () => {
  const PLATES: PlateMeasures = {
    upper: {
      wholePx: 440,
      middleLinesPx: [20, 20, 40],
      pinnedLines: 3,
      bodyPx: 420,
      positionPx: 18,
      widthPx: 384,
    },
    lower: {
      wholePx: 120,
      readoutYieldedPx: 60,
      surveyYieldedPx: null,
      passing: [],
      wholeWidthPx: 384,
      readoutYieldedWidthPx: 384,
      surveyYieldedWidthPx: null,
    },
  };

  /** The stage of a 1280 × 720 window at 100%, CSS px. */
  const STAGE: StageRoom = { widthPx: 814, heightPx: 541, remPx: 16 };

  /**
   * The measures with the plates at both widths, the upper plate whole `wholePx` tall, taken at
   * {@link STAGE}.
   */
  function withWhole(wholePx: number): RoomMeasures {
    const plates = { ...PLATES, upper: { ...PLATES.upper, wholePx } };
    return { full: plates, beside: plates, stage: STAGE };
  }

  /** The measures of {@link withWhole}'s 440 px plate taken at `stage`. */
  function takenAt(stage: StageRoom | null): RoomMeasures {
    return { ...withWhole(440), stage };
  }

  it("counts measures within a twentieth of a pixel the same", () => {
    expect(sameRoomMeasures(withWhole(440), withWhole(440.04))).toBe(true);
  });

  it("counts measures more than a twentieth of a pixel apart as changed", () => {
    expect(sameRoomMeasures(withWhole(440), withWhole(440.06))).toBe(false);
  });

  it("counts a plate grown wider as changed, since its cover of the picture grows (the owner's delegated call)", () => {
    const lower = PLATES.lower;
    const wider: PlateMeasures = {
      ...PLATES,
      lower: lower === null ? null : { ...lower, readoutYieldedWidthPx: 390 },
    };
    expect(
      sameRoomMeasures(
        { full: PLATES, beside: null, stage: STAGE },
        { full: wider, beside: null, stage: STAGE },
      ),
    ).toBe(false);
  });

  it("counts an upper plate grown wider as changed", () => {
    const wider: PlateMeasures = { ...PLATES, upper: { ...PLATES.upper, widthPx: 390 } };
    expect(
      sameRoomMeasures(
        { full: PLATES, beside: null, stage: STAGE },
        { full: wider, beside: null, stage: STAGE },
      ),
    ).toBe(false);
  });

  it("counts a line more before STARS as changed", () => {
    const more: PlateMeasures = {
      ...PLATES,
      upper: { ...PLATES.upper, middleLinesPx: [20, 20, 40, 20] },
    };
    expect(
      sameRoomMeasures(
        { full: PLATES, beside: null, stage: STAGE },
        { full: more, beside: null, stage: STAGE },
      ),
    ).toBe(false);
  });

  it("counts a lower plate that comes as changed", () => {
    const upperOnly: PlateMeasures = { ...PLATES, lower: null };
    expect(
      sameRoomMeasures(
        { full: upperOnly, beside: null, stage: STAGE },
        { full: PLATES, beside: null, stage: STAGE },
      ),
    ).toBe(false);
  });

  it("counts a lower plate whose readout's run comes to pass it as changed (R10.T12.c)", () => {
    const lower = PLATES.lower;
    const passing: PlateMeasures = {
      ...PLATES,
      lower: lower === null ? null : { ...lower, passing: ["readout"] },
    };
    expect(
      sameRoomMeasures(
        { full: PLATES, beside: null, stage: STAGE },
        { full: passing, beside: null, stage: STAGE },
      ),
    ).toBe(false);
  });

  it("counts plates first measured as changed", () => {
    expect(sameRoomMeasures(NOT_MEASURED, { full: PLATES, beside: null, stage: STAGE })).toBe(
      false,
    );
  });

  it("counts nothing measured twice the same", () => {
    expect(sameRoomMeasures(NOT_MEASURED, { full: null, beside: null, stage: null })).toBe(true);
  });

  it("counts the same plates measured at a stage within half a pixel the same", () => {
    expect(sameRoomMeasures(takenAt(STAGE), takenAt({ ...STAGE, heightPx: 541.4 }))).toBe(true);
  });

  it("counts the same plates measured at another stage as changed", () => {
    expect(sameRoomMeasures(takenAt(STAGE), takenAt({ ...STAGE, heightPx: 541.5 }))).toBe(false);
  });

  it("counts the same plates measured at another interface scale as changed", () => {
    expect(sameRoomMeasures(takenAt(STAGE), takenAt({ ...STAGE, remPx: 24 }))).toBe(false);
  });

  it("counts the same plates first measured at a stage as changed", () => {
    expect(sameRoomMeasures(takenAt(null), takenAt(STAGE))).toBe(false);
  });
});

/**
 * A side column laid out as the tests give it: the `Targets` panel `targetsPx` tall over its 160 px
 * least height, its head `headPx` and its list the rest (or `listPx`) over its 104 px least, a panel
 * stretched past them left empty under them, then the `GROUND` page's plate `platePx`
 * tall, the column's box and content `clientPx` and `scrollPx`, and the page's forms 220, 120 and
 * 60 px tall; in the compact layout's side, its column dissolved, where `compact`.
 */
function sideColumn({
  targetsPx = 300,
  headPx = 100,
  listPx,
  platePx = 250,
  clientPx = 0,
  scrollPx = 0,
  compact = false,
}: {
  readonly targetsPx?: number;
  readonly headPx?: number;
  readonly listPx?: number;
  readonly platePx?: number;
  readonly clientPx?: number;
  readonly scrollPx?: number;
  readonly compact?: boolean;
} = {}): {
  readonly targets: HTMLElement;
  readonly plate: HTMLElement;
  readonly sizer: HTMLElement;
} {
  const side = laidOutPlate(
    <div data-client-px={compact ? clientPx : 0} data-scroll-px={compact ? scrollPx : 0}>
      <div
        className="column"
        style={compact ? { display: "contents" } : undefined}
        data-client-px={compact ? 0 : clientPx}
        data-scroll-px={compact ? 0 : scrollPx}
      >
        <section className="targets" data-px={targetsPx} style={{ minHeight: "160px" }}>
          <p data-px={headPx} />
          <div
            className="view-list"
            data-px={listPx ?? targetsPx - headPx}
            style={{ minHeight: "104px" }}
          />
        </section>
        <div className="plate" data-px={platePx} />
        <div className="sizer">
          <div data-form="whole">
            <div className="view-label" data-px="220" />
          </div>
          <div data-form="readout">
            <div className="view-label" data-px="120" />
          </div>
          <div data-form="survey">
            <div className="view-label" data-px="60" />
          </div>
        </div>
      </div>
    </div>,
  );
  const find = (selector: string): HTMLElement => {
    const found = side.querySelector(selector);
    if (!(found instanceof HTMLElement)) {
      throw new Error(`no ${selector} is laid out`);
    }
    return found;
  };
  return { targets: find(".targets"), plate: find(".plate"), sizer: find(".sizer") };
}

describe("an instrument's GROUND page as laid out (R10.T12.d)", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("reads its forms' heights from their copies", () => {
    const { targets, plate, sizer } = sideColumn();
    expect(measurePage({ plate, sizer, targets }).lower).toEqual({
      wholePx: 220,
      readoutYieldedPx: 120,
      surveyYieldedPx: 60,
      passing: [],
      wholeWidthPx: 300,
      readoutYieldedWidthPx: 300,
      surveyYieldedWidthPx: 300,
    });
  });

  it("reckons its room as its plate's height and what the list can still give", () => {
    const { targets, plate, sizer } = sideColumn();
    // The list's floor: 300 less its 200 px, with its 104 px least, 204 px; it can give 96 px.
    expect(measurePage({ plate, sizer, targets }).roomPx).toBe(250 + 96);
  });

  it("counts the empty height of a panel stretched past its short list as given up too", () => {
    const { targets, plate, sizer } = sideColumn({ targetsPx: 547, headPx: 24, listPx: 129.5 });
    // Its content, 24 and 129.5 px; with the list at its 104 px least, 128 px, under the panel's
    // 160 px least: it can give 387 px, the 393.5 px it was stretched past its content among them.
    expect(measurePage({ plate, sizer, targets }).roomPx).toBe(250 + 387);
  });

  it("takes the panel's own least height where that is the higher floor", () => {
    const { targets, plate, sizer } = sideColumn({ targetsPx: 180 });
    // The list's floor, 180 less 80 with its 104 px least, 204 px, passes the panel's height:
    // nothing more to give, whatever the panel's 160 px least.
    expect(measurePage({ plate, sizer, targets }).roomPx).toBe(250);
  });

  it("leaves out what the column already runs past its box", () => {
    const { targets, plate, sizer } = sideColumn({ clientPx: 500, scrollPx: 520 });
    expect(measurePage({ plate, sizer, targets }).roomPx).toBe(250 + 96 - 20);
  });

  it("reads the compact layout's side as the box, its column dissolved", () => {
    const { targets, plate, sizer } = sideColumn({ clientPx: 500, scrollPx: 530, compact: true });
    expect(measurePage({ plate, sizer, targets }).roomPx).toBe(250 + 96 - 30);
  });

  it("reckons no room before its plate is laid out", () => {
    const { targets, sizer } = sideColumn();
    expect(measurePage({ plate: null, sizer, targets }).roomPx).toBeNull();
  });

  it("counts two measures within a twentieth of a pixel the same", () => {
    const a: PageMeasures = { lower: null, roomPx: 300, remPx: 16 };
    expect([
      samePageMeasures(a, { ...a, roomPx: 300.04 }),
      samePageMeasures(a, { ...a, roomPx: 300.1 }),
      samePageMeasures(PAGE_NOT_MEASURED, PAGE_NOT_MEASURED),
    ]).toEqual([true, false, true]);
  });
});
