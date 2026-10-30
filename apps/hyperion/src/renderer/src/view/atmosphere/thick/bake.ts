/**
 * The thick-atmosphere bake (plan R08, R08.T14.b; Design notes 5, 9 and 17): the view-dependent
 * multiple-scattering source table J_ms(h, μ₀, μ_v, m), m = 0, 1 and 2, of one latitude band of a
 * world, solved by discrete ordinates (`discreteOrdinates.ts`) in `f64`, spectrally in the 15 bake
 * bins and stored in the render channels, in the optics worker.
 *
 * @remarks
 * **The column** is the drawn medium by its sources (`spectralColumn.ts`), its optics taken in each
 * bin. The solve's {@link THICK_BAKE_GRID} layers lie in the gravity-scaled height h\*, evenly in
 * ξ(h) = ½(1 − τ(h) ÷ τ(0)) + ½ h ÷ top, τ(h) the vertical extinction above h in the middle bin
 * (570 nm) without the gases' curves: half the layers by optical depth, so that a cloud takes
 * layers of τ about 1, and half by height, so that the thin air above keeps its own. Each layer's
 * coefficients are its terms' exact integrals ∫ρ dh over it (`medium.ts`'s `columnLengthM`),
 * mixed by their scattering.
 *
 * **The band** (Design note 17, `tables.ts`'s `tablesPlanOf` rule, mirrored in
 * {@link thickBakeBands}): one band is the reference medium on R_ref; with several, band b is the
 * reference medium at s_b on a sphere of s_b √(MN) at its latitude, every coefficient and every
 * absorber's amount divided by s_b, so that its vertical optical depth is the reference's over s_b.
 *
 * **The beam** is pseudo-spherical (`sphericalBeam.ts`, Dahlback and Stamnes 1991) on the band's
 * sphere, at each of {@link THICK_BAKE_GRID}'s μ₀: a ray with μ₀ > 0 climbs from its point and
 * never meets the ground, so the beam is defined at every height. Below 0, a sun below the point's
 * horizontal (R08.T14.d.2), the beam crosses the column under each lit interface twice, through its
 * tangent point, and is 0 in the planet's shadow, where the solve's diffuse field from the lit air
 * above remains, and a column whose top lies in the shadow (μ₀ below −√(1 − (R ÷ (R + h_top))²))
 * holds no light at all. {@link THICK_BAKE_GRID} holds 14 rows there, to the cell edge −7/32, below
 * which the kernels read the analytic term (`read.ts`). Past about 90° the pseudo-spherical solve
 * is not demonstrated (libRadtran's guide, §2.2.1; zenith-sky models agree within 6.6% only up to
 * 94°, F. Hendrick et al., ACP 6 (2006) 93), and R. P. Loughman et al., JGR 109 (2004) D06303,
 * find plane-parallel diffuse fields up to 8% high at 60 km tangent height in Earth's limb, which
 * Design note 9 takes as its warning for Titan.
 *
 * **Delta-M** (W. J. Wiscombe, J. Atmos. Sci. 34 (1977) 1408–1422) on each layer's mixed moments,
 * 16 streams in all ({@link THICK_BAKE_STREAMS}, `aerosol.ts`'s `deltaMTruncation(…, 8)`): f = χ₁₆,
 * held at least the layer's forward delta f_c (R08.T5.c's `forwardPeak`, the direct light the
 * kernels' medium already leaves out; decision-r08-aerosol-labels) and at least 0; τ′ = (1 − ωf)τ,
 * ω′ = (1 − f)ω ÷ (1 − ωf) and χ′_l = (χ_l − f) ÷ (1 − f), l < 16.
 *
 * **What is stored, and Nakajima and Tanaka's corrections** (T. Nakajima and M. Tanaka, JQSRT 40
 * (1988) 51–69, cited for TMS and IMS, not re-read). The kernels march with the full extinction less
 * the forward delta, σ_k = σ_ext − f_c σ_s, add the exact single scattering of their own beam T_k,
 * σ_s(p − f_c δ)(Θ₀) T_k, and read this table, times their own σ_s,k = σ_s (1 − f_c), in place of
 * σ_s Ψ_ms p_u. So the bake forms the emission of orders two and up in the kernels' own frame,
 * σ_s ∫ (p − f_c δ) I_d dΩ′, with I_d the diffuse field, here delta-M's I′_d, and the truncated peak
 * p − (1 − f)p′ (of weight f) taken as narrow beside I′_d, and the table holds it per unit of σ_s,k
 * ("What the table holds is J", below):
 *
 * E = σ_s [(f − f_c) I′_d(Ω) + (1 − f) ∫ p′ I′_d dΩ′ + (1 − f) p′(Θ₀) (T′ − T_k)]
 *
 * - TMS: the kernels' exact single scattering stands in place of Nakajima and Tanaka's
 *   truncated-single-scattering correction, so the table holds no single scattering at all. TMS
 *   scatters the delta-M beam T′ with the exact phase function; the kernels scatter their own T_k,
 *   and the table's last term below scatters the rest, T′ − T_k, by the smooth part.
 * - The first term is the truncated lobe's re-emission of the diffuse field into its own direction,
 *   which delta-M carries by its scaled extinction and the kernels' unscaled extinction would
 *   otherwise lose.
 * - The last is the light delta-M counts as direct beam, T′ − T_k ≥ 0 (the aureole, scattered
 *   inside the lobe), scattered once more by the smooth part.
 * - IMS is left out: the aureole scattered again inside the lobe, σ_s (f − f_c)(T′ − T_k) toward
 *   the sun's own direction, stays within the lobe of the sun, finer than m ≤ 2 and 16 view cosines
 *   can hold. Its share of each point's multiply scattered light, (f − f_c)(T′ − T_k) ÷
 *   [(1 − f_c)(4π Ī′_d + T′ − T_k)], is recorded ({@link MsSourceSlot.doubleForwardShare}).
 *
 * E is per unit of true length at h\* (the reference medium's σ_s there, which a band's frame takes
 * as σ_s ÷ s_b per unit of h\*), per steradian, per unit of the star's irradiance on a plane normal
 * to its beam at the top of the atmosphere: m⁻¹ sr⁻¹. Its azimuth is E = Σ_m Eᵐ cos mΔφ,
 * m = 0 … {@link THICK_BAKE_MODES} − 1, Δφ the azimuth of the light's direction of travel less the
 * beam's: for an observer, the azimuth they look toward less the sun's (`discreteOrdinates.ts`'s
 * convention).
 *
 * **What the table holds is J** (R08.T14.d.1): in each bin, E over the bin's own kernel scattering
 * at the altitude, J_b = E_b ÷ (σ_s,b (1 − f_c,b)), sr⁻¹ per unit of the kernels' scattering and of
 * the star's irradiance, then converted on storage (`binConversion.ts`), so that the kernels
 * multiply it by their own σ_s,k and the star's colour. A channel's J is thus the bins' J weighted
 * by the channel, and its σ_s the kernels' own at the channel's wavelength, as their single
 * scattering, Hillaire's Ψ_ms, the polarisation correction and the references take it (the gases
 * alone are weighted by channel, through their curves of growth, with the same weights). Converting
 * E and dividing by the channel's σ_s,k instead takes σ_s's own spectral weighting into the channel:
 * on a Rayleigh column under the reference Sun's bake spectrum, Σ_b W_cb λ_b⁻⁴ ÷ C_c is 0.917, 0.979
 * and 0.973 of λ_c⁻⁴ at the channels' 433.6, 544 and 645 nm (blue, green, red), which left a
 * Saturn-class sky's blue 7–10% dim against its reference (the research advice after R08.T14.f).
 * The weighted J is J at the channel's wavelength only as far as J is flat across the channel's
 * weights, whose first moments lie at 639, 544 and 440 nm, the red channel's negative from about
 * 440 to 550 nm: where J follows part of τ's λ⁻⁴ (a thin column, or a low sun's reddened field), a
 * channel reads J off its wavelength (`saturn-oblate`'s s0 blue 0.97 of its reference; Earth's red
 * at μ₀ 1/64 some 0.74 of a solve at 645 nm, plan R08's record of T14.d.1).
 *
 * **Spectral** (Design note 5): each absorber in each bin is an exponential sum fitted to its bin
 * curve (`expSum.ts`), each layer reading k at its own Curtis–Godson pressure; several absorbers in
 * a bin overlap at random (the product of their sums, Lacis and Oinas 1991), one solve a product
 * term, the weakest taken as one term each past {@link THICK_BAKE_MAX_TERMS_PER_BIN}.
 *
 * **The ground** is Lambertian in each solve, its albedo in a bin linear in wavelength between the
 * render channels' wavelengths and held at the nearer channel outside them.
 *
 * **Thermal emission** (plan R08, R08.T18.b): a column with a temperature (`SpectralColumn`'s) is
 * solved for its thermal field beside its beams, each bin's Planck radiance B_λ(λ_b, T(h\*)) at the
 * layers' interfaces, linear in τ through each, and the ground's at the column's bottom emitted at
 * 1 − its albedo (`discreteOrdinates.ts`' `ThermalEmission`). The field's source of every order is
 * formed as a beam's diffuse field's is, E_th = σ_s [(f − f_c) I′ + (1 − f) ∫ p′ I′ dΩ′] at mode 0,
 * with no beam's terms, its gases' terms weighted by the first light's (lean, 2026-10-30: the
 * Planck spectrum across a bin weights a gas's curve of growth otherwise than a star's), and held
 * per unit of the kernels' scattering in each bin, J_th = E_th ÷ (σ_s (1 − f_c)), converted to the
 * channels' luminance by V_cb (`thermal.ts`' `EMISSION_CHANNEL_WEIGHTS`), cd m⁻² per unit of σ_s,k
 * ({@link MsSourceTable.thermal}). The kernels add the emission (1 − ω) B itself, step by step
 * (`thermalSource.ts`).
 */

