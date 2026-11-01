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
 * **Own wavelengths** (R08.T14.d.4's Titan-class gate; a haze route's bake alone,
 * {@link OwnChannels}). A channel may take its J from one more solve point instead, at the
 * channel's own wavelength (`medium.ts`'s `CHANNEL_WAVELENGTHS_NM`), its terms' optics there and its
 * gases' channel curves (`absorbers.ts`'s `AbsorberCurve`, the curves the kernels and the
 * references read) fitted as a bin's are, its J stored as it is. The conversion's signed weights
 * otherwise reach past the channel: the blue channel's are negative over 500–620 nm, where a
 * Titan-class column's methane absorbs and its haze's ω̄ rises, and a channel curve clamped at 1 there
 * (`absorbers.ts`'s `channelTransmittance`) reads no methane at all, so the bins' blue is bright
 * where the reference's is not. A channel takes its own wavelength only where every absorber's
 * channel curve, under every light, stays off the curves' floor out to {@link OWN_WAVELENGTH_COLUMNS}
 * times its vertical amount ({@link ownWavelengthChannels}): a curve at the floor is light that has
 * left Rec. 709 (methane's red past some 20 times a Titan-class column), which no sum of
 * exponentials carries and the bins' signed conversion does.
 *
 * **Curved streams** (R08.T14.d.4's Titan-class gate; a haze route's bake alone,
 * {@link BakeBandOptions.curvedStreams}). The column is plane-parallel, so a stream that reaches a
 * point from below at a shallow angle has crossed the whole column beneath it, where on the sphere
 * its ray, traced back, falls only to its tangent radius and climbs out again through thin air; and
 * one from above has crossed τ ÷ μ above it, where on the sphere it climbs more steeply through each
 * shell. Where the source is formed, each diffuse stream's intensity takes its curved path's share
 * of the plane-parallel path's opacity ({@link curvedStreamShares}), the first order in the
 * sphere's curvature of the radiance its path gathers: the plane-parallel diffuse field at altitude,
 * which a limb view reads, is otherwise lit from a full slab below (Loughman et al. 2004's limb
 * error, above). The beam and the solve are unchanged.
 *
 * **The horizon's first-order terms** (R08.T14.d.4's Titan-class gate; a haze route's bake alone,
 * {@link BakeBandOptions.horizonTerms}). A pseudo-spherical column holds one μ₀ at every height, so
 * it holds none of the diffuse light that crosses the terminator from the sunward air under a higher
 * sun, and its streams keep their cosines where on the sphere they turn: with the sun near the
 * horizon, R ÷ H some 120, the exact pseudo-spherical answer reads Titan-class's sky at 0.69–0.84
 * of its reference in red (the research behind the task, its reference arm A). In the column's frame
 * the sphere's streaming operator adds, to first order in 1 ÷ r, −(sin θ₀ ÷ r) √(1 − μ²) cos Δφ
 * ∂I ÷ ∂μ₀ (the sun's cosine moving with the sunward distance, Δφ = 0 travelling away from the sun),
 * ((1 − μ²) ÷ r) ∂I ÷ ∂μ (each stream's cosine rising along its ray) and −(cot θ₀ ÷ r) √(1 − μ²)
 * sin Δφ ∂I ÷ ∂Δφ (the frame turning about the vertical), the terms the pseudo-spherical
 * approximation drops (Dahlback and Stamnes 1991, above, cited not re-read: derived here from
 * Ω·∇ with the sun fixed in space). For each row of μ₀ ≤ {@link HORIZON_SUN_COSINE} the field is
 * then I + B, B the solve's own field of the first two applied to the row's I, a source per stream
 * and mode on the row's factorisation (`discreteOrdinates.ts`'s "Stream sources"): the lateral term (sin θ₀ ÷ β′r) √(1 − μ²) cos Δφ ∂I ÷ ∂μ₀ and the
 * curvature term −((1 − μ²) ÷ β′r) ∂I ÷ ∂μ, β′ the delta-M extinction a metre, each linear in τ
 * through a layer (its mean in one thinner than {@link HORIZON_THIN_DEPTH}). The third is left
 * out: it vanishes with the sun on the horizon. ∂ ÷ ∂μ₀ is taken across the table's own rows,
 * whose fields every row solves, so a row costs a source, not a solve. The terms are B's alone,
 * so B is linear in them, and the beam, the flux records and the thermal field are unchanged.
 *
 * In the thin air above some 100 km the plane-parallel field brightens as τ ÷ |μ| towards the
 * horizon, where 1 ÷ β′r passes 1 and the curvature term is no longer small: there its value follows
 * the resolution of ∂I ÷ ∂μ. It is taken over each stream set's own cosines, the nodes' over the
 * nodes and the view cosines' over the view cosines (lean, 2026-11-01: the research's "on the user
 * cosines"); over both sets together it moved Titan-class's s90 some 5% brighter, and the formal
 * solution's exact slope some 4% dimmer.
 *
 * **Spectral** (Design note 5): each absorber in each bin is an exponential sum fitted to its bin
 * curve (`expSum.ts`), each layer reading k at its own Curtis–Godson pressure; several absorbers in
 * a bin overlap at random (the product of their sums, Lacis and Oinas 1991), one solve a product
 * term, the weakest taken as one term each past {@link THICK_BAKE_MAX_TERMS_PER_BIN}.
 *
 * **Bin shares** (R08.T14.d's bake time): a band's bins are independent until their conversion to
 * the channels, so a preparation of some bins alone ({@link prepareBake}'s `bins`) and its
 * {@link bakeBins} give those bins' E as the whole band's bake does, and {@link gatherBins} of
 * shares holding every bin once is {@link bakeMultipleScattering}'s table to the bit.
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
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberBinCurves,
  type AbsorberCurve,
  absorberCurvesBySun,
  type AbsorberSpec,
} from "../absorbers";
import { gaussLegendre } from "../irradianceQuadrature";
import {
  ABSORBER_PRESSURE_REFERENCE_PA,
  BAKE_BIN_COUNT,
  BAKE_WAVELENGTHS_NM,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  densityAt,
  type RenderChannel,
  type TabulatedDensity,
} from "../medium";
import { bandLatitudeRad, type BodyGravity, principalRadiiM, type SpheroidFigure } from "../oblate";
import type { BakeSpectrum } from "../spectralColour";
import { type BinConversion, binConversion } from "./binConversion";
import {
  DISCRETE_ORDINATE_STREAMS,
  type DiscreteOrdinatesProblem,
  type DiscreteOrdinatesSolution,
  doubleGaussQuadrature,
  type GroundCosines,
  groundCosinesOf,
  type GroundReflectance,
  type ModeIntensities,
  normalizedAssociatedLegendre,
  type OrdinateLayer,
  solveDiscreteOrdinates,
  solveDiscreteOrdinatesWithSources,
  type StreamSource,
  type StreamSourceMode,
} from "./discreteOrdinates";
import {
  EXP_SUM_OPTIONS,
  expSumCoefficientsAt,
  expSumRowsAt,
  type ExpSumFit,
  type ExpSumOptions,
  fitExpSum,
} from "./expSum";
import { type SphericalBeam, sphericalBeamPath, sphericalSlantDepths } from "./sphericalBeam";
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
  type SpectralTermOptics,
} from "./spectralColumn";
import { expm1, log } from "../../../lib/pinnedMath";

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
  /**
   * The same terms' channel curves under this light, in the same order (`absorbers.ts`'s
   * `AbsorberCurve`), which a channel at its own wavelength takes (the module's remarks, "Own
   * wavelengths"); absent, no channel may.
   */
  readonly channels?: ReadonlyArray<AbsorberCurve>;
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
    channels: curves.map((bySun) => {
      const channels = bySun[j]?.channels;
      if (channels === undefined) {
        throw new RangeError(`an absorber has no channel curves for light ${j}`);
      }
      return channels;
    }),
  }));
}

