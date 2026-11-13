/**
 * The measured sizes' counting of one terrain pass (plan R10, R10.T14.b; decision-r10-t14b-measure
 * §2, §3 and §5): once a second of script time, five unbudgeted re-selections of the pass's own
 * selection input, from which each cache's peaks, the sizing rule's figures, the all-round peak's
 * residency and the diagnostic occluder's price follow.
 *
 * @remarks
 * The counter listens to a pass through `TerrainPass.observe` and only listens: it draws nothing,
 * demands nothing and hands nothing back. Each count is `selectPatches` over the pass's own input,
 * its views, contacts, baked ranges, survey and skirt margin exactly, with no `maxPatches`, and over
 * the counter's own copy of the pass's planet object, so that selection's memo of the planet
 * (`select.ts`' tables, keyed by the planet object, and the `SelectedPatch` objects it keeps) is
 * the counter's and the pass's is never read or written by a count. A new planet object, a new
 * level table, gets a copy of its own.
 *
 * The five counts (§2):
 * 1. F, at the pass's own views and frustum margins;
 * 2. F₀, every margin 0, for the margin's factor F ÷ F₀;
 * 3. AR, every view all round (`ViewSelectionInput.allRound`), its horizon alone culling;
 * 4. F′, count 1 with the diagnostic occluder (`SelectionInput.occluderHeightM`) at §5's depth;
 * 5. AR′, count 3 with it.
 *
 * A view's count is the leaves with its bit in `SelectedPatch.views` and not forced; a count's
 * union is the leaves any view sees, not forced; the forced leaves, seen or not, are counted once a
 * count, never in a view's. The depth is a running minimum, over the run, of the field header's
 * lowest sampled height and of the resident bakes' lowest range; the primed counts are made from
 * the first sample with a depth on.
 *
 * The resident bakes are read as the cache lets them be read: `PatchCache` gives no walk of its
 * resident patches, so each sample visits the resident patch at every level over each leaf of
 * counts 1 and 3 and of the pass's own selection (`PatchCache.residentOver`), every resident
 * ancestor of a leaf once. A resident patch over no leaf of these, behind the views or below a
 * coarsened leaf, is not read. Over ranges that are not a `PatchCache` the leaves' own ranges are
 * read alone.
 */

import type { RenderStyle } from "../camera/state";
import { type CachedPatch, PatchCache } from "../terrain/cache";
import {
  ALL_ROUND_FACTOR,
  FRUSTUM_FACTOR,
  frustumRuleSlots,
  REFERENCE_PX_PER_RAD,
  sizingRuleSlots,
  slotBudgetBytes,
  type StreamingViewPeaks,
} from "../terrain/cacheBudget";
import { PATCH_QUADS, vertexSpacing } from "../terrain/cube";
import { levelBoundM, type PlanetGeometry } from "../terrain/planet";
import {
  type HeightRangeLookup,
  type Selection,
  type SelectionInput,
  selectionErrorM,
  selectPatches,
  type ViewSelectionInput,
} from "../terrain/select";
import type { TerrainPass } from "../terrain/terrainPass";
import { type MsSpread, msSpread } from "./bakeClock";

/** Which cache a pass is: the lit one (photorealistic views) or the wireframe's (§2, "per cache"). */
export type SizeCountCache = "lit" | "wireframe";

/** One of the pass's views as the spike knows it, for the record. */
export interface SizeCountView {
  /** The view's name in the spike's session (its `ViewId`, or the spike's label for it). */
  readonly id: string;
  readonly style: RenderStyle;
}

/** The script's moment at a selection, as the spike gives it. */
export interface SizeCountContext {
  /** The script's time, s. */
  readonly scriptTimeS: number;
  /** The script's segment, or `null` outside one. */
  readonly segment: string | null;
  /** The craft's height above the ground under it, metres, or `null` where none is known. */
  readonly heightAboveGroundM: number | null;
}

/** What a {@link PassSizeCounter} counts and reads. */
export interface PassSizeCounterOptions {
  /** The cache the pass holds. */
  readonly cache: SizeCountCache;
  /**
   * The pass's views: by default in its selection input's order, view k `SelectionInput.views[k]`
   * and bit k of `SelectedPatch.views` ({@link PassSizeCounterOptions.viewIndices} places them
   * otherwise). A view the input does not hold at a sample counts `null` there; an input view none
   * of these is counts in the unions alone.
   */
  readonly views: ReadonlyArray<SizeCountView>;
  /**
   * Where each of {@link PassSizeCounterOptions.views} is among a selection's input views: for
   * view k, the index of its entry (its bit in `SelectedPatch.views`), or `null` where the input
   * holds none of it; read at each sample. An index that is not one of the input's, or that an
   * earlier view already took, counts `null`. Absent, view k is the input's k-th.
   */
  readonly viewIndices?: (views: ReadonlyArray<ViewSelectionInput>) => ReadonlyArray<number | null>;
  /** The cache's bytes a slot, B, above 0: what the rule's bytes are reckoned at. */
  readonly bytesPerSlot: number;
  /** The script's moment now, read at each selection. */
  readonly context: () => SizeCountContext;
  /**
   * The field header's lowest sampled height, metres above the datum, or `null` where none is known
   * (the test planet's, or a field not yet held): one term of §5's running minimum.
   */
  readonly sampledLowestM: () => number | null;
  /** The clock the counting's CPU time is read on, ms; `performance.now()` by default. */
  readonly nowMs?: () => number;
}

/** The five counts, in the order they are made (§2). */
export const SIZE_COUNTS = [
  "frustum",
  "frustumNoMargin",
  "allRound",
  "frustumDiagnostic",
  "allRoundDiagnostic",
] as const;

/**
 * One of the five counts: F, F₀, AR, F′ and AR′ (§2): at the pass's margins, at margin 0, all round,
 * and the first and third with §5's diagnostic occluder.
 */
export type SizeCountName = (typeof SIZE_COUNTS)[number];

/** A count's peak over the run: the most patches at a sample, and where it fell. */
export interface SizePeak {
  /** Patches; 0 where no sample counted it. */
  readonly patches: number;
  /** The sample it fell at, the first if tied, or `null` where no sample counted it. */
  readonly at: SizeCountContext | null;
}

/** One count's peaks: each view's and the union's. */
export interface CountPeaks {
  /** Per view, in {@link PassSizeCounterOptions.views}' order: the leaves with its bit, not forced. */
  readonly views: ReadonlyArray<SizePeak>;
  /** The leaves any view sees, not forced: F∪, F₀∪, AR∪, F′∪ and AR′∪. */
  readonly union: SizePeak;
}