import type { Rgb } from "../../photometry/toneCurve";
import { type AbsorberBinCurves, absorberCurvesBySun, type AbsorberSpec } from "../absorbers";
import { gaussLegendre } from "../irradianceQuadrature";
import {
  ABSORBER_PRESSURE_REFERENCE_PA,
  BAKE_BIN_COUNT,
  BAKE_WAVELENGTHS_NM,
  columnLengthM,
  densityAt,
  type TabulatedDensity,
} from "../medium";
import { bandLatitudeRad, type BodyGravity, principalRadiiM, type SpheroidFigure } from "../oblate";
import type { BakeSpectrum } from "../spectralColour";
import { type BinConversion, binConversion } from "./binConversion";
import {
  DISCRETE_ORDINATE_STREAMS,
  type DiscreteOrdinatesSolution,
  doubleGaussQuadrature,
  type GroundCosines,
  groundCosinesOf,
  type GroundReflectance,
  normalizedAssociatedLegendre,
  type OrdinateLayer,
  solveDiscreteOrdinates,
} from "./discreteOrdinates";
import {
  EXP_SUM_OPTIONS,
  expSumCoefficientsAt,
  expSumRowsAt,
  type ExpSumFit,
  type ExpSumOptions,
  fitExpSum,
} from "./expSum";
import { type SphericalBeam, sphericalBeamPath } from "./sphericalBeam";
import { EMISSION_CHANNEL_WEIGHTS, planckRadiance } from "../thermal";
import {
  mediumTemperatureK,
  THERMAL_TRAVEL_COSINES,
  type ThermalScattering,
} from "../thermalSource";
import {
  channelRuleAt,
  forwardDeltaShare,
  phaseMoments,
  type SpectralColumn,
  spectralOptics,
  type SpectralPhase,
} from "./spectralColumn";
import { log } from "../../../lib/pinnedMath";

/**
 * The bake's ceiling, s a world: 5 s on the UHD 620's host, timed by the owner (R08.T14.d); a
 * development machine records its own time beside it and fails only if over it (plan R08, Design
 * note 9; Risks, "CPU budgets on the desktop", decided 2026-09-30). The band count multiplies the
 * bake against it.
 */
export const BAKE_CEILING_S = 5;

/**
 * The bake's streams, NSTR, 16 in all, 8 a hemisphere (`discreteOrdinates.ts`'s
 * `DISCRETE_ORDINATE_STREAMS`; decision-r08-t14a-scope ruling 3): its delta-M truncates at
 * f = χ₁₆, `aerosol.ts`'s `deltaMTruncation(table, channel, 8)`.
 */
export const THICK_BAKE_STREAMS = DISCRETE_ORDINATE_STREAMS;

/**
 * The azimuthal modes the table holds: m = 0, 1 and 2 (Design note 9; R08.T14.d's precondition 1,
 * the m = 2 that a forward-peaked aerosol's second-order source carries; no m = 3, main 2026-10-29).
 */
export const THICK_BAKE_MODES = 3;

/**
 * The most solves one bin may take, the product of its absorbers' terms: past it the weakest
 * absorber (by its vertical bin optical depth) is taken as one term, then the next.
 */
export const THICK_BAKE_MAX_TERMS_PER_BIN = 64;

/** The bin the layers are placed by: its middle, 570 nm. */
export const THICK_BAKE_REFERENCE_BIN = 7;

/** A bake's grids. */
export interface ThickBakeGrid {
  /** L, the solve's layers. */
  readonly layers: number;
  /** The table holds every `altitudeStride`-th interface from the ground: L ÷ stride + 1 altitudes. */
  readonly altitudeStride: number;
  /** μ₀, ascending in [−1, 1], none 0: below 0, a sun below the point's horizontal. */
  readonly sunCosines: Float64Array;
  /** The view cosines of one hemisphere, ascending in (0, 1]: each is stored upward and downward. */
  readonly upwardCosines: Float64Array;
}

/**
 * Design note 9's grid: 64 layers, 33 altitudes (every second interface from the ground), 46 μ₀
 * and 16 μ_v, each at the centre of its cell: μ_v = ±(v + ½) ÷ 8, and μ₀ = −(13.5 − s) ÷ 64 for
 * s = 0 … 13, 14 rows of 1/64 below the horizontal to the cell edge −7/32 (R08.T14.d.2), then
 * (s − 13.5) ÷ 32 for s = 14 … 45, the 32 daylit rows of R08.T14.b. `read.ts` reads the table
 * linearly between the cells' centres above μ₀ = 1/64 and J⁰ geometrically below it, and the
 * analytic term below the rows (its remarks). The deck's upper medium takes all 46, its own tables
 * and the table below it the daylit rows alone (`deck.ts`, R08.T15.a.2).
 */
export const THICK_BAKE_GRID: ThickBakeGrid = {
  layers: 64,
  altitudeStride: 2,
  sunCosines: Float64Array.from({ length: 46 }, (_, s) =>
    s < 14 ? -(13.5 - s) / 64 : (s - 13.5) / 32,
  ),
  upwardCosines: Float64Array.from({ length: 8 }, (_, v) => (v + 0.5) / 8),
};

/**
 * {@link THICK_BAKE_GRID} with its 32 rows above the horizontal alone, μ₀ = (s + ½) ÷ 32: the rows
 * the correction's daylit tests and a deck's own tables take (`deck.ts`, R08.T15.a.2).
 */
export const THICK_BAKE_DAY_GRID: ThickBakeGrid = {
  ...THICK_BAKE_GRID,
  sunCosines: THICK_BAKE_GRID.sunCosines.filter((mu) => mu > 0),
};

// ---------------------------------------------------------------------------------------------
// The body and its bands.

/** The body a bake is for: `assemble.ts`'s `AssembledBody` and R_ref (`DrawnMedium.bottomRadiusM`). */
export interface ThickBakeBody {
  readonly figure: SpheroidFigure;
  readonly gravity: BodyGravity;
  /** R_ref, m: the tables' ground, (2a + c) ÷ 3. */
  readonly bottomRadiusM: number;
}

/** One latitude band of a bake (`tables.ts`'s `TablesBand`, the same rule). */
export interface ThickBakeBand {
  /** s_b. */
  readonly gravityRatio: number;
  /** The geodetic latitude whose s is s_b, rad. */
  readonly latitudeRad: number;
  /** The band's ground sphere, m: s_b √(MN) at its latitude, or R_ref for a body of one band. */
  readonly bottomRadiusM: number;
  /** The factor every coefficient and absorber amount takes in the band's frame: 1 ÷ s_b. */
  readonly coefficientFactor: number;
}

/**
 * A body's latitude bands, one a band of `gravity.slicing.bandGravityRatio`, by `tables.ts`'s
 * `tablesPlanOf` rule (held equal to it by `bake.test.ts`, since the worker cannot import the
 * tables): one band at R_ref, several each at s_b √(MN) of its latitude.
 */
export function thickBakeBands(body: ThickBakeBody): ReadonlyArray<ThickBakeBand> {
  const ratios = body.gravity.slicing.bandGravityRatio;
  const oneBand = ratios.length === 1;
  return Array.from(ratios, (s): ThickBakeBand => {
    const latitudeRad = bandLatitudeRad(body.gravity, s);
    if (oneBand) {
      return {
        gravityRatio: s,
        latitudeRad,
        bottomRadiusM: body.bottomRadiusM,
        coefficientFactor: 1 / s,
      };
    }
    const { meridionalM, primeVerticalM } = principalRadiiM(body.figure, latitudeRad);
    return {
      gravityRatio: s,
      latitudeRad,
      bottomRadiusM: s * Math.sqrt(meridionalM * primeVerticalM),
      coefficientFactor: 1 / s,
    };
  });
}

// ---------------------------------------------------------------------------------------------
// The table.

/** One bin's record in a light's bake. */
export interface MsBinRecord {
  /** The solves the bin took: the product of its absorbers' terms, 1 with none. */
  readonly terms: number;
  /** The largest |ΔT| of its absorbers' exponential sums over their curves' nodes and rows; 0 with none. */
  readonly largestFitError: number;
  /**
   * The same over the rows the bulk of each absorber reads: those about the Curtis–Godson pressures
   * of its fullest layers that together hold {@link READ_COLUMN_SHARE} of its column.
   */
  readonly readFitError: number;
  /** The pressure rows of its absorbers' sums whose k cross (`expSum.ts`'s `crossedRows`). */
  readonly crossedRows: number;
}

/** One light slot's table. */
export interface MsSourceSlot {
  /**
   * J per channel, sr⁻¹ per unit of the kernels' scattering σ_s,k and of the star's irradiance
   * normal to its beam at the top (the module's remarks, "What the table holds is J"),
   * A × S × V × M × 3 row-major, entry `(((a S + s) V + v) M + m) 3 + c`: a the altitude, s the
   * sun's cosine, v the travel cosine and m the mode of {@link MsSourceTable}, c red, green, blue.
   */
  readonly values: Float32Array;
  /** Each bin's solves and fit. */
  readonly bins: ReadonlyArray<MsBinRecord>;
  /** The solves in all. */
  readonly solves: number;
  /**
   * The largest share of a point's multiply scattered light that IMS would carry and the table
   * leaves out (the module's remarks), over every bin, altitude and μ₀ whose light is at least
   * 10⁻³ of its bin's brightest.
   */
  readonly doubleForwardShare: number;
  /** The layers whose slant factor took `sphericalBeam.ts`'s floor, over every solve and beam. */
  readonly flooredSlantFactors: number;
  /**
   * The largest beam the floor withheld at an interface (`sphericalBeam.ts`'s `largestDeficit`),
   * of the incident 1 normal to it, over every solve's delta-M beams.
   */
  readonly largestBeamDeficit: number;
}

