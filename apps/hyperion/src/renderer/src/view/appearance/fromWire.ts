/**
 * A body's photometry and figure from plan 14's sections (plan R07, T2.a and T2.b; Design notes 5
 * and 19).
 *
 * @remarks
 * A body whose photometric section is `ok` is shaded with the law it states (P14.T47): p and s per
 * band, the template and L, `modelled` (or `modelled-provisional` where the section flags its curve
 * provisional, which no view labels: decision-r07-provisional-photometry), q found from that law. A
 * body without one (an older server, or a section not modelled or not resolved) takes
 * {@link PROVISIONAL_PHOTOMETRY}, labelled
 * `BODY PHOTOMETRY: NOT YET MODELLED`. Its figure is the section's spheroid (P14.T46.d), or where
 * that is missing a sphere of its mean radius, `bulk.radius_m`, about the rotation section's pole,
 * or about none; a body without a granted radius has no figure and stays R02's mark. A `contact`
 * entry of R03's frame is not lit (decisions-r06-r07, item 4).
 *
 * The stated ratio p_V q_V ÷ A_Bond is a check, never an input: where it departs from the law's by
 * more than {@link BOND_RATIO_TOLERANCE} the body is a finding for plan 14's owner (Design note 5),
 * which the drawing loop reports once ({@link reportBondRatioFinding}).
 */
import type { PhaseTemplateDto } from "@hyperion/protocol";

import type { SystemBody, SystemBodyPhotometry } from "../../lib/system/model";
import type { BodyFigure } from "../terrain/planet";
import type { Rgb } from "../photometry/toneCurve";
import type { PhaseTemplateId, PhotometricLaw } from "./law";
import { lawFor, lawPhaseIntegral } from "./phase";
import { shapeGeometricAlbedo } from "./shapes";

export type { BodyFigure } from "../terrain/planet";

/** A body's photometry, as R08 and R10 read it through `BodyAppearance` (Design note 5). */
export interface BodyPhotometry {
  /** p per display channel (r, g, b). */
  readonly geometricAlbedo: Rgb;
  /** q per display channel (r, g, b), from the law. */
  readonly phaseIntegral: Rgb;
  readonly law: PhotometricLaw;
  /** p_V q_V ÷ A_Bond as plan 14 states it, a check only; `null` without a photometric section. */
  readonly bondRatioCheck: number | null;
  /**
   * Where the law comes from: plan 14's section (`modelled`); that section on a curve it marks
   * provisional, borrowed from another surface (magma, airless ice, a snowball) or a cold giant's
   * standing for a hot one (`modelled-provisional`, P14.T47.b; labelled on no view,
   * decision-r07-provisional-photometry); or no section, Design note 5's
   * Lambert sphere (`provisional`).
   */
  readonly provenance: "modelled" | "modelled-provisional" | "provisional";
}

/**
 * The label a body without a photometric section carries (the guide's
 * `BODY PHOTOMETRY: NOT YET MODELLED` row).
 */
export type AppearanceLabel = "BODY PHOTOMETRY: NOT YET MODELLED";

/** The provisional photometry's geometric albedo, every channel (Design note 5). */
const PROVISIONAL_ALBEDO = 0.2;

/** The provisional photometry's phase integral, a Lambert sphere's 3 ÷ 2. */
const PROVISIONAL_PHASE_INTEGRAL = 1.5;

/**
 * Design note 5's photometry for a body with no photometric section: a Lambert sphere of spherical
 * albedo 0.3 (p = 0.2, q = 1.5), brighter at large phase than any real body, labelled.
 */
export const PROVISIONAL_PHOTOMETRY: BodyPhotometry = {
  geometricAlbedo: [PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO],
  phaseIntegral: [
    PROVISIONAL_PHASE_INTEGRAL,
    PROVISIONAL_PHASE_INTEGRAL,
    PROVISIONAL_PHASE_INTEGRAL,
  ],
  law: lawFor(
    [PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO],
    [PROVISIONAL_PHASE_INTEGRAL, PROVISIONAL_PHASE_INTEGRAL, PROVISIONAL_PHASE_INTEGRAL],
    "lambert",
  ),
  bondRatioCheck: null,
  provenance: "provisional",
};

/** The labels of a body shaded with {@link PROVISIONAL_PHOTOMETRY}. */
export const PROVISIONAL_LABELS: ReadonlyArray<AppearanceLabel> = [
  "BODY PHOTOMETRY: NOT YET MODELLED",
];

