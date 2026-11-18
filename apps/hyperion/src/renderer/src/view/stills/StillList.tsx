import { type ReactNode, useEffect, useId, useState } from "react";

import type { SavedStill, StillLabel, StillSurveyCoverage } from "../../../../preload/api";
import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { StatusText } from "../../components/StatusText";
import { formatListPosition, formatNumber, formatUniverseTimeDhms } from "../../lib/format";
import { QUALITY_NAMES } from "../quality/qualitySetting";
import { formatCoverage, formatSurveyResolution } from "../terrain/readouts";
import type { StillListState } from "./stillStore";

/** Props of {@link StillList}. */
export interface StillListProps {
  readonly state: StillListState;
}

/** What the list says before the first listing answers. */
export const STILLS_PENDING = "STILLS: PENDING";

/** What the list says where no still is saved. */
export const NO_STILLS = "NO STILLS SAVED";

/** What the list says where the folder could not be listed; a new mount lists it again. */
const NOT_LISTED = "NOT LISTED: the stills folder could not be read, reopen VIEW to retry";

/** A still's size: `3,840 × 2,160 px`. */
function sizeText(label: StillLabel): string {
  return `${formatNumber(label.size.widthPx, 0)} × ${formatNumber(label.size.heightPx, 0)} px`;
}

/** The still's time as the view's `TIME` reads it: universe time, `UT +13 yr 024/06:27:58`. */
function timeText(label: StillLabel): string {
  return `UT ${formatUniverseTimeDhms(label.time)}`;
}

/**
 * A body's coverage as the view's survey line reads it, after the body's name: `TEST EARTH
 * 34.0 % · FINEST 1.2 km`, or `TEST EARTH NONE` where none of it was surveyed.
 */
function coverageText(coverage: StillSurveyCoverage): string {
  const share = formatCoverage({
    surveyedFraction: () => coverage.surveyedFraction,
    bestResolutionM: () => coverage.bestResolutionM,
  });
  if (coverage.bestResolutionM === null) {
    return `${coverage.name} ${share}`;
  }
  const finest = formatSurveyResolution(coverage.bestResolutionM, null);
  return `${coverage.name} ${share} · FINEST ${finest.value} ${finest.unit}`;
}

/** One row of a label's record: its term and its value, one line or several. */
interface LabelRow {
  readonly term: string;
  readonly value: ReactNode;
}

/**
 * Every field of a still's label as the list shows it (Design note 17): the view's own readings at
 * the request, in its words and order (its frame, time, camera, field of view, exposure), then the
 * still's: its quality, its decoration, its survey coverage where any body's survey was held (none
 * held is no value, so not `NONE`, the guide's "nothing surveyed"), its annunciations, set as the
 * view sets them, and its size. A still's row a reading already gave is not given again.
 */
function labelRows(label: StillLabel): ReadonlyArray<LabelRow> {
  const readings: LabelRow[] = label.readings.map(({ label: term, value }) => ({ term, value }));
  const survey: LabelRow[] =
    label.survey.length === 0
      ? []
      : [
          {
            term: "SURVEY COVERAGE",
            value: label.survey.map((coverage) => (
              <span className="still-list__line" key={coverage.body}>
                {coverageText(coverage)}
              </span>
            )),
          },
        ];
  const own: LabelRow[] = [
    { term: "QUALITY", value: QUALITY_NAMES[label.setting] },
    { term: "DECORATION", value: label.decoration ? "ON" : "OFF" },
    ...survey,
    {
      term: "ANNUNCIATIONS",
      value:
        label.annunciations.length === 0 ? (
          "NONE"
        ) : (
          <ul className="still-list__annunciations">
            {label.annunciations.map((words) => (
              <li key={words}>
                <StatusText text={words} />
              </li>
            ))}
          </ul>
        ),
    },
    { term: "SIZE", value: sizeText(label) },
  ];
  const terms = new Set(readings.map(({ term }) => term));
  return [...readings, ...own.filter(({ term }) => !terms.has(term))];
}

interface StillEntryProps {
  readonly still: SavedStill;
  readonly open: boolean;
  readonly onToggle: () => void;
}