/**
 * J_ms(h, μ₀, μ_v, m) of one latitude band (Provides' `MsSourceTable`), sr⁻¹ per unit of the
 * kernels' scattering σ_s,k and of the star's irradiance, which the kernels read times their own
 * σ_s,k in place of σ_s·Ψ_ms·p_u (the module's remarks), one table a light slot.
 *
 * @remarks
 * `f64` in the solve, stored as `Float32Array`; packing for the GPU is R08.T14.c's and T14.f's
 * (`read.ts`). A band of the default grid is 33 × 46 × 16 × 3 × 3 values a slot, 874,368 B as
 * `f32`, and packs into 388,608 B of texels and a 256 B info row (Design note 9's 0.26 MB was 32
 * altitudes and 32 μ₀, two modes as `rgba16float`; R08.T14.d.2 added the 14 rows below the
 * horizontal, and `read.ts` holds the third mode in the texel's spare word). Each light slot holds
 * its own table, since μ₀ and the curves of growth are a sun's.
 */
export interface MsSourceTable {
  readonly band: ThickBakeBand;
  /** The table's altitudes, h\* above the ground, m, ascending: interfaces of the solve's layers. */
  readonly altitudesM: Float64Array;
  /** μ₀, ascending: the grid's, 14 below the horizontal and 32 above on {@link THICK_BAKE_GRID}. */
  readonly sunCosines: Float64Array;
  /**
   * The cosines of the light's direction of travel, positive upward, ascending: the downward
   * half, then the upward. An observer looking at elevation e reads −sin e.
   */
  readonly travelCosines: Float64Array;
  /** M, the modes, m = 0 … M − 1: {@link THICK_BAKE_MODES}. */
  readonly modes: number;
  /** The band's vertical extinction optical depth in each bin, without the gases' curves. */
  readonly verticalOpticalDepth: Float64Array;
  /** One a light of the bake, in its order. */
  readonly slots: ReadonlyArray<MsSourceSlot>;
  /**
   * The thermal field's source J_th of a column with a temperature (the module's remarks,
   * "Thermal emission"), at the table's altitudes and travel cosines; absent for one without.
   */
  readonly thermal?: ThermalScattering;
}

// ---------------------------------------------------------------------------------------------
// The preparation: everything that does not depend on the band.

/** A light of a bake: its spectrum and its absorbers' bin curves. */
export interface ThickBakeLight {
  readonly spectrum: BakeSpectrum;
  /**
   * One per absorber term of the column, in the column's order, its kind's: the term's curves in
   * each bin under this light (`absorbers.ts`'s `absorberBinCurves` or `absorberCurves`).
   */
  readonly absorbers: ReadonlyArray<AbsorberBinCurves>;
}

/**
 * A bake's lights from its column, its body's absorber specs and its lights' spectra: each of the
 * column's absorber terms takes the spec of its name, and each light the bin curves of those specs
 * under its spectrum (`absorbers.ts`'s `absorberCurvesBySun`, the lines' exponentials shared by
 * the lights), as the optics worker forms them for a `thick-bake` or `polarisation-bake`.
 *
 * @param spectra - The lights' spectra, one a light, in the lights' order.
 * @throws RangeError for a column absorber term whose name no spec has, or as the curves refuse.
 */
export function thickBakeLights(
  column: SpectralColumn,
  absorbers: ReadonlyArray<AbsorberSpec>,
  spectra: ReadonlyArray<BakeSpectrum>,
): ReadonlyArray<ThickBakeLight> {
  const specs = column.terms.flatMap((term) => {
    if (term.source.kind !== "absorber") {
      return [];
    }
    const spec = absorbers.find((each) => each.name === term.name);
    if (spec === undefined) {
      throw new RangeError(`the body has no absorber ${term.name} for the bake's column`);
    }
    return [spec];
  });
  const curves = specs.map((spec) => absorberCurvesBySun(spec, spectra));
  return spectra.map((spectrum, j): ThickBakeLight => ({
    spectrum,
    absorbers: curves.map((bySun) => {
      const bins = bySun[j]?.bins;
      if (bins === undefined) {
        throw new RangeError(`an absorber has no curves for light ${j}`);
      }
      return bins;
    }),
  }));
}

/** A term's optics in one bin, at relative density 1. */
interface BinTermOptics {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
  /** χ₀ … χ₁₆. */
  readonly moments: Float64Array;
  readonly forwardDelta: number;
}

/** The medium's scattering at one table altitude in one bin, band-independent. */
interface PointOptics {
  /** σ_s at the altitude, m⁻¹ of true length. */
  readonly scatteringPerM: number;
  /** f − f_c. */
  readonly lobe: number;
  /** 1 − f. */
  readonly kept: number;
  /** f_c. */
  readonly forwardDelta: number;
  /** χ′_l, l < 16, χ′₀ = 1. */
  readonly moments: Float64Array;
}

/** One solve of a bin: its weight and each layer's gas absorption in the reference frame. */
export interface BinTerm {
  readonly weight: number;
  /** Per layer from the top, the gases' vertical optical depth before a band's factor. */
  readonly absorptionDepth: Float64Array;
}

/**
 * A light's bin: the solves its gases take, one a term of their exponential sums' random overlap,
 * and its fits' errors.
 */
export interface BakeLightBin {
  readonly terms: ReadonlyArray<BinTerm>;
  readonly largestFitError: number;
  readonly readFitError: number;
  readonly crossedRows: number;
}

/** The share of an absorber's column whose layers' rows count as read: its bulk, 99%. */
export const READ_COLUMN_SHARE = 0.99;

/** A fit's largest error over the rows the bulk of its absorber reads ({@link MsBinRecord.readFitError}). */
function readError(fit: ExpSumFit, absorber: LaidAbsorber): number {
  const total = absorber.amounts.reduce((sum, amount) => sum + amount, 0);
  const fullest = Array.from(absorber.amounts.keys()).toSorted(
    (p, q) => (absorber.amounts[q] ?? 0) - (absorber.amounts[p] ?? 0),
  );
  let held = 0;
  let worst = 0;
  for (const p of fullest) {
    if (!(held < READ_COLUMN_SHARE * total)) {
      break;
    }
    held += absorber.amounts[p] ?? 0;
    const { row, next } = expSumRowsAt(fit, absorber.pressuresPa?.[p]);
    worst = Math.max(worst, fit.rowErrors[row] ?? Number.NaN, fit.rowErrors[next] ?? Number.NaN);
  }
  return worst;
}

interface PreparedLight {
  readonly conversion: BinConversion;
  readonly bins: ReadonlyArray<BakeLightBin>;
}

/** Everything of a bake that does not depend on the band (the module's remarks). */
export interface PreparedBake {
  readonly column: SpectralColumn;
  readonly grid: ThickBakeGrid;
  /** The interfaces' h\*, m, from the top down: L + 1. */
  readonly interfaceHeightsM: Float64Array;
  /** The table's altitudes, ascending. */
  readonly altitudesM: Float64Array;
  /** Per bin, per term. */
  readonly binOptics: ReadonlyArray<ReadonlyArray<BinTermOptics>>;
  /**
   * Per bin, per term, its whole phase function at the bin's wavelength (`spectralColumn.ts`'s
   * `SpectralPhase`), of which `binOptics` holds the first moments: what the deck's exact single
   * scattering reads (R08.T15.a.2, `deck.ts`).
   */
  readonly binPhases: ReadonlyArray<ReadonlyArray<SpectralPhase>>;
  /** Per bin, the ground's albedo. */
  readonly groundAlbedo: Float64Array;
  /** Per term, per layer from the top, ∫ρ dh over it, m. */
  readonly layerColumnsM: ReadonlyArray<Float64Array>;
  /** Per bin, per table altitude. */
  readonly pointOptics: ReadonlyArray<ReadonlyArray<PointOptics>>;
  readonly lights: ReadonlyArray<PreparedLight>;
  /**
   * Per bin, B_λ(λ_b, T) at each interface from the top down, W m⁻² sr⁻¹ nm⁻¹, the last the
   * ground's (R08.T18.b); absent for a column without a temperature.
   */
  readonly interfacePlanck?: ReadonlyArray<Float64Array>;
}

/**
 * The ground's albedo at a wavelength from its channels' (the module's remarks): the ground rule
 * that a channel source's optics take too (`spectralColumn.ts`'s `channelRuleAt`), linear in
 * wavelength between the channels' wavelengths, each channel's own at its wavelength, and held at
 * the nearer channel outside them.
 */
export function binGroundAlbedo(albedo: Rgb, wavelengthNm: number): number {
  return channelRuleAt(albedo, wavelengthNm);
}

/** A term's extinction at relative density 1 in the bin the layers are placed by, m⁻¹. */
export interface TermExtinction {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
}