/**
 * How far the law's p_V q_V ÷ A_Bond may stand from the ratio plan 14 states before the body is a
 * finding for plan 14's owner: 5% (Design note 5).
 */
export const BOND_RATIO_TOLERANCE = 0.05;

/**
 * A body whose stated ratio p_V q_V ÷ A_Bond departs from its law's by more than
 * {@link BOND_RATIO_TOLERANCE}: a finding for plan 14's owner, not an input the client reconciles.
 */
export interface BondRatioFinding {
  /** The ratio the section states. */
  readonly statedRatio: number;
  /** p_V times the law's q_V, over the section's Bond albedo. */
  readonly lawRatio: number;
}

/** What the wire gives a body's shading: its photometry, its figure and its labels. */
export interface WireAppearance {
  readonly photometry: BodyPhotometry;
  /** `null` where the body's radius is not granted: it stays R02's mark. */
  readonly figure: BodyFigure | null;
  readonly labels: ReadonlyArray<AppearanceLabel>;
  /** The section's ratio where it departs from the law's; `null` where it agrees, or with none. */
  readonly bondRatioFinding: BondRatioFinding | null;
}

/** The client's template of the wire's (P14.T47.d: the wire's `airless_ice` is `airless-ice`). */
function templateOf(template: PhaseTemplateDto): PhaseTemplateId {
  let id: PhaseTemplateId;
  switch (template) {
    case "airless_ice":
      id = "airless-ice";
      break;
    case "moon":
    case "mercury":
    case "mars":
    case "venus":
    case "earth":
    case "jupiter":
    case "saturn":
    case "uranus":
    case "neptune":
    case "snowball":
    case "magma":
      id = template;
      break;
  }
  return id;
}

/** A modelled photometry and how its stated ratio stands against its law's. */
export interface ModelledPhotometry {
  readonly photometry: BodyPhotometry;
  /** p_V times the law's q_V, over the section's Bond albedo. */
  readonly lawRatio: number;
  /** The section's ratio where it departs from the law's; `null` where it agrees. */
  readonly bondRatioFinding: BondRatioFinding | null;
}

/** The finding a stated ratio makes against its law's, or `null` within the tolerance. */
function bondRatioFindingOf(statedRatio: number, lawRatio: number): BondRatioFinding | null {
  return Math.abs(lawRatio / statedRatio - 1) > BOND_RATIO_TOLERANCE
    ? { statedRatio, lawRatio }
    : null;
}

/**
 * The photometry a section states: its law, A = p ÷ [L + ⅔(1 − L)] and s per band on its template
 * at its L, with B, V and R as the display's b, g and r; q found from that law; `modelled`.
 */
export function modelledPhotometry(section: SystemBodyPhotometry): ModelledPhotometry {
  const { geometricAlbedo: p, phaseExponent: s, lunarLambertShare: l } = section;
  const k = shapeGeometricAlbedo(l);
  const law: PhotometricLaw = {
    a: [p.r / k, p.v / k, p.b / k],
    lommelSeeligerShare: l,
    template: templateOf(section.phaseTemplate),
    phaseExponent: [s.r, s.v, s.b],
  };
  const q = lawPhaseIntegral(law);
  const lawRatio = (p.v * q[1]) / section.bondAlbedo;
  return {
    photometry: {
      geometricAlbedo: [p.r, p.v, p.b],
      phaseIntegral: q,
      law,
      bondRatioCheck: section.bondRatio,
      provenance: section.provisional ? "modelled-provisional" : "modelled",
    },
    lawRatio,
    bondRatioFinding: bondRatioFindingOf(section.bondRatio, lawRatio),
  };
}

/**
 * How many sections' photometries {@link heldPhotometryOf} keeps: far more than a system's bodies
 * have distinct sections, which share a state's albedos.
 */
export const HELD_PHOTOMETRIES_KEPT = 256;

/** The photometries made, by their sections' values, the least recently used first. */
const HELD_PHOTOMETRIES = new Map<string, ModelledPhotometry>();