/** The forced region's peak, counted once (§2, "Forced"). */
export interface ForcedPeak extends SizePeak {
  /** The forced leaves some view sees at that sample, patches. */
  readonly seen: number;
}

/** A share's peak over the run, and where it fell. */
export interface SharePeak {
  /** The share, 0 to 1, or `null` where no sample gave one. */
  readonly share: number | null;
  readonly at: SizeCountContext | null;
}

/** A sizing figure, in slots and in bytes at the counter's bytes a slot. */
export interface RuleFigure {
  /** Slots, unrounded, as `cacheBudget.ts`' rules give them. */
  readonly slots: number;
  /** `slotBudgetBytes(slots, bytesPerSlot)`, B: whole slots. */
  readonly bytes: number;
}

/** The rule's figures per cache (§2, "The rule, per cache"). */
export interface RuleFigures {
  /** The ruled form: Σ_views max(1.3 AR_peak, 3 F_peak) + the forced peak (`sizingRuleSlots`). */
  readonly ruled: RuleFigure;
  /** (a) The largest same-sample value of Σ_views max(1.3 AR, 3 F) + forced. */
  readonly sameSample: RuleFigure;
  /** (b) The union form: max(1.3 AR∪_peak, 3 F∪_peak) + the forced peak. */
  readonly union: RuleFigure;
  /** (c) The frustum rule: Σ_views 3 F_peak + the forced peak (`frustumRuleSlots`). */
  readonly frustum: RuleFigure;
}

/**
 * The diagnostic occluder's price on one view, or on the union (§5, "Price"), in the rule's slots:
 * each term's difference at the peaks weighed as the rule weighs it.
 */
export interface OccluderPriceTerm {
  /** AR_peak − AR′_peak, patches. */
  readonly allRoundPatches: number;
  /** F_peak − F′_peak, patches. */
  readonly frustumPatches: number;
  /** The all-round term's price, 1.3 (AR_peak − AR′_peak) (`ALL_ROUND_FACTOR`), slots. */
  readonly allRoundSlots: number;
  /** The frustum term's price, 3 (F_peak − F′_peak) (`FRUSTUM_FACTOR`), slots. */
  readonly frustumSlots: number;
  /**
   * {@link OccluderPriceTerm.allRoundSlots} ÷ the sized all-round peak
   * ({@link OccluderPrice.sizedAllRoundSlots}): for a pass's only view, (AR − AR′) ÷ AR_peak;
   * `null` where the peak is 0.
   */
  readonly allRoundShare: number | null;
  /** {@link OccluderPriceTerm.frustumSlots} ÷ the sized all-round peak; `null` where it is 0. */
  readonly frustumShare: number | null;
}

/**
 * The A/B's price (§5): the slots the as-built occluder's looseness costs at the peaks, over the
 * samples that made both the plain and the diagnostic counts, each term's and the rule's own, as
 * shares of the sized all-round peak.
 */
export interface OccluderPrice {
  /** Per view, in {@link PassSizeCounterOptions.views}' order. */
  readonly views: ReadonlyArray<OccluderPriceTerm>;
  readonly union: OccluderPriceTerm;
  /** The sized all-round peak: the ruled sum's all-round term, Σ_views 1.3 AR_peak, slots. */
  readonly sizedAllRoundSlots: number;
  /** The rule's own difference, the ruled figure less its diagnostic one (rule − rule′), slots. */
  readonly ruledSlots: number;
  /**
   * {@link OccluderPrice.ruledSlots} ÷ the sized all-round peak; `null` where it is 0. §5's 10% test
   * reads this share: over 0.1 is a finding.
   */
  readonly ruledShare: number | null;
}

/** One level's bound ratio k_n = ε_n P₀ ÷ e_n (§2, "Logged beside"; T14.a's k₀). */
export interface LevelBoundRatio {
  readonly level: number;
  /** ε_n, the level table's bound, metres (`levelBoundM`). */
  readonly boundM: number;
  /** The error selection charges the level, ε_n plus the chord's sagitta, metres (`selectionErrorM`). */
  readonly chargedM: number;
  /** e_n, the level's mean patch edge, 64 mean vertex spacings, metres, as the count model takes it. */
  readonly edgeM: number;
  /** k_n = ε_n P₀ ÷ e_n, P₀ the reference view's 1,662.8 px rad⁻¹ (`REFERENCE_PX_PER_RAD`). */
  readonly ratio: number;
  /** The same of {@link LevelBoundRatio.chargedM}. */
  readonly chargedRatio: number;
}

/** The bound ratios of one level table the pass selected over, levels 0 to the finest. */
export interface LevelTableRatios {
  /** The table's number, from 0 in the order the pass's level tables were first met. */
  readonly version: number;
  /** The script time of the first selection over it, s. */
  readonly firstSeenS: number;
  readonly levels: ReadonlyArray<LevelBoundRatio>;
}

/** One level's box heights: count 1's seen leaves' extents along their centres' normals. */
export interface LevelBoxHeights {
  readonly level: number;
  /** The largest over the run, metres. */
  readonly maxM: number;
  /** The median at F∪'s peak sample, metres, or `null` where none of the level was seen there. */
  readonly medianAtFrustumPeakM: number | null;
}

/** A moment of the forced region's residency: when, and the forced patches of the pass then. */
export interface ForcedMark {
  /** Script time, s. */
  readonly atS: number;
  /** The forced patches of the pass's last selection. */
  readonly forced: number;
  /** Those of them resident. */
  readonly forcedResident: number;
}

/** The forced region's lead before contact and its residency at contact (T14.c's finding 3). */
export interface ContactLead {
  /** The script time of the pass's first selection holding a forced patch, s, or `null`. */
  readonly firstForcedS: number | null;
  /**
   * The script time of the first sample with forced patches every one of which was resident, s, or
   * `null` where none had.
   */
  readonly allResidentS: number | null;
  /**
   * Whether every sample after it kept its forced patches all resident (a sample with none keeps
   * them); `null` where no sample had them all.
   */
  readonly allResidentHeld: boolean | null;
  /** Touchdown as the spike marked it ({@link PassSizeCounter.markTouchdown}), or `null`. */
  readonly touchdown: ForcedMark | null;
  /** The lead before contact, touchdown − all resident, s; `null` where either is unknown. */
  readonly leadS: number | null;
  /** The contact by the session's rule ({@link PassSizeCounter.markContact}), or `null`. */
  readonly sessionContact: ForcedMark | null;
}

/**
 * A pass's own counts in one segment, each at its largest over its samples there (§2, "Logged
 * beside"): its selection, its budget, its resident patches, and the cache's slots in use and pins.
 */