/** The layer interfaces in h\*, from the top down (the module's remarks), over `span`. */
function layerInterfaces(
  column: SpectralColumn,
  reference: ReadonlyArray<TermExtinction>,
  layers: number,
  span: BakeSpan = { bottomM: 0, topM: column.topHeightM },
): Float64Array {
  const top = span.topM;
  const bottom = span.bottomM;
  if (!(Number.isFinite(bottom) && Number.isFinite(top) && bottom < top)) {
    throw new RangeError(
      `a bake's layers fill heights from a bottom below their top, not ${bottom}–${top} m`,
    );
  }
  const totals = column.terms.map((term) => columnLengthM(term.density, top));
  const depthAbove = (h: number): number => {
    let depth = 0;
    for (const [t, term] of column.terms.entries()) {
      const optics = reference[t];
      if (optics === undefined) {
        continue;
      }
      const extinction = optics.scatteringPerM + optics.absorptionPerM;
      if (extinction > 0) {
        depth += extinction * ((totals[t] ?? Number.NaN) - columnLengthM(term.density, h));
      }
    }
    return depth;
  };
  const whole = depthAbove(bottom);
  const xi = (h: number): number =>
    whole > 0
      ? 0.5 * (1 - depthAbove(h) / whole) + (0.5 * (h - bottom)) / (top - bottom)
      : (h - bottom) / (top - bottom);
  const heights = new Float64Array(layers + 1);
  heights[0] = top;
  heights[layers] = bottom;
  for (let k = 1; k < layers; k += 1) {
    const target = 1 - k / layers;
    let low = bottom;
    let high = top;
    for (let i = 0; i < 52; i += 1) {
      const middle = 0.5 * (low + high);
      if (xi(middle) < target) {
        low = middle;
      } else {
        high = middle;
      }
    }
    heights[k] = 0.5 * (low + high);
  }
  for (let k = 1; k <= layers; k += 1) {
    if (!((heights[k] ?? Number.NaN) < (heights[k - 1] ?? Number.NaN))) {
      throw new Error(`the bake's interfaces do not descend at ${k}`);
    }
  }
  return heights;
}

/** The Gauss–Legendre rule each layer's Curtis–Godson pressure is taken by. */
const PRESSURE_RULE = gaussLegendre(8);

/** A layer's Curtis–Godson pressure for a density profile: ∫ p ρ dh ÷ ∫ ρ dh, Pa. */
function layerPressurePa(
  pressureBar: TabulatedDensity,
  density: Parameters<typeof densityAt>[0],
  bottomM: number,
  topM: number,
): number {
  let weighted = 0;
  let amount = 0;
  for (const [i, t] of PRESSURE_RULE.nodes.entries()) {
    const h = bottomM + t * (topM - bottomM);
    const w = (PRESSURE_RULE.weights[i] ?? Number.NaN) * densityAt(density, h);
    weighted += w * densityAt(pressureBar, h) * ABSORBER_PRESSURE_REFERENCE_PA;
    amount += w;
  }
  return amount > 0
    ? weighted / amount
    : densityAt(pressureBar, 0.5 * (bottomM + topM)) * ABSORBER_PRESSURE_REFERENCE_PA;
}

/** An absorber term of the column, laid on the layers. */
interface LaidAbsorber {
  /** Its amount in each layer before a band's factor, m⁻² or m⁻⁵. */
  readonly amounts: Float64Array;
  /** Its Curtis–Godson pressure in each layer, Pa, or `undefined` for a column with no pressure profile. */
  readonly pressuresPa: Float64Array | undefined;
  /** The whole column's Curtis–Godson pressure, Pa, where known. */
  readonly columnPressurePa: number | undefined;
}

/** A fit's terms laid on the layers: each term's gas optical depth per layer. */
function laidTerms(fit: ExpSumFit, absorber: LaidAbsorber): ReadonlyArray<BinTerm> {
  const n = fit.weights.length;
  const layers = absorber.amounts.length;
  const perTerm = Array.from({ length: n }, () => new Float64Array(layers));
  for (let p = 0; p < layers; p += 1) {
    const k = expSumCoefficientsAt(fit, absorber.pressuresPa?.[p]);
    const amount = absorber.amounts[p] ?? Number.NaN;
    for (let i = 0; i < n; i += 1) {
      const layer = perTerm[i];
      if (layer !== undefined) {
        layer[p] = (k[i] ?? Number.NaN) * amount;
      }
    }
  }
  return perTerm.map((absorptionDepth, i) => ({
    weight: fit.weights[i] ?? Number.NaN,
    absorptionDepth,
  }));
}

/** The random overlap of absorbers' terms: every product of one term of each. */
function overlapped(sets: ReadonlyArray<ReadonlyArray<BinTerm>>, layers: number): BinTerm[] {
  let terms: BinTerm[] = [{ weight: 1, absorptionDepth: new Float64Array(layers) }];
  for (const set of sets) {
    const next: BinTerm[] = [];
    for (const term of terms) {
      for (const other of set) {
        next.push({
          weight: term.weight * other.weight,
          absorptionDepth: term.absorptionDepth.map(
            (tau, p) => tau + (other.absorptionDepth[p] ?? Number.NaN),
          ),
        });
      }
    }
    terms = next;
  }
  return terms;
}

/**
 * An absorber's vertical bin optical depth at its vertical amount and its fit's reference row: the
 * curve's own −ln T, which `absorbers.ts` keeps finite however deep.
 */
function verticalBinDepth(curves: AbsorberBinCurves, bin: number, row: number): number {
  const nodes = curves.logColumns.length;
  return curves.opticalDepth[bin]?.[row * nodes + ((nodes - 1) >> 1)] ?? 0;
}

/** One light's bins `bins`, in their order (the module's remarks, "Spectral"). */
function lightBins(
  light: ThickBakeLight,
  absorberTerms: ReadonlyArray<LaidAbsorber>,
  none: ReadonlyArray<BinTerm>,
  options: ExpSumOptions,
  bins: ReadonlyArray<number>,
): ReadonlyArray<BakeLightBin> {
  const layers = none[0]?.absorptionDepth.length ?? 0;
  return bins.map((b): BakeLightBin => {
    if (absorberTerms.length === 0) {
      return { terms: none, largestFitError: 0, readFitError: 0, crossedRows: 0 };
    }
    const fitsOf = (maxTerms: number, a: number): ExpSumFit => {
      const curves = light.absorbers[a];
      const absorber = absorberTerms[a];
      const transmittance = curves?.transmittance[b];
      if (curves === undefined || absorber === undefined || transmittance === undefined) {
        throw new RangeError(`absorber ${a} has no curve in bin ${b}`);
      }
      return fitExpSum(
        {
          logColumns: curves.logColumns,
          logPressures: curves.logPressures,
          readTopLogPressure: curves.readTopLogPressure,
          transmittance,
        },
        absorber.columnPressurePa === undefined
          ? (curves.logPressures[0] ?? Number.NaN)
          : log(absorber.columnPressurePa),
        { ...options, maxTerms: Math.min(maxTerms, options.maxTerms) },
      );
    };
    const fits = Array.from(absorberTerms.keys(), (a) => fitsOf(options.maxTerms, a));
    const count = (): number => fits.reduce((product, fit) => product * fit.weights.length, 1);
    // Past the cap, the weakest absorber with more than one term is taken as one, then the next.
    const byDepth = Array.from(absorberTerms.keys()).toSorted((x, y) => {
      const depth = (a: number): number => {
        const curves = light.absorbers[a];
        return curves === undefined ? 0 : verticalBinDepth(curves, b, fits[a]?.referenceRow ?? 0);
      };
      return depth(x) - depth(y);
    });
    for (const a of byDepth) {
      if (count() <= THICK_BAKE_MAX_TERMS_PER_BIN) {
        break;
      }
      if ((fits[a]?.weights.length ?? 1) > 1) {
        fits[a] = fitsOf(1, a);
      }
    }
    const sets = fits.map((fit, a) => {
      const absorber = absorberTerms[a];
      if (absorber === undefined) {
        throw new Error(`absorber ${a} is not laid`);
      }
      return fit.coefficients.every((k) => k === 0) ? none : laidTerms(fit, absorber);
    });
    return {
      terms: sets.every((set) => set === none) ? none : overlapped(sets, layers),
      largestFitError: Math.max(0, ...fits.map((fit) => fit.largestError)),
      readFitError: Math.max(
        0,
        ...fits.map((fit, a) => {
          const absorber = absorberTerms[a];
          return absorber === undefined ? Number.NaN : readError(fit, absorber);
        }),
      ),
      crossedRows: fits.reduce((sum, fit) => sum + fit.crossedRows, 0),
    };
  });
}

/** A layer as delta-M scales it, and the fraction it truncated. */
export interface DeltaMLayer {
  readonly layer: OrdinateLayer;
  /** f. */
  readonly truncated: number;
}

/**
 * The delta-M scaled layer of a layer's mixed optics (Wiscombe 1977; the module's remarks), at
 * `streams` in all: f = max(χ_NSTR, f_c, 0).
 *
 * @param moments - The layer's mixed χ₀ … χ_NSTR, χ₀ = 1.
 * @param forwardDelta - f_c, the forward delta's share of its scattering.
 * @throws RangeError for moments shorter than NSTR + 1, or an f of 1 or more.
 */
export function deltaMLayer(
  opticalDepth: number,
  scatteringDepth: number,
  moments: Float64Array,
  forwardDelta: number,
  streams: number = THICK_BAKE_STREAMS,
): DeltaMLayer {
  if (moments.length < streams + 1) {
    throw new RangeError(`delta-M at ${streams} streams needs χ₀ … χ_${streams}`);
  }
  const omega = opticalDepth > 0 ? Math.min(scatteringDepth / opticalDepth, 1) : 0;
  const f = omega > 0 ? Math.max(moments[streams] ?? Number.NaN, forwardDelta, 0) : 0;
  if (!(f < 1)) {
    throw new RangeError(`delta-M cannot truncate a fraction of ${f}`);
  }
  const scaled = Float64Array.from({ length: streams }, (_, l) =>
    l === 0 ? 1 : ((moments[l] ?? Number.NaN) - f) / (1 - f),
  );
  return {
    layer: {
      opticalDepth: opticalDepth * (1 - omega * f),
      singleScatteringAlbedo: omega > 0 ? (omega * (1 - f)) / (1 - omega * f) : 0,
      moments: scaled,
    },
    truncated: f,
  };
}