/** A section's values, each number as its shortest round-trip decimal. */
function sectionKey(section: SystemBodyPhotometry): string {
  const { geometricAlbedo: p, phaseExponent: s } = section;
  return [
    section.phaseTemplate,
    section.provisional,
    section.lunarLambertShare,
    p.b,
    p.v,
    p.r,
    s.b,
    s.v,
    s.r,
    section.bondAlbedo,
    section.bondRatio,
  ]
    .map(String)
    .join(" ");
}

/**
 * The section's {@link modelledPhotometry}, made once for equal sections and kept among the
 * {@link HELD_PHOTOMETRIES_KEPT} used most recently, so that a body's record parsed again (a body
 * re-sent at a new level parses its whole system again) keeps its law object, and its phase table
 * is not tabulated and uploaded again.
 */
export function heldPhotometryOf(section: SystemBodyPhotometry): ModelledPhotometry {
  const key = sectionKey(section);
  const held = HELD_PHOTOMETRIES.get(key);
  if (held !== undefined) {
    HELD_PHOTOMETRIES.delete(key);
    HELD_PHOTOMETRIES.set(key, held);
    return held;
  }
  const made = modelledPhotometry(section);
  HELD_PHOTOMETRIES.set(key, made);
  if (HELD_PHOTOMETRIES.size > HELD_PHOTOMETRIES_KEPT) {
    const oldest = HELD_PHOTOMETRIES.keys().next();
    if (oldest.done !== true) {
      HELD_PHOTOMETRIES.delete(oldest.value);
    }
  }
  return made;
}

/** The body's figure: its section's spheroid, else a sphere of its granted radius, else none. */
function figureOf(body: SystemBody): BodyFigure | null {
  if (body.figure.state === "ok") {
    const { equatorialRadiusM, polarRadiusM, pole } = body.figure.value;
    return { equatorialRadiusM, polarRadiusM, pole };
  }
  const radiusM = body.bulk.state === "ok" ? body.bulk.value.radiusM : null;
  if (radiusM === null || radiusM <= 0) {
    return null;
  }
  const pole = body.rotation.state === "ok" ? body.rotation.value.pole : null;
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole };
}

/** A body's photometry, figure, labels and any ratio finding from its record's sections. */
export function appearanceFromWire(body: SystemBody): WireAppearance {
  const figure = figureOf(body);
  if (body.photometry.state !== "ok") {
    return {
      photometry: PROVISIONAL_PHOTOMETRY,
      figure,
      labels: PROVISIONAL_LABELS,
      bondRatioFinding: null,
    };
  }
  const { photometry, bondRatioFinding } = heldPhotometryOf(body.photometry.value);
  return { photometry, figure, labels: [], bondRatioFinding };
}

/** Each record's appearance, made once, so that a drawing loop reads one object a body. */
const HELD_APPEARANCES = new WeakMap<SystemBody, WireAppearance>();

/**
 * The body's {@link appearanceFromWire}, made once per record and kept while the record is: a pure
 * memo, which a render may call.
 */
export function heldAppearanceOf(body: SystemBody): WireAppearance {
  const held = HELD_APPEARANCES.get(body);
  if (held !== undefined) {
    return held;
  }
  const appearance = appearanceFromWire(body);
  HELD_APPEARANCES.set(body, appearance);
  return appearance;
}

/** The designations each finding has been reported for. */
const REPORTED = new WeakMap<BondRatioFinding, Set<string>>();

/**
 * Reports a body's ratio finding with `console.warn`, once a body and finding: a diagnostic for
 * plan 14's owner (Design note 5), which changes nothing drawn.
 *
 * @remarks
 * A side effect: call it from the drawing loop, never from a render. A finding is held with its
 * section's photometry ({@link heldPhotometryOf}), so a body re-sent with an equal section is not
 * reported again.
 *
 * @param designation - The body's designation as the view labels it.
 */
export function reportBondRatioFinding(designation: string, finding: BondRatioFinding): void {
  const reported = REPORTED.get(finding) ?? new Set<string>();
  if (reported.has(designation)) {
    return;
  }
  reported.add(designation);
  REPORTED.set(finding, reported);
  const departurePercent = (finding.lawRatio / finding.statedRatio - 1) * 100;
  console.warn(
    `body ${designation}: plan 14 states p_V q_V ÷ A_Bond ${String(finding.statedRatio)}, ` +
      `its law gives ${String(finding.lawRatio)} (${departurePercent.toFixed(1)}%), beyond ` +
      `R07 Design note 5's 5%: a finding for plan 14's owner`,
  );
}