/** A saved still: a disclosure of its time and size, which shows or hides its whole label. */
function StillEntry({ still, open, onToggle }: StillEntryProps) {
  const labelId = useId();
  const { label } = still;
  return (
    <li className="still-list__entry">
      <button
        type="button"
        className="control disclosure still-list__still"
        aria-expanded={open}
        aria-controls={labelId}
        onClick={onToggle}
      >
        <DisclosureGlyph expanded={open} />
        {timeText(label)} · {sizeText(label)}
      </button>
      <dl className="readout still-list__label" id={labelId} hidden={!open}>
        {labelRows(label).map(({ term, value }) => (
          <div className="still-list__row" key={term}>
            <dt>{term}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
    </li>
  );
}

/** The entries of a scrolling list in view, 0-based and inclusive. */
interface InView {
  readonly first: number;
  readonly last: number;
}

/**
 * The entries of `list` in view, or `null` where it does not overflow, measured from their boxes,
 * since an open label makes its entry taller than the rest.
 */
function inViewOf(list: HTMLElement): InView | null {
  if (list.scrollHeight <= list.clientHeight) {
    return null;
  }
  const box = list.getBoundingClientRect();
  let first = -1;
  let last = -1;
  for (const [index, entry] of [...list.children].entries()) {
    const rect = entry.getBoundingClientRect();
    if (rect.bottom > box.top && rect.top < box.bottom) {
      first = first < 0 ? index : first;
      last = index;
    }
  }
  return first < 0 ? null : { first, last };
}

/** `next`, or `last` where it says the same, so that an unchanged reading renders nothing. */
function keptInView(last: InView | null, next: InView | null): InView | null {
  return last?.first === next?.first && last?.last === next?.last ? last : next;
}

/**
 * The entries of `list` that are in view, kept as it scrolls, as it or an entry changes size and as
 * entries come and go; `null` while it does not overflow, or there is no list.
 *
 * @remarks
 * Measured first when its `ResizeObserver` reports the list's first size, as a browser does once a
 * target is observed and laid out.
 */
function useInView(list: HTMLUListElement | null): InView | null {
  const [inView, setInView] = useState<InView | null>(null);
  useEffect(() => {
    if (list === null) {
      return undefined;
    }
    const target = list;
    const measure = (): void => {
      setInView((last) => keptInView(last, inViewOf(target)));
    };
    const sizes = new ResizeObserver(measure);
    // Observing a target again is a no-op, so each entry, the new among them, is observed once.
    const observeAll = (): void => {
      sizes.observe(target);
      for (const entry of target.children) {
        sizes.observe(entry);
      }
    };
    const entries = new MutationObserver(() => {
      observeAll();
      measure();
    });
    entries.observe(target, { childList: true });
    target.addEventListener("scroll", measure, { passive: true });
    observeAll();
    return () => {
      target.removeEventListener("scroll", measure);
      entries.disconnect();
      sizes.disconnect();
    };
  }, [list]);
  return list === null ? null : inView;
}

/**
 * The client's saved stills with their labels (plan R11, R11.T10.c, Design note 17), newest first,
 * in B612 Mono: each a disclosure of its time and size, which shows or hides every field of its
 * label.
 *
 * @remarks
 * One still's label stands open at a time; the disclosures are in the tab order, and each says by
 * `aria-expanded` whether its label is shown. Where the column leaves the list too little room it
 * scrolls, with the entries in view and their total under it (`1-3 of 12`), as every scrolling
 * list does.
 */
export function StillList({ state }: StillListProps) {
  const [openId, setOpenId] = useState<string | null>(null);
  const [list, setList] = useState<HTMLUListElement | null>(null);
  const total = state.kind === "listed" ? state.stills.length : 0;
  const inView = useInView(list);
  switch (state.kind) {
    case "listing":
      return (
        <p className="panel__empty still-list__empty">
          <StatusText text={STILLS_PENDING} />
        </p>
      );
    case "failed":
      return (
        <p className="panel__empty still-list__empty">
          <StatusText text={NOT_LISTED} />
        </p>
      );
    case "listed":
      break;
  }
  if (state.stills.length === 0) {
    return <p className="panel__empty still-list__empty">{NO_STILLS}</p>;
  }
  return (
    <>
      <ul className="still-list" aria-label="Saved stills" ref={setList}>
        {state.stills.map((still) => (
          <StillEntry
            key={still.id}
            still={still}
            open={openId === still.id}
            onToggle={() => {
              setOpenId(openId === still.id ? null : still.id);
            }}
          />
        ))}
      </ul>
      {inView === null ? null : (
        <p className="list-position">{formatListPosition(inView.first, inView.last, total)}</p>
      )}
    </>
  );
}