/**
 * Everything of a bake that does not depend on the band (the module's remarks): the column's optics
 * in every bin (an aerosol's Mie or MMF once a bin), the layers and their integrals, the table's
 * altitudes and their point optics, and each light's exponential sums and conversion.
 *
 * @param layout - The layers laid by the caller (R08.T15's deck split), in place of the whole
 *   column's on the grid; absent, the bake's own.
 * @throws RangeError for a grid that is not whole, a light whose absorbers do not match the
 *   column's absorber terms in count and kind, a curve with pressure rows on a column with no
 *   pressure profile, a `layout` whose table does not hold the grid's layers or whose fit does not
 *   begin with them, or as the column's optics refuse.
 */
export function prepareBake(
  column: SpectralColumn,
  lights: ReadonlyArray<ThickBakeLight>,
  grid: ThickBakeGrid = THICK_BAKE_GRID,
  options: ExpSumOptions = EXP_SUM_OPTIONS,
  layout?: BakeLayout,
): PreparedBake {
  checkBakeInputs(column, lights, grid);
  if (layout !== undefined) {
    checkBakeLayout(layout, grid);
  }
  const optics = spectralOptics(column, BAKE_WAVELENGTHS_NM);
  const order = THICK_BAKE_STREAMS;
  const binOptics = optics.map((terms) =>
    terms.map((t): BinTermOptics => ({
      scatteringPerM: t.scatteringPerM,
      absorptionPerM: t.absorptionPerM,
      moments: phaseMoments(t.phase, order),
      forwardDelta: forwardDeltaShare(t.phase),
    })),
  );
  const layering =
    layout?.layering ?? bakeLayering(column, binOptics[THICK_BAKE_REFERENCE_BIN] ?? [], grid);
  const { interfaceHeightsM, layerColumnsM, altitudesM } = layering;
  const pointOptics = binOptics.map((terms) =>
    Array.from(altitudesM, (h): PointOptics => {
      let scattering = 0;
      let delta = 0;
      const mixed = new Float64Array(order + 1);
      for (const [t, term] of column.terms.entries()) {
        const o = terms[t];
        if (o === undefined || o.scatteringPerM === 0) {
          continue;
        }
        const s = o.scatteringPerM * densityAt(term.density, h);
        scattering += s;
        delta += s * o.forwardDelta;
        for (let l = 0; l <= order; l += 1) {
          mixed[l] = (mixed[l] ?? 0) + s * (o.moments[l] ?? Number.NaN);
        }
      }
      if (!(scattering > 0)) {
        return {
          scatteringPerM: 0,
          lobe: 0,
          kept: 1,
          forwardDelta: 0,
          moments: new Float64Array(order),
        };
      }
      const forwardDelta = delta / scattering;
      const f = Math.max((mixed[order] ?? Number.NaN) / scattering, forwardDelta, 0);
      return {
        scatteringPerM: scattering,
        lobe: f - forwardDelta,
        kept: 1 - f,
        forwardDelta,
        moments: Float64Array.from({ length: order }, (_, l) =>
          l === 0 ? 1 : ((mixed[l] ?? Number.NaN) / scattering - f) / (1 - f),
        ),
      };
    }),
  );
  const everyBin = Array.from({ length: BAKE_BIN_COUNT }, (_, b) => b);
  const bins = bakeLightBins(column, layout?.fitLayering ?? layering, lights, everyBin, options);
  const { temperature } = column;
  const interfacePlanck =
    temperature === undefined
      ? undefined
      : BAKE_WAVELENGTHS_NM.map((nm) =>
          Float64Array.from(interfaceHeightsM, (h) =>
            planckRadiance(nm, mediumTemperatureK(temperature, h)),
          ),
        );
  return {
    column,
    grid,
    interfaceHeightsM,
    altitudesM,
    ...(interfacePlanck === undefined ? {} : { interfacePlanck }),
    binOptics,
    binPhases: optics.map((terms) => terms.map((t) => t.phase)),
    groundAlbedo: Float64Array.from(BAKE_WAVELENGTHS_NM, (nm) =>
      binGroundAlbedo(column.groundAlbedo, nm),
    ),
    layerColumnsM,
    pointOptics,
    lights: lights.map((light, j): PreparedLight => ({
      conversion: binConversion(light.spectrum),
      bins: bins[j] ?? [],
    })),
  };
}

/**
 * Checks a bake's grid and lights against its column, as {@link prepareBake} does first: whole
 * strides, sun cosines in [−1, 1] other than 0 (below 0 a sun below the horizontal, R08.T14.d.2),
 * view cosines in (0, 1], and each light's absorbers' curves one a column's absorber term of its
 * kind, with pressure rows only where the column has a pressure profile.
 *
 * @throws RangeError as {@link prepareBake} does for them.
 */
export function checkBakeInputs(
  column: SpectralColumn,
  lights: ReadonlyArray<ThickBakeLight>,
  grid: ThickBakeGrid,
): void {
  const { layers, altitudeStride } = grid;
  if (
    !(Number.isInteger(layers) && layers >= 1) ||
    !(Number.isInteger(altitudeStride) && altitudeStride >= 1 && layers % altitudeStride === 0)
  ) {
    throw new RangeError(`a bake's ${layers} layers take whole strides, not ${altitudeStride}`);
  }
  for (const mu of grid.sunCosines) {
    if (!(mu >= -1 && mu <= 1 && mu !== 0)) {
      throw new RangeError(`a bake's sun cosines lie in [−1, 1] and are not 0, not ${mu}`);
    }
  }
  for (const mu of grid.upwardCosines) {
    if (!(mu > 0 && mu <= 1)) {
      throw new RangeError(`a bake's view cosines lie in (0, 1], not ${mu}`);
    }
  }
  const absorberIndices = absorberTermsOf(column);
  for (const [slot, light] of lights.entries()) {
    if (light.absorbers.length !== absorberIndices.length) {
      throw new RangeError(
        `light ${slot} brings ${light.absorbers.length} absorbers' curves for ${absorberIndices.length}`,
      );
    }
    for (const [a, curves] of light.absorbers.entries()) {
      const source = column.terms[absorberIndices[a] ?? -1]?.source;
      if (source?.kind !== "absorber" || source.amount.kind !== curves.kind) {
        throw new RangeError(`light ${slot}'s absorber ${a} is not of its term's kind`);
      }
      if (curves.logPressures.length > 1 && column.pressureBar === undefined) {
        throw new RangeError(
          `absorber ${a}'s curves have pressure rows and the column no pressure`,
        );
      }
    }
  }
}

/** The indices of a column's absorber terms, in its order. */
function absorberTermsOf(column: SpectralColumn): number[] {
  return column.terms.flatMap((term, t) => (term.source.kind === "absorber" ? [t] : []));
}

/**
 * The heights a bake's layers fill, h\* in m, `bottomM` below `topM`: the whole column, 0 to its
 * top, by default; R08.T15's deck split lays the table above the split and the deck below it.
 */
export interface BakeSpan {
  readonly bottomM: number;
  readonly topM: number;
}

/**
 * A bake's layers laid by its caller (R08.T15's deck split, `deck.ts`): the table's, and the whole
 * column's, over which its lights' exponential sums are fitted and laid, so that each term of a
 * bin's gases is one term from the column's top to its floor (Design note 5's correlated sums).
 */
export interface BakeLayout {
  /** The table's layers on the bake's grid ({@link bakeLayering} over a span). */
  readonly layering: BakeLayering;
  /**
   * The layers the lights' sums are fitted over and laid on ({@link bakeLightBins}): the table's
   * from the top down, then any below them, so that a term's `absorptionDepth` holds the table's
   * layers first.
   */
  readonly fitLayering: BakeLayering;
}

/**
 * Checks a {@link BakeLayout} against its grid, as {@link prepareBake} does: the table's layers the
 * grid's count, and the fit's layers beginning with them.
 *
 * @throws RangeError for a layout that is not so.
 */
export function checkBakeLayout(layout: BakeLayout, grid: ThickBakeGrid): void {
  const table = layout.layering.interfaceHeightsM;
  const fit = layout.fitLayering.interfaceHeightsM;
  if (table.length !== grid.layers + 1 || fit.length < table.length) {
    throw new RangeError(
      `a layout lays the grid's ${grid.layers} layers first, not ${table.length - 1} of ${fit.length - 1}`,
    );
  }
  table.forEach((h, k) => {
    if (fit[k] !== h) {
      throw new RangeError(`a layout's fit departs from the table's layers at interface ${k}`);
    }
  });
}

/** A bake's layers: their interfaces, each term's column in each, and the table's altitudes. */
export interface BakeLayering {
  /** The interfaces' h\*, m, from the top down: L + 1. */
  readonly interfaceHeightsM: Float64Array;
  /** Per term, per layer from the top, ∫ρ dh over it, m. */
  readonly layerColumnsM: ReadonlyArray<Float64Array>;
  /** The table's altitudes, ascending: every `altitudeStride`-th interface from the ground. */
  readonly altitudesM: Float64Array;
}