export interface LoggedSegment {
  readonly segment: string | null;
  readonly samples: number;
  readonly selectedMax: number;
  /** The samples whose budget limited the pass's selection. */
  readonly limitedSamples: number;
  /** `null` where no sample's input held ranges. */
  readonly selectedResidentMax: number | null;
  /** `null` where no sample's ranges were a `PatchCache`. */
  readonly usedSlotsMax: number | null;
  readonly pinnedForcedMax: number | null;
  readonly pinnedDrawnMax: number | null;
}

/** A pass's counted sizes over the run. */
export interface PassSizeSummary {
  readonly cache: SizeCountCache;
  readonly views: ReadonlyArray<SizeCountView>;
  /** B. */
  readonly bytesPerSlot: number;
  /** The cache's slots, or `null` where the pass's ranges are not a `PatchCache`. */
  readonly slots: number | null;
  /** The samples counted. */
  readonly samples: number;
  /** Samples whose counting threw, and the first one's message; none were recorded. */
  readonly failedSamples: number;
  readonly firstFailure: string | null;
  /** Each count's peaks. */
  readonly counts: Readonly<Record<SizeCountName, CountPeaks>>;
  /** The forced peak, of count 1 (the forced leaves are the same in every count). */
  readonly forced: ForcedPeak;
  /** §3: |AR∪ ∩ resident| ÷ |AR∪|' peak. */
  readonly residentShare: SharePeak;
  /** §3: |AR∪ ∩ every key the pass has selected so far| ÷ |AR∪|' peak. */
  readonly sweptShare: SharePeak;
  /** §3's criterion: whether the resident share reached 100% at some sample. */
  readonly allRoundEverResident: boolean;
  /** §3's cap, slots ÷ |AR∪|, at AR∪'s peak; `null` without slots or a peak. */
  readonly capAtAllRoundPeak: number | null;
  /** The deepest diagnostic occluder the primed counts used, metres above the datum, or `null`. */
  readonly deepestOccluderM: number | null;
  /** k_n for each level table the pass selected over, one for a run of objects over the same. */
  readonly boundRatios: ReadonlyArray<LevelTableRatios>;
  /** Count 1's box heights by level, ascending. */
  readonly boxHeights: ReadonlyArray<LevelBoxHeights>;
  /** The five counts' CPU time a sample, ms. */
  readonly countingMs: MsSpread;
  /** The rule's figures on the plain counts. */
  readonly rule: RuleFigures;
  /** The same on the diagnostic counts (F′, AR′), or `null` where none was made. */
  readonly ruleDiagnostic: RuleFigures | null;
  /** The A/B's price, or `null` where no diagnostic count was made. */
  readonly occluderPrice: OccluderPrice | null;
  readonly contact: ContactLead;
  /** The pass's own counts by segment, in the order the samples met the segments. */
  readonly logged: ReadonlyArray<LoggedSegment>;
}

/** One view's columns, one entry a sample; `null` where the input did not hold the view. */
export interface ViewSizeSeries {
  readonly frustum: ReadonlyArray<number | null>;
  readonly frustumNoMargin: ReadonlyArray<number | null>;
  readonly allRound: ReadonlyArray<number | null>;
  /** `null` too at a sample with no diagnostic depth yet. */
  readonly frustumDiagnostic: ReadonlyArray<number | null>;
  readonly allRoundDiagnostic: ReadonlyArray<number | null>;
  /** The pass's own budgeted selection's leaves with the view's bit, not forced. */
  readonly selectedSeen: ReadonlyArray<number | null>;
  /** The view's frustum margin in the pass's input, rad. */
  readonly marginRad: ReadonlyArray<number | null>;
}

/** One level's box heights at each sample, metres; `null` where count 1 saw none of the level. */
export interface LevelBoxHeightSeries {
  readonly level: number;
  readonly medianM: ReadonlyArray<number | null>;
  readonly maxM: ReadonlyArray<number | null>;
}

/** The 1 Hz samples in column form: one entry a sample in every array. */
export interface PassSizeSeries {
  /** s. */
  readonly scriptTimeS: ReadonlyArray<number>;
  readonly segment: ReadonlyArray<string | null>;
  /** Metres. */
  readonly heightAboveGroundM: ReadonlyArray<number | null>;
  /** Per view, in {@link PassSizeCounterOptions.views}' order. */
  readonly views: ReadonlyArray<ViewSizeSeries>;
  /** Each count's union; the diagnostic ones `null` at a sample with no depth yet. */
  readonly union: Readonly<Record<SizeCountName, ReadonlyArray<number | null>>>;
  /** Count 1's forced leaves, and those some view sees. */
  readonly forced: ReadonlyArray<number>;
  readonly forcedSeen: ReadonlyArray<number>;
  /**
   * The pass's own selection's forced patches resident, `null` with no ranges: the same patches as
   * count 1's forced leaves, since selection refuses no forced split for its budget.
   */
  readonly forcedResident: ReadonlyArray<number | null>;
  /** The pass's own selection: its patches, and whether its budget limited it. */
  readonly selected: ReadonlyArray<number>;
  readonly limited: ReadonlyArray<boolean>;
  /** The pass's selected patches resident, `null` with no ranges. */
  readonly selectedResident: ReadonlyArray<number | null>;
  /**
   * The cache's slots in use, and its pins (`PatchCache.pressure`, as the last retain set them,
   * a frame behind the selection): forced, and drawn outside the forced region; `null` where the
   * pass's ranges are not a `PatchCache`.
   */
  readonly usedSlots: ReadonlyArray<number | null>;
  readonly pinnedForced: ReadonlyArray<number | null>;
  readonly pinnedDrawn: ReadonlyArray<number | null>;
  /** §3's shares, 0 to 1, and the cap, slots ÷ |AR∪|. */
  readonly residentShare: ReadonlyArray<number | null>;
  readonly sweptShare: ReadonlyArray<number | null>;
  readonly cap: ReadonlyArray<number | null>;
  /** The five counts' CPU time, ms. */
  readonly countingMs: ReadonlyArray<number>;
  /** The diagnostic occluder's height the primed counts used, metres above the datum. */
  readonly occluderM: ReadonlyArray<number | null>;
  /** Count 1's box heights, each level met, ascending. */
  readonly boxHeights: ReadonlyArray<LevelBoxHeightSeries>;
}

/** One count's tally at a sample. */
interface CountTally {
  /** Per described view; `null` where the input did not hold it. */
  readonly views: ReadonlyArray<number | null>;
  readonly union: number;
  readonly forced: number;
  readonly forcedSeen: number;
}