/**
 * The render channels a bake solves at their own wavelengths, red, green and blue (the module's
 * remarks, "Own wavelengths"): each such channel's J is its solve point's
 * ({@link ownChannelPoint}), the others' the bins' converted.
 */
export type OwnChannels = readonly [boolean, boolean, boolean];

/** No channel at its own wavelength: every bake's but a haze route's (R08.T14.d.4). */
export const NO_OWN_CHANNELS: OwnChannels = [false, false, false];

/** The solve point of channel `c` at its own wavelength: after the 15 bins, 15 + c. */
export function ownChannelPoint(c: RenderChannel): number {
  return BAKE_BIN_COUNT + c;
}

/**
 * The columns out to which a channel's curves stay off their floor for the channel to take its own
 * wavelength, in units of each curve's vertical amount U: 100 U, over three times the grazing beam's
 * column on a body of R ÷ H 120 (Chapman's √(2πR ÷ H), some 28 U), past the columns a bake's beams
 * cross (the module's remarks, "Own wavelengths").
 */
export const OWN_WAVELENGTH_COLUMNS = 100;

/**
 * Which channels `lights` let a bake solve at their own wavelengths (the module's remarks, "Own
 * wavelengths"): each channel in which every absorber's channel curve under every light stays above
 * `absorbers.ts`'s `ABSORBER_TRANSMITTANCE_FLOOR` at every pressure row and node out to
 * {@link OWN_WAVELENGTH_COLUMNS} U, the curve's middle node; none for lights without channel curves.
 */
export function ownWavelengthChannels(lights: ReadonlyArray<ThickBakeLight>): OwnChannels {
  const floorDepth = -log(ABSORBER_TRANSMITTANCE_FLOOR);
  const own = (c: RenderChannel): boolean =>
    lights.length > 0 &&
    lights.every(
      (light) =>
        light.channels !== undefined &&
        light.channels.every((curve) => {
          const nodes = curve.logColumns.length;
          const vertical = curve.logColumns[(nodes - 1) >> 1] ?? Number.NaN;
          const reach = vertical + log(OWN_WAVELENGTH_COLUMNS);
          const depths = curve.opticalDepth[c];
          const rows = depths.length / nodes;
          for (let r = 0; r < rows; r += 1) {
            for (let j = 0; j < nodes && (curve.logColumns[j] ?? Number.NaN) <= reach; j += 1) {
              if (!((depths[r * nodes + j] ?? Number.NaN) < floorDepth)) {
                return false;
              }
            }
          }
          return true;
        }),
    );
  return [own(0), own(1), own(2)];
}

/** The solve points of a bake with `own` channels: every bin, then each own channel's point. */
export function bakePointsOf(own: OwnChannels): ReadonlyArray<number> {
  return [
    ...EVERY_BAKE_BIN,
    ...([0, 1, 2] as const).filter((c) => own[c]).map((c) => ownChannelPoint(c)),
  ];
}

/** The channel whose own wavelength's point `point` is ({@link ownChannelPoint}), or `null` for a bin. */
function ownPointChannel(point: number): RenderChannel | null {
  switch (point - BAKE_BIN_COUNT) {
    case 0:
      return 0;
    case 1:
      return 1;
    case 2:
      return 2;
    default:
      return null;
  }
}

/** A solve point's wavelength, nm: a bin's middle, or a channel's own (the module's remarks). */
function pointWavelengthNm(point: number): number {
  return point < BAKE_BIN_COUNT
    ? (BAKE_WAVELENGTHS_NM[point] ?? Number.NaN)
    : (CHANNEL_WAVELENGTHS_NM[point - BAKE_BIN_COUNT] ?? Number.NaN);
}

/**
 * A light's conversion with its `own` channels at their points: the bins' converted factors for the
 * others, 0 at the points, and each own channel's point alone; the conversion itself where none is.
 */