/**
 * A bake's layers on `grid` (the module's remarks): the interfaces evenly in ξ by the terms'
 * extinction in the reference bin, each term's exact column in each layer, and the table's
 * altitudes; {@link prepareBake}'s, which R08.T14.e's correction lays the same way.
 *
 * @param reference - Each term's extinction at relative density 1 in
 *   {@link THICK_BAKE_REFERENCE_BIN}, in the column's order.
 * @param span - The heights the layers fill, the whole column by default (R08.T15's split lays a
 *   part of it): ξ is then taken over the span, its own optical depth and height.
 * @throws RangeError for a span that is not finite with its bottom below its top.
 */
export function bakeLayering(
  column: SpectralColumn,
  reference: ReadonlyArray<TermExtinction>,
  grid: ThickBakeGrid,
  span?: BakeSpan,
): BakeLayering {
  const { layers, altitudeStride } = grid;
  const interfaceHeightsM = layerInterfaces(column, reference, layers, span);
  const layerColumnsM = column.terms.map((term) => {
    const totals = Float64Array.from(interfaceHeightsM, (h) => columnLengthM(term.density, h));
    return Float64Array.from(
      { length: layers },
      (_, p) => (totals[p] ?? Number.NaN) - (totals[p + 1] ?? Number.NaN),
    );
  });
  const altitudesM = Float64Array.from(
    { length: layers / altitudeStride + 1 },
    (_, a) => interfaceHeightsM[layers - a * altitudeStride] ?? Number.NaN,
  );
  return { interfaceHeightsM, layerColumnsM, altitudesM };
}

/**
 * Each light's bins `bins` (the module's remarks, "Spectral"), `[light][i]` for `bins[i]`: its
 * absorbers laid on the layers, each a Curtis–Godson pressure a layer, and their exponential sums'
 * random overlap in each bin, one solve a term; {@link prepareBake}'s for every bin.
 *
 * @remarks
 * A light's bin with no gases takes one unabsorbed term, the same object in every light's, so that
 * a band solves it once for them all.
 *
 * @throws RangeError as {@link prepareBake} does for a light's curves.
 */
export function bakeLightBins(
  column: SpectralColumn,
  layering: BakeLayering,
  lights: ReadonlyArray<ThickBakeLight>,
  bins: ReadonlyArray<number>,
  options: ExpSumOptions = EXP_SUM_OPTIONS,
): ReadonlyArray<ReadonlyArray<BakeLightBin>> {
  const { interfaceHeightsM, layerColumnsM } = layering;
  const layers = interfaceHeightsM.length - 1;
  const absorbers = absorberTermsOf(column).map((t): LaidAbsorber => {
    const term = column.terms[t];
    if (term === undefined || term.source.kind !== "absorber") {
      throw new Error(`term ${t} is not an absorber`);
    }
    const { amount } = term.source;
    const perUnit = amount.kind === "column" ? amount.numberDensityPerM3 : amount.pairDensityPerM6;
    const columns = layerColumnsM[t] ?? new Float64Array(layers);
    const { pressureBar } = column;
    const pressuresPa =
      pressureBar === undefined
        ? undefined
        : Float64Array.from({ length: layers }, (_, p) =>
            layerPressurePa(
              pressureBar,
              term.density,
              interfaceHeightsM[p + 1] ?? Number.NaN,
              interfaceHeightsM[p] ?? Number.NaN,
            ),
          );
    let weighted = 0;
    let total = 0;
    for (let p = 0; p < layers; p += 1) {
      weighted += (pressuresPa?.[p] ?? 0) * (columns[p] ?? 0);
      total += columns[p] ?? 0;
    }
    return {
      amounts: columns.map((c) => c * perUnit),
      pressuresPa,
      columnPressurePa: pressuresPa === undefined || !(total > 0) ? undefined : weighted / total,
    };
  });
  // One unabsorbed term, shared by every light's bins with no gases, so that a band solves it once.
  const none: ReadonlyArray<BinTerm> = [{ weight: 1, absorptionDepth: new Float64Array(layers) }];
  return lights.map((light) => lightBins(light, absorbers, none, options, bins));
}

// ---------------------------------------------------------------------------------------------
// One band.

/** A band's layers in one bin, before the gases: per layer from the top. */
interface BandBin {
  readonly extinction: Float64Array;
  readonly scattering: Float64Array;
  /** The layer's mixed χ₀ … χ₁₆, scattering-weighted, row-major `[p × 17 + l]`. */
  readonly moments: Float64Array;
  readonly forwardDelta: Float64Array;
}

function bandBinOf(prepared: PreparedBake, bin: number, factor: number): BandBin {
  const layers = prepared.grid.layers;
  const order = THICK_BAKE_STREAMS;
  const terms = prepared.binOptics[bin] ?? [];
  const extinction = new Float64Array(layers);
  const scattering = new Float64Array(layers);
  const moments = new Float64Array(layers * (order + 1));
  const forwardDelta = new Float64Array(layers);
  for (let p = 0; p < layers; p += 1) {
    let tau = 0;
    let tauS = 0;
    let delta = 0;
    for (const [t, o] of terms.entries()) {
      const c = (prepared.layerColumnsM[t]?.[p] ?? Number.NaN) * factor;
      tau += (o.scatteringPerM + o.absorptionPerM) * c;
      const s = o.scatteringPerM * c;
      if (s > 0) {
        tauS += s;
        delta += s * o.forwardDelta;
        for (let l = 0; l <= order; l += 1) {
          const k = p * (order + 1) + l;
          moments[k] = (moments[k] ?? 0) + s * (o.moments[l] ?? Number.NaN);
        }
      }
    }
    extinction[p] = tau;
    scattering[p] = tauS;
    forwardDelta[p] = tauS > 0 ? delta / tauS : 0;
    for (let l = 0; l <= order; l += 1) {
      const k = p * (order + 1) + l;
      moments[k] = tauS > 0 ? (moments[k] ?? 0) / tauS : l === 0 ? 1 : 0;
    }
  }
  return { extinction, scattering, moments, forwardDelta };
}

/** The Legendre functions the source takes, per mode: at the nodes, the view cosines and the suns. */
interface SourceLegendre {
  /** Λ_l^m(μ_n), `[m][l × N + n]`. */
  readonly nodes: ReadonlyArray<Float64Array>;
  /** Λ_l^m(μ_u), `[m][l × U + u]`. */
  readonly views: ReadonlyArray<Float64Array>;
  /** Λ_l^m(μ₀), `[m][l × S + s]`. */
  readonly suns: ReadonlyArray<Float64Array>;
}

function legendreOf(cosines: Float64Array, m: number, lmax: number): Float64Array {
  const count = cosines.length;
  const out = new Float64Array((lmax + 1) * count);
  for (const [i, mu] of cosines.entries()) {
    const values = normalizedAssociatedLegendre(m, lmax, mu);
    for (let l = 0; l <= lmax; l += 1) {
      out[l * count + i] = values[l] ?? Number.NaN;
    }
  }
  return out;
}

/** One solve of a bin, and its source at the table's points. */
export interface BinSolve {
  readonly solution: DiscreteOrdinatesSolution;
  /** The delta-M beams, one a μ₀. */
  readonly scaledBeams: ReadonlyArray<SphericalBeam>;
  /** The kernels' beams, extinction less the forward delta, one a μ₀. */
  readonly kernelBeams: ReadonlyArray<SphericalBeam>;
  /** E of this solve alone, A × S × V × M, in the bin (no conversion), weight 1. */
  readonly source: Float64Array;
  /** Per altitude and μ₀, `[a × S + s]`: IMS's left-out light, and the point's light (the module's remarks). */
  readonly leftOut: Float64Array;
  readonly light: Float64Array;
  /** The ground other than Lambertian the solve took (R08.T15's deck), or `undefined` for the albedo. */
  readonly groundReflectance: GroundReflectance | undefined;
  /**
   * E_th of this solve alone (the module's remarks, "Thermal emission"), A × V, in the bin's
   * radiance unit per metre, weight 1; absent for a column without a temperature.
   */
  readonly thermalSource?: Float64Array;
}

/**
 * A ground other than Lambertian for a band's solves (R08.T15's deck): its reflectance in bin
 * `bin` for one term of the bin's gases, at the cosines the solve reads it (`discreteOrdinates.ts`'s
 * `groundCosinesOf`) and modes 0 … `maxMode`, in place of the column's albedo.
 */
export type BakeGround = (
  bin: number,
  term: BinTerm,
  cosines: GroundCosines,
  maxMode: number,
) => GroundReflectance;

/** A band's bake options (R08.T15's deck split). */
export interface BakeBandOptions {
  /** A ground other than Lambertian for every solve, in place of the column's albedo. */
  readonly ground?: BakeGround;
  /**
   * Called with each solve once it is made, once a bin and term of the band, before the next: so
   * that a caller reads its fields (the deck, its radiance at the ground) without a second solve.
   */
  readonly onSolve?: (bin: number, term: BinTerm, solve: BinSolve) => void;
  /**
   * The azimuthal modes each solve takes, its ground's among them: {@link THICK_BAKE_MODES} by
   * default, more for a reader of the solves' own fields (R08.T15.a.2's test-only option, the deck's
   * sky-lit radiance at its own modes). The table holds J to {@link THICK_BAKE_MODES} whatever.
   */
  readonly solveModes?: number;
}

/**
 * One solve of a band's bin for one term of its gases (the module's remarks): the delta-M layers,
 * the pseudo-spherical beams, the solve, and E at every table point of this solve alone.
 *
 * @param ground - A ground other than Lambertian ({@link BakeBandOptions}); the column's albedo
 *   when absent.
 * @throws RangeError as {@link bakeMultipleScattering}.
 */