/** One level's box heights at a sample, metres. */
interface BoxHeightSample {
  readonly medianM: number;
  readonly maxM: number;
}

/** One sample's figures. */
interface SampleRow {
  readonly at: SizeCountContext;
  readonly counts: Readonly<Record<SizeCountName, CountTally | null>>;
  readonly occluderM: number | null;
  readonly marginsRad: ReadonlyArray<number | null>;
  /** The pass's own selection, tallied as a count is. */
  readonly pass: CountTally;
  readonly selected: number;
  readonly limited: boolean;
  readonly selectedResident: number | null;
  /** The pass's own selection's forced patches resident, `null` with no ranges. */
  readonly forcedResident: number | null;
  readonly slots: number | null;
  readonly usedSlots: number | null;
  readonly pinnedForced: number | null;
  readonly pinnedDrawn: number | null;
  readonly residentShare: number | null;
  readonly sweptShare: number | null;
  readonly cap: number | null;
  readonly countingMs: number;
  readonly boxHeights: ReadonlyMap<number, BoxHeightSample>;
}

/** The pass's last selection, for the contact's forced keys. */
interface LastSelection {
  readonly input: SelectionInput;
  readonly selection: Selection;
}

/**
 * The counting of one terrain pass for R10.T14.b (decision-r10-t14b-measure §2, §3, §5).
 *
 * @remarks
 * A sample is taken at the pass's first selection at or after each whole second of script time,
 * synchronously inside the pass's `onSelect`; a second with no reselection gives none, and a script
 * time that does not move forward gives none. The keys the pass selects are gathered from every
 * selection, sampled or not, for §3's swept share, and the first selection holding a forced patch
 * is noted for the contact's lead. A sample whose counting throws is counted as failed and left
 * out, so that the pass's frame goes on.
 *
 * @example
 * ```ts
 * const counter = new PassSizeCounter({
 *   cache: "lit",
 *   views: [{ id: "spike", style: "photorealistic" }],
 *   viewIndices: (inputs) => matchViewsByWeight([{ weight: 1, viewport }], inputs),
 *   bytesPerSlot: pass.layout.slots.bytesPerSlot,
 *   context: () => ({ scriptTimeS, segment, heightAboveGroundM }),
 *   sampledLowestM: () => (header?.main_liquid === null ? header.sea_level_m : null),
 * });
 * const detach = counter.attach(pass);
 * // at the first frame whose pose has reached the ground, and at the session's own contact:
 * counter.markTouchdown();
 * counter.markContact();
 * ```
 */
export class PassSizeCounter {
  readonly #cache: SizeCountCache;
  readonly #views: ReadonlyArray<SizeCountView>;
  readonly #bytesPerSlot: number;
  readonly #context: () => SizeCountContext;
  readonly #viewIndices:
    ((views: ReadonlyArray<ViewSelectionInput>) => ReadonlyArray<number | null>) | null;
  readonly #sampledLowestM: () => number | null;
  readonly #now: () => number;
  /** The counter's own copy of each planet object the pass selected over. */
  readonly #planets = new WeakMap<PlanetGeometry, PlanetGeometry>();
  readonly #tables: LevelTableRatios[] = [];
  /** Every key the pass has selected, by `patchKeyString`. */
  readonly #swept = new Set<string>();
  readonly #rows: SampleRow[] = [];
  /** The whole second of the last sample. */
  #sampledSecond: number | null = null;
  /** §5's running minimum, metres above the datum. */
  #depthM: number | null = null;
  #failures = 0;
  #firstFailure: string | null = null;
  #firstForcedS: number | null = null;
  #last: LastSelection | null = null;
  #touchdown: ForcedMark | null = null;
  #sessionContact: ForcedMark | null = null;

  /** @throws RangeError for bytes a slot that are not finite and above 0. */
  constructor(options: PassSizeCounterOptions) {
    if (!(Number.isFinite(options.bytesPerSlot) && options.bytesPerSlot > 0)) {
      throw new RangeError(
        `a size counter's bytes a slot must be finite and above 0, got ${options.bytesPerSlot}`,
      );
    }
    this.#cache = options.cache;
    this.#views = options.views;
    this.#bytesPerSlot = options.bytesPerSlot;
    this.#context = options.context;
    this.#viewIndices = options.viewIndices ?? null;
    this.#sampledLowestM = options.sampledLowestM;
    this.#now = options.nowMs ?? (() => performance.now());
  }

  /**
   * Listens to `pass`'s selections from its next on.
   *
   * @returns The listener's removal.
   */
  attach(pass: Pick<TerrainPass, "observe">): () => void {
    return pass.observe({
      onSelect: (input, selection) => {
        this.onSelect(input, selection);
      },
    });
  }

