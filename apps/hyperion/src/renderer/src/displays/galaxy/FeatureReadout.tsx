import type { ReactNode } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import {
  formatAge,
  formatListPosition,
  formatNumber,
  formatSci,
  formatSignificant,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import { emissionWords, featureKindWords } from "../../lib/galaxy/featureWords";
import type {
  BlackHoleModel,
  ChartFeature,
  FeatureDetailModel,
  FeatureStructure,
} from "../../lib/galaxy/model";
import { useItemsInView } from "../../lib/itemsInView";
import { formatHex64 } from "../../lib/seed";
import { type ChartDistanceUnit, formatFeatureDistance } from "./chartModel";
import { SystemReading as Reading } from "./SystemReading";

/** The words for a count that is zero: none expected, none left. */
const NONE = "NONE";

/** Props of {@link FeatureReadout}. */
export interface FeatureReadoutProps {
  /** The selected feature, as the chart's answer holds it. */
  readonly feature: ChartFeature;
  /** The chart time, in years from the epoch: the time its age and size are given at. */
  readonly timeYr: number;
  /**
   * The unit and decimals every distance on this chart is written with (plan 05, design note D21;
   * plan 09, P09.T42).
   */
  readonly distanceUnit: ChartDistanceUnit;
  /**
   * Whether the chart's answer, which the feature is read from, is a snapshot the link no longer
   * backs: each of its values is then muted and trails its `S`, the structure's and the black
   * hole's with them.
   */
  readonly stale: boolean;
  /** Its structure as `feature_detail` answered for it, or `null` before the answer. */
  readonly detail: FeatureDetailModel | null;
  /** Whether `detail` is a snapshot of its own: an answer for a time the chart has since left. */
  readonly detailStale: boolean;
  /**
   * The central black hole as `black_hole_state` answered at the chart's time, for the galactic
   * centre; `null` for any other feature and before the answer.
   */
  readonly blackHole: BlackHoleModel | null;
  /** Whether `blackHole` is a snapshot of its own, for a time the chart has since left. */
  readonly blackHoleStale: boolean;
}

/** An expected count, Estimated, or `NONE` for one of zero: `~459`, `~9.14E7`. */
function expectedWords(expected: number): string {
  return expected > 0 ? `~${formatSignificant(expected)}` : NONE;
}

/**
 * A feature's expected members, Estimated, or `null`, Missing, for the zero the server sends for a
 * feature it gives no cluster model, a cloud, an association or an unbound region, whose stars it
 * does not count as members: a value it does not compute, never none.
 */
function membersWords(expected: number): string | null {
  return expected > 0 ? `~${formatSignificant(expected)}` : null;
}

interface StructureReadingsProps {
  readonly structure: FeatureStructure;
  readonly stale: boolean;
}

/** The feature's structure, by what it is, each reading to the guide's three figures. */
function StructureReadings({ structure, stale }: StructureReadingsProps) {
  let content: ReactNode;
  switch (structure.kind) {
    case "cluster":
      content = (
        <>
          <Reading
            label="CORE RADIUS"
            value={formatSignificant(structure.coreRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="HALF-MASS RADIUS"
            value={formatSignificant(structure.halfMassRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="TIDAL RADIUS"
            value={formatSignificant(structure.tidalRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="ESCAPE SPEED"
            value={formatSignificant(structure.centralEscapeSpeedKmS)}
            unit="km/s"
            stale={stale}
          />
          <Reading
            label="RELAXATION TIME"
            value={formatSignificant(structure.relaxationTimeMyr)}
            unit="Myr"
            stale={stale}
          />
          <Reading
            label="CORE COLLAPSED"
            value={structure.coreCollapsed ? "YES" : "NO"}
            stale={stale}
          />
          <Reading label="BLACK HOLES" value={expectedWords(structure.blackHoles)} stale={stale} />
          <Reading
            label="MEMBER CLASSES"
            value={formatNumber(structure.classes.length, 0)}
            stale={stale}
          />
        </>
      );
      break;
    case "centre":
      content = (
        <>
          <Reading
            label="INFLUENCE RADIUS"
            value={formatSignificant(structure.influenceRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="LOSS-CONE RADIUS"
            value={formatSignificant(structure.lossConeRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="MEMBER CLASSES"
            value={formatNumber(structure.classes.length, 0)}
            stale={stale}
          />
        </>
      );
      break;
    case "cloud":
      content = (
        <>
          <Reading
            label="CORE RADIUS"
            value={formatSignificant(structure.coreRadiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="CENTRAL DENSITY"
            value={formatSci(structure.centralDensityPerCm3)}
            unit="/cm³"
            stale={stale}
          />
        </>
      );
      break;
    case "association":
      content = (
        <>
          <Reading
            label="RADIUS"
            value={formatSignificant(structure.radiusLy)}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="EXPANSION"
            value={formatSignificant(structure.expansionSpeedKmS)}
            unit="km/s"
            stale={stale}
          />
        </>
      );
      break;
  }
  return content;
}

/**
 * Everything known about the selected feature (plan 09, P09.T41.c): what it is called and what it
 * is, where it is, how large, old and heavy it is, how many systems it is expected to hold, what
 * it shines as, and, once `feature_detail` has answered, its structure; for the galactic centre,
 * once `black_hole_state` has, its black hole.
 *
 * @remarks
 * A live region read as a whole, as the system readout is ({@link SystemReadout}), so that a
 * selection made on the chart or in the list is announced once, in the same scrolling region with
 * its total under it. `KIND` names the kind in words (`OPEN CLUSTER`), the abbreviation its mark
 * and the list carry being on the guide's list; `DIST` is to its centre, as the list writes it
 * ({@link formatFeatureDistance}); `EXTENT` its radius at
 * the chart's time, the sphere the chart draws it at. `AGE` is given at the chart's time, after a
 * middle dot, as a system's is, and is the em dash for a molecular cloud, which has none, and for
 * the centre, whose six episodes have no one age. `MEMBERS` is the model's expected count over
 * its member classes, an Estimated value, `~459`, or the em dash for the zero the server sends for a
 * feature it gives no cluster model, a cloud, an association or an unbound region: an expectation,
 * never the number of systems a range query returns. `EMISSION` names its emission class (`H II`, `REFLECTION`, `DARK CLOUD`), or
 * `NONE` for a feature that shines as nothing but its stars; `SUPERBUBBLE` is a nursery's bubble's
 * radius, or `NONE`.
 *
 * Under `STRUCTURE`, a cluster's radii, central escape speed, relaxation time, whether its core has
 * collapsed, its expected black holes and how many member classes it has; the centre's influence
 * and loss-cone radii and member classes; a cloud's core radius and central density; an
 * association's radius and expansion speed. The heading and its readings come with the answer,
 * since the feature's structure, not its kind, decides them; where the request stands is said
 * beside the readout. For the centre, under `BLACK HOLE`, its mass, its unabsorbed 2–10 keV
 * luminosity (`X-RAY LUMINOSITY`, the em dash from a server that does not say), and its tidal
 * disruptions back to the source horizon, their count and the newest's time.
 *
 * A stale value is muted and trails the guide's `S`, as the system readout's are: the chart's
 * with the chart's answer, the structure's and the black hole's with it or when they lag the chart.
 */
export function FeatureReadout({
  feature,
  timeYr,
  distanceUnit,
  stale,
  detail,
  detailStale,
  blackHole,
  blackHoleStale,
}: FeatureReadoutProps) {
  const age = feature.ageMyr === null ? null : formatAge(feature.ageMyr);
  const structure = detail?.id === feature.id ? detail.structure : null;
  const latest = blackHole?.disruptions[0] ?? null;
  const { ref, range } = useItemsInView(
    "dt",
    `${feature.id}:${structure?.kind ?? "none"}:${blackHole === null ? "none" : "black hole"}`,
    feature.id,
  );
  return (
    <>
      <section
        className="system-readout__scroll"
        ref={ref}
        aria-label="Readings"
        // A scrolling region with nothing focusable in it must take focus itself to be scrolled
        // from the keyboard (WCAG 2.1.1); the rule allows tabIndex on interactive roles only.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        <div
          className="system-readout"
          // A `dl` cannot sit in an `output`, as the system readout's cannot.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="status"
          aria-label="Selected feature"
          aria-atomic="true"
        >
          <dl className="readout system-readout__values">
            <Reading label="DESIG" value={feature.designation} wide stale={stale} />
            <Reading label="ID" value={formatHex64(feature.id)} wide stale={stale} />
            <Reading label="KIND" value={featureKindWords(feature.kind)} wide stale={stale} />
            <Reading
              label="DIST"
              value={formatFeatureDistance(feature.distanceLy, distanceUnit)}
              unit={distanceUnit.symbol}
              stale={stale}
            />
            <Reading
              label="EXTENT"
              value={formatSignificant(feature.extentLy)}
              unit="ly"
              stale={stale}
            />
            <Reading
              label="MASS"
              value={formatSignificant(feature.massMsun)}
              unit={<SolarMassUnit />}
              stale={stale}
            />
            <Reading
              label="AGE"
              value={age?.value ?? null}
              unit={age?.unit}
              beside={
                <>
                  · AT {TIME_SYSTEM_LABEL} {formatUniverseTimeYr(timeYr)} yr
                </>
              }
              wide
              stale={stale}
            />
            <Reading label="MEMBERS" value={membersWords(feature.expectedMembers)} stale={stale} />
            <Reading
              label="EMISSION"
              value={feature.emission === null ? NONE : emissionWords(feature.emission)}
              stale={stale}
            />
            <Reading
              label="SUPERBUBBLE"
              value={
                feature.bubbleRadiusLy === null ? NONE : formatSignificant(feature.bubbleRadiusLy)
              }
              unit={feature.bubbleRadiusLy === null ? undefined : "ly"}
              stale={stale}
            />
          </dl>
          {structure === null ? null : (
            <>
              <h3 className="system-readout__heading">STRUCTURE</h3>
              <dl className="readout system-readout__values">
                <StructureReadings structure={structure} stale={stale || detailStale} />
              </dl>
            </>
          )}
          {blackHole === null ? null : (
            <>
              <h3 className="system-readout__heading">BLACK HOLE</h3>
              <dl className="readout system-readout__values">
                <Reading
                  label="MASS"
                  value={formatSci(blackHole.massMsun)}
                  unit={<SolarMassUnit />}
                  stale={stale || blackHoleStale}
                />
                <Reading
                  label="X-RAY LUMINOSITY"
                  value={blackHole.luminosityW === null ? null : formatSci(blackHole.luminosityW)}
                  unit="W"
                  stale={stale || blackHoleStale}
                />
                <Reading
                  label="DISRUPTIONS"
                  value={formatNumber(blackHole.disruptions.length, 0)}
                  stale={stale || blackHoleStale}
                />
                <Reading
                  label="LAST DISRUPTION"
                  value={
                    latest === null
                      ? NONE
                      : `${TIME_SYSTEM_LABEL} ${formatUniverseTimeYr(latest.timeYr)}`
                  }
                  unit={latest === null ? undefined : "yr"}
                  wide
                  stale={stale || blackHoleStale}
                />
              </dl>
            </>
          )}
        </div>
      </section>
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, range.total)}
        </p>
      )}
    </>
  );
}