export function bakeBinSolve(
  prepared: PreparedBake,
  band: ThickBakeBand,
  bin: number,
  term: BinTerm,
  ground?: BakeGround,
): BinSolve {
  return solveOf(
    prepared,
    band,
    bin,
    bandBinOf(prepared, bin, band.coefficientFactor),
    term,
    sourceLegendre(prepared),
    ground,
  );
}

function sourceLegendre(prepared: PreparedBake): SourceLegendre {
  const lmax = THICK_BAKE_STREAMS - 1;
  const quadrature = doubleGaussQuadrature(THICK_BAKE_STREAMS);
  const modes = Array.from({ length: THICK_BAKE_MODES }, (_, m) => m);
  return {
    nodes: modes.map((m) => legendreOf(quadrature.nodes, m, lmax)),
    views: modes.map((m) => legendreOf(prepared.grid.upwardCosines, m, lmax)),
    suns: modes.map((m) => legendreOf(prepared.grid.sunCosines, m, lmax)),
  };
}

function solveOf(
  prepared: PreparedBake,
  band: ThickBakeBand,
  bin: number,
  base: BandBin,
  term: BinTerm,
  legendre: SourceLegendre,
  ground?: BakeGround,
  solveModes: number = THICK_BAKE_MODES,
): BinSolve {
  const { grid, interfaceHeightsM } = prepared;
  const layers = grid.layers;
  const order = THICK_BAKE_STREAMS;
  const factor = band.coefficientFactor;
  const scaled: OrdinateLayer[] = [];
  const scaledDepths = new Float64Array(layers);
  const kernelDepths = new Float64Array(layers);
  for (let p = 0; p < layers; p += 1) {
    const tau =
      (base.extinction[p] ?? Number.NaN) + factor * (term.absorptionDepth[p] ?? Number.NaN);
    const tauS = base.scattering[p] ?? Number.NaN;
    const delta = base.forwardDelta[p] ?? Number.NaN;
    const { layer } = deltaMLayer(
      tau,
      tauS,
      base.moments.subarray(p * (order + 1), (p + 1) * (order + 1)),
      delta,
    );
    scaled.push(layer);
    scaledDepths[p] = layer.opticalDepth;
    kernelDepths[p] = Math.max(tau - tauS * delta, 0);
  }
  const radius = band.bottomRadiusM;
  const planck = prepared.interfacePlanck?.[bin];
  const scaledBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, interfaceHeightsM, scaledDepths, mu0),
  );
  const kernelBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, interfaceHeightsM, kernelDepths, mu0),
  );
  const groundReflectance = ground?.(
    bin,
    term,
    groundCosinesOf(THICK_BAKE_STREAMS, grid.upwardCosines, grid.sunCosines),
    solveModes - 1,
  );
  const solution = solveDiscreteOrdinates({
    layers: scaled,
    groundAlbedo: groundReflectance === undefined ? (prepared.groundAlbedo[bin] ?? Number.NaN) : 0,
    ...(groundReflectance === undefined ? {} : { groundReflectance }),
    beamCosines: grid.sunCosines,
    userCosines: grid.upwardCosines,
    maxMode: solveModes - 1,
    streams: THICK_BAKE_STREAMS,
    beamPaths: scaledBeams,
    ...(planck === undefined
      ? {}
      : { thermal: { interfacePlanck: planck, groundPlanck: planck[layers] ?? Number.NaN } }),
  });
  const points = prepared.pointOptics[bin] ?? [];
  const altitudes = prepared.altitudesM.length;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const v2 = 2 * views;
  const source = new Float64Array(altitudes * suns * v2 * THICK_BAKE_MODES);
  const leftOut = new Float64Array(altitudes * suns);
  const light = new Float64Array(altitudes * suns);
  const { nodes, weights } = solution.quadrature;
  const n = nodes.length;
  const half = new Float64Array(order);
  for (let a = 0; a < altitudes; a += 1) {
    const point = points[a];
    if (point === undefined || point.scatteringPerM === 0) {
      continue;
    }
    const k = layers - a * grid.altitudeStride;
    for (let s = 0; s < suns; s += 1) {
      const beam = solution.beams[s];
      const tScaled = scaledBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
      const tKernel = kernelBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
      if (beam === undefined) {
        throw new Error(`no beam ${s}`);
      }
      const aureole = Math.max(tScaled - tKernel, 0);
      const meanDiffuse = (beam.fluxes.meanIntensity[k] ?? Number.NaN) - tScaled / (4 * Math.PI);
      leftOut[a * suns + s] = point.scatteringPerM * point.lobe * aureole;
      light[a * suns + s] =
        point.scatteringPerM *
        ((1 - point.forwardDelta) * 4 * Math.PI * meanDiffuse + point.kept * aureole);
      for (let m = 0; m < THICK_BAKE_MODES; m += 1) {
        const mode = beam.modes[m];
        const nodeL = legendre.nodes[m];
        const viewL = legendre.views[m];
        const sunL = legendre.suns[m];
        if (
          mode === undefined ||
          nodeL === undefined ||
          viewL === undefined ||
          sunL === undefined
        ) {
          throw new Error(`no mode ${m}`);
        }
        // H_l = Σ_n w_n Λ_l(μ_n)(I⁺_n + (−1)^{l+m} I⁻_n), the diffuse field's half-range moments.
        for (let l = m; l < order; l += 1) {
          const sign = (l + m) % 2 === 0 ? 1 : -1;
          let sum = 0;
          for (let j = 0; j < n; j += 1) {
            sum +=
              (weights[j] ?? Number.NaN) *
              (nodeL[l * n + j] ?? Number.NaN) *
              ((mode.quadratureUp[k * n + j] ?? Number.NaN) +
                sign * (mode.quadratureDown[k * n + j] ?? Number.NaN));
          }
          half[l] = sum;
        }
        const azimuthal = (m === 0 ? 1 : 2) / (4 * Math.PI);
        for (let u = 0; u < views; u += 1) {
          let even = 0;
          let odd = 0;
          let beamEven = 0;
          let beamOdd = 0;
          for (let l = m; l < order; l += 1) {
            const c =
              (2 * l + 1) * (point.moments[l] ?? Number.NaN) * (viewL[l * views + u] ?? Number.NaN);
            const diffuse = c * (half[l] ?? Number.NaN);
            const scatteredBeam = c * (sunL[l * suns + s] ?? Number.NaN);
            if ((l + m) % 2 === 0) {
              even += diffuse;
              beamEven += scatteredBeam;
            } else {
              odd += diffuse;
              beamOdd += scatteredBeam;
            }
          }
          // Upward at +μ_u: ½ Σ c H, and the beam at −μ₀ through (−1)^{l+m}; downward the reverse.
          const smoothUp = 0.5 * (even + odd) + azimuthal * (beamEven - beamOdd) * aureole;
          const smoothDown = 0.5 * (even - odd) + azimuthal * (beamEven + beamOdd) * aureole;
          const intensityUp = mode.userUp[k * views + u] ?? Number.NaN;
          const intensityDown = mode.userDown[k * views + u] ?? Number.NaN;
          const cell = (a * suns + s) * v2;
          const up = (cell + views + u) * THICK_BAKE_MODES + m;
          const down = (cell + views - 1 - u) * THICK_BAKE_MODES + m;
          source[up] = point.scatteringPerM * (point.lobe * intensityUp + point.kept * smoothUp);
          source[down] =
            point.scatteringPerM * (point.lobe * intensityDown + point.kept * smoothDown);
        }
      }
    }
  }
  const thermalSource =
    solution.thermal === undefined
      ? undefined
      : thermalSourceOf(solution, points, layers, grid.altitudeStride, views, legendre);
  return {
    solution,
    scaledBeams,
    kernelBeams,
    source,
    leftOut,
    light,
    groundReflectance,
    ...(thermalSource === undefined ? {} : { thermalSource }),
  };
}

/**
 * E_th at the table's points (the module's remarks, "Thermal emission"): mode 0 of the thermal
 * field as {@link solveOf} forms a beam's diffuse field's, with no beam's terms, A × V, the travel
 * cosines downward then upward.
 */
function thermalSourceOf(
  solution: DiscreteOrdinatesSolution,
  points: ReadonlyArray<PointOptics>,
  layers: number,
  stride: number,
  views: number,
  legendre: SourceLegendre,
): Float64Array {
  const mode = solution.thermal?.mode;
  const nodeL = legendre.nodes[0];
  const viewL = legendre.views[0];
  if (mode === undefined || nodeL === undefined || viewL === undefined) {
    throw new Error("a thermal solve has mode 0");
  }
  const order = THICK_BAKE_STREAMS;
  const { nodes, weights } = solution.quadrature;
  const n = nodes.length;
  const v2 = 2 * views;
  const out = new Float64Array(points.length * v2);
  const half = new Float64Array(order);
  for (const [a, point] of points.entries()) {
    if (point.scatteringPerM === 0) {
      continue;
    }
    const k = layers - a * stride;
    for (let l = 0; l < order; l += 1) {
      const sign = l % 2 === 0 ? 1 : -1;
      let sum = 0;
      for (let j = 0; j < n; j += 1) {
        sum +=
          (weights[j] ?? Number.NaN) *
          (nodeL[l * n + j] ?? Number.NaN) *
          ((mode.quadratureUp[k * n + j] ?? Number.NaN) +
            sign * (mode.quadratureDown[k * n + j] ?? Number.NaN));
      }
      half[l] = sum;
    }
    for (let u = 0; u < views; u += 1) {
      let even = 0;
      let odd = 0;
      for (let l = 0; l < order; l += 1) {
        const c =
          (2 * l + 1) *
          (point.moments[l] ?? Number.NaN) *
          (viewL[l * views + u] ?? Number.NaN) *
          (half[l] ?? Number.NaN);
        if (l % 2 === 0) {
          even += c;
        } else {
          odd += c;
        }
      }
      const intensityUp = mode.userUp[k * views + u] ?? Number.NaN;
      const intensityDown = mode.userDown[k * views + u] ?? Number.NaN;
      out[a * v2 + views + u] =
        point.scatteringPerM * (point.lobe * intensityUp + point.kept * 0.5 * (even + odd));
      out[a * v2 + views - 1 - u] =
        point.scatteringPerM * (point.lobe * intensityDown + point.kept * 0.5 * (even - odd));
    }
  }
  return out;
}