  /**
   * One of the pass's selections, its input and its result (`TerrainPassObserver.onSelect`):
   * gathered, and counted where it is the second's first.
   */
  onSelect(input: SelectionInput, selection: Selection): void {
    const at = this.#context();
    const own = this.#ownPlanet(input.planet, at);
    let forced = false;
    for (const [keyString, patch] of selection.patches) {
      this.#swept.add(keyString);
      forced ||= patch.forced;
    }
    if (forced && this.#firstForcedS === null) {
      this.#firstForcedS = at.scriptTimeS;
    }
    this.#last = { input, selection };
    if (!Number.isFinite(at.scriptTimeS)) {
      return;
    }
    const second = Math.floor(at.scriptTimeS);
    if (this.#sampledSecond !== null && second <= this.#sampledSecond) {
      return;
    }
    this.#sampledSecond = second;
    try {
      this.#rows.push(this.#sample(input, selection, own, at));
    } catch (error: unknown) {
      this.#failures += 1;
      this.#firstFailure ??= error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Touchdown, which the spike marks on the first frame whose pose has reached the ground
   * (`touchedDown`): records the script time now and how many of the pass's current forced
   * patches are resident, against how many there are, the end of the forced region's lead. Later
   * calls change nothing.
   */
  markTouchdown(): ContactLead {
    this.#touchdown ??= this.#forcedMark();
    return this.#contactLead();
  }

  /**
   * The contact by the session's rule, which the spike marks on the first frame whose pass's
   * selection holds a contact (`passContact`): recorded as {@link PassSizeCounter.markTouchdown}
   * records touchdown. It fires in the step that first selects forced patches, so that it bounds
   * no lead. Later calls change nothing.
   */
  markContact(): ContactLead {
    this.#sessionContact ??= this.#forcedMark();
    return this.#contactLead();
  }

  /** The counted sizes so far: the peaks, the shares, the rule's figures and the A/B's price. */
  summary(): PassSizeSummary {
    const rows = this.#rows;
    const described = this.#views.length;
    const counts: Record<SizeCountName, CountPeaks> = {
      frustum: countPeaks(rows, "frustum", described),
      frustumNoMargin: countPeaks(rows, "frustumNoMargin", described),
      allRound: countPeaks(rows, "allRound", described),
      frustumDiagnostic: countPeaks(rows, "frustumDiagnostic", described),
      allRoundDiagnostic: countPeaks(rows, "allRoundDiagnostic", described),
    };
    const forcedRow = peakRow(rows, (row) => row.counts.frustum?.forced ?? null);
    const forced: ForcedPeak = {
      patches: forcedRow?.value ?? 0,
      at: forcedRow?.row.at ?? null,
      seen: forcedRow?.row.counts.frustum?.forcedSeen ?? 0,
    };
    const paired = rows.filter((row) => row.counts.frustumDiagnostic !== null);
    const rule = this.#rule(rows, counts.frustum, counts.allRound, forced.patches, "frustum");
    const ruleDiagnostic =
      paired.length === 0
        ? null
        : this.#rule(
            paired,
            counts.frustumDiagnostic,
            counts.allRoundDiagnostic,
            forced.patches,
            "frustumDiagnostic",
          );
    const allRoundPeak = peakRow(rows, (row) => row.counts.allRound?.union ?? null);
    const residentShare = sharePeak(rows, (row) => row.residentShare);
    const frustumPeak = peakRow(rows, (row) => row.counts.frustum?.union ?? null);
    return {
      cache: this.#cache,
      views: this.#views,
      bytesPerSlot: this.#bytesPerSlot,
      slots: rows.findLast((row) => row.slots !== null)?.slots ?? null,
      samples: rows.length,
      failedSamples: this.#failures,
      firstFailure: this.#firstFailure,
      counts,
      forced,
      residentShare,
      sweptShare: sharePeak(rows, (row) => row.sweptShare),
      allRoundEverResident: residentShare.share !== null && residentShare.share >= 1,
      capAtAllRoundPeak: allRoundPeak?.row.cap ?? null,
      deepestOccluderM: deepestOf(paired.map((row) => row.occluderM)),
      boundRatios: [...this.#tables],
      boxHeights: boxHeightPeaks(rows, frustumPeak?.row ?? null),
      countingMs: msSpread(rows.map((row) => row.countingMs)),
      rule,
      ruleDiagnostic,
      occluderPrice: paired.length === 0 ? null : this.#price(paired),
      contact: this.#contactLead(),
      logged: loggedBySegment(rows),
    };
  }

  /** The samples so far in column form. */
  series(): PassSizeSeries {
    const rows = this.#rows;
    const column = <T>(of: (row: SampleRow) => T): T[] => rows.map(of);
    const met = new Set<number>();
    for (const row of rows) {
      for (const level of row.boxHeights.keys()) {
        met.add(level);
      }
    }
    const levels = [...met].toSorted((a, b) => a - b);
    const viewOf = (name: SizeCountName, k: number): Array<number | null> =>
      column((row) => row.counts[name]?.views[k] ?? null);
    const unionOf = (name: SizeCountName): Array<number | null> =>
      column((row) => row.counts[name]?.union ?? null);
    return {
      scriptTimeS: column((row) => row.at.scriptTimeS),
      segment: column((row) => row.at.segment),
      heightAboveGroundM: column((row) => row.at.heightAboveGroundM),
      views: this.#views.map((_, k) => ({
        frustum: viewOf("frustum", k),
        frustumNoMargin: viewOf("frustumNoMargin", k),
        allRound: viewOf("allRound", k),
        frustumDiagnostic: viewOf("frustumDiagnostic", k),
        allRoundDiagnostic: viewOf("allRoundDiagnostic", k),
        selectedSeen: column((row) => row.pass.views[k] ?? null),
        marginRad: column((row) => row.marginsRad[k] ?? null),
      })),
      union: {
        frustum: unionOf("frustum"),
        frustumNoMargin: unionOf("frustumNoMargin"),
        allRound: unionOf("allRound"),
        frustumDiagnostic: unionOf("frustumDiagnostic"),
        allRoundDiagnostic: unionOf("allRoundDiagnostic"),
      },
      forced: column((row) => row.counts.frustum?.forced ?? 0),
      forcedSeen: column((row) => row.counts.frustum?.forcedSeen ?? 0),
      forcedResident: column((row) => row.forcedResident),
      selected: column((row) => row.selected),
      limited: column((row) => row.limited),
      selectedResident: column((row) => row.selectedResident),
      usedSlots: column((row) => row.usedSlots),
      pinnedForced: column((row) => row.pinnedForced),
      pinnedDrawn: column((row) => row.pinnedDrawn),
      residentShare: column((row) => row.residentShare),
      sweptShare: column((row) => row.sweptShare),
      cap: column((row) => row.cap),
      countingMs: column((row) => row.countingMs),
      occluderM: column((row) => row.occluderM),
      boxHeights: levels.map((level) => ({
        level,
        medianM: column((row) => row.boxHeights.get(level)?.medianM ?? null),
        maxM: column((row) => row.boxHeights.get(level)?.maxM ?? null),
      })),
    };
  }

  /**
   * The counter's copy of `source`, made when first met, with its bound ratios where its level
   * table is not the last one met's: a planet object made anew over the same table adds none, so
   * that the tables logged are as many as the tables, not the objects.
   */
  #ownPlanet(source: PlanetGeometry, at: SizeCountContext): PlanetGeometry {
    const known = this.#planets.get(source);
    if (known !== undefined) {
      return known;
    }
    const own: PlanetGeometry = { ...source };
    this.#planets.set(source, own);
    const levels = boundRatiosOf(source);
    if (!sameLevels(this.#tables.at(-1)?.levels ?? [], levels)) {
      this.#tables.push({ version: this.#tables.length, firstSeenS: at.scriptTimeS, levels });
    }
    return own;
  }

  /** Takes §5's running minimum down to `heightM` where it is a finite height. */
  #deepen(heightM: number | null): void {
    if (heightM !== null && Number.isFinite(heightM)) {
      this.#depthM = this.#depthM === null ? heightM : Math.min(this.#depthM, heightM);
    }
  }

  /** One sample: the five counts over `own`, and what is logged beside them. */
  #sample(
    input: SelectionInput,
    selection: Selection,
    own: PlanetGeometry,
    at: SizeCountContext,
  ): SampleRow {
    const indices = this.#indicesOf(input.views);
    const lookup = input.heightRanges;
    const base = unbudgetedInput(input, own);
    const allRoundViews = input.views.map(allRoundView);
    const startMs = this.#now();
    const frustum = selectPatches(base);
    const frustumNoMargin = selectPatches({ ...base, views: input.views.map(marginlessView) });
    const allRound = selectPatches({ ...base, views: allRoundViews });
    this.#deepen(this.#sampledLowestM());
    this.#deepen(lowestResidentM(lookup, [frustum, allRound, selection]));
    const occluderM = this.#depthM;
    const frustumDiagnostic =
      occluderM === null ? null : selectPatches({ ...base, occluderHeightM: occluderM });
    const allRoundDiagnostic =
      occluderM === null
        ? null
        : selectPatches({ ...base, views: allRoundViews, occluderHeightM: occluderM });
    const countingMs = this.#now() - startMs;

    let allRoundUnion = 0;
    let resident = 0;
    let swept = 0;
    for (const [keyString, patch] of allRound.patches) {
      if (patch.forced || patch.views === 0) {
        continue;
      }
      allRoundUnion += 1;
      resident += lookup?.heightRangeM(patch.key) === undefined ? 0 : 1;
      swept += this.#swept.has(keyString) ? 1 : 0;
    }
    let selectedResident = 0;
    let forcedResident = 0;
    for (const patch of selection.patches.values()) {
      const held = lookup?.heightRangeM(patch.key) === undefined ? 0 : 1;
      selectedResident += held;
      forcedResident += patch.forced ? held : 0;
    }
    const cache = lookup instanceof PatchCache ? lookup : null;
    // `pressure` recounts the drawn pins from the entries' flags and reads the rest; it evicts
    // nothing and moves no recency.
    const pressure = cache === null ? null : cache.pressure();
    const slots = cache === null ? null : cache.slotCount;
    return {
      at,
      counts: {
        frustum: tallyOf(frustum, indices),
        frustumNoMargin: tallyOf(frustumNoMargin, indices),
        allRound: tallyOf(allRound, indices),
        frustumDiagnostic: frustumDiagnostic === null ? null : tallyOf(frustumDiagnostic, indices),
        allRoundDiagnostic:
          allRoundDiagnostic === null ? null : tallyOf(allRoundDiagnostic, indices),
      },
      occluderM,
      marginsRad: indices.map((index) => {
        const view = index === null ? undefined : input.views[index];
        return view === undefined ? null : (view.frustumMarginRad ?? 0);
      }),
      pass: tallyOf(selection, indices),
      selected: selection.patches.size,
      limited: selection.limited,
      selectedResident: lookup === undefined ? null : selectedResident,
      forcedResident: lookup === undefined ? null : forcedResident,
      slots,
      usedSlots: cache === null ? null : cache.usedSlots,
      pinnedForced: pressure === null ? null : pressure.forced,
      pinnedDrawn: pressure === null ? null : pressure.drawn,
      residentShare: lookup === undefined ? null : shareOf(resident, allRoundUnion),
      sweptShare: shareOf(swept, allRoundUnion),
      cap: slots === null ? null : shareOf(slots, allRoundUnion),
      countingMs,
      boxHeights: boxHeightsOf(frustum),
    };
  }

  /**
   * Each described view's index among `views`, the input's
   * ({@link PassSizeCounterOptions.viewIndices}), or `null`: an index out of range, not whole, or
   * already taken counts `null`.
   */
  #indicesOf(views: ReadonlyArray<ViewSelectionInput>): Array<number | null> {
    const asked =
      this.#viewIndices === null
        ? this.#views.map((_, k) => k)
        : this.#viewIndices(views).slice(0, this.#views.length);
    const taken = new Set<number>();
    return this.#views.map((_, k) => {
      const index = asked[k] ?? null;
      if (
        index === null ||
        !Number.isInteger(index) ||
        index < 0 ||
        index >= views.length ||
        taken.has(index)
      ) {
        return null;
      }
      taken.add(index);
      return index;
    });
  }

  /**
   * The rule's figures from `frustum`'s and `allRound`'s peaks and the forced peak, the same-sample
   * form over `rows` with `frustumName`'s forced leaves.
   */
  #rule(
    rows: ReadonlyArray<SampleRow>,
    frustum: CountPeaks,
    allRound: CountPeaks,
    forcedPatches: number,
    frustumName: "frustum" | "frustumDiagnostic",
  ): RuleFigures {
    const allRoundName = frustumName === "frustum" ? "allRound" : "allRoundDiagnostic";
    const peaks: StreamingViewPeaks[] = this.#views.map((_, k) => ({
      allRound: allRound.views[k]?.patches ?? 0,
      frustum: frustum.views[k]?.patches ?? 0,
    }));
    let sameSample = 0;
    for (const row of rows) {
      const f = row.counts[frustumName];
      const ar = row.counts[allRoundName];
      if (f === null || ar === null) {
        continue;
      }
      const atSample: StreamingViewPeaks[] = [];
      for (let k = 0; k < this.#views.length; k += 1) {
        const fk = f.views[k] ?? null;
        const ark = ar.views[k] ?? null;
        if (fk !== null && ark !== null) {
          atSample.push({ allRound: ark, frustum: fk });
        }
      }
      sameSample = Math.max(sameSample, sizingRuleSlots(atSample, f.forced));
    }
    const figure = (slots: number): RuleFigure => ({
      slots,
      bytes: slotBudgetBytes(slots, this.#bytesPerSlot),
    });
    return {
      ruled: figure(sizingRuleSlots(peaks, forcedPatches)),
      sameSample: figure(sameSample),
      union: figure(
        sizingRuleSlots(
          [{ allRound: allRound.union.patches, frustum: frustum.union.patches }],
          forcedPatches,
        ),
      ),
      frustum: figure(frustumRuleSlots(peaks, forcedPatches)),
    };
  }

  /** §5's price over `paired`, the samples with both the plain and the diagnostic counts. */
  #price(paired: ReadonlyArray<SampleRow>): OccluderPrice {
    const described = this.#views.length;
    const plain = {
      frustum: countPeaks(paired, "frustum", described),
      allRound: countPeaks(paired, "allRound", described),
    };
    const diagnostic = {
      frustum: countPeaks(paired, "frustumDiagnostic", described),
      allRound: countPeaks(paired, "allRoundDiagnostic", described),
    };
    const forcedPatches = peakRow(paired, (row) => row.counts.frustum?.forced ?? null)?.value ?? 0;
    const ruled = this.#rule(paired, plain.frustum, plain.allRound, forcedPatches, "frustum");
    const primedRuled = this.#rule(
      paired,
      diagnostic.frustum,
      diagnostic.allRound,
      forcedPatches,
      "frustumDiagnostic",
    );
    // The sized all-round peak, the ruled sum's all-round term: every share's whole.
    let sizedAllRoundSlots = 0;
    for (const peak of plain.allRound.views) {
      sizedAllRoundSlots += ALL_ROUND_FACTOR * peak.patches;
    }
    const term = (
      allRound: SizePeak | undefined,
      allRoundPrimed: SizePeak | undefined,
      frustum: SizePeak | undefined,
      frustumPrimed: SizePeak | undefined,
    ): OccluderPriceTerm => {
      const allRoundPatches = (allRound?.patches ?? 0) - (allRoundPrimed?.patches ?? 0);
      const frustumPatches = (frustum?.patches ?? 0) - (frustumPrimed?.patches ?? 0);
      const allRoundSlots = ALL_ROUND_FACTOR * allRoundPatches;
      const frustumSlots = FRUSTUM_FACTOR * frustumPatches;
      return {
        allRoundPatches,
        frustumPatches,
        allRoundSlots,
        frustumSlots,
        allRoundShare: shareOf(allRoundSlots, sizedAllRoundSlots),
        frustumShare: shareOf(frustumSlots, sizedAllRoundSlots),
      };
    };
    const ruledSlots = ruled.ruled.slots - primedRuled.ruled.slots;
    return {
      views: this.#views.map((_, k) =>
        term(
          plain.allRound.views[k],
          diagnostic.allRound.views[k],
          plain.frustum.views[k],
          diagnostic.frustum.views[k],
        ),
      ),
      union: term(
        plain.allRound.union,
        diagnostic.allRound.union,
        plain.frustum.union,
        diagnostic.frustum.union,
      ),
      sizedAllRoundSlots,
      ruledSlots,
      ruledShare: shareOf(ruledSlots, sizedAllRoundSlots),
    };
  }

  /** The script time now and the pass's last selection's forced patches, and those resident. */
  #forcedMark(): ForcedMark {
    let forced = 0;
    let forcedResident = 0;
    const last = this.#last;
    if (last !== null) {
      for (const patch of last.selection.patches.values()) {
        if (patch.forced) {
          forced += 1;
          forcedResident += last.input.heightRanges?.heightRangeM(patch.key) === undefined ? 0 : 1;
        }
      }
    }
    return { atS: this.#context().scriptTimeS, forced, forcedResident };
  }

  #contactLead(): ContactLead {
    const rows = this.#rows;
    const first = rows.findIndex(allResident);
    const firstRow = rows[first];
    // A later sample with no forced patch keeps them: it holds none that is not resident.
    const held =
      firstRow === undefined
        ? null
        : rows.slice(first + 1).every((row) => row.pass.forced === 0 || allResident(row));
    const allResidentS = firstRow?.at.scriptTimeS ?? null;
    const touchdown = this.#touchdown;
    return {
      firstForcedS: this.#firstForcedS,
      allResidentS,
      allResidentHeld: held,
      touchdown,
      leadS: touchdown === null || allResidentS === null ? null : touchdown.atS - allResidentS,
      sessionContact: this.#sessionContact,
    };
  }
}