function withOwnChannels(conversion: BinConversion, own: OwnChannels): BinConversion {
  if (!own.some(Boolean)) {
    return conversion;
  }
  const factorsOf = (c: RenderChannel): Float64Array => {
    const out = new Float64Array(BAKE_BIN_COUNT + 3);
    if (own[c]) {
      out[ownChannelPoint(c)] = 1;
    } else {
      out.set(conversion.factors[c]);
    }
    return out;
  };
  return {
    factors: [factorsOf(0), factorsOf(1), factorsOf(2)],
    guarded: [
      conversion.guarded[0] && !own[0],
      conversion.guarded[1] && !own[1],
      conversion.guarded[2] && !own[2],
    ],
  };
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
   * The bins prepared, ascending: every bin ({@link EVERY_BAKE_BIN}), or a share of them, whose
   * optics, points and fits alone the arrays by bin hold (R08.T14.d's bake time).
   */
  readonly bins: ReadonlyArray<number>;
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
function verticalBinDepth(curves: PointCurve, row: number): number {
  const nodes = curves.logColumns.length;
  return curves.opticalDepth[row * nodes + ((nodes - 1) >> 1)] ?? 0;
}

/** An absorber's curve at one solve point: a bin's, or a channel's at its own wavelength. */
interface PointCurve {
  readonly logColumns: Float64Array;
  readonly logPressures: Float64Array;
  readonly readTopLogPressure: number;
  readonly transmittance: Float64Array;
  readonly opticalDepth: Float64Array;
}

/**
 * Absorber `a`'s curve under `light` at solve point `point` (the module's remarks, "Own
 * wavelengths"), or `undefined` where the light holds none there.
 */
function pointCurveOf(light: ThickBakeLight, a: number, point: number): PointCurve | undefined {
  if (point < BAKE_BIN_COUNT) {
    const curves = light.absorbers[a];
    const transmittance = curves?.transmittance[point];
    const opticalDepth = curves?.opticalDepth[point];
    return curves === undefined || transmittance === undefined || opticalDepth === undefined
      ? undefined
      : { ...curves, transmittance, opticalDepth };
  }
  const curve = light.channels?.[a];
  const c = ownPointChannel(point);
  return curve === undefined || c === null
    ? undefined
    : {
        logColumns: curve.logColumns,
        logPressures: curve.logPressures,
        readTopLogPressure: curve.readTopLogPressure,
        transmittance: curve.transmittance[c],
        opticalDepth: curve.opticalDepth[c],
      };
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
      const curves = pointCurveOf(light, a, b);
      const absorber = absorberTerms[a];
      if (curves === undefined || absorber === undefined) {
        throw new RangeError(`absorber ${a} has no curve in bin ${b}`);
      }
      return fitExpSum(
        {
          logColumns: curves.logColumns,
          logPressures: curves.logPressures,
          readTopLogPressure: curves.readTopLogPressure,
          transmittance: curves.transmittance,
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
        const curves = pointCurveOf(light, a, b);
        return curves === undefined ? 0 : verticalBinDepth(curves, fits[a]?.referenceRow ?? 0);
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
 * @param bins - The bins prepared, ascending: every bin, or a share of them for a job that bakes
 *   those alone ({@link bakeBins}), the reference bin's optics taken for the layers whatever; and
 *   after them, of `own`'s channels, their points ({@link ownChannelPoint}).
 * @param own - The channels solved at their own wavelengths (the module's remarks, "Own
 *   wavelengths"), whose points the conversion reads in every share: none by default.
 * @throws RangeError for a grid that is not whole, a light whose absorbers do not match the
 *   column's absorber terms in count and kind, a curve with pressure rows on a column with no
 *   pressure profile, a `layout` whose table does not hold the grid's layers or whose fit does not
 *   begin with them, bins that are not ascending whole bins each once or points of channels not
 *   `own`, an `own` channel under a light with no channel curves, or as the column's optics refuse.
 */
export function prepareBake(
  column: SpectralColumn,
  lights: ReadonlyArray<ThickBakeLight>,
  grid: ThickBakeGrid = THICK_BAKE_GRID,
  options: ExpSumOptions = EXP_SUM_OPTIONS,
  layout?: BakeLayout,
  bins: ReadonlyArray<number> = EVERY_BAKE_BIN,
  own: OwnChannels = NO_OWN_CHANNELS,
): PreparedBake {
  checkBakeInputs(column, lights, grid);
  if (layout !== undefined) {
    checkBakeLayout(layout, grid);
  }
  checkBakeBins(bins, own);
  if (own.some(Boolean) && lights.some((light) => light.channels === undefined)) {
    throw new RangeError(
      "a bake's channels at their own wavelengths take every light's channel curves",
    );
  }
  // The solve points the arrays by bin hold: the bins, and the channels' after them where any is own.
  const pointCount = own.some(Boolean) ? BAKE_BIN_COUNT + 3 : BAKE_BIN_COUNT;
  const whole = bins.length === BAKE_BIN_COUNT;
  // The bins prepared, and the reference bin where the layers are placed by it.
  const needed =
    whole || layout !== undefined
      ? bins
      : [...new Set([...bins, THICK_BAKE_REFERENCE_BIN])].toSorted((p, q) => p - q);
  const opticsOf = spectralOptics(column, needed.map(pointWavelengthNm));
  const optics = Array.from(
    { length: pointCount },
    (_, b): ReadonlyArray<SpectralTermOptics> => opticsOf[needed.indexOf(b)] ?? [],
  );
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
  const pointOptics = binOptics.map((terms, b) =>
    !bins.includes(b)
      ? []
      : Array.from(altitudesM, (h): PointOptics => {
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
  const fitted = bakeLightBins(column, layout?.fitLayering ?? layering, lights, bins, options);
  const { temperature } = column;
  const interfacePlanck =
    temperature === undefined
      ? undefined
      : Array.from({ length: pointCount }, (_, b) =>
          Float64Array.from(interfaceHeightsM, (h) =>
            planckRadiance(pointWavelengthNm(b), mediumTemperatureK(temperature, h)),
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
    groundAlbedo: Float64Array.from({ length: pointCount }, (_, b) =>
      binGroundAlbedo(column.groundAlbedo, pointWavelengthNm(b)),
    ),
    layerColumnsM,
    pointOptics,
    lights: lights.map((light, j): PreparedLight => {
      const fittedBins = fitted[j] ?? [];
      // A share's bins at their own indices, the others holes that no bake reads.
      const byBin: BakeLightBin[] = [];
      for (const [i, b] of bins.entries()) {
        const bin = fittedBins[i];
        if (bin !== undefined) {
          byBin[b] = bin;
        }
      }
      return {
        conversion: withOwnChannels(binConversion(light.spectrum), own),
        bins: whole ? fittedBins : byBin,
      };
    }),
    bins,
  };
}

/** Every bake bin, ascending: the bins a whole band's bake prepares and solves. */
export const EVERY_BAKE_BIN: ReadonlyArray<number> = Array.from(
  { length: BAKE_BIN_COUNT },
  (_, b) => b,
);

/**
 * Refuses a share of bake bins that is not ascending whole bins, each once, at least one, with only
 * `own`'s channels' points after them.
 */
function checkBakeBins(bins: ReadonlyArray<number>, own: OwnChannels): void {
  const isPoint = (b: number): boolean => {
    const c = ownPointChannel(b);
    return b < BAKE_BIN_COUNT || (c !== null && own[c]);
  };
  if (
    bins.length === 0 ||
    bins.some(
      (b, i) =>
        !Number.isInteger(b) || b < 0 || !isPoint(b) || (i > 0 && !(b > (bins[i - 1] ?? b))),
    )
  ) {
    throw new RangeError(
      `a bake's bins are ascending, each once, of 0 to ${BAKE_BIN_COUNT - 1} and its own channels' points, not ${bins.join(", ")}`,
    );
  }
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

/**
 * A term's optics at relative density 1 at one wavelength, as the bake mixes a layer by them: its
 * bins', a deck's, or the correction's a₁ at a channel.
 */
export interface LayerTermOptics {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
  readonly forwardDelta: number;
  /** χ₀ … χ_NSTR at least. */
  readonly moments: ArrayLike<number>;
}

/**
 * A run of a column's layers in one bin of a band, their terms mixed by their scattering (plan R08,
 * R08.T15.a.1's "Two layer mixers": the bake's and the deck's one): each term's column times the
 * band's factor, term by term in the column's order.
 */
export interface MixedLayers {
  /** Each layer's vertical extinction optical depth before the gases' sums, in the band's frame. */
  readonly extinction: Float64Array;
  /** Its scattering optical depth. */
  readonly scattering: Float64Array;
  /**
   * Its mixed χ₀ … χ_NSTR, scattering-weighted, row-major `[p × (NSTR + 1) + l]`: χ₀ = 1 and the
   * rest 0 where it scatters nothing.
   */
  readonly moments: Float64Array;
  /** Its f_c, scattering-weighted; 0 where it scatters nothing. */
  readonly forwardDelta: Float64Array;
  /** Per layer, each term's share of its scattering, `[p][t]`, all 0 where it scatters nothing. */
  readonly scatteringShares: ReadonlyArray<Float64Array>;
}

/**
 * The {@link MixedLayers} of `layerCount` layers from `terms`' optics in one bin.
 *
 * @param layerColumnsM - Per term, per layer from the top, ∫ρ dh over it, m.
 * @param factor - The band's factor, 1 ÷ s_b.
 * @param streams - NSTR, whose χ_NSTR the moments carry for delta-M.
 */
export function mixedLayers(
  terms: ReadonlyArray<LayerTermOptics>,
  layerColumnsM: ReadonlyArray<Float64Array>,
  layerCount: number,
  factor: number,
  streams: number = THICK_BAKE_STREAMS,
): MixedLayers {
  const extinction = new Float64Array(layerCount);
  const scattering = new Float64Array(layerCount);
  const moments = new Float64Array(layerCount * (streams + 1));
  const forwardDelta = new Float64Array(layerCount);
  const scatteringShares: Float64Array[] = [];
  for (let p = 0; p < layerCount; p += 1) {
    let tau = 0;
    let tauS = 0;
    let delta = 0;
    const shares = new Float64Array(terms.length);
    for (const [t, o] of terms.entries()) {
      const c = (layerColumnsM[t]?.[p] ?? Number.NaN) * factor;
      tau += (o.scatteringPerM + o.absorptionPerM) * c;
      const s = o.scatteringPerM * c;
      if (s > 0) {
        tauS += s;
        delta += s * o.forwardDelta;
        shares[t] = s;
        for (let l = 0; l <= streams; l += 1) {
          const k = p * (streams + 1) + l;
          moments[k] = (moments[k] ?? 0) + s * (o.moments[l] ?? Number.NaN);
        }
      }
    }
    extinction[p] = tau;
    scattering[p] = tauS;
    forwardDelta[p] = tauS > 0 ? delta / tauS : 0;
    for (let l = 0; l <= streams; l += 1) {
      const k = p * (streams + 1) + l;
      moments[k] = tauS > 0 ? (moments[k] ?? 0) / tauS : l === 0 ? 1 : 0;
    }
    if (tauS > 0) {
      for (let t = 0; t < shares.length; t += 1) {
        shares[t] = (shares[t] ?? 0) / tauS;
      }
    }
    scatteringShares.push(shares);
  }
  return { extinction, scattering, moments, forwardDelta, scatteringShares };
}

/** Mixed layers delta-M scaled for one term of a bin's gases ({@link deltaMLayersOf}). */
export interface DeltaMLayers {
  /** The delta-M layers, from the top down. */
  readonly layers: ReadonlyArray<OrdinateLayer>;
  /** Their optical depths, `layers`' own. */
  readonly scaledDepths: Float64Array;
  /** Each layer's depth as the kernels march it: the gases' and the extinction less the forward delta. */
  readonly kernelDepths: Float64Array;
  /** Each layer's truncated fraction f ({@link DeltaMLayer.truncated}). */
  readonly truncated: Float64Array;
}

/**
 * Mixed layers with one term of a bin's gases added, then delta-M by {@link deltaMLayer} at
 * `streams`, layer by layer.
 *
 * @param absorptionDepth - The term's gases' depth per layer of the whole column, in the reference
 *   frame: the run's layers are its entries from `offset` on, each times the band's `factor`.
 */
export function deltaMLayersOf(
  mixed: MixedLayers,
  factor: number,
  absorptionDepth: ArrayLike<number>,
  offset: number,
  streams: number = THICK_BAKE_STREAMS,
): DeltaMLayers {
  const layerCount = mixed.extinction.length;
  const layers: OrdinateLayer[] = [];
  const scaledDepths = new Float64Array(layerCount);
  const kernelDepths = new Float64Array(layerCount);
  const truncated = new Float64Array(layerCount);
  for (let p = 0; p < layerCount; p += 1) {
    const tau =
      (mixed.extinction[p] ?? Number.NaN) + factor * (absorptionDepth[offset + p] ?? Number.NaN);
    const tauS = mixed.scattering[p] ?? Number.NaN;
    const delta = mixed.forwardDelta[p] ?? Number.NaN;
    const scaled = deltaMLayer(
      tau,
      tauS,
      mixed.moments.subarray(p * (streams + 1), (p + 1) * (streams + 1)),
      delta,
      streams,
    );
    layers.push(scaled.layer);
    scaledDepths[p] = scaled.layer.opticalDepth;
    kernelDepths[p] = Math.max(tau - tauS * delta, 0);
    truncated[p] = scaled.truncated;
  }
  return { layers, scaledDepths, kernelDepths, truncated };
}

/** A band's layers in one bin, mixed. */
function bandBinOf(prepared: PreparedBake, bin: number, factor: number): MixedLayers {
  return mixedLayers(
    prepared.binOptics[bin] ?? [],
    prepared.layerColumnsM,
    prepared.grid.layers,
    factor,
  );
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
   * Its node intensities and fluxes are made at the table's altitudes' interfaces alone, the
   * ground's among them (NaN at the others); its user intensities at every interface.
   */
  readonly onSolve?: (bin: number, term: BinTerm, solve: BinSolve) => void;
  /**
   * The azimuthal modes each solve takes, its ground's among them: {@link THICK_BAKE_MODES} by
   * default, more for a reader of the solves' own fields (R08.T15.a.2's test-only option, the deck's
   * sky-lit radiance at its own modes). The table holds J to {@link THICK_BAKE_MODES} whatever.
   */
  readonly solveModes?: number;
  /**
   * Whether each diffuse stream takes its curved path's share where the source is formed, as a haze
   * route's bake does (the module's remarks, "Curved streams"; R08.T14.d.4): none by default.
   */
  readonly curvedStreams?: boolean;
  /**
   * The horizon's first-order spherical terms each row of μ₀ ≤ {@link HORIZON_SUN_COSINE} takes, as
   * a haze route's bake does (the module's remarks, "The horizon's first-order terms"; R08.T14.d.4):
   * none by default.
   */
  readonly horizonTerms?: HorizonTerms;
}

/**
 * The horizon's first-order spherical terms (the module's remarks, "The horizon's first-order
 * terms"; R08.T14.d.4), each taken or not.
 */
export interface HorizonTerms {
  /** The lateral term: the sun's cosine moving across the sunward distance, through ∂I ÷ ∂μ₀. */
  readonly lateral: boolean;
  /** The curvature term: each stream's cosine moving along its own ray, through ∂I ÷ ∂μ. */
  readonly curvature: boolean;
}

/** Both of the horizon's terms, as a haze route's bake takes them. */
export const HORIZON_TERMS: HorizonTerms = { lateral: true, curvature: true };

/**
 * The largest μ₀ whose rows take the horizon's terms: about cos 70°, the rows from s70 to the
 * table's last below the horizontal, where the pseudo-spherical field misses its reference
 * (R08.T14.d.4's research: the terms go as sin θ₀ ∂ln I ÷ ∂μ₀ ÷ R, small at a higher sun).
 */
export const HORIZON_SUN_COSINE = 0.35;

/**
 * The delta-M optical depth below which a layer takes its horizon source's mean at both faces in
 * place of its linear run (`horizonSources`): the source a unit of τ is G ÷ β′, so in a thin layer
 * its slope grows as 1 ÷ τ² in the particular solution, whose faces then cancel to ε ÷ τ² of it,
 * 10⁻⁸ here, where a thin layer's own gain ∫Q dt ÷ μ is its mean's to (τ ÷ μ)². Measured
 * 2026-11-01 on Titan-class air: the terms' sum missed their fields' sum by 2.1 × 10⁻⁵ of the
 * table's largest J with every layer linear, and by 3.8 × 10⁻⁸, the f32 table's rounding, at
 * 10⁻⁴ and at 10⁻².
 */
export const HORIZON_THIN_DEPTH = 1e-4;

/**
 * Each diffuse stream's curved path's share of the plane-parallel path's opacity at every interface
 * from the top down (the module's remarks, "Curved streams"), `[q × cosines + j]`: for a stream
 * travelling `up` at μ_j, (1 − e^(−τ_c)) ÷ (1 − e^(−τ_↓ ÷ μ_j)), τ_c the optical depth of its ray
 * traced back through the shells, down to its tangent radius and up to the top (`sphericalBeam.ts`'s
 * path at −μ_j, below the horizontal) and τ_↓ the column below the interface, and 1 where that ray
 * meets the ground, crossing the column the plane-parallel path crosses; for one travelling `down`,
 * (1 − e^(−τ_s)) ÷ (1 − e^(−τ_↑ ÷ μ_j)), τ_s the climbing ray's to the top at μ_j and τ_↑ the
 * column above. 1 where the plane-parallel path holds no depth.
 *
 * @param radiusM - The band's ground sphere, m.
 * @param interfaceHeightsM - The interfaces' h\*, m, from the top down: L + 1.
 * @param depths - Each layer's vertical optical depth from the top, the solve's: L.
 * @param cosines - The streams' cosines, each in (0, 1].
 * @param interfaces - The interfaces whose shares are made, indices from the top; every one by
 *   default. The others hold 1.
 */
export function curvedStreamShares(
  radiusM: number,
  interfaceHeightsM: Float64Array,
  depths: Float64Array,
  cosines: ArrayLike<number>,
  travel: "up" | "down",
  interfaces: ArrayLike<number> = Array.from({ length: depths.length + 1 }, (_, q) => q),
): Float64Array {
  const layers = depths.length;
  const count = cosines.length;
  // The plane-parallel column below each interface, or above it.
  const column = new Float64Array(layers + 1);
  switch (travel) {
    case "up":
      for (let q = layers - 1; q >= 0; q -= 1) {
        column[q] = (column[q + 1] ?? 0) + (depths[q] ?? Number.NaN);
      }
      break;
    case "down":
      for (let q = 1; q <= layers; q += 1) {
        column[q] = (column[q - 1] ?? 0) + (depths[q - 1] ?? Number.NaN);
      }
      break;
  }
  const shares = new Float64Array((layers + 1) * count).fill(1);
  for (let j = 0; j < count; j += 1) {
    const mu = cosines[j] ?? Number.NaN;
    const slants = sphericalSlantDepths(
      radiusM,
      interfaceHeightsM,
      depths,
      travel === "up" ? -mu : mu,
      interfaces,
    );
    for (let i = 0; i < interfaces.length; i += 1) {
      const q = interfaces[i] ?? Number.NaN;
      const flat = -expm1(-(column[q] ?? 0) / mu);
      const slant = slants[i] ?? Number.POSITIVE_INFINITY;
      if (flat > 0 && Number.isFinite(slant)) {
        shares[q * count + j] = -expm1(-slant) / flat;
      }
    }
  }
  return shares;
}

/** x + y entry by entry; NaN where y is absent. */
function sumOf(x: Float64Array, y: Float64Array | undefined): Float64Array {
  const out = new Float64Array(x.length);
  for (let i = 0; i < x.length; i += 1) {
    out[i] = (x[i] ?? Number.NaN) + (y?.[i] ?? Number.NaN);
  }
  return out;
}

/** What the horizon rows' terms read of a band's solve (`horizonSources`). */
interface HorizonGeometry {
  /** The band's ground sphere, m. */
  readonly radiusM: number;
  /** The interfaces' h\*, m, from the top down: L + 1. */
  readonly interfaceHeightsM: Float64Array;
  /** Each layer's delta-M optical depth from the top, the solve's: L. */
  readonly depths: Float64Array;
  readonly sunCosines: Float64Array;
  readonly upwardCosines: Float64Array;
  /** The interfaces whose node fields the solve makes, the table's. */
  readonly read: ReadonlyArray<number>;
}

/**
 * The weights of the slope at point `i` of ascending abscissae `xs` by Lagrange's parabola through
 * three of them, `i` and its neighbours (or the first or last three at an end): `[index, weight]`.
 *
 * @throws RangeError for fewer than three abscissae, or two that are equal.
 */
function slopeStencil(xs: ArrayLike<number>, i: number): ReadonlyArray<readonly [number, number]> {
  if (xs.length < 3) {
    throw new RangeError(`a slope by three points needs three abscissae, not ${xs.length}`);
  }
  const first = Math.min(Math.max(i - 1, 0), xs.length - 3);
  const index = [first, first + 1, first + 2] as const;
  const x = index.map((j) => xs[j] ?? Number.NaN);
  const at = xs[i] ?? Number.NaN;
  return index.map((j, a): readonly [number, number] => {
    const [b, c] = [0, 1, 2].filter((other) => other !== a);
    const xa = x[a] ?? Number.NaN;
    const xb = x[b ?? -1] ?? Number.NaN;
    const xc = x[c ?? -1] ?? Number.NaN;
    if (xa === xb || xa === xc) {
      throw new RangeError(`a slope by three points needs distinct abscissae, not ${xa} twice`);
    }
    return [j, (at - xb + (at - xc)) / ((xa - xb) * (xa - xc))];
  });
}

/**
 * The horizon rows' terms as the sources of their first-order fields B (the module's remarks, "The
 * horizon's first-order terms"), one a row of `rows`, per mode: at each table interface k, on every
 * node and user stream of signed cosine μ, G = (sin θ₀ ÷ r_k) √(1 − μ²) Σ_j c_mj ∂Iʲ ÷ ∂μ₀ −
 * ((1 − μ²) ÷ r_k) ∂Iᵐ ÷ ∂μ, c the modes' coupling through cos Δφ (½ each to m ± 1, and all of mode 0
 * into mode 1), linear in τ between the table's interfaces; in each layer G ÷ β′, β′ its delta-M
 * extinction a metre, at both faces its mean where the layer is thinner than
 * {@link HORIZON_THIN_DEPTH}. ∂ ÷ ∂μ₀ is taken across the table's own rows, and ∂ ÷ ∂μ over each
 * hemisphere's nodes for a node and over its view cosines for a view cosine, each by Lagrange's
 * parabola through three of them.
 *
 * @throws RangeError for a grid of fewer than three rows or three view cosines.
 */
function horizonSources(
  solution: DiscreteOrdinatesSolution,
  rows: ReadonlyArray<number>,
  terms: HorizonTerms,
  geometry: HorizonGeometry,
): StreamSource[] {
  const { radiusM, interfaceHeightsM, depths, sunCosines, upwardCosines, read } = geometry;
  const { nodes } = solution.quadrature;
  const n = nodes.length;
  const users = upwardCosines.length;
  const span = 2 * n;
  const userSpan = 2 * users;
  // Every stream: the nodes +μ, −μ, then the view cosines +μ, −μ.
  const streams = span + userSpan;
  const layers = depths.length;
  const modes = solution.beams[rows[0] ?? 0]?.modes.length ?? 0;
  const cosineOf = (e: number): number => {
    const hemisphere = e < span ? e % n : (e - span) % users;
    return e < span ? (nodes[hemisphere] ?? Number.NaN) : (upwardCosines[hemisphere] ?? Number.NaN);
  };
  const upward = (e: number): boolean => (e < span ? e < n : e - span < users);
  // ∂ ÷ ∂μ in the signed μ, each hemisphere's nodes over its nodes and its view cosines over its
  // view cosines, by |μ|: three entries and weights a stream, the downward hemisphere's negated.
  const along = new Int32Array(streams * 3);
  const alongWeights = new Float64Array(streams * 3);
  for (const [up, nodal] of [
    [true, true],
    [false, true],
    [true, false],
    [false, false],
  ] as const) {
    const hemisphere = Array.from({ length: streams }, (_, e) => e)
      .filter((e) => upward(e) === up && e < span === nodal)
      .toSorted((x, y) => cosineOf(x) - cosineOf(y));
    const cosines = hemisphere.map(cosineOf);
    hemisphere.forEach((e, i) => {
      slopeStencil(cosines, i).forEach(([j, w], t) => {
        along[e * 3 + t] = hemisphere[j] ?? -1;
        alongWeights[e * 3 + t] = up ? w : -w;
      });
    });
  }
  // Each row's field at the table's interfaces, `[(m × (L + 1) + k) × streams + e]`, made once for
  // every row a slope reads.
  const fields = new Map<number, Float64Array>();
  const fieldOf = (row: number): Float64Array => {
    const held = fields.get(row);
    if (held !== undefined) {
      return held;
    }
    const field = new Float64Array(modes * (layers + 1) * streams);
    for (let m = 0; m < modes; m += 1) {
      const mode = solution.beams[row]?.modes[m];
      if (mode === undefined) {
        throw new Error(`row ${row} has no mode ${m}`);
      }
      for (const k of read) {
        const base = (m * (layers + 1) + k) * streams;
        field.set(mode.quadratureUp.subarray(k * n, (k + 1) * n), base);
        field.set(mode.quadratureDown.subarray(k * n, (k + 1) * n), base + n);
        field.set(mode.userUp.subarray(k * users, (k + 1) * users), base + span);
        field.set(mode.userDown.subarray(k * users, (k + 1) * users), base + span + users);
      }
    }
    fields.set(row, field);
    return field;
  };
  const roots = Float64Array.from({ length: streams }, (_, e) => {
    const mu = cosineOf(e);
    return Math.sqrt(1 - mu * mu);
  });
  // τ from the top at each interface, and each layer's metres a unit of its optical depth.
  const tau = new Float64Array(layers + 1);
  for (let p = 0; p < layers; p += 1) {
    tau[p + 1] = (tau[p] ?? Number.NaN) + (depths[p] ?? Number.NaN);
  }
  const perDepth = Float64Array.from(depths, (d, p) => {
    const thickness =
      (interfaceHeightsM[p] ?? Number.NaN) - (interfaceHeightsM[p + 1] ?? Number.NaN);
    return d > 0 ? thickness / d : 0;
  });
  // Each interface's bracketing table interfaces and its share between them in τ.
  const made = new Uint8Array(layers + 1);
  for (const k of read) {
    made[k] = 1;
  }
  const above = new Int32Array(layers + 1);
  const below = new Int32Array(layers + 1);
  const shares = new Float64Array(layers + 1);
  for (let k = 0; k <= layers; k += 1) {
    let a = k;
    while (a > 0 && made[a] !== 1) {
      a -= 1;
    }
    let b = k;
    while (b < layers && made[b] !== 1) {
      b += 1;
    }
    above[k] = a;
    below[k] = b;
    const range = (tau[b] ?? Number.NaN) - (tau[a] ?? Number.NaN);
    shares[k] = range > 0 ? ((tau[k] ?? Number.NaN) - (tau[a] ?? Number.NaN)) / range : 0;
  }
  const slopes = new Float64Array(modes * streams);
  const g = new Float64Array(modes * (layers + 1) * streams);
  return rows.map((s): StreamSource => {
    const mu0 = sunCosines[s] ?? Number.NaN;
    const sinSun = Math.sqrt(Math.max(1 - mu0 * mu0, 0));
    const acrossRows = slopeStencil(sunCosines, s).map(([row, w]) => [fieldOf(row), w] as const);
    const own = fieldOf(s);
    g.fill(0);
    for (const k of read) {
      const r = radiusM + (interfaceHeightsM[k] ?? Number.NaN);
      // ∂Iʲ ÷ ∂μ₀ at k, `[j × streams + e]`.
      slopes.fill(0);
      for (const [field, w] of acrossRows) {
        for (let j = 0; j < modes; j += 1) {
          const base = (j * (layers + 1) + k) * streams;
          for (let e = 0; e < streams; e += 1) {
            slopes[j * streams + e] = (slopes[j * streams + e] ?? 0) + w * (field[base + e] ?? 0);
          }
        }
      }
      for (let m = 0; m < modes; m += 1) {
        const base = (m * (layers + 1) + k) * streams;
        for (let e = 0; e < streams; e += 1) {
          const root = roots[e] ?? Number.NaN;
          let value = 0;
          if (terms.lateral) {
            let coupled = 0;
            if (m >= 1) {
              coupled += (m === 1 ? 1 : 0.5) * (slopes[(m - 1) * streams + e] ?? 0);
            }
            if (m + 1 < modes) {
              coupled += 0.5 * (slopes[(m + 1) * streams + e] ?? 0);
            }
            value += (sinSun / r) * root * coupled;
          }
          if (terms.curvature) {
            let slope = 0;
            for (let t = 0; t < 3; t += 1) {
              slope +=
                (alongWeights[e * 3 + t] ?? 0) *
                (own[base + (along[e * 3 + t] ?? 0)] ?? Number.NaN);
            }
            value -= ((root * root) / r) * slope;
          }
          g[base + e] = value;
        }
      }
    }
    // Linear in τ between the table's interfaces.
    for (let k = 0; k <= layers; k += 1) {
      if (made[k] === 1) {
        continue;
      }
      const share = shares[k] ?? Number.NaN;
      for (let m = 0; m < modes; m += 1) {
        const at = (m * (layers + 1) + k) * streams;
        const a = (m * (layers + 1) + (above[k] ?? 0)) * streams;
        const b = (m * (layers + 1) + (below[k] ?? 0)) * streams;
        for (let e = 0; e < streams; e += 1) {
          const top = g[a + e] ?? Number.NaN;
          g[at + e] = top + share * ((g[b + e] ?? Number.NaN) - top);
        }
      }
    }
    return Array.from({ length: modes }, (_, m): StreamSourceMode => {
      const source = {
        nodesTop: new Float64Array(layers * span),
        nodesBottom: new Float64Array(layers * span),
        usersTop: new Float64Array(layers * userSpan),
        usersBottom: new Float64Array(layers * userSpan),
      };
      for (let p = 0; p < layers; p += 1) {
        const factor = perDepth[p] ?? Number.NaN;
        const top = (m * (layers + 1) + p) * streams;
        const bottom = top + streams;
        // A thin layer takes its mean (HORIZON_THIN_DEPTH).
        const thin = (depths[p] ?? 0) < HORIZON_THIN_DEPTH;
        const face = (at: number, other: number): number =>
          factor *
          (thin ? 0.5 * ((g[at] ?? Number.NaN) + (g[other] ?? Number.NaN)) : (g[at] ?? Number.NaN));
        for (let e = 0; e < span; e += 1) {
          source.nodesTop[p * span + e] = face(top + e, bottom + e);
          source.nodesBottom[p * span + e] = face(bottom + e, top + e);
        }
        for (let e = 0; e < userSpan; e += 1) {
          source.usersTop[p * userSpan + e] = face(top + span + e, bottom + span + e);
          source.usersBottom[p * userSpan + e] = face(bottom + span + e, top + span + e);
        }
      }
      return source;
    });
  });
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
    THICK_BAKE_MODES,
    "every",
    false,
    undefined,
  );
}

/**
 * The interfaces of a bake's solve whose node fields are made: the table's altitudes alone, all the
 * band's sources read (R08.T14.b's lever), or every one, as {@link bakeBinSolve} gives a test.
 */
type SolveInterfaces = "table" | "every";

/** The table's interfaces, k = L − a × stride, one an altitude: the solves' fields the bake reads. */
function tableInterfaces(prepared: PreparedBake): number[] {
  const { layers, altitudeStride } = prepared.grid;
  return Array.from(prepared.altitudesM, (_, a) => layers - a * altitudeStride);
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
  base: MixedLayers,
  term: BinTerm,
  legendre: SourceLegendre,
  ground: BakeGround | undefined,
  solveModes: number,
  interfaces: SolveInterfaces,
  curved: boolean,
  horizon: HorizonTerms | undefined,
): BinSolve {
  const { grid, interfaceHeightsM } = prepared;
  const layers = grid.layers;
  const order = THICK_BAKE_STREAMS;
  const factor = band.coefficientFactor;
  const {
    layers: scaled,
    scaledDepths,
    kernelDepths,
  } = deltaMLayersOf(base, factor, term.absorptionDepth, 0);
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
  const problem: DiscreteOrdinatesProblem = {
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
    ...(interfaces === "table" ? { fieldInterfaces: tableInterfaces(prepared) } : {}),
  };
  // The rows that take the horizon's terms (the module's remarks), their fields I + B by row.
  const rows =
    horizon === undefined || !(horizon.lateral || horizon.curvature)
      ? []
      : Array.from(grid.sunCosines.keys()).filter(
          (s) => (grid.sunCosines[s] ?? Number.NaN) <= HORIZON_SUN_COSINE,
        );
  let solution: DiscreteOrdinatesSolution;
  const horizonFields = new Map<number, ReadonlyArray<ModeIntensities>>();
  if (horizon === undefined || rows.length === 0) {
    solution = solveDiscreteOrdinates(problem);
  } else {
    const geometry: HorizonGeometry = {
      radiusM: radius,
      interfaceHeightsM,
      depths: scaledDepths,
      sunCosines: grid.sunCosines,
      upwardCosines: grid.upwardCosines,
      read: tableInterfaces(prepared),
    };
    const sourced = solveDiscreteOrdinatesWithSources(problem, (solved) =>
      horizonSources(solved, rows, horizon, geometry),
    );
    solution = sourced.solution;
    rows.forEach((s, i) => {
      const own = solution.beams[s]?.modes ?? [];
      const terms = sourced.sourced[i] ?? [];
      horizonFields.set(
        s,
        own.map((mode, m): ModeIntensities => {
          const b = terms[m];
          return {
            mode: mode.mode,
            quadratureUp: sumOf(mode.quadratureUp, b?.quadratureUp),
            quadratureDown: sumOf(mode.quadratureDown, b?.quadratureDown),
            userUp: sumOf(mode.userUp, b?.userUp),
            userDown: sumOf(mode.userDown, b?.userDown),
          };
        }),
      );
    });
  }
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
  // Each stream's curved path's share (the module's remarks, "Curved streams"), or none.
  // At the table's interfaces alone, where the source is formed.
  const read = curved ? tableInterfaces(prepared) : [];
  const sharesOf = (cosines: ArrayLike<number>, travel: "up" | "down"): Float64Array | null =>
    curved
      ? curvedStreamShares(radius, interfaceHeightsM, scaledDepths, cosines, travel, read)
      : null;
  const nodesUp = sharesOf(nodes, "up");
  const nodesDown = sharesOf(nodes, "down");
  const usersUp = sharesOf(grid.upwardCosines, "up");
  const usersDown = sharesOf(grid.upwardCosines, "down");
  const half = new Float64Array(order);
  // Per mode w_n Λ_l(μ_n), and per point and mode (2l + 1) χ_l Λ_l(μ_u): the products the sums
  // below took term by term, formed once as they formed them (R08.T14.b's lever).
  const weightedNodes = legendre.nodes.map((nodeL) =>
    nodeL.map((value, i) => (weights[i % n] ?? Number.NaN) * value),
  );
  const viewMoments = legendre.views.map((viewL) => new Float64Array(viewL.length));
  for (let a = 0; a < altitudes; a += 1) {
    const point = points[a];
    if (point === undefined || point.scatteringPerM === 0) {
      continue;
    }
    const k = layers - a * grid.altitudeStride;
    for (let m = 0; m < THICK_BAKE_MODES; m += 1) {
      const viewL = legendre.views[m];
      const moments = viewMoments[m];
      if (viewL === undefined || moments === undefined) {
        throw new Error(`no mode ${m}`);
      }
      for (let l = m; l < order; l += 1) {
        for (let u = 0; u < views; u += 1) {
          moments[l * views + u] =
            (2 * l + 1) * (point.moments[l] ?? Number.NaN) * (viewL[l * views + u] ?? Number.NaN);
        }
      }
    }
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
      // A horizon row's field is I + B (the module's remarks, "The horizon's first-order terms").
      const field = horizonFields.get(s) ?? beam.modes;
      for (let m = 0; m < THICK_BAKE_MODES; m += 1) {
        const mode = field[m];
        const weightedNodeL = weightedNodes[m];
        const viewMomentsL = viewMoments[m];
        const sunL = legendre.suns[m];
        if (
          mode === undefined ||
          weightedNodeL === undefined ||
          viewMomentsL === undefined ||
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
              (weightedNodeL[l * n + j] ?? Number.NaN) *
              ((mode.quadratureUp[k * n + j] ?? Number.NaN) * (nodesUp?.[k * n + j] ?? 1) +
                sign *
                  (mode.quadratureDown[k * n + j] ?? Number.NaN) *
                  (nodesDown?.[k * n + j] ?? 1));
          }
          half[l] = sum;
        }
        const azimuthal = (m === 0 ? 1 : 2) / (4 * Math.PI);
        for (let u = 0; u < views; u += 1) {
          // The terms of l + m even, then odd, each summed in l's order.
          let even = 0;
          let beamEven = 0;
          for (let l = m; l < order; l += 2) {
            const c = viewMomentsL[l * views + u] ?? Number.NaN;
            even += c * (half[l] ?? Number.NaN);
            beamEven += c * (sunL[l * suns + s] ?? Number.NaN);
          }
          let odd = 0;
          let beamOdd = 0;
          for (let l = m + 1; l < order; l += 2) {
            const c = viewMomentsL[l * views + u] ?? Number.NaN;
            odd += c * (half[l] ?? Number.NaN);
            beamOdd += c * (sunL[l * suns + s] ?? Number.NaN);
          }
          // Upward at +μ_u: ½ Σ c H, and the beam at −μ₀ through (−1)^{l+m}; downward the reverse.
          const smoothUp = 0.5 * (even + odd) + azimuthal * (beamEven - beamOdd) * aureole;
          const smoothDown = 0.5 * (even - odd) + azimuthal * (beamEven + beamOdd) * aureole;
          const intensityUp =
            (mode.userUp[k * views + u] ?? Number.NaN) * (usersUp?.[k * views + u] ?? 1);
          const intensityDown =
            (mode.userDown[k * views + u] ?? Number.NaN) * (usersDown?.[k * views + u] ?? 1);
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
 * the altitude (sr⁻¹ per unit of that scattering), converted to the channels: the band's
 * {@link bakeBins} of every bin, gathered by {@link gatherBins}.
 *
 * @param band - The band's index in {@link thickBakeBands}' order.
 * @param options - A ground other than Lambertian, a reader of each solve and the solves' modes
 *   (R08.T15's deck); none and {@link THICK_BAKE_MODES} by default.
 * @throws RangeError for a band the body does not have, solves' modes not whole in
 *   [{@link THICK_BAKE_MODES}, {@link THICK_BAKE_STREAMS}], a preparation of some bins alone, or as
 *   the solver refuses a layer (which delta-M's truncation is there to prevent).
 */
export function bakeMultipleScattering(
  prepared: PreparedBake,
  body: ThickBakeBody,
  band: number,
  options?: BakeBandOptions,
): MsSourceTable {
  return gatherBins([bakeBins(prepared, body, band, options)]);
}

/** One light's E in one bin of a band, summed over the bin's terms, and its records. */
export interface MsBinLight {
  /** E, A × S × V × M as {@link MsSourceSlot.values} holds J, in the bin, m⁻¹ sr⁻¹. */
  readonly values: Float64Array;
  readonly record: MsBinRecord;
  /** The solves the light took in the bin. */
  readonly solves: number;
  /** As {@link MsSourceSlot.doubleForwardShare}, over the bin alone. */
  readonly leftOutShare: number;
  /** As {@link MsSourceSlot.flooredSlantFactors}, over the bin alone. */
  readonly floored: number;
  /** As {@link MsSourceSlot.largestBeamDeficit}, over the bin alone. */
  readonly deficit: number;
}

/** One bin of a band's bake, before the channels (the bins' share, {@link bakeBins}). */
export interface MsBinPart {
  readonly bin: number;
  /** The band's vertical extinction optical depth in the bin, without the gases' curves. */
  readonly verticalOpticalDepth: number;
  /** 1 ÷ (σ_s (1 − f_c)) at each table altitude in the bin, 0 where it has no scattering. */
  readonly perScattering: Float64Array;
  /** One a light of the preparation, in its order. */
  readonly lights: ReadonlyArray<MsBinLight>;
  /** E_th, A × V, over the first light's terms; absent for a column without a temperature. */
  readonly thermal?: Float64Array;
}

/**
 * A band's bake over some of its bins (plan R08, R08.T14.d's bake time): what one optics job of a
 * band split by bins answers, for {@link gatherBins} to take with the band's other bins.
 */
export interface MsBinShare {
  readonly band: ThickBakeBand;
  readonly altitudesM: Float64Array;
  readonly sunCosines: Float64Array;
  readonly travelCosines: Float64Array;
  /** Each light's bin conversion (`binConversion.ts`'s factors), in the lights' order. */
  readonly conversions: ReadonlyArray<BinConversion["factors"]>;
  /** Whether the column has a temperature, its bins' E_th among the parts. */
  readonly thermal: boolean;
  /** The bins baked, ascending. */
  readonly bins: ReadonlyArray<MsBinPart>;
}

/**
 * One band's bake in the bins `prepared` holds ({@link prepareBake}'s `bins`), each bin's E summed
 * over its terms, before the channels: {@link bakeMultipleScattering}'s solves, a bin share of them.
 *
 * @throws RangeError as {@link bakeMultipleScattering}.
 */
export function bakeBins(
  prepared: PreparedBake,
  body: ThickBakeBody,
  band: number,
  options?: BakeBandOptions,
): MsBinShare {
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
  const thermal = prepared.interfacePlanck !== undefined;
  const parts = prepared.bins.map((b): MsBinPart => {
    const base = bandBinOf(prepared, b, geometry.coefficientFactor);
    // E_th, A × V, over the first light's terms (the module's remarks, "Thermal emission").
    const thermalBin = thermal ? new Float64Array(altitudes * views) : undefined;
    // A solve shared by lights whose bin has the same terms (a column without absorbers).
    const solved = new Map<BinTerm, BinSolve>();
    const lights = prepared.lights.map((light, slot): MsBinLight => {
      const lightBin = light.bins[b];
      if (lightBin === undefined) {
        throw new Error(`light ${slot} has no bin ${b}`);
      }
      const binValues = new Float64Array(points);
      let solves = 0;
      let leftOutShare = 0;
      let floored = 0;
      let deficit = 0;
      const leftOut = new Float64Array(altitudes * suns);
      const pointLight = new Float64Array(altitudes * suns);
      for (const term of lightBin.terms) {
        let solve = solved.get(term);
        if (solve === undefined) {
          solve = solveOf(
            prepared,
            geometry,
            b,
            base,
            term,
            legendre,
            options?.ground,
            solveModes,
            "table",
            options?.curvedStreams === true,
            options?.horizonTerms,
          );
          solved.set(term, solve);
          options?.onSolve?.(b, term, solve);
        }
        solves += 1;
        const w = term.weight;
        for (const [i, value] of solve.source.entries()) {
          binValues[i] = (binValues[i] ?? 0) + w * value;
        }
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
          floored += beam.flooredLayers;
          deficit = Math.max(deficit, beam.largestDeficit);
        }
      }
      const brightest = Math.max(0, ...pointLight);
      for (const [i, value] of leftOut.entries()) {
        const kept = pointLight[i] ?? Number.NaN;
        if (kept >= 1e-3 * brightest && kept > 0) {
          // Of the whole: the light kept and the light left out.
          leftOutShare = Math.max(leftOutShare, value / (kept + value));
        }
      }
      return {
        values: binValues,
        record: {
          terms: lightBin.terms.length,
          largestFitError: lightBin.largestFitError,
          readFitError: lightBin.readFitError,
          crossedRows: lightBin.crossedRows,
        },
        solves,
        leftOutShare,
        floored,
        deficit,
      };
    });
    // J = E ÷ (σ_s (1 − f_c)) in each bin, its own kernel scattering at the altitude; none where it
    // has no scattering, whose E the solve leaves 0.
    const perScattering = Float64Array.from(prepared.pointOptics[b] ?? [], (point) => {
      const kernel = point.scatteringPerM * (1 - point.forwardDelta);
      return kernel > 0 ? 1 / kernel : 0;
    });
    return {
      bin: b,
      verticalOpticalDepth: base.extinction.reduce((sum, tau) => sum + tau, 0),
      perScattering,
      lights,
      ...(thermalBin === undefined ? {} : { thermal: thermalBin }),
    };
  });
  const upward = grid.upwardCosines;
  return {
    band: geometry,
    altitudesM: prepared.altitudesM.slice(),
    sunCosines: grid.sunCosines.slice(),
    travelCosines: Float64Array.from({ length: views }, (_, v) =>
      v < upward.length
        ? -(upward[upward.length - 1 - v] ?? Number.NaN)
        : (upward[v - upward.length] ?? Number.NaN),
    ),
    conversions: prepared.lights.map((light) => light.conversion.factors),
    thermal,
    bins: parts,
  };
}

/**
 * A band's thick table from its bin shares ({@link bakeBins}), which together hold every bin once,
 * and each point a channel at its own wavelength reads (the module's remarks, "Own wavelengths"):
 * each bin's and point's J converted to the channels in their order and the records over every one,
 * so that a band baked a share of its bins a job is the bits of one baked whole.
 *
 * @throws RangeError for shares that do not hold every bin and read point once, or of other bands,
 *   grids, lights or thermal fields than the first's.
 */
export function gatherBins(shares: ReadonlyArray<MsBinShare>): MsSourceTable {
  const first = shares[0];
  if (first === undefined) {
    throw new RangeError("a band's bake gathers at least one share of its bins");
  }
  const byBin: MsBinPart[] = [];
  for (const share of shares) {
    if (
      share.band.gravityRatio !== first.band.gravityRatio ||
      share.altitudesM.length !== first.altitudesM.length ||
      share.sunCosines.length !== first.sunCosines.length ||
      share.travelCosines.length !== first.travelCosines.length ||
      share.conversions.length !== first.conversions.length ||
      share.thermal !== first.thermal
    ) {
      throw new RangeError("a band's bin shares are of other bands, grids, lights or fields");
    }
    for (const part of share.bins) {
      if (byBin[part.bin] !== undefined) {
        throw new RangeError(`a band's bin shares hold bin ${part.bin} twice`);
      }
      byBin[part.bin] = part;
    }
  }
  // The points the conversions read: the bins, and each channel's own where a light reads it.
  const pointCount = first.conversions[0]?.[0].length ?? BAKE_BIN_COUNT;
  const read = (b: number): boolean =>
    b < BAKE_BIN_COUNT ||
    first.conversions.some((factors) => factors.some((channel) => (channel[b] ?? 0) !== 0));
  const parts = Array.from({ length: pointCount }, (_, b): MsBinPart | undefined => {
    const part = byBin[b];
    if (part === undefined && read(b)) {
      throw new RangeError(`a band's bin shares hold no bin ${b}`);
    }
    return part;
  });
  const held = parts.filter((part): part is MsBinPart => part !== undefined);
  const altitudes = first.altitudesM.length;
  const suns = first.sunCosines.length;
  const views = first.travelCosines.length;
  const points = altitudes * suns * views * THICK_BAKE_MODES;
  const perAltitude = suns * views * THICK_BAKE_MODES;
  const perScattering = parts.map((part) => part?.perScattering);
  const slots = first.conversions.map((factors, slot): MsSourceSlot => {
    const lights = parts.map((part) => {
      const light = part?.lights[slot];
      if (part !== undefined && light === undefined) {
        throw new RangeError(`bin ${part.bin} holds no light ${slot}`);
      }
      return light;
    });
    const heldLights = lights.filter((light): light is MsBinLight => light !== undefined);
    const values = new Float32Array(points * 3);
    const [red, green, blue] = factors;
    for (let i = 0; i < points; i += 1) {
      const a = Math.floor(i / perAltitude);
      let r = 0;
      let g = 0;
      let bl = 0;
      for (let b = 0; b < pointCount; b += 1) {
        if (lights[b] === undefined) {
          continue;
        }
        const x = (lights[b]?.values[i] ?? Number.NaN) * (perScattering[b]?.[a] ?? Number.NaN);
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
      bins: heldLights.map((light) => light.record),
      solves: heldLights.reduce((sum, light) => sum + light.solves, 0),
      doubleForwardShare: heldLights.reduce((most, light) => Math.max(most, light.leftOutShare), 0),
      flooredSlantFactors: heldLights.reduce((sum, light) => sum + light.floored, 0),
      largestBeamDeficit: heldLights.reduce((most, light) => Math.max(most, light.deficit), 0),
    };
  });
  // The scattered thermal light and the depths are the bins' alone, as before any point.
  const bins = held.filter((part) => part.bin < BAKE_BIN_COUNT);
  const thermal = first.thermal
    ? thermalScatteringOf(
        first.altitudesM,
        bins.map((part) => part.thermal ?? new Float64Array(altitudes * views)),
        bins.map((part) => part.perScattering),
        views,
      )
    : undefined;
  return {
    band: first.band,
    altitudesM: first.altitudesM.slice(),
    sunCosines: first.sunCosines.slice(),
    travelCosines: first.travelCosines.slice(),
    modes: THICK_BAKE_MODES,
    verticalOpticalDepth: Float64Array.from(bins, (part) => part.verticalOpticalDepth),
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
  altitudesM: Float64Array,
  thermalBins: ReadonlyArray<Float64Array>,
  perScattering: ReadonlyArray<Float64Array>,
  views: number,
): ThermalScattering {
  if (views !== THERMAL_TRAVEL_COSINES) {
    throw new Error(
      `the scattered thermal light is read at ${THERMAL_TRAVEL_COSINES} travel cosines, not ${views}`,
    );
  }
  const altitudes = altitudesM.length;
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
  return { altitudesM: altitudesM.slice(), values };
}