/**
 * The thick bake of one latitude band (Provides' `bakeMultipleScattering`; the module's remarks):
 * for every light of the preparation, every bin and every term of its gases, one solve, its E
 * (m⁻¹ sr⁻¹) summed by the terms' weights; then each bin's J, E over the bin's kernel scattering at
 * the altitude (sr⁻¹ per unit of that scattering), converted to the channels.
 *
 * @param band - The band's index in {@link thickBakeBands}' order.
 * @param options - A ground other than Lambertian, a reader of each solve and the solves' modes
 *   (R08.T15's deck); none and {@link THICK_BAKE_MODES} by default.
 * @throws RangeError for a band the body does not have, solves' modes not whole in
 *   [{@link THICK_BAKE_MODES}, {@link THICK_BAKE_STREAMS}], or as the solver refuses a layer (which
 *   delta-M's truncation is there to prevent).
 */
export function bakeMultipleScattering(
  prepared: PreparedBake,
  body: ThickBakeBody,
  band: number,
  options?: BakeBandOptions,
): MsSourceTable {
  const bands = thickBakeBands(body);
  const geometry = bands[band];
  if (geometry === undefined || !Number.isInteger(band)) {
    throw new RangeError(`the body has ${bands.length} latitude bands, not a band ${band}`);
  }
  const solveModes = options?.solveModes ?? THICK_BAKE_MODES;
  if (!(
    Number.isInteger(solveModes) &&
    solveModes >= THICK_BAKE_MODES &&
    solveModes <= THICK_BAKE_STREAMS
  )) {
    throw new RangeError(
      `a band's solves take ${THICK_BAKE_MODES} to ${THICK_BAKE_STREAMS} modes, not ${solveModes}`,
    );
  }
  const { grid } = prepared;
  const legendre = sourceLegendre(prepared);
  const altitudes = prepared.altitudesM.length;
  const suns = grid.sunCosines.length;
  const views = 2 * grid.upwardCosines.length;
  const points = altitudes * suns * views * THICK_BAKE_MODES;
  const lights = prepared.lights.map(() => ({
    bins: Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(points)),
    solves: 0,
    leftOutShare: 0,
    floored: 0,
    deficit: 0,
  }));
  const verticalOpticalDepth = new Float64Array(BAKE_BIN_COUNT);
  // E_th per bin, A × V, over the first light's terms (the module's remarks, "Thermal emission").
  const thermalBins =
    prepared.interfacePlanck === undefined
      ? null
      : Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(altitudes * views));
  for (let b = 0; b < BAKE_BIN_COUNT; b += 1) {
    const base = bandBinOf(prepared, b, geometry.coefficientFactor);
    verticalOpticalDepth[b] = base.extinction.reduce((sum, tau) => sum + tau, 0);
    // A solve shared by lights whose bin has the same terms (a column without absorbers).
    const solved = new Map<BinTerm, BinSolve>();
    for (const [slot, light] of prepared.lights.entries()) {
      const out = lights[slot];
      const binValues = out?.bins[b];
      const lightBin = light.bins[b];
      if (out === undefined || binValues === undefined || lightBin === undefined) {
        throw new Error(`light ${slot} has no bin ${b}`);
      }
      const leftOut = new Float64Array(altitudes * suns);
      const pointLight = new Float64Array(altitudes * suns);
      for (const term of lightBin.terms) {
        let solve = solved.get(term);
        if (solve === undefined) {
          solve = solveOf(prepared, geometry, b, base, term, legendre, options?.ground, solveModes);
          solved.set(term, solve);
          options?.onSolve?.(b, term, solve);
        }
        out.solves += 1;
        const w = term.weight;
        for (const [i, value] of solve.source.entries()) {
          binValues[i] = (binValues[i] ?? 0) + w * value;
        }
        const thermalBin = thermalBins?.[b];
        if (slot === 0 && thermalBin !== undefined && solve.thermalSource !== undefined) {
          for (const [i, value] of solve.thermalSource.entries()) {
            thermalBin[i] = (thermalBin[i] ?? 0) + w * value;
          }
        }
        for (const [i, value] of solve.leftOut.entries()) {
          leftOut[i] = (leftOut[i] ?? 0) + w * value;
          pointLight[i] = (pointLight[i] ?? 0) + w * (solve.light[i] ?? Number.NaN);
        }
        for (const beam of solve.scaledBeams) {
          out.floored += beam.flooredLayers;
          out.deficit = Math.max(out.deficit, beam.largestDeficit);
        }
      }
      const brightest = Math.max(0, ...pointLight);
      for (const [i, value] of leftOut.entries()) {
        const kept = pointLight[i] ?? Number.NaN;
        if (kept >= 1e-3 * brightest && kept > 0) {
          // Of the whole: the light kept and the light left out.
          out.leftOutShare = Math.max(out.leftOutShare, value / (kept + value));
        }
      }
    }
  }
  // J = E ÷ (σ_s (1 − f_c)) in each bin, its own kernel scattering at the altitude; none where it
  // has no scattering, whose E the solve leaves 0.
  const perAltitude = suns * views * THICK_BAKE_MODES;
  const perScattering = prepared.pointOptics.map((bin) =>
    Float64Array.from(bin, (point) => {
      const kernel = point.scatteringPerM * (1 - point.forwardDelta);
      return kernel > 0 ? 1 / kernel : 0;
    }),
  );
  const slots = prepared.lights.map((light, slot): MsSourceSlot => {
    const out = lights[slot];
    if (out === undefined) {
      throw new Error(`no light ${slot}`);
    }
    const values = new Float32Array(points * 3);
    const [red, green, blue] = light.conversion.factors;
    for (let i = 0; i < points; i += 1) {
      const a = Math.floor(i / perAltitude);
      let r = 0;
      let g = 0;
      let bl = 0;
      for (let b = 0; b < BAKE_BIN_COUNT; b += 1) {
        const x = (out.bins[b]?.[i] ?? Number.NaN) * (perScattering[b]?.[a] ?? Number.NaN);
        r += (red[b] ?? Number.NaN) * x;
        g += (green[b] ?? Number.NaN) * x;
        bl += (blue[b] ?? Number.NaN) * x;
      }
      values[i * 3] = r;
      values[i * 3 + 1] = g;
      values[i * 3 + 2] = bl;
    }
    return {
      values,
      bins: light.bins.map((bin) => ({
        terms: bin.terms.length,
        largestFitError: bin.largestFitError,
        readFitError: bin.readFitError,
        crossedRows: bin.crossedRows,
      })),
      solves: out.solves,
      doubleForwardShare: out.leftOutShare,
      flooredSlantFactors: out.floored,
      largestBeamDeficit: out.deficit,
    };
  });
  const upward = grid.upwardCosines;
  const thermal =
    thermalBins === null
      ? undefined
      : thermalScatteringOf(prepared, thermalBins, perScattering, views);
  return {
    band: geometry,
    altitudesM: prepared.altitudesM.slice(),
    sunCosines: grid.sunCosines.slice(),
    travelCosines: Float64Array.from({ length: views }, (_, v) =>
      v < upward.length
        ? -(upward[upward.length - 1 - v] ?? Number.NaN)
        : (upward[v - upward.length] ?? Number.NaN),
    ),
    modes: THICK_BAKE_MODES,
    verticalOpticalDepth,
    slots,
    ...(thermal === undefined ? {} : { thermal }),
  };
}

/**
 * A band's J_th from its bins' E_th (the module's remarks, "Thermal emission"): each over its bin's
 * kernel scattering at the altitude, then V_cb into the channels, cd m⁻² per unit of σ_s,k.
 *
 * @throws Error for a grid whose travel cosines are not the read's {@link THERMAL_TRAVEL_COSINES}.
 */
function thermalScatteringOf(
  prepared: PreparedBake,
  thermalBins: ReadonlyArray<Float64Array>,
  perScattering: ReadonlyArray<Float64Array>,
  views: number,
): ThermalScattering {
  if (views !== THERMAL_TRAVEL_COSINES) {
    throw new Error(
      `the scattered thermal light is read at ${THERMAL_TRAVEL_COSINES} travel cosines, not ${views}`,
    );
  }
  const altitudes = prepared.altitudesM.length;
  const values = new Float32Array(altitudes * views * 3);
  for (let i = 0; i < altitudes * views; i += 1) {
    const a = Math.floor(i / views);
    const rgb = [0, 0, 0];
    for (let b = 0; b < BAKE_BIN_COUNT; b += 1) {
      const x = (thermalBins[b]?.[i] ?? Number.NaN) * (perScattering[b]?.[a] ?? Number.NaN);
      const weights = EMISSION_CHANNEL_WEIGHTS[b];
      if (weights === undefined) {
        throw new Error(`bin ${b} has no channel weights`);
      }
      for (const c of [0, 1, 2] as const) {
        rgb[c] = (rgb[c] ?? 0) + weights[c] * x;
      }
    }
    values.set(rgb, i * 3);
  }
  return { altitudesM: prepared.altitudesM.slice(), values };
}