/**
 * `input` over `planet` with no patch budget and no diagnostic occluder: the pass's own views,
 * contacts, ranges, survey and skirt margin, and anything else it gives selection.
 */
function unbudgetedInput(input: SelectionInput, planet: PlanetGeometry): SelectionInput {
  const { maxPatches, occluderHeightM, ...rest } = input;
  void maxPatches;
  void occluderHeightM;
  return { ...rest, planet };
}

/** `view` at margin 0, with no margin key, and not all round. */
function marginlessView(view: ViewSelectionInput): ViewSelectionInput {
  const { frustumMarginRad, allRound, ...rest } = view;
  void frustumMarginRad;
  void allRound;
  return rest;
}

/** `view` all round: its horizon alone culls (its margin, then, is ignored). */
function allRoundView(view: ViewSelectionInput): ViewSelectionInput {
  return { ...view, allRound: true };
}

/** `part` ÷ `whole`, or `null` where the whole is 0. */
function shareOf(part: number, whole: number): number | null {
  return whole > 0 ? part / whole : null;
}

/** Whether two tables' bound ratios are the same, level for level. */
function sameLevels(a: ReadonlyArray<LevelBoundRatio>, b: ReadonlyArray<LevelBoundRatio>): boolean {
  return (
    a.length === b.length &&
    a.every((level, n) => {
      const other = b[n];
      return (
        other !== undefined &&
        level.boundM === other.boundM &&
        level.chargedM === other.chargedM &&
        level.edgeM === other.edgeM
      );
    })
  );
}

/** k_n for each level of `planet` from 0 to its finest. */
function boundRatiosOf(planet: PlanetGeometry): LevelBoundRatio[] {
  const radiusM = planet.figure.equatorialRadiusM;
  return Array.from({ length: planet.finestLevel + 1 }, (_, level) => {
    const boundM = levelBoundM(planet, level);
    const chargedM = selectionErrorM(planet, level);
    const edgeM = PATCH_QUADS * vertexSpacing(radiusM, level).meanM;
    return {
      level,
      boundM,
      chargedM,
      edgeM,
      ratio: (boundM * REFERENCE_PX_PER_RAD) / edgeM,
      chargedRatio: (chargedM * REFERENCE_PX_PER_RAD) / edgeM,
    };
  });
}

/**
 * `selection`'s tally: per described view, the leaves with its input view's bit, `indices` giving
 * each one's index among the input's views, or `null` where the input held none of it.
 */
function tallyOf(selection: Selection, indices: ReadonlyArray<number | null>): CountTally {
  const seen: Array<number | null> = indices.map((index) => (index === null ? null : 0));
  let union = 0;
  let forced = 0;
  let forcedSeen = 0;
  for (const patch of selection.patches.values()) {
    if (patch.forced) {
      forced += 1;
      forcedSeen += patch.seen ? 1 : 0;
      continue;
    }
    if (patch.views === 0) {
      continue;
    }
    union += 1;
    for (const [k, index] of indices.entries()) {
      const count = seen[k];
      if (
        index !== null &&
        count !== undefined &&
        count !== null &&
        (patch.views & (1 << index)) !== 0
      ) {
        seen[k] = count + 1;
      }
    }
  }
  return {
    views: seen,
    union,
    forced,
    forcedSeen,
  };
}

/** Whether `row`'s pass selected forced patches, every one of them resident. */
function allResident(row: SampleRow): boolean {
  return row.pass.forced > 0 && row.forcedResident === row.pass.forced;
}

/** The lower of `lowestM` and `range`'s low end, metres; `lowestM` where there is no range. */
function lowerOf(
  lowestM: number | null,
  range: readonly [number, number] | undefined,
): number | null {
  return range === undefined || (lowestM !== null && range[0] >= lowestM) ? lowestM : range[0];
}

/**
 * The lowest baked height of the resident bakes over `selections`' leaves, metres, or `null` with
 * none: over a `PatchCache`, every resident patch at a leaf or above it, each visited once
 * (`PatchCache.residentOver`); over other ranges, the leaves' own.
 */
function lowestResidentM(
  lookup: HeightRangeLookup | undefined,
  selections: ReadonlyArray<Selection>,
): number | null {
  if (lookup === undefined) {
    return null;
  }
  let lowest: number | null = null;
  if (!(lookup instanceof PatchCache)) {
    for (const selection of selections) {
      for (const patch of selection.patches.values()) {
        lowest = lowerOf(lowest, lookup.heightRangeM(patch.key));
      }
    }
    return lowest;
  }
  const visited = new Set<CachedPatch>();
  for (const selection of selections) {
    for (const { key } of selection.patches.values()) {
      for (let level = key.level; level >= 0; level -= 1) {
        const resident = lookup.residentOver(key, level);
        if (resident === undefined) {
          continue;
        }
        // Met over an earlier leaf, whose walk went on to the root: its ancestors are visited.
        if (visited.has(resident)) {
          break;
        }
        visited.add(resident);
        lowest = lowerOf(lowest, resident.heightRangeM);
      }
    }
  }
  return lowest;
}

/** The largest of `values`, `null`s passed over; `null` where none is a number. */
function maxOf(values: ReadonlyArray<number | null>): number | null {
  let max: number | null = null;
  for (const value of values) {
    if (value !== null && (max === null || value > max)) {
      max = value;
    }
  }
  return max;
}

/** `rows`' pass's own counts by segment, in the order the rows met the segments. */
function loggedBySegment(rows: ReadonlyArray<SampleRow>): LoggedSegment[] {
  const bySegment = new Map<string | null, SampleRow[]>();
  for (const row of rows) {
    const known = bySegment.get(row.at.segment);
    if (known === undefined) {
      bySegment.set(row.at.segment, [row]);
    } else {
      known.push(row);
    }
  }
  return [...bySegment].map(([segment, of]) => ({
    segment,
    samples: of.length,
    selectedMax: maxOf(of.map((row) => row.selected)) ?? 0,
    limitedSamples: of.filter((row) => row.limited).length,
    selectedResidentMax: maxOf(of.map((row) => row.selectedResident)),
    usedSlotsMax: maxOf(of.map((row) => row.usedSlots)),
    pinnedForcedMax: maxOf(of.map((row) => row.pinnedForced)),
    pinnedDrawnMax: maxOf(of.map((row) => row.pinnedDrawn)),
  }));
}

/** The seen leaves' box heights by level: twice the box's half-extent along its centre's normal. */
function boxHeightsOf(selection: Selection): Map<number, BoxHeightSample> {
  const byLevel = new Map<number, number[]>();
  for (const patch of selection.patches.values()) {
    if (!patch.seen) {
      continue;
    }
    // The box's first axis is its centre's spheroid normal (`bounds.ts`' packed layout).
    const heightM = 2 * patch.bounds.box.halfExtentsM[0];
    const heights = byLevel.get(patch.key.level);
    if (heights === undefined) {
      byLevel.set(patch.key.level, [heightM]);
    } else {
      heights.push(heightM);
    }
  }
  const out = new Map<number, BoxHeightSample>();
  for (const [level, heights] of byLevel) {
    const sorted = heights.toSorted((a, b) => a - b);
    // The median by nearest rank, the spike's convention (`msSpread`'s).
    const medianM = sorted[Math.max(0, Math.ceil(sorted.length / 2) - 1)] ?? 0;
    out.set(level, { medianM, maxM: sorted.at(-1) ?? 0 });
  }
  return out;
}

/** The least of `heightsM`, metres, its `null`s passed over; `null` where none is a number. */
function deepestOf(heightsM: ReadonlyArray<number | null>): number | null {
  let deepest: number | null = null;
  for (const heightM of heightsM) {
    if (heightM !== null && (deepest === null || heightM < deepest)) {
      deepest = heightM;
    }
  }
  return deepest;
}

/** The row with the largest `value`, the first if tied, and the value; `null` where none has one. */
function peakRow(
  rows: ReadonlyArray<SampleRow>,
  value: (row: SampleRow) => number | null,
): { readonly row: SampleRow; readonly value: number } | null {
  let best: { readonly row: SampleRow; readonly value: number } | null = null;
  for (const row of rows) {
    const v = value(row);
    if (v !== null && (best === null || v > best.value)) {
      best = { row, value: v };
    }
  }
  return best;
}

/** {@link peakRow} as a {@link SizePeak}. */
function sizePeak(
  rows: ReadonlyArray<SampleRow>,
  value: (row: SampleRow) => number | null,
): SizePeak {
  const peak = peakRow(rows, value);
  return { patches: peak?.value ?? 0, at: peak?.row.at ?? null };
}

/** {@link peakRow} as a {@link SharePeak}. */
function sharePeak(
  rows: ReadonlyArray<SampleRow>,
  value: (row: SampleRow) => number | null,
): SharePeak {
  const peak = peakRow(rows, value);
  return { share: peak?.value ?? null, at: peak?.row.at ?? null };
}

/** Count `name`'s peaks over `rows`, for `described` views. */
function countPeaks(
  rows: ReadonlyArray<SampleRow>,
  name: SizeCountName,
  described: number,
): CountPeaks {
  return {
    views: Array.from({ length: described }, (_, k) =>
      sizePeak(rows, (row) => row.counts[name]?.views[k] ?? null),
    ),
    union: sizePeak(rows, (row) => row.counts[name]?.union ?? null),
  };
}

/** Count 1's box heights by level: the run's largest, and the median at `frustumPeak`. */
function boxHeightPeaks(
  rows: ReadonlyArray<SampleRow>,
  frustumPeak: SampleRow | null,
): LevelBoxHeights[] {
  const maxByLevel = new Map<number, number>();
  for (const row of rows) {
    for (const [level, sample] of row.boxHeights) {
      maxByLevel.set(level, Math.max(maxByLevel.get(level) ?? 0, sample.maxM));
    }
  }
  return [...maxByLevel]
    .toSorted(([a], [b]) => a - b)
    .map(([level, maxM]) => ({
      level,
      maxM,
      medianAtFrustumPeakM: frustumPeak?.boxHeights.get(level)?.medianM ?? null,
    }));
}
