//! Pulsar glitches: the sudden spin-ups of rotation-powered neutron stars, the second single-star
//! event kind (plan 06, P06.T28.b).
//!
//! Two kinds, after Melatos, Peralta and Wyithe (2008, ApJ 672, 1103): small glitches that come as
//! a Poisson process in every young pulsar ("Crab-like", their section 5, eq. 5–6), and in some
//! pulsars of characteristic age 10⁴–10⁵ years a quasi-periodic train of large ones
//! ("Vela-like", their section 5.2, eq. 9), which coexists with the Poisson one. Fuentes et al.
//! (2017, A&A 608, A131, section 3) divide the two at Δν = 10 μHz: every fit of their size
//! distribution with more than one Gaussian has "a common component ... that contains all the
//! glitches with magnitudes Δν ≥ 10 μHz".
//!
//! # Who glitches
//!
//! A star glitches if it leaves a neutron star ([`StarModel::neutron_star`]) whose characteristic
//! age `τ_c` = P ÷ 2Ṗ is below 10⁷ years ([`MAX_CHARACTERISTIC_AGE`]) and which is not a magnetar
//! ([`PulsarState::is_magnetar`]), both at the reference instant ("Time" below).
//!
//! - The plan's "pulsars younger than 10⁷ years" is read as `τ_c`, in which the observed
//!   relations are stated: Espinoza et al. (2011, MNRAS 414, 1679, Table 5) find glitches in the
//!   `τ_c` bins up to 1.8 × 10⁴ kyr and none from 6.1 × 10⁴ kyr.
//! - Its "pulsars" are Fuentes et al.'s rotation-powered pulsars: their Table 1 counts the 898 of
//!   them and no magnetar, and their Fig. 2's fits leave magnetar glitches out. Magnetars glitch
//!   with large Δν ÷ ν but small Δν (Espinoza et al. 2011, section 5.3.1), which the Crab-like
//!   law, one law in Δν ÷ ν, would not give them. T21's mean reversal of 1% below the |ν̇| limit
//!   applies to magnetars as to pulsars ([`GLITCH_REVERSAL`], ruling 110.3).
//! - Whether the pulsar shines in the radio is not asked: whether a glitch can be seen is plan 12's
//!   matter.
//!
//! Living stars, white dwarfs, black holes, magnetars and stars with no neutron star by the end of
//! the clock window do not glitch here. The recycled millisecond pulsars of binaries are plan
//! 11's.
//!
//! # Time
//!
//! Rates, periods and sizes are read once, at the reference instant of the star's pulse clock
//! ([`NeutronStar::pulse_clock`]; plan 06, design note 23), so that the glitch clock and T21.d's
//! pulse clock agree: the epoch for a neutron star that exists then, and its birth for one born
//! after the epoch, within the clock window. The pulsar there ([`PulsarState`]) gives the spin
//! frequency ν = 1 ÷ P, the spin-down rate |ν̇| = Ṗ ÷ P², the characteristic age (from the
//! glitched Ṗ, as an observer quotes it) and whether it is a magnetar. The star then glitches
//! wherever [`StarModel::pulsar_at`] is `Some`, at the reference's rate and with the reference's
//! ν, and the cuts above are the reference's, so a star never crosses them inside the horizon. No
//! fast quantity is formed from a difference of two large ages.
//!
//! # The Crab-like kind
//!
//! The small glitches, Δν below 10 μHz, of every star that glitches, the Vela-like ones included,
//! on Poisson bins of [`BIN_SECONDS`] = 30 days (series `star.ev.glitch`).
//!
//! - **Rate:** R(|ν̇|) = R₁₂ × (|ν̇| ÷ 10⁻¹² Hz s⁻¹)^b, with R₁₂ = 0.0564 a year
//!   ([`SMALL_GLITCH_RATE_PER_YEAR`]) and b = 0.372 ([`SMALL_GLITCH_RATE_INDEX`]), the Poisson
//!   maximum-likelihood fit to the small-glitch counts of Fuentes et al.'s Table 1, held at
//!   |ν̇| = 10⁻⁹ Hz s⁻¹ above it ([`RATE_CAP_HZ_PER_S`]; [`small_glitch_rate`]). The rate is that
//!   constant wherever the star glitches, and the bins' bound is the constant.
//! - **Size:** x = Δν ÷ ν from the Crab's truncated power law, p(x) ∝ x^−1.2 on
//!   [1.1 × 10⁻⁹, 2.1 × 10⁻⁷] (Melatos et al. 2008, eq. 1–2, Tables 1 and 3;
//!   [`crab_fractional_size_at`]), one law in x for every star, so Δν scales with ν; it stays
//!   below 10 μHz while ν is below 47.6 Hz.
//! - **Recovery:** a step Δν that recovers in part exponentially, the fraction Q = 0.835
//!   ([`RECOVERY_FRACTION`]) with the time `τ_d` = 3.1 days ([`RECOVERY_TIME_DAYS`]), and whose
//!   lasting part, 1 − Q, is taken back at a constant rate, so that the offset is exactly 0 a
//!   look-back, T = [`LOOK_BACK_BINS`] × Δ (100.04 years), after the onset
//!   ([`Glitch::frequency_offset_hz_at`]).
//!   The take-back is this model's own rule. It makes each glitch's net spin-up zero, so the kind
//!   adds nothing to the mean spin-down, whose glitch activity is T21.b's closed form (the factor
//!   1 − [`GLITCH_REVERSAL`] below the |ν̇| limit, none above it), and no event may leave a mark
//!   that needs history replayed (brainstorm, "Events in time"). The offset's own mean in ν is not
//!   zero but `R ⟨x⟩ ν [Q τ_d + (1 − Q) T ÷ 2]`, about 1.2 × 10⁻⁷ ν at the Crab's |ν̇|.
//!
//! # The Vela-like kind
//!
//! The large glitches of the stars that glitch with `τ_c` in [10⁴, 10⁵) years and |ν̇| below
//! [`GLITCH_NU_DOT_LIMIT`] (10⁻¹⁰·⁵ Hz s⁻¹, above which Fuentes et al., section 4.2 and Table 2,
//! reject the large-glitch trend), both at the reference, and whose fixed mark, word 0 of
//! `star.ns.glitch_class`, lies below [`VELA_LIKE_SHARE`] = 0.648 ([`is_vela_like_marked`]). They
//! fall on a monotone phase (series `star.ev.glitch_cycle`; plan 06, design note 16), one large
//! glitch a cycle and none skipped ([`GlitchCycle`]).
//!
//! - **Clock:** a [`LinearClock`] from the reference whose period is P = C ÷ |ν̇|, inversely
//!   proportional to |ν̇| as Fuentes et al.'s large-glitch rate is proportional to it, with
//!   C = 3.12 years × Vela's |ν̇| = 1.5425 × 10⁻³ Hz ([`PERIOD_SPIN_DOWN_PRODUCT_HZ`]): 3.12 years
//!   is the mean interval of Vela's 13 large glitches in Melatos et al.'s Table 1. P is at least
//!   1.546 years.
//! - **Noise:** octaves of value noise of amplitude [`NOISE_AMPLITUDE_CYCLES`] on a lattice of
//!   [`NOISE_LATTICE_CYCLES`] cycle, as many as span the source horizon ([`octaves_to_span`]), as
//!   steep as the slope bound of 0.9 allows.
//! - **Size:** log₁₀(Δν ÷ μHz) ~ N(1.38, 0.13²) truncated at ±4σ (7.24–79.4 μHz), the large
//!   component of Fuentes et al.'s three-Gaussian fit (Table A.1, weight 0.17), whatever ν or
//!   |ν̇| ([`large_size_hz_at`]); its mean is [`LARGE_MEAN_SIZE_HZ`], 25.09 μHz.
//! - **Share:** [`VELA_LIKE_SHARE`] is Fuentes et al.'s `Ṅ_ℓ` ÷ |ν̇| = 420 Hz⁻¹ (section 4.2,
//!   Fig. 6) times C, the share at which the Vela-like stars' large glitches add up to their
//!   population's rate of large glitches, which counts the pulsars that never glitched.
//! - **Offset:** a sawtooth in the phase's fraction ([`GlitchCycle::frequency_offset_hz_at`]): in
//!   cycle n, at fraction f, Δνₙ (1 − f) − ⟨Δν⟩ ÷ 2. It jumps by exactly Δνₙ at glitch n, falls
//!   linearly through the cycle and ends each at −⟨Δν⟩ ÷ 2; its mean is zero over cycles, so the
//!   spin-down stays T21.b's closed form. It reads cycle n's marks alone.
//!
//! [`frequency_offset_hz_at`] sums both kinds' offsets for one star; P06.T28.g composes it.
//!
//! # Marks
//!
//! Each glitch's marks come from its event's own stream, at word 0, so marks never move a time:
//!
//! - **a Crab-like glitch, word 0** ([`BinEvent::marks`]): x = Δν ÷ ν, by inverse transform of the
//!   Crab's power law from one uniform in [0, 1);
//! - **a Vela-like glitch, word 0** of cycle n's event stream ([`CycleEvent::marks`]): Δν, by
//!   inverse transform of the truncated log-normal from one uniform in [0, 1);
//! - **`star.ns.glitch_class`, word 0** (keyed by the star's body): the Vela-like mark, one
//!   decision; words 1–7 are reserved.
//!
//! Nothing else is drawn; scatter added later takes words 1, 2, … in a fixed order.
//!
//! An event may change what a console reads, never what the star's track is (design note 14):
//! nothing here changes the star.
//!
//! [`GLITCH_REVERSAL`]: crate::stellar::remnant::neutron_star::GLITCH_REVERSAL
//! [`NeutronStar::pulse_clock`]: crate::stellar::remnant::NeutronStar::pulse_clock

use core::f64::consts::SQRT_2;

use crate::events::{
    BinEvent, CycleEvent, EventBin, EventId, EventSeries, EventsPerSecond, LinearClock,
    MonotonePhase, PoissonBins, RateModel, TimeWindow, octaves_to_span, tags,
};
use crate::id::BodyId;
use crate::math;
use crate::rng::tags::STAR_NS_GLITCH_CLASS;
use crate::rng::{ObjectKey, Seed, Stream, Threshold};
use crate::stellar::remnant::PulsarState;
use crate::stellar::remnant::neutron_star::GLITCH_NU_DOT_LIMIT;
use crate::stellar::system::StarModel;
use crate::time::{ClockWindow, SourceHorizon, Span, UniverseTime};
use crate::units::Years;
use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR};

/// The length of a Crab-like glitch bin, s: 30 days (plan 06, P06.T28.b).
pub const BIN_SECONDS: i64 = 2_592_000;

/// How many whole bins [`glitches_active_at`] looks back: 1,218, 36,540 days or 100.04 Julian
/// years, the span over which a Crab-like glitch's offset returns to 0 ([`RECOVERY_SPAN`]).
///
/// A century rather than the fewest bins that hold the exponential recovery, because the
/// take-back of the lasting part runs over the whole look-back: its frequency-derivative offset is
/// (1 − Q) Δν ÷ T, whose mean over the active glitches is the kind's lasting activity whatever T
/// is, and whose peak falls as T grows. At the oldest glitching stars (ν ≈ 1 Hz,
/// |ν̇| ≈ 1.6 × 10⁻¹⁵ Hz s⁻¹ at `τ_c` = 10⁷ years) it is, with Q = 0, 0.56% of |ν̇| for the law's
/// mean glitch and 4.2% for its largest over a century, against 5.6% and 42% over ten years.
pub const LOOK_BACK_BINS: u32 = 1_218;

/// T, the span over which a Crab-like glitch's frequency offset returns to 0: the look-back,
/// [`LOOK_BACK_BINS`] × [`BIN_SECONDS`] = 3,157,056,000 s.
///
/// [`glitches_active_at`] lists the onsets from the start of the bin [`LOOK_BACK_BINS`] before the
/// one that holds its instant, so a glitch whose onset lies before that has been 0 for at least as
/// long as the instant lies into its bin.
pub const RECOVERY_SPAN: Span = Span::from_seconds(BIN_SECONDS * 1_218);

/// The characteristic age `τ_c` at the reference below which a pulsar glitches: 10⁷ years.
///
/// The plan's "pulsars younger than 10⁷ years", read as `τ_c` (main's ruling, 2026-10-15):
/// Espinoza et al. (2011, MNRAS 414, 1679, Table 5) find 3 glitches in the bin of mean `τ_c`
/// 1.8 × 10⁴ kyr and none in those from 6.1 × 10⁴ kyr; their abstract puts the edge at 20 Myr.
pub const MAX_CHARACTERISTIC_AGE: Years = Years::new(1.0e7);

/// R₁₂, the Crab-like rate at |ν̇| = 10⁻¹² Hz s⁻¹, per Julian year: 0.0564 (σ of ln R₁₂, 0.058).
///
/// The Poisson maximum-likelihood fit of R(|ν̇|) = R₁₂ (|ν̇| ÷ 10⁻¹² Hz s⁻¹)^b to the small-glitch
/// counts `N_t` − `N_ℓ` of Fuentes et al.'s (2017, A&A 608, A131, section 4.1) Table 1 against
/// its observing time Σ `T_i`, over all sixteen bins, each at its central log |ν̇|: R₁₂ = 0.05639
/// a year and b = 0.3718 ± 0.0155, at a deviance of 63.7 on 14 degrees of freedom. One power law is
/// overdispersed against the table; the bins' observed against expected counts are 0/0.11,
/// 0/0.64, 0/2.80, 3/8.65, 11/14.3, 25/16.2, 35/26.3, 28/33.0, 23/38.9, 34/33.9, 66/34.4,
/// 41/38.3, 6/10.9, 4/11.1, 2/6.19 and 25/27.3, from log |ν̇| = −16.75 to −9.25.
///
/// It is not the plan's "rate proportional to |ν̇|", which is Fuentes et al.'s law for the large
/// glitches (section 4.2, Fig. 6) that the Vela-like kind carries; Espinoza et al.'s (2011,
/// section 5.2, Table 4, Fig. 10) `⟨Ṅ_g⟩` ∝ |ν̇|^0.47(4) counts large glitches too, and Basu et
/// al.'s (2022, MNRAS 510, 4049, section 4.3) 76 |ν̇|^0.187 is fitted to pulsars glitching more
/// than 0.05 times a year (their Fig. 10), a rate conditional on glitching (main's ruling,
/// 2026-10-15).
pub const SMALL_GLITCH_RATE_PER_YEAR: f64 = 0.0564;

/// b, the power of |ν̇| in the Crab-like rate: 0.372 ± 0.016 (the fit of
/// [`SMALL_GLITCH_RATE_PER_YEAR`]).
pub const SMALL_GLITCH_RATE_INDEX: f64 = 0.372;

/// The spin-down rate at which [`SMALL_GLITCH_RATE_PER_YEAR`] is stated, Hz s⁻¹: 10⁻¹².
pub const RATE_REFERENCE_HZ_PER_S: f64 = 1.0e-12;

/// The spin-down rate at which the Crab-like rate is held, Hz s⁻¹: 10⁻⁹, the top of Fuentes et
/// al.'s (2017) Table 1's last bin, where the rate is 0.737 a year (0.736 at the fit's unrounded
/// R₁₂ and b), 0.0605 a bin.
///
/// No glitching pulsar in the data spins down faster than the Crab's 3.8 × 10⁻¹⁰ Hz s⁻¹; without
/// the cap a newborn of the birth field's 5σ tail at P₀ = 10 ms would draw about 52 glitches a
/// bin (main's ruling, 2026-10-15). Above [`GLITCH_NU_DOT_LIMIT`] the law runs on to the cap
/// unchanged: Fuentes et al. reject the large-glitch trend there (section 4.2, Table 2), not the
/// small glitches, and their bins 14–16 are in the fit.
pub const RATE_CAP_HZ_PER_S: f64 = 1.0e-9;

/// a, the index of the Crab's power law in x = Δν ÷ ν, p(x) ∝ x^−a: 1.2 (1σ range 1.1–1.4).
///
/// Melatos, Peralta and Wyithe (2008, ApJ 672, 1103, eq. 1–2, Table 3). It is also the median of
/// Table 3's seven aperiodic glitchers (0.36–2.4), and their section 6's "a ≈ 1.2 for the youngest
/// pulsars".
pub const CRAB_SIZE_INDEX: f64 = 1.2;

/// The smallest x = Δν ÷ ν of the Crab-like law: 1.1 × 10⁻⁹, the Crab's smallest glitch in Melatos
/// et al.'s (2008) Table 1 (MJD 43023 and 52587.84), the lower end of the domain on which their
/// section 4.1 normalises each pulsar's law.
pub const MIN_FRACTIONAL_SIZE: f64 = 1.1e-9;

/// The largest x = Δν ÷ ν of the Crab-like law: 2.1 × 10⁻⁷, the Crab's largest glitch in Melatos
/// et al.'s (2008) Table 1 (MJD 53067.059, 210 × 10⁻⁹).
pub const MAX_FRACTIONAL_SIZE: f64 = 2.1e-7;

/// Q, the fraction of a Crab-like glitch's step that recovers exponentially: 0.835.
///
/// The median of the six single-exponential recoveries that Wong, Backer and Lyne (2001) fitted to
/// the Crab's glitches of Δν ÷ ν = 2.1–31.9 × 10⁻⁹ (Q = 0.68–0.9), as Yu et al. (2013, MNRAS 429,
/// 688, Table A4) list them: the Crab's own small glitches, on the scale of the Crab-like law. Lyne
/// et al. (2015, MNRAS 446, 857, section 3) describe the Crab's steps as "followed by a nearly
/// complete recovery within about 20 days".
pub const RECOVERY_FRACTION: f64 = 0.835;

/// `τ_d`, the time of a Crab-like glitch's exponential recovery, days: 3.1, the median of the same
/// six recoveries (2.0–10.3 days) as [`RECOVERY_FRACTION`].
pub const RECOVERY_TIME_DAYS: f64 = 3.1;

/// The lower edge of the Vela-like characteristic ages, at the reference: 10⁴ years (plan 06,
/// P06.T28.b).
pub const VELA_LIKE_MIN_AGE: Years = Years::new(1.0e4);

/// The upper edge, exclusive, of the Vela-like characteristic ages: 10⁵ years.
pub const VELA_LIKE_MAX_AGE: Years = Years::new(1.0e5);

/// `P_Vela`, the mean interval between the Vela pulsar's large glitches, Julian years: 3.12.
///
/// Its 13 glitches with Δν ≥ 10 μHz among the 17 of Melatos et al.'s (2008) Table 1, from MJD
/// 40280 to 53959.93: twelve intervals over 37.45 years, 3.121 years on average (standard
/// deviation 1.15 years, coefficient of variation 0.37). Melatos et al.'s own `Δt_c` = 2.8 ± 0.1
/// years (Table 6) is the δ-component of a two-component fit to all of Vela's waiting times, small
/// glitches included; this is the period of a phase in which every cycle is a large glitch.
pub const VELA_PERIOD_YEARS: f64 = 3.12;

/// The Vela pulsar's spin frequency ν, Hz: 11.1946499395 (the ATNF pulsar catalogue, version
/// 2.9.0, Manchester et al. 2005, AJ 129, 1993; from Dodson et al. 2002).
pub const VELA_FREQUENCY_HZ: f64 = 11.194_649_939_5;

/// The Vela pulsar's spin-down rate |ν̇|, Hz s⁻¹: 1.5666 × 10⁻¹¹ (the ATNF pulsar catalogue,
/// version 2.9.0, as [`VELA_FREQUENCY_HZ`]).
pub const VELA_SPIN_DOWN_HZ_PER_S: f64 = 1.5666e-11;

/// C = `P_Vela` × |`ν̇_Vela`|, Hz: 1.5425 × 10⁻³, so that a Vela-like star's period is C ÷ |ν̇|.
pub const PERIOD_SPIN_DOWN_PRODUCT_HZ: f64 =
    VELA_PERIOD_YEARS * SECONDS_PER_JULIAN_YEAR * VELA_SPIN_DOWN_HZ_PER_S;

/// Fuentes et al.'s (2017, section 4.2, Fig. 6) rate of large glitches per unit spin-down rate,
/// `Ṅ_ℓ` ÷ |ν̇|, Hz⁻¹: (4.2 ± 0.5) × 10², over −13.5 < log |ν̇| < −10.5.
pub const LARGE_GLITCHES_PER_SPIN_DOWN: f64 = 420.0;

/// The share of the stars that meet the Vela-like cuts that glitch as Vela-like pulsars: 0.648
/// (± 0.077 from the 420 ± 50 Hz⁻¹).
///
/// It is [`LARGE_GLITCHES_PER_SPIN_DOWN`] × [`PERIOD_SPIN_DOWN_PRODUCT_HZ`], the share at which
/// one large glitch a period in the Vela-like stars adds up to Fuentes et al.'s rate of large
/// glitches, which counts the pulsars that never glitched; it does not depend on |ν̇|, since both
/// laws are proportional to it. The activities then agree: a Vela-like star returns ⟨Δν⟩ ÷ P =
/// 1.63% of |ν̇|, the population 0.648 × 1.63% = 1.05%, against Fuentes et al.'s 0.010 ± 0.001
/// and T21's 1% (main's ruling, 2026-10-15).
pub const VELA_LIKE_SHARE: f64 = 0.648;

/// The mean of log₁₀(Δν ÷ μHz) of a Vela-like glitch: 1.38, the large component of Fuentes et
/// al.'s (2017, Table A.1) three-Gaussian model of the glitch sizes (weight 0.17; their 2- and
/// 4-Gaussian models give 1.37–1.38).
pub const LARGE_LOG_SIZE_MEAN: f64 = 1.38;

/// The standard deviation of log₁₀(Δν ÷ μHz) of a Vela-like glitch: 0.13 (Fuentes et al. 2017,
/// Table A.1, in every model with more than one Gaussian).
pub const LARGE_LOG_SIZE_SIGMA: f64 = 0.13;

/// Where the Vela-like size law is truncated, in standard deviations either side of its mean: 4,
/// so Δν lies in 7.24–79.4 μHz.
pub const LARGE_SIZE_TRUNCATION: f64 = 4.0;

/// ⟨Δν⟩, the mean of the Vela-like size law, Hz: 25.09 μHz.
///
/// The closed form 10^μ e^(s²/2) [Φ(4 − s) − Φ(−4 − s)] ÷ [Φ(4) − Φ(−4)] μHz, with μ = 1.38 and
/// s = 0.13 ln 10, to a few units in the last place.
pub const LARGE_MEAN_SIZE_HZ: f64 = 2.508_614_549_760_652_7e-5;

/// The noise's amplitude at its finest octave, cycles: 0.087, which with
/// [`NOISE_LATTICE_CYCLES`] puts the slope bound, 10.3 a ÷ ℓ, at 0.896, as near the bound of 0.9
/// as two figures allow.
///
/// The slope bound caps the scatter of the intervals between large glitches: measured over 300
/// years of 182 Vela-like series, their coefficient of variation is 0.093, with a correlation of
/// +0.20 between consecutive intervals, against Vela's own 0.37 and −0.24 (Melatos et al. 2008,
/// Table 1). A coarser lattice at the same bound reaches at most 0.11, at correlations of 0.54 (ℓ =
/// 2) to 0.98 (ℓ = 16): a slowly wandering clock rather than scattered intervals, so the finest
/// lattice is kept (plan 06, P06.T28.b; the shortfall is a deferred finding).
pub const NOISE_AMPLITUDE_CYCLES: f64 = 0.087;

/// The noise's lattice spacing at its finest octave, whole cycles: 1, so that consecutive
/// intervals take nearly independent lattice values (see [`NOISE_AMPLITUDE_CYCLES`]).
pub const NOISE_LATTICE_CYCLES: u32 = 1;

/// The Crab-like bins: 30 days long, looking back a century.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("30 days lies between 16 s and 2^44 s")
}

/// The Crab-like rate of a pulsar whose spin-down rate is `spin_down_hz_per_s` (|ν̇|, Hz s⁻¹):
/// R₁₂ × (|ν̇| ÷ 10⁻¹² Hz s⁻¹)^b, with |ν̇| held at [`RATE_CAP_HZ_PER_S`] above it.
///
/// # Panics
///
/// In debug builds, unless `spin_down_hz_per_s` is finite and positive.
#[must_use]
pub fn small_glitch_rate(spin_down_hz_per_s: f64) -> EventsPerSecond {
    debug_assert!(
        spin_down_hz_per_s.is_finite() && spin_down_hz_per_s > 0.0,
        "a pulsar spins down, got |ν̇| = {spin_down_hz_per_s}"
    );
    let held = spin_down_hz_per_s.min(RATE_CAP_HZ_PER_S);
    EventsPerSecond::per_julian_year(
        SMALL_GLITCH_RATE_PER_YEAR
            * math::powf(held / RATE_REFERENCE_HZ_PER_S, SMALL_GLITCH_RATE_INDEX),
    )
}

/// x = Δν ÷ ν of the Crab-like glitch whose word 0 is `u`, by the inverse of the truncated power
/// law, `x = [x_min^(1−a) + u (x_max^(1−a) − x_min^(1−a))]^(1 ÷ (1−a))`, from
/// [`MIN_FRACTIONAL_SIZE`] at u = 0 towards [`MAX_FRACTIONAL_SIZE`] as u nears 1.
///
/// Its mean is 2.78 × 10⁻⁸ and its median 7.9 × 10⁻⁹.
///
/// # Panics
///
/// In debug builds, unless `u` lies in [0, 1).
#[must_use]
pub fn crab_fractional_size_at(u: f64) -> f64 {
    debug_assert!((0.0..1.0).contains(&u), "a uniform lies in [0, 1), got {u}");
    let power = 1.0 - CRAB_SIZE_INDEX;
    let low = math::powf(MIN_FRACTIONAL_SIZE, power);
    let high = math::powf(MAX_FRACTIONAL_SIZE, power);
    let size = math::powf(math::mul_add(u, high - low, low), 1.0 / power);
    size.clamp(MIN_FRACTIONAL_SIZE, MAX_FRACTIONAL_SIZE)
}

/// Δν of the Vela-like glitch whose word 0 is `u`, Hz: 10^(1.38 + 0.13 z) μHz, with z the
/// standard normal truncated at ±4 by inverse transform, z = Φ⁻¹(Φ(−4) + u [Φ(4) − Φ(−4)]),
/// through [`math::normal_quantile`].
///
/// It lies in 7.24–79.4 μHz, with mean [`LARGE_MEAN_SIZE_HZ`].
///
/// # Panics
///
/// In debug builds, unless `u` lies in [0, 1).
#[must_use]
pub fn large_size_hz_at(u: f64) -> f64 {
    debug_assert!((0.0..1.0).contains(&u), "a uniform lies in [0, 1), got {u}");
    let edge = LARGE_SIZE_TRUNCATION / SQRT_2;
    // Φ(−4), and Φ(4) − Φ(−4), each from one call that keeps its precision in the tail.
    let tail = 0.5 * math::erfc(edge);
    let kept = math::erf(edge);
    let z = math::normal_quantile(math::mul_add(u, kept, tail))
        .clamp(-LARGE_SIZE_TRUNCATION, LARGE_SIZE_TRUNCATION);
    math::exp10(math::mul_add(LARGE_LOG_SIZE_SIGMA, z, LARGE_LOG_SIZE_MEAN)) * 1.0e-6
}

/// Whether the neutron star of `body` in the universe of `seed` is marked to glitch as a
/// Vela-like pulsar: word 0 of `star.ns.glitch_class`, decided against [`VELA_LIKE_SHARE`].
///
/// The mark depends on the seed and the body alone, and is fixed for the star's life: it is read
/// whatever the star's age, its time or its kick loop's attempt, and every star has one, though
/// only a star that meets the Vela-like cuts glitches by it ([`GlitchCycle::of`]).
#[must_use]
pub fn is_vela_like_marked(seed: Seed, body: BodyId) -> bool {
    Stream::open(seed, STAR_NS_GLITCH_CLASS, ObjectKey::from(body))
        .decide(Threshold::from_probability(VELA_LIKE_SHARE))
}

/// The glitches of one star: its reference instant, its pulsar there and the Crab-like rate read
/// from it (plan 06, P06.T28.b; see the [module documentation](self)).
///
/// It is the [`RateModel`] of the star's `star.ev.glitch` series: its [`bound`](RateModel::bound)
/// is the constant [`rate`](Self::small_glitch_rate) in every bin, and its
/// [`rate`](RateModel::rate) at t is that where the star is a neutron star at t and 0 elsewhere.
///
/// # Examples
///
/// A pulsar of nearly Vela's spin-down glitches small about once in six and a half years, and
/// would glitch large every 3.25 years if it were a Vela-like one:
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
/// use hyperion_sim::stellar::events::glitches::{GlitchModel, VELA_PERIOD_YEARS};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A 15 M☉ star whose neutron star is born at 20 ms with a field of 10^12.53 G, 11,000 years
/// // before the epoch.
/// let z = |value: f64| StandardNormal::new(value).ok_or("a finite draw");
/// let draws = StarDraws::from_parts(StarDrawsParts {
///     ns_spin: [z(-2.3)?; 8],
///     ns_field: z(-0.7)?,
///     ..StarDrawsParts::MEDIAN
/// });
/// let (mass, solar) = (SolarMasses::new(15.0), Composition::SOLAR);
/// let lifetime = StarModel::new(mass, solar, draws.clone(), Years::new(1e9))?
///     .lifetime()
///     .ok_or("a 15 M☉ star dies")?;
/// let pulsar = StarModel::new(mass, solar, draws, Years::new(lifetime.value() + 1.1e4))?;
/// let glitches = GlitchModel::of(&pulsar).ok_or("a young pulsar glitches")?;
/// let per_year = glitches.small_glitch_rate().value() * 31_557_600.0;
/// assert!((0.1..0.25).contains(&per_year));
/// let period = glitches.vela_like_period_years();
/// assert!((period / VELA_PERIOD_YEARS - 1.0).abs() < 0.3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlitchModel<'a> {
    star: &'a StarModel,
    reference: UniverseTime,
    pulsar: PulsarState,
    frequency_hz: f64,
    spin_down_hz_per_s: f64,
    rate: EventsPerSecond,
}

impl<'a> GlitchModel<'a> {
    /// The glitches of `star`, or `None` for a star that does not glitch (see "Who glitches" in
    /// the [module documentation](self)): one with no neutron star by the end of the clock window,
    /// a magnetar at the reference, or a pulsar of `τ_c` from 10⁷ years there.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let neutron_star = star.neutron_star()?;
        let age_at_epoch = star.remnant_age_at_epoch()?;
        let reference = neutron_star.pulse_clock(age_at_epoch)?.reference();
        // A star whose track runs past +H may report a death after it; its neutron star is not one
        // of the clock window's.
        if reference > ClockWindow::END {
            return None;
        }
        // The pulse clock's own age at its reference: the age at the epoch, or 0 at a birth after
        // it, which `state_at` clamps to.
        let pulsar = neutron_star.state_at(age_at_epoch);
        if pulsar.is_magnetar()
            || pulsar.characteristic_age().value() >= MAX_CHARACTERISTIC_AGE.value()
        {
            return None;
        }
        let period = pulsar.period().value();
        let spin_down_hz_per_s = pulsar.period_derivative() / (period * period);
        let rate = small_glitch_rate(spin_down_hz_per_s);
        debug_assert!(
            rate.value() * BIN_SECONDS_F64 <= PoissonBins::MAX_MEAN_PER_BIN,
            "a glitch bin's mean is {}",
            rate.value() * BIN_SECONDS_F64
        );
        Some(Self {
            star,
            reference,
            pulsar,
            frequency_hz: 1.0 / period,
            spin_down_hz_per_s,
            rate,
        })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The reference instant, at which every rate, period and size is read: the epoch, or the
    /// birth of a neutron star born after it (the pulse clock's,
    /// [`PulseClock::reference`](crate::stellar::remnant::PulseClock::reference)).
    #[must_use]
    pub const fn reference(&self) -> UniverseTime {
        self.reference
    }

    /// The pulsar at the reference.
    #[must_use]
    pub const fn pulsar(&self) -> PulsarState {
        self.pulsar
    }

    /// ν, the spin frequency at the reference, Hz.
    #[must_use]
    pub const fn frequency_hz(&self) -> f64 {
        self.frequency_hz
    }

    /// |ν̇| = Ṗ ÷ P², the spin-down rate at the reference, Hz s⁻¹.
    #[must_use]
    pub const fn spin_down_hz_per_s(&self) -> f64 {
        self.spin_down_hz_per_s
    }

    /// `τ_c` = P ÷ 2Ṗ at the reference, Julian years, below [`MAX_CHARACTERISTIC_AGE`].
    #[must_use]
    pub const fn characteristic_age(&self) -> Years {
        self.pulsar.characteristic_age()
    }

    /// The rate of Crab-like glitches while the star is a neutron star, [`small_glitch_rate`] at
    /// the reference's |ν̇|: at most 0.737 a year, 0.0605 a bin.
    #[must_use]
    pub const fn small_glitch_rate(&self) -> EventsPerSecond {
        self.rate
    }

    /// a, the index of the Crab-like size law: [`CRAB_SIZE_INDEX`], the same for every star.
    #[must_use]
    pub const fn size_index(&self) -> f64 {
        CRAB_SIZE_INDEX
    }

    /// The Crab-like size law's bounds in x = Δν ÷ ν: [`MIN_FRACTIONAL_SIZE`] and
    /// [`MAX_FRACTIONAL_SIZE`], the same for every star.
    #[must_use]
    pub const fn fractional_size_bounds(&self) -> (f64, f64) {
        (MIN_FRACTIONAL_SIZE, MAX_FRACTIONAL_SIZE)
    }

    /// The period a Vela-like star of this spin-down would have, C ÷ |ν̇|, Julian years, whether
    /// or not the star is one ([`GlitchCycle`]).
    #[must_use]
    pub fn vela_like_period_years(&self) -> f64 {
        PERIOD_SPIN_DOWN_PRODUCT_HZ / self.spin_down_hz_per_s / SECONDS_PER_JULIAN_YEAR
    }

    /// Whether the star glitches at `t`: whether its neutron star exists then, where
    /// [`StarModel::pulsar_at`] is `Some`.
    #[must_use]
    pub fn glitches_at(&self, t: UniverseTime) -> bool {
        // `pulsar_at(t)` is `Some` exactly where the remnant's age is, given a neutron star, which
        // the model has; this skips building its state.
        self.star.remnant_age_at(t).is_some()
    }

    /// Whether the star meets the Vela-like cuts at the reference, mark aside: `τ_c` in
    /// [10⁴, 10⁵) years and |ν̇| below [`GLITCH_NU_DOT_LIMIT`] (it is no magnetar, or it would
    /// have no model).
    #[must_use]
    pub fn meets_vela_like_cuts(&self) -> bool {
        let age = self.characteristic_age().value();
        (VELA_LIKE_MIN_AGE.value()..VELA_LIKE_MAX_AGE.value()).contains(&age)
            && self.spin_down_hz_per_s < GLITCH_NU_DOT_LIMIT
    }

    /// The Crab-like glitch of `event`, an event of this model's series: its size from word 0 of
    /// its marks.
    #[must_use]
    fn glitch_of(&self, event: BinEvent) -> Glitch {
        let id = event.id();
        let onset = event.time();
        let fractional_size = crab_fractional_size_at(event.into_marks().uniform());
        debug_assert!(
            recovery_profile(RECOVERY_SPAN_SECONDS).abs() < f64::MIN_POSITIVE,
            "a glitch's offset must be 0 by the end of the look-back"
        );
        Glitch {
            id,
            onset,
            fractional_size,
            size_hz: fractional_size * self.frequency_hz,
        }
    }
}

impl RateModel for GlitchModel<'_> {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        if self.glitches_at(t) {
            self.rate
        } else {
            EventsPerSecond::ZERO
        }
    }
}

/// Δ in seconds as an `f64`, exact.
const BIN_SECONDS_F64: f64 = 2_592_000.0;

/// T in seconds as an `f64`, exact.
const RECOVERY_SPAN_SECONDS: f64 = BIN_SECONDS_F64 * 1_218.0;

/// `τ_d` in seconds.
const RECOVERY_TIME_SECONDS: f64 = RECOVERY_TIME_DAYS * SECONDS_PER_DAY;

/// The share of a Crab-like glitch's step left `elapsed` seconds after its onset, 0 ≤ elapsed ≤ T:
/// `Q (e^(−Δt/τ_d) − e^(−T/τ_d)) ÷ (1 − e^(−T/τ_d)) + (1 − Q)(1 − Δt/T)`, 1 at the onset and
/// exactly 0 at T.
#[must_use]
fn recovery_profile(elapsed: f64) -> f64 {
    let end = math::exp(-RECOVERY_SPAN_SECONDS / RECOVERY_TIME_SECONDS);
    let recovering = (math::exp(-elapsed / RECOVERY_TIME_SECONDS) - end) / (1.0 - end);
    let lasting = 1.0 - elapsed / RECOVERY_SPAN_SECONDS;
    math::mul_add(
        RECOVERY_FRACTION,
        recovering,
        (1.0 - RECOVERY_FRACTION) * lasting,
    )
}

/// One Crab-like glitch: its ID, onset and size, and its frequency offset as it recovers.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Glitch {
    id: EventId,
    onset: UniverseTime,
    fractional_size: f64,
    size_hz: f64,
}

impl Glitch {
    /// The glitch's ID, from which [`glitch`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The onset, the instant of the step.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// x = Δν ÷ ν, dimensionless, in [`MIN_FRACTIONAL_SIZE`]–[`MAX_FRACTIONAL_SIZE`].
    #[must_use]
    pub const fn fractional_size(&self) -> f64 {
        self.fractional_size
    }

    /// Δν, the step in spin frequency, Hz: x times ν at the reference.
    #[must_use]
    pub const fn size_hz(&self) -> f64 {
        self.size_hz
    }

    /// Q, the share of the step that recovers exponentially: [`RECOVERY_FRACTION`].
    #[must_use]
    pub const fn recovery_fraction(&self) -> f64 {
        RECOVERY_FRACTION
    }

    /// `τ_d`, the time of the exponential recovery, s: [`RECOVERY_TIME_DAYS`] in seconds.
    #[must_use]
    pub const fn recovery_time_seconds(&self) -> f64 {
        RECOVERY_TIME_SECONDS
    }

    /// The first instant at which the offset is 0 again, the onset plus [`RECOVERY_SPAN`].
    ///
    /// # Panics
    ///
    /// If that lies beyond the universe clock, which no glitch listed inside it reaches.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        self.onset
            .checked_add(RECOVERY_SPAN)
            .expect("a glitch recovers within a century of an onset the clock holds")
    }

    /// Whether the glitch's offset is not yet 0 at `t`: from its onset, inclusive, to its end,
    /// exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        t.checked_since(self.onset)
            .is_some_and(|elapsed| !elapsed.is_negative() && elapsed < RECOVERY_SPAN)
    }

    /// The glitch's offset of the spin frequency at `t`, Hz, on top of T21.b's state.
    ///
    /// It is 0 before the onset and from the end, and in between, Δt after the onset,
    ///
    /// > `Δν × [Q (e^(−Δt/τ_d) − e^(−T/τ_d)) ÷ (1 − e^(−T/τ_d)) + (1 − Q)(1 − Δt/T)]`,
    ///
    /// with T = [`RECOVERY_SPAN`]: Δν at the onset, continuous after it, and exactly 0 at T. The
    /// first term is the exponential recovery of Yu et al. (2013, section 5.2.1), rescaled to reach
    /// 0 at T; the second takes the lasting part back at a constant rate, this model's own rule
    /// (see the [module documentation](self)). P06.T28.g composes it into the star's summary.
    #[must_use]
    pub fn frequency_offset_hz_at(&self, t: UniverseTime) -> f64 {
        let Some(elapsed) = t.checked_since(self.onset) else {
            return 0.0;
        };
        if elapsed.is_negative() || elapsed >= RECOVERY_SPAN {
            return 0.0;
        }
        self.size_hz * recovery_profile(elapsed.as_seconds_f64())
    }
}

/// Appends to `out` the Crab-like glitches of `star` with onsets in `window`, sorted by onset and
/// then by ID.
///
/// A star that does not glitch ([`GlitchModel::of`]) has none. `series` is the star's
/// `star.ev.glitch` series, `EventSeries::new(seed, tags::STAR_GLITCH, body.into())`: [`StarModel`]
/// holds no seed or ID, so it comes in beside the star, as for the flares. Every bin the window
/// touches is generated whole, so a listing over a window is the union of the listings over any
/// partition of it, in any order of asking. `out` is only appended to; nothing in it is read.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_GLITCH`].
///
/// # Examples
///
/// A Vela-like pulsar's glitches over thirty years, small and large, and one regenerated from its
/// ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
/// use hyperion_sim::stellar::events::glitches::{
///     glitch, glitches_in, is_vela_like_marked, large_glitches_in,
/// };
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A pulsar of about Vela's spin-down, 11,000 years old (see `GlitchModel`).
/// let z = |value: f64| StandardNormal::new(value).ok_or("a finite draw");
/// let draws = StarDraws::from_parts(StarDrawsParts {
///     ns_spin: [z(-2.3)?; 8],
///     ns_field: z(-0.7)?,
///     ..StarDrawsParts::MEDIAN
/// });
/// let (mass, solar) = (SolarMasses::new(15.0), Composition::SOLAR);
/// let lifetime = StarModel::new(mass, solar, draws.clone(), Years::new(1e9))?
///     .lifetime()
///     .ok_or("a 15 M☉ star dies")?;
/// let pulsar = StarModel::new(mass, solar, draws, Years::new(lifetime.value() + 1.1e4))?;
///
/// // A body whose star is marked to glitch as a Vela-like pulsar.
/// let seed = Seed::new(7);
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let body = (0..64)
///     .map(|i| BodyId::new(system, i))
///     .find(|&body| is_vela_like_marked(seed, body))
///     .ok_or("about two stars in three are marked")?;
/// let small = EventSeries::new(seed, tags::STAR_GLITCH, body.into());
/// let thirty_years = UniverseTime::from_julian_years(30).ok_or("in range")?;
/// let window = TimeWindow::new(UniverseTime::EPOCH, thirty_years)?;
///
/// let mut glitches = Vec::new();
/// glitches_in(&pulsar, &small, window, &mut glitches);
/// let mut large_glitches = Vec::new();
/// large_glitches_in(&pulsar, seed, body, window, &mut large_glitches);
/// // About nine large glitches of some 25 μHz, one every 3.25 years, and a few small ones.
/// assert!((6..=14).contains(&large_glitches.len()));
/// assert!(large_glitches.iter().all(|g| (7e-6..80e-6).contains(&g.size_hz())));
/// assert!(glitches.iter().all(|g| g.size_hz() < 10e-6));
/// if let Some(first) = glitches.first() {
///     assert_eq!(glitch(&pulsar, &small, first.id()).as_ref(), Some(first));
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn glitches_in(
    star: &StarModel,
    series: &EventSeries,
    window: TimeWindow,
    out: &mut Vec<Glitch>,
) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_GLITCH,
        "a star's Crab-like glitches are its star.ev.glitch series"
    );
    let Some(model) = GlitchModel::of(star) else {
        return;
    };
    let mut events = Vec::new();
    bins().events_in(series, window, &model, &mut events);
    out.extend(events.into_iter().map(|event| model.glitch_of(event)));
}

/// Appends to `out` the Crab-like glitches of `star` whose offset is not yet 0 at `t`, sorted by
/// onset and then by ID.
///
/// They are those with an onset from the start of the bin [`LOOK_BACK_BINS`] before `t`'s up to
/// `t` that are still active ([`Glitch::is_active_at`]); every offset is 0 a look-back after its
/// onset, so none is missed. `series` is as for [`glitches_in`]. The result depends on `star`,
/// `series` and `t` alone, never on what was asked before.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_GLITCH`].
pub fn glitches_active_at(
    star: &StarModel,
    series: &EventSeries,
    t: UniverseTime,
    out: &mut Vec<Glitch>,
) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_GLITCH,
        "a star's Crab-like glitches are its star.ev.glitch series"
    );
    let Some(model) = GlitchModel::of(star) else {
        return;
    };
    let mut events = Vec::new();
    bins().active_at(series, t, &model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(|event| model.glitch_of(event))
            .filter(|glitch| glitch.is_active_at(t)),
    );
}

/// The Crab-like glitch `id` names, regenerated on its own.
///
/// `None` if `id` is not of `series`, names a candidate that was rejected (one before the star's
/// neutron star is born) or a number beyond its bin's count, or `star` does not glitch.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_GLITCH`].
#[must_use]
pub fn glitch(star: &StarModel, series: &EventSeries, id: EventId) -> Option<Glitch> {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_GLITCH,
        "a star's Crab-like glitches are its star.ev.glitch series"
    );
    let model = GlitchModel::of(star)?;
    bins()
        .event(series, id, &model)
        .map(|event| model.glitch_of(event))
}

/// The large glitches of one Vela-like star: its glitch model, its series, and the monotone phase
/// they fall on (plan 06, P06.T28.b; see the [module documentation](self)).
///
/// One large glitch falls where the phase Φ = φ + N(φ) of the star's `star.ev.glitch_cycle` series
/// crosses each whole cycle, φ on a [`LinearClock`] from the reference with period C ÷ |ν̇|, while
/// the star is a neutron star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlitchCycle<'a> {
    model: GlitchModel<'a>,
    series: EventSeries,
    clock: LinearClock,
    noise: MonotonePhase,
}

impl<'a> GlitchCycle<'a> {
    /// The large glitches of `star`, whose body in the universe of `seed` is `body`, or `None`
    /// for a star that is not Vela-like: one that does not glitch ([`GlitchModel::of`]), whose
    /// `τ_c` at the reference lies outside [10⁴, 10⁵) years, whose |ν̇| there is at or above
    /// [`GLITCH_NU_DOT_LIMIT`], or which is not marked ([`is_vela_like_marked`]).
    ///
    /// The mark and the star's `star.ev.glitch_cycle` series,
    /// `EventSeries::new(seed, tags::STAR_GLITCH_CYCLE, body.into())`, are both opened here from
    /// `seed` and `body`, so that they cannot come from two universes.
    ///
    /// # Panics
    ///
    /// Only on a broken invariant: a star the cuts admit has a period of 1.546 years to well under
    /// 10¹² s, which the clock holds, and the noise's constants pass its slope check.
    #[must_use]
    pub fn of(star: &'a StarModel, seed: Seed, body: BodyId) -> Option<Self> {
        let model = GlitchModel::of(star)?;
        if !model.meets_vela_like_cuts() || !is_vela_like_marked(seed, body) {
            return None;
        }
        // Finite and in range: |ν̇| is positive, and with τ_c under 10⁵ years and a period of at
        // least a millisecond P is under 10¹² s.
        let period = Span::from_seconds_f64(PERIOD_SPIN_DOWN_PRODUCT_HZ / model.spin_down_hz_per_s)
            .expect("a Vela-like period is a span the clock holds");
        let clock = LinearClock::new(period, model.reference)
            .expect("a Vela-like period is at least 1.546 years, far above 16 s");
        let horizon = SourceHorizon::END
            .checked_since(SourceHorizon::START)
            .expect("the source horizon is a span the clock holds");
        let noise = MonotonePhase::new(
            NOISE_AMPLITUDE_CYCLES,
            NOISE_LATTICE_CYCLES,
            octaves_to_span(NOISE_LATTICE_CYCLES, clock.cycles_in(horizon)),
        )
        .expect("the noise's constants keep its slope bound at 0.896, under 0.9");
        Some(Self {
            model,
            series: EventSeries::new(seed, tags::STAR_GLITCH_CYCLE, body.into()),
            clock,
            noise,
        })
    }

    /// The star's glitch model.
    #[must_use]
    pub const fn model(&self) -> &GlitchModel<'a> {
        &self.model
    }

    /// The star's `star.ev.glitch_cycle` series.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The clock: origin the reference, period C ÷ |ν̇|.
    #[must_use]
    pub const fn clock(&self) -> LinearClock {
        self.clock
    }

    /// The phase's noise.
    #[must_use]
    pub const fn noise(&self) -> MonotonePhase {
        self.noise
    }

    /// The period P = C ÷ |ν̇| at the reference.
    #[must_use]
    pub const fn period(&self) -> Span {
        self.clock.period()
    }

    /// The star's frequency offset from its large glitches at `t`, Hz, on top of T21.b's state:
    /// in cycle n of the phase at `t`, at fraction f through it, Δνₙ (1 − f) − ⟨Δν⟩ ÷ 2, and 0
    /// where the star is not a neutron star.
    ///
    /// It jumps by exactly Δνₙ at glitch n (within the microseconds over which the phase in `f64`
    /// is not monotone about a whole cycle), falls linearly in f, and ends every cycle at
    /// −⟨Δν⟩ ÷ 2, with ⟨Δν⟩ the size law's exact mean, [`LARGE_MEAN_SIZE_HZ`]. Its mean is zero
    /// over cycles, not within each: cycle n's is (Δνₙ − ⟨Δν⟩) ÷ 2, which does not accumulate.
    /// Its slope is a frequency-derivative offset of −Δνₙ ÷ Pₙ, on average −⟨Δν⟩ ÷ P, 1.63% of
    /// |ν̇|. It reads cycle n's marks alone: no history, and nothing of cycle n + 1. A star born
    /// part-way through a cycle starts at that cycle's sawtooth.
    #[must_use]
    pub fn frequency_offset_hz_at(&self, t: UniverseTime) -> f64 {
        if !self.model.glitches_at(t) {
            return 0.0;
        }
        let (cycle, fraction) = self.noise.cycle_at(&self.series, &self.clock, t);
        // A cycle beyond the 40-bit cycle numbers has no event, and so no glitch to recover from.
        let Ok(bin) = EventBin::new(cycle) else {
            return 0.0;
        };
        // Cycle n's event stream at word 0, as its `CycleEvent::marks` would give it, without the
        // bisection that finds the cycle's time.
        let size = large_size_hz_at(self.series.key().event_stream(bin, 0).uniform());
        math::mul_add(size, 1.0 - fraction, -0.5 * LARGE_MEAN_SIZE_HZ)
    }

    /// The large glitch of `event`, a cycle of this star's series: its size from word 0 of its
    /// marks.
    #[must_use]
    fn large_glitch_of(&self, event: CycleEvent) -> LargeGlitch {
        let id = event.id();
        let cycle = event.cycle();
        let onset = event.time();
        let size_hz = large_size_hz_at(event.into_marks().uniform());
        LargeGlitch {
            id,
            cycle,
            onset,
            size_hz,
            fractional_size: size_hz / self.model.frequency_hz,
        }
    }
}

/// One Vela-like glitch: its ID, cycle, onset and size.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct LargeGlitch {
    id: EventId,
    cycle: i64,
    onset: UniverseTime,
    size_hz: f64,
    fractional_size: f64,
}

impl LargeGlitch {
    /// The glitch's ID: the cycle as its bin, number 0.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The cycle whose start the glitch is, from which [`large_glitch`] regenerates it.
    #[must_use]
    pub const fn cycle(&self) -> i64 {
        self.cycle
    }

    /// The onset, where the phase reaches the cycle.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// Δν, the step in spin frequency, Hz, in 7.24–79.4 μHz.
    #[must_use]
    pub const fn size_hz(&self) -> f64 {
        self.size_hz
    }

    /// Δν ÷ ν, dimensionless, with ν at the reference.
    #[must_use]
    pub const fn fractional_size(&self) -> f64 {
        self.fractional_size
    }
}

/// Appends to `out` the Vela-like glitches of `star` with onsets in `window`, in time order,
/// which is cycle order.
///
/// A star that is not Vela-like ([`GlitchCycle::of`]) has none, and cycles before its neutron
/// star's birth are not listed. `body` is the star's body in the universe of `seed`, from which
/// the mark and the star's `star.ev.glitch_cycle` series are opened. Each glitch's time is a
/// function of its cycle alone, so a listing over a window is the union of the listings over any
/// partition of it, in any order of asking. `out` is only appended to; nothing in it is read.
pub fn large_glitches_in(
    star: &StarModel,
    seed: Seed,
    body: BodyId,
    window: TimeWindow,
    out: &mut Vec<LargeGlitch>,
) {
    let Some(cycles) = GlitchCycle::of(star, seed, body) else {
        return;
    };
    let mut events = Vec::new();
    cycles
        .noise
        .events_in(&cycles.series, &cycles.clock, window, &mut events);
    out.extend(
        events
            .into_iter()
            .filter(|event| cycles.model.glitches_at(event.time()))
            .map(|event| cycles.large_glitch_of(event)),
    );
}

/// The Vela-like glitch of cycle `cycle` of `star`, regenerated on its own.
///
/// `None` if `star` is not Vela-like, the cycle falls before its neutron star's birth, or it lies
/// beyond the 40-bit cycle numbers. `seed` and `body` are as for [`large_glitches_in`].
#[must_use]
pub fn large_glitch(star: &StarModel, seed: Seed, body: BodyId, cycle: i64) -> Option<LargeGlitch> {
    let cycles = GlitchCycle::of(star, seed, body)?;
    cycles
        .noise
        .event(&cycles.series, &cycles.clock, cycle)
        .filter(|event| cycles.model.glitches_at(event.time()))
        .map(|event| cycles.large_glitch_of(event))
}

/// The frequency offset of all of `star`'s glitches at `t`, Hz: the Crab-like glitches still
/// active then, summed in their listing order, plus the Vela-like sawtooth for a Vela-like star.
///
/// `body` is the star's body in the universe of `seed`, from which both series and the mark are
/// opened. It is what P06.T28.g composes into the star's summary; nothing reads it yet. It depends
/// on its arguments alone, never on what was asked before.
#[must_use]
pub fn frequency_offset_hz_at(star: &StarModel, seed: Seed, body: BodyId, t: UniverseTime) -> f64 {
    let small = EventSeries::new(seed, tags::STAR_GLITCH, body.into());
    let mut active = Vec::new();
    glitches_active_at(star, &small, t, &mut active);
    let crab_like = active
        .iter()
        .fold(0.0, |sum, glitch| sum + glitch.frequency_offset_hz_at(t));
    let vela_like =
        GlitchCycle::of(star, seed, body).map_or(0.0, |cycles| cycles.frequency_offset_hz_at(t));
    crab_like + vela_like
}

#[cfg(test)]
mod tests {
    use core::f64::consts::LN_10;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample, normal_cdf,
        poisson_interval,
    };

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::coords::UnitVector;
    use crate::events::event_id;
    use crate::events::testing::{ConstantRate, assert_partition_independent, partition};
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::{Galaxy, Population};
    use crate::id::{SystemId, event_tags};
    use crate::stellar::Composition;
    use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
    use crate::stellar::remnant::NeutronStar;
    use crate::stellar::testing::{Sampling, components_of, reference_position, sample_inputs};
    use crate::units::{Gauss, Radians, Seconds, SolarMasses};

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000b);
    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// Fuentes et al. (2017), Table 1: per bin, log |ν̇| (Hz s⁻¹, the bin's centre), Σ `T_i` (yr),
    /// `N_ℓ` (large glitches) and `N_t` (all glitches).
    const FUENTES_TABLE_1: [(f64, f64, u32, u32); 16] = [
        (-16.75, 117.0, 0, 0),
        (-16.25, 430.0, 0, 0),
        (-15.75, 1_233.0, 0, 0),
        (-15.25, 2_478.0, 0, 3),
        (-14.75, 2_675.0, 0, 11),
        (-14.25, 1_973.0, 0, 25),
        (-13.75, 2_083.0, 0, 35),
        (-13.25, 1_706.0, 1, 29),
        (-12.75, 1_312.0, 3, 26),
        (-12.25, 745.0, 4, 38),
        (-11.75, 493.0, 8, 74),
        (-11.25, 357.0, 37, 78),
        (-10.75, 66.0, 13, 19),
        (-10.25, 44.0, 4, 8),
        (-9.75, 16.0, 0, 2),
        (-9.25, 46.0, 0, 25),
    ];

    /// Melatos et al. (2008), Table 1: the Vela pulsar's 17 glitches, epoch (MJD) and Δν ÷ ν
    /// (10⁻⁹).
    const VELA_GLITCHES: [(f64, f64); 17] = [
        (40_280.0, 2_340.0),
        (41_192.0, 2_050.0),
        (41_312.0, 12.0),
        (42_683.0, 1_990.0),
        (43_693.0, 3_060.0),
        (44_888.070_7, 1_145.0),
        (45_192.0, 2_050.0),
        (46_257.228_4, 1_601.0),
        (47_519.803, 1_807.1),
        (48_457.382, 2_715.0),
        (48_550.0, 5.6),
        (49_559.057, 835.0),
        (49_591.158, 199.0),
        (50_369.345, 2_110.0),
        (51_559.345, 3_120.0),
        (53_195.09, 2_100.0),
        (53_959.93, 2_620.0),
    ];

    /// Melatos et al. (2008), Table 1: the Crab pulsar's 26 glitches' Δν ÷ ν (10⁻⁹), `None` for
    /// the three listed "n/a".
    const CRAB_SIZES: [Option<f64>; 26] = [
        Some(4.0),
        Some(2.2),
        Some(2.0),
        Some(44.0),
        Some(1.1),
        Some(2.8),
        Some(4.1),
        Some(85.0),
        Some(4.5),
        Some(2.7),
        Some(22.0),
        Some(7.67),
        Some(6.67),
        Some(8.67),
        Some(9.67),
        Some(24.0),
        Some(3.3),
        Some(23.6),
        Some(8.0),
        Some(2.6),
        Some(1.1),
        Some(210.0),
        Some(4.84),
        None,
        None,
        None,
    ];

    /// The Crab pulsar's ν (Hz) and |ν̇| (Hz s⁻¹): the ATNF pulsar catalogue, version 2.9.0
    /// (from Lyne et al. 2015).
    const CRAB: (f64, f64) = (29.946_923_0, 3.775_35e-10);

    /// PSR J0537−6910's |ν̇|, Hz s⁻¹: the ATNF pulsar catalogue, version 2.9.0.
    const J0537_SPIN_DOWN: f64 = 1.992_272_0e-10;

    /// Espinoza et al. (2011), Table 4, the rows up to log |ν̇| = −13.5 with glitches: log⟨|ν̇|⟩
    /// in 10⁻¹⁵ Hz s⁻¹, Σ `T_i` (yr), `N_g`, Σ Δν (μHz) and `ν̇_glitch` (10⁻¹⁵ Hz s⁻¹) as printed.
    const ESPINOZA_TABLE_4: [(f64, f64, u32, f64, f64); 4] = [
        (-0.22, 1_895.0, 2, 0.0009, 0.000_01),
        (0.25, 2_104.0, 7, 0.003, 0.000_05),
        (0.77, 1_552.0, 18, 0.84, 0.017),
        (1.24, 1_671.0, 28, 13.0, 0.024),
    ];

    /// Espinoza et al. (2011), Table 4: log⟨|ν̇|⟩ (10⁻¹⁵ Hz s⁻¹) and ⟨`Ṅ_g`⟩ (a year), every row.
    const ESPINOZA_RATES: [(f64, f64); 17] = [
        (-2.03, 0.0),
        (-1.75, 0.0),
        (-1.22, 0.0),
        (-0.73, 0.0),
        (-0.22, 0.001),
        (0.25, 0.003),
        (0.77, 0.012),
        (1.24, 0.016),
        (1.73, 0.027),
        (2.25, 0.04),
        (2.77, 0.08),
        (3.26, 0.29),
        (3.78, 0.24),
        (4.31, 0.3),
        (4.74, 0.2),
        (5.29, 1.6),
        (5.58, 0.6),
    ];

    fn draws(spin: f64, field: f64) -> StarDraws {
        StarDraws::from_parts(StarDrawsParts {
            ns_spin: [StandardNormal::new(spin).unwrap(); 8],
            ns_field: StandardNormal::new(field).unwrap(),
            ..StarDrawsParts::MEDIAN
        })
    }

    /// The lifetime of a 15 M☉ star at the median draws, years: the neutron star's draws do not
    /// change it.
    fn lifetime() -> f64 {
        StarModel::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(1e9),
        )
        .unwrap()
        .lifetime()
        .unwrap()
        .value()
    }

    /// A 15 M☉ star whose neutron star is born at the birth-period draw `spin` and the field draw
    /// `field`, `remnant_age` years before the epoch (after it, if negative), given the star's
    /// `life`.
    fn pulsar_with(life: f64, spin: f64, field: f64, remnant_age: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            draws(spin, field),
            Years::new(life + remnant_age),
        )
        .unwrap()
    }

    fn pulsar(spin: f64, field: f64, remnant_age: f64) -> StarModel {
        pulsar_with(lifetime(), spin, field, remnant_age)
    }

    /// A young Crab-like pulsar above the |ν̇| limit: born at 15.0 ms with 10^12.59 G, 970 years
    /// before the epoch (P 33.6 ms, `τ_c` 1,212 years).
    fn crab() -> StarModel {
        pulsar(-2.35, -0.6, 970.0)
    }

    /// A Vela-like pulsar: born at 20 ms with 10^12.53 G, 11,000 years before the epoch (P 90.1
    /// ms, `τ_c` 11,682 years, |ν̇| 1.50 × 10⁻¹¹ Hz s⁻¹).
    fn vela() -> StarModel {
        pulsar(-2.3, -0.7, 1.1e4)
    }

    /// A pulsar of `τ_c` about 10⁶ years: the median draws, 630,000 years before the epoch.
    fn old() -> StarModel {
        pulsar(0.0, 0.0, 6.3e5)
    }

    /// A high-field pulsar, not a magnetar at the reference: the median period with 10^14 G, 1,000
    /// years before the epoch (B 9.9 × 10¹³ G).
    fn high_field() -> StarModel {
        pulsar(0.0, 1.75, 1_000.0)
    }

    /// A magnetar: the median period with 10^14.51 G, 3,000 years before the epoch.
    fn magnetar() -> StarModel {
        pulsar(0.0, 2.6, 3_000.0)
    }

    /// A neutron star born 500 years after the epoch, at 11 ms with 10^13.01 G, whose Crab-like
    /// rate is the cap's.
    fn newborn() -> StarModel {
        pulsar(-2.39, 0.1, -500.0)
    }

    fn system(n: u64) -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000 | (n << 13)).unwrap()
    }

    fn body(n: u64) -> BodyId {
        BodyId::new(system(n), 0)
    }

    fn small(body: BodyId) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_GLITCH, body.into())
    }

    fn large(body: BodyId) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_GLITCH_CYCLE, body.into())
    }

    /// The first body, by `n`, marked to glitch as a Vela-like pulsar at [`SEED`], and the first
    /// that is not.
    fn marked_and_unmarked() -> (BodyId, BodyId) {
        let marked = (0..64).map(body).find(|&b| is_vela_like_marked(SEED, b));
        let unmarked = (0..64).map(body).find(|&b| !is_vela_like_marked(SEED, b));
        (marked.unwrap(), unmarked.unwrap())
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        if seconds >= 0.0 {
            t.checked_add(Span::from_seconds_f64(seconds).unwrap())
                .unwrap()
        } else {
            t.checked_sub(Span::from_seconds_f64(-seconds).unwrap())
                .unwrap()
        }
    }

    fn years(y: f64) -> UniverseTime {
        after(UniverseTime::EPOCH, y * YEAR)
    }

    fn span(from: f64, to: f64) -> TimeWindow {
        TimeWindow::new(years(from), years(to)).unwrap()
    }

    fn list(star: &StarModel, series: &EventSeries, w: TimeWindow) -> Vec<Glitch> {
        let mut out = Vec::new();
        glitches_in(star, series, w, &mut out);
        out
    }

    fn list_large(star: &StarModel, body: BodyId, w: TimeWindow) -> Vec<LargeGlitch> {
        let mut out = Vec::new();
        large_glitches_in(star, SEED, body, w, &mut out);
        out
    }

    fn active(star: &StarModel, series: &EventSeries, t: UniverseTime) -> Vec<Glitch> {
        let mut out = Vec::new();
        glitches_active_at(star, series, t, &mut out);
        out
    }

    /// A count as an `f64`, exact.
    fn real(n: usize) -> f64 {
        f64::from(u32::try_from(n).unwrap())
    }

    /// A count as an `f64`, exact.
    fn real64(n: u64) -> f64 {
        f64::from(u32::try_from(n).unwrap())
    }

    fn per_year(rate: EventsPerSecond) -> f64 {
        rate.value() * YEAR
    }

    /// Φ(z) by the error function, as the size law's closed form needs it.
    fn phi(z: f64) -> f64 {
        0.5 * math::erfc(-z / SQRT_2)
    }

    /// The Poisson maximum-likelihood fit of R₁₂ and b to Table 1's small-glitch counts, by
    /// Newton's method on (ln R₁₂, b): (R₁₂, b, σ of ln R₁₂, σ of b, deviance, the expected
    /// counts).
    fn fit_table_1() -> (f64, f64, f64, f64, f64, Vec<f64>) {
        let rows: Vec<(f64, f64, f64)> = FUENTES_TABLE_1
            .iter()
            .map(|&(log, t, nl, nt)| ((log + 12.0) * LN_10, t, f64::from(nt - nl)))
            .collect();
        let expected = |c: f64, b: f64| -> Vec<f64> {
            rows.iter()
                .map(|&(x, t, _)| t * math::exp(c + b * x))
                .collect()
        };
        let (mut c, mut b) = (math::ln(0.05), 0.37);
        let mut curvature = (0.0, 0.0, 0.0);
        for _ in 0..60 {
            let mu = expected(c, b);
            let (mut g0, mut g1, mut h00, mut h01, mut h11) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for (&(x, _, n), &m) in rows.iter().zip(&mu) {
                g0 += n - m;
                g1 += (n - m) * x;
                h00 += m;
                h01 += m * x;
                h11 += m * x * x;
            }
            let det = h00 * h11 - h01 * h01;
            c += (h11 * g0 - h01 * g1) / det;
            b += (h00 * g1 - h01 * g0) / det;
            curvature = (h00, h01, h11);
        }
        let mu = expected(c, b);
        let deviance = 2.0
            * rows
                .iter()
                .zip(&mu)
                .map(|(&(_, _, n), &m)| {
                    if n > 0.0 {
                        n * math::ln(n / m) - (n - m)
                    } else {
                        m
                    }
                })
                .sum::<f64>();
        let (h00, h01, h11) = curvature;
        let det = h00 * h11 - h01 * h01;
        (
            math::exp(c),
            b,
            (h11 / det).sqrt(),
            (h00 / det).sqrt(),
            deviance,
            mu,
        )
    }

    /// R₁₂ and b are the fit to Fuentes et al.'s Table 1 that their documentation states, to 10⁻³,
    /// with its errors and deviance; the cap holds the rate at 0.737 a year.
    #[test]
    fn the_crab_like_rate_is_the_fit_to_fuentes_et_al_s_table_1() {
        let (r12, b, sigma_ln, sigma_b, deviance, mu) = fit_table_1();
        assert!(
            (r12 / SMALL_GLITCH_RATE_PER_YEAR - 1.0).abs() < 1e-3,
            "R₁₂ {r12}"
        );
        assert!((b / SMALL_GLITCH_RATE_INDEX - 1.0).abs() < 1e-3, "b {b}");
        assert!((sigma_ln - 0.058).abs() < 1e-3, "σ of ln R₁₂ {sigma_ln}");
        assert!((sigma_b - 0.016).abs() < 1e-3, "σ of b {sigma_b}");
        assert!((deviance - 63.7).abs() < 0.05, "deviance {deviance}");
        let rows: Vec<String> = FUENTES_TABLE_1
            .iter()
            .zip(&mu)
            .map(|(&(log, _, nl, nt), m)| format!("{log}: {} / {m:.2}", nt - nl))
            .collect();
        eprintln!(
            "the fit: R12 {r12:.5} a year (sigma of ln {sigma_ln:.4}), b {b:.4} ± {sigma_b:.4}, \
             deviance {deviance:.2} on 14; observed / expected: {}",
            rows.join(", ")
        );
        // The constants against the three highest bins: 11.1 against 4, 6.2 against 2, 27.3
        // against 25.
        for (i, (low, high)) in [(13, (10.9, 11.3)), (14, (6.0, 6.4)), (15, (27.0, 27.6))] {
            let (log, t, _, _) = FUENTES_TABLE_1[i];
            let expected = per_year(small_glitch_rate(math::exp10(log))) * t;
            assert!((low..high).contains(&expected), "bin {}: {expected}", i + 1);
        }
        let cap = per_year(small_glitch_rate(RATE_CAP_HZ_PER_S));
        assert!((0.736..0.738).contains(&cap), "{cap}");
        assert_same_bits(per_year(small_glitch_rate(1e-6)), cap);
        assert!(cap * f64::from(30) / 365.25 < 0.061);
    }

    /// `P_Vela` is the mean of the intervals between Vela's 13 large glitches in Melatos et al.'s
    /// Table 1, to 10⁻³; their sizes are the large component's; the share is 420 Hz⁻¹ × C, to
    /// 10⁻³; and the three checks of the period hold.
    #[test]
    fn the_vela_like_constants_are_their_sources() {
        let large: Vec<(f64, f64)> = VELA_GLITCHES
            .iter()
            .copied()
            .filter(|&(_, x)| x * 1e-9 * VELA_FREQUENCY_HZ >= 10e-6)
            .collect();
        assert_eq!(large.len(), 13);
        let intervals: Vec<f64> = large
            .windows(2)
            .map(|p| (p[1].0 - p[0].0) / 365.25)
            .collect();
        let n = 12.0;
        let mean = intervals.iter().sum::<f64>() / n;
        let sd = (intervals
            .iter()
            .map(|x| (x - mean) * (x - mean))
            .sum::<f64>()
            / (n - 1.0))
            .sqrt();
        assert!((mean / VELA_PERIOD_YEARS - 1.0).abs() < 1e-3, "{mean}");
        assert!(
            (sd - 1.15).abs() < 0.005 && (sd / mean - 0.37).abs() < 0.005,
            "{sd}"
        );
        let logs: Vec<f64> = large
            .iter()
            .map(|&(_, x)| math::log10(x * 1e-3 * VELA_FREQUENCY_HZ))
            .collect();
        let log_mean = logs.iter().sum::<f64>() / 13.0;
        let log_sd = (logs
            .iter()
            .map(|x| (x - log_mean) * (x - log_mean))
            .sum::<f64>()
            / 12.0)
            .sqrt();
        assert!((log_mean - 1.379).abs() < 5e-4, "{log_mean}");
        assert!((log_sd - 0.118).abs() < 5e-4, "{log_sd}");
        // C, the share and its three checks.
        assert!((PERIOD_SPIN_DOWN_PRODUCT_HZ / 1.5425e-3 - 1.0).abs() < 1e-4);
        let share = LARGE_GLITCHES_PER_SPIN_DOWN * PERIOD_SPIN_DOWN_PRODUCT_HZ;
        assert!((share / VELA_LIKE_SHARE - 1.0).abs() < 1e-3, "{share}");
        let fuentes_interval =
            1.0 / (LARGE_GLITCHES_PER_SPIN_DOWN * VELA_SPIN_DOWN_HZ_PER_S) / YEAR;
        assert!(
            (fuentes_interval - 4.82).abs() < 0.005,
            "{fuentes_interval}"
        );
        assert!((fuentes_interval / (VELA_PERIOD_YEARS / share) - 1.0).abs() < 1e-12);
        let j0537 = PERIOD_SPIN_DOWN_PRODUCT_HZ / J0537_SPIN_DOWN / YEAR;
        assert!((j0537 - 0.2453).abs() < 5e-4, "{j0537}");
        // The activities: 1.63% for a Vela-like star, 1.05% for the population.
        let star = LARGE_MEAN_SIZE_HZ / PERIOD_SPIN_DOWN_PRODUCT_HZ;
        assert!((star - 0.0163).abs() < 5e-5, "{star}");
        assert!((VELA_LIKE_SHARE * star - 0.0105).abs() < 5e-5, "{star}");
        eprintln!(
            "Vela: {} intervals, mean {mean:.4} yr, sd {sd:.3} (cv {:.3}); log10 dnu \
             {log_mean:.4} ± {log_sd:.4}; C {:.5e} Hz; share {share:.5}; Fuentes et al.'s \
             interval {fuentes_interval:.3} yr; J0537-6910 {j0537:.4} yr; activity {:.4} \
             (population {:.4})",
            intervals.len(),
            sd / mean,
            PERIOD_SPIN_DOWN_PRODUCT_HZ,
            star,
            VELA_LIKE_SHARE * star,
        );
    }

    /// The Crab-like law's domain is the Crab's smallest and largest glitch in Melatos et al.'s
    /// Table 1; its inverse transform meets its ends, and its mean and median are the stated ones.
    #[test]
    fn the_crab_like_size_law_has_its_domain_mean_and_median() {
        let sizes: Vec<f64> = CRAB_SIZES.iter().flatten().copied().collect();
        assert_eq!((CRAB_SIZES.len(), sizes.len()), (26, 23));
        let low = sizes.iter().copied().fold(f64::INFINITY, f64::min);
        let high = sizes.iter().copied().fold(0.0, f64::max);
        assert!(
            (low * 1e-9 / MIN_FRACTIONAL_SIZE - 1.0).abs() < 1e-15,
            "{low}"
        );
        assert!(
            (high * 1e-9 / MAX_FRACTIONAL_SIZE - 1.0).abs() < 1e-15,
            "{high}"
        );
        let bottom = crab_fractional_size_at(0.0);
        assert!(
            bottom >= MIN_FRACTIONAL_SIZE && bottom / MIN_FRACTIONAL_SIZE - 1.0 < 1e-14,
            "{bottom:e}"
        );
        let top = crab_fractional_size_at(1.0 - f64::EPSILON / 2.0);
        assert!(
            top <= MAX_FRACTIONAL_SIZE && top > 0.999 * MAX_FRACTIONAL_SIZE,
            "{top}"
        );
        let a = CRAB_SIZE_INDEX;
        let mean = (1.0 - a) / (2.0 - a)
            * (math::powf(MAX_FRACTIONAL_SIZE, 2.0 - a) - math::powf(MIN_FRACTIONAL_SIZE, 2.0 - a))
            / (math::powf(MAX_FRACTIONAL_SIZE, 1.0 - a) - math::powf(MIN_FRACTIONAL_SIZE, 1.0 - a));
        assert!((mean / 2.78e-8 - 1.0).abs() < 1e-3, "{mean}");
        // The mean of the inverse transform by the midpoint rule in u.
        let n = 200_000_u32;
        let numeric = (0..n)
            .map(|i| crab_fractional_size_at((f64::from(i) + 0.5) / f64::from(n)))
            .sum::<f64>()
            / f64::from(n);
        assert!((numeric / mean - 1.0).abs() < 1e-4, "{numeric}");
        let median = crab_fractional_size_at(0.5);
        assert!((median / 7.86e-9 - 1.0).abs() < 2e-3, "{median}");
        // Every Crab-like step is below 10 μHz while ν is below 47.6 Hz.
        assert!((10e-6 / top - 47.62).abs() < 0.01, "{}", 10e-6 / top);
    }

    /// The Vela-like size law meets its truncation at both ends, its median is 10^1.38 μHz, and
    /// its mean is the closed form's, to the last bits.
    #[test]
    fn the_vela_like_size_law_is_the_truncated_log_normal() {
        let s = LARGE_LOG_SIZE_SIGMA * LN_10;
        let closed = math::exp10(LARGE_LOG_SIZE_MEAN)
            * math::exp(s * s / 2.0)
            * (phi(4.0 - s) - phi(-4.0 - s))
            / (phi(4.0) - phi(-4.0))
            * 1e-6;
        assert!(
            (closed / LARGE_MEAN_SIZE_HZ - 1.0).abs() < 1e-14,
            "{closed:e}"
        );
        let low = large_size_hz_at(0.0);
        let high = large_size_hz_at(1.0 - f64::EPSILON / 2.0);
        assert!((low / 7.244e-6 - 1.0).abs() < 1e-3, "{low:e}");
        assert!((high / 79.43e-6 - 1.0).abs() < 1e-3, "{high:e}");
        assert!((large_size_hz_at(0.5) / (math::exp10(1.38) * 1e-6) - 1.0).abs() < 1e-12);
        let n = 200_000_u32;
        let numeric = (0..n)
            .map(|i| large_size_hz_at((f64::from(i) + 0.5) / f64::from(n)))
            .sum::<f64>()
            / f64::from(n);
        assert!(
            (numeric / LARGE_MEAN_SIZE_HZ - 1.0).abs() < 1e-5,
            "{numeric:e}"
        );
        // The checks: Melatos et al.'s (Δν ÷ ν)_c = 2.5 × 10⁻⁶ is 28.0 μHz at Vela's ν, and the
        // median reaches 10⁻⁶ in Δν ÷ ν only at ν = 24 Hz.
        assert!((2.5e-6 * VELA_FREQUENCY_HZ / 28.0e-6 - 1.0).abs() < 1e-3);
        let median = large_size_hz_at(0.5);
        assert!((median / 1e-6 - 24.0).abs() < 0.05);
        assert!((median / VELA_FREQUENCY_HZ - 2.14e-6).abs() < 5e-9);
    }

    /// Living stars, white dwarfs, black holes, magnetars, old pulsars and neutron stars born
    /// after the clock window have no glitch model; and only a marked pulsar of `τ_c` 10⁴–10⁵ years
    /// below the |ν̇| limit has large glitches.
    #[test]
    fn only_young_rotation_powered_pulsars_glitch_and_only_marked_vela_like_ones_cycle() {
        let life = lifetime();
        let star = |mass: f64, age: f64| {
            StarModel::new(
                SolarMasses::new(mass),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
            .unwrap()
        };
        let living = star(15.0, 1e6);
        let white_dwarf = star(3.0, 1e9);
        let black_hole = star(40.0, 2e7);
        assert!(
            black_hole.black_hole().is_some(),
            "a 40 M☉ star leaves a black hole"
        );
        assert!(
            white_dwarf.neutron_star().is_none(),
            "a 3 M☉ star leaves a white dwarf"
        );
        let magnetar = magnetar();
        let state = magnetar.pulsar_at(UniverseTime::EPOCH).unwrap();
        assert!(
            state.is_magnetar() && state.field() > Gauss::new(4.4e13),
            "{state:?}"
        );
        let ancient = pulsar_with(life, 0.0, 0.0, 5.0e7);
        let tau = ancient
            .pulsar_at(UniverseTime::EPOCH)
            .unwrap()
            .characteristic_age();
        assert!(tau.value() >= MAX_CHARACTERISTIC_AGE.value(), "{tau:?}");
        let unborn = pulsar_with(life, 0.0, 0.0, -2_000.0);
        assert!(unborn.remnant_age_at(ClockWindow::END).is_none());
        for (name, model) in [
            ("a living star", &living),
            ("a white dwarf", &white_dwarf),
            ("a black hole", &black_hole),
            ("a magnetar", &magnetar),
            ("a pulsar of τ_c 10⁷ years and more", &ancient),
            ("a neutron star born after +H", &unborn),
        ] {
            assert_eq!(GlitchModel::of(model), None, "{name}");
            let (marked, _) = marked_and_unmarked();
            assert!(
                list(model, &small(marked), span(0.0, 300.0)).is_empty(),
                "{name}"
            );
            assert!(
                list_large(model, marked, span(0.0, 300.0)).is_empty(),
                "{name}"
            );
            assert_same_bits(
                frequency_offset_hz_at(model, SEED, marked, years(10.0)),
                0.0,
            );
        }
        let (marked, unmarked) = marked_and_unmarked();
        let vela = vela();
        assert!(GlitchCycle::of(&vela, SEED, marked).is_some());
        assert!(GlitchCycle::of(&vela, SEED, unmarked).is_none());
        // Outside 10⁴–10⁵ years, at or above the |ν̇| limit with τ_c inside, and a magnetar.
        let fast = pulsar_with(life, -2.39, -1.25, 1.0e4);
        let fast_model = GlitchModel::of(&fast).unwrap();
        assert!(
            fast_model.spin_down_hz_per_s() >= GLITCH_NU_DOT_LIMIT
                && (1e4..1e5).contains(&fast_model.characteristic_age().value()),
            "{fast_model:?}"
        );
        for (name, model) in [
            ("a Crab-like pulsar", crab()),
            ("a pulsar of τ_c 10⁶ years", old()),
            ("a pulsar above the |ν̇| limit", fast),
            ("a magnetar", magnetar),
        ] {
            assert!(GlitchCycle::of(&model, SEED, marked).is_none(), "{name}");
        }
    }

    /// The parent's test: the bound is never exceeded, over T21.a's birth periods and fields
    /// (the magnetar tail included), ages from birth to `τ_c` = 10⁷ years, both sides of the |ν̇|
    /// limit and the cap's range; and a bin's mean is at most 64.
    #[test]
    fn the_bound_holds_across_the_kind_s_range() {
        // The law over a dense sweep of neutron stars at their states.
        let (mut below, mut above, mut capped) = (0_u32, 0_u32, 0_u32);
        for p0 in [0.0101, 0.02, 0.05, 0.15, 0.25, 0.45, 0.75] {
            for log_b in [11.0, 11.8, 12.5, 12.95, 13.5, 13.9, 14.4, 15.0, 15.95] {
                let ns = NeutronStar::new(
                    Seconds::new(p0),
                    Gauss::new(math::exp10(log_b)),
                    UnitVector::NORTH,
                    Radians::new(1.0),
                    0.0,
                );
                for k in 0..=40 {
                    let age = if k == 0 {
                        0.0
                    } else {
                        math::exp10(f64::from(k) / 5.0)
                    };
                    let state = ns.state_at(Years::new(age));
                    if state.is_magnetar()
                        || state.characteristic_age().value() >= MAX_CHARACTERISTIC_AGE.value()
                    {
                        continue;
                    }
                    let p = state.period().value();
                    let spin_down = state.period_derivative() / (p * p);
                    let rate = small_glitch_rate(spin_down);
                    assert!(
                        rate.value() * BIN_SECONDS_F64 <= PoissonBins::MAX_MEAN_PER_BIN
                            && rate <= small_glitch_rate(RATE_CAP_HZ_PER_S),
                        "P0 {p0} s, log B0 {log_b}, age {age} yr: {rate:?}"
                    );
                    if spin_down >= RATE_CAP_HZ_PER_S {
                        capped += 1;
                    }
                    if spin_down < GLITCH_NU_DOT_LIMIT {
                        below += 1;
                    } else {
                        above += 1;
                    }
                }
            }
        }
        assert!(
            below > 200 && above > 50 && capped > 10,
            "{below}, {above}, {capped}"
        );
        // rate(t) ≤ bound(bin) on star models, at probes across the horizon and a newborn's birth.
        let life = lifetime();
        let bins = bins();
        let mut models = 0;
        for spin in [-2.39, 0.0, 2.0] {
            for field in [-2.0, 0.0, 1.75, 5.0] {
                for age in [-500.0, 0.0, 1e3, 1e5, 3e6] {
                    let star = pulsar_with(life, spin, field, age);
                    let Some(model) = GlitchModel::of(&star) else {
                        continue;
                    };
                    models += 1;
                    let mut probes = vec![
                        SourceHorizon::START,
                        ClockWindow::START,
                        UniverseTime::EPOCH,
                        ClockWindow::END,
                    ];
                    probes.push(model.reference());
                    for t in probes {
                        let window = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
                        let bound = model.bound(window);
                        assert!(bound.value() * BIN_SECONDS_F64 <= PoissonBins::MAX_MEAN_PER_BIN);
                        for fraction in [0.0, 0.25, 0.5, 0.999_999] {
                            let t = after(window.start(), fraction * BIN_SECONDS_F64);
                            assert!(model.rate(t) <= bound, "{spin}, {field}, {age}: {t}");
                        }
                    }
                }
            }
        }
        assert!(models > 30, "{models} glitching models");
    }

    /// A Crab-like glitch's offset is its step at the onset, falls continuously and monotonically
    /// after it, keeps about its lasting part after 20 days, and is exactly 0 from the end of the
    /// look-back.
    #[test]
    fn a_crab_like_offset_recovers_continuously_to_zero_by_the_end_of_the_look_back() {
        assert_same_bits(recovery_profile(RECOVERY_SPAN_SECONDS), 0.0);
        assert!((recovery_profile(0.0) - 1.0).abs() < 1e-15);
        assert_eq!(RECOVERY_SPAN, Span::from_seconds(3_157_056_000));
        assert_eq!(
            RECOVERY_SPAN,
            Span::from_seconds(BIN_SECONDS * i64::from(LOOK_BACK_BINS))
        );
        assert_same_bits(RECOVERY_SPAN_SECONDS, RECOVERY_SPAN.as_seconds_f64());
        #[expect(clippy::cast_precision_loss, reason = "2,592,000 is exact in f64")]
        let bin = BIN_SECONDS as f64;
        assert_same_bits(BIN_SECONDS_F64, bin);
        assert!((RECOVERY_SPAN.as_julian_years_f64() - 100.04).abs() < 0.005);
        let star = crab();
        let glitches = list(&star, &small(body(1)), span(0.0, 30.0));
        assert!(glitches.len() > 5, "{} glitches", glitches.len());
        for g in &glitches {
            let size = g.size_hz();
            assert!(size > 0.0 && size < 10e-6, "{g:?}");
            assert!((g.frequency_offset_hz_at(g.onset()) / size - 1.0).abs() < 1e-14);
            assert_same_bits(g.frequency_offset_hz_at(after(g.onset(), -1e-9)), 0.0);
            assert_same_bits(g.frequency_offset_hz_at(g.end()), 0.0);
            assert!(g.frequency_offset_hz_at(after(g.end(), -1.0)) > 0.0);
            assert!(
                g.is_active_at(g.onset()) && !g.is_active_at(g.end()),
                "{g:?}"
            );
            // Nearly complete within about 20 days (Lyne et al. 2015): the lasting part is left.
            let left = g.frequency_offset_hz_at(after(g.onset(), 20.0 * 86_400.0)) / size;
            assert!((left - (1.0 - RECOVERY_FRACTION)).abs() < 2e-3, "{left}");
            // Continuous and falling: a millisecond moves it by a sliver of the step anywhere.
            let mut previous = size;
            for k in 0..=200 {
                let elapsed = RECOVERY_SPAN_SECONDS * f64::from(k) / 200.0 * (1.0 - 1e-12);
                let elapsed = elapsed.max(f64::from(k.min(1)) * 1e-3);
                let t = after(g.onset(), elapsed);
                let here = g.frequency_offset_hz_at(t);
                let step = (g.frequency_offset_hz_at(after(t, 1e-3)) - here).abs();
                assert!(step < 1e-8 * size, "{step} at {elapsed} s");
                assert!(here <= previous + 1e-20, "rising at {elapsed} s");
                previous = here;
            }
        }
    }

    /// `glitch(id)` regenerates every listed glitch and returns `None` for a rejected candidate
    /// (before the neutron star's birth), a number beyond its bin's count, an ID of another series
    /// and a star that does not glitch.
    #[test]
    fn an_id_regenerates_its_glitch_and_names_nothing_else() {
        let star = newborn();
        let model = GlitchModel::of(&star).unwrap();
        let birth = model.reference();
        assert!(
            birth > UniverseTime::EPOCH && !model.glitches_at(after(birth, -1.0)),
            "born at {birth}"
        );
        let k = birth.seconds().div_euclid(BIN_SECONDS);
        let s = small(body(2));
        let window = TimeWindow::new(
            at((k - 300) * BIN_SECONDS, 0),
            at((k + 300) * BIN_SECONDS, 0),
        )
        .unwrap();
        let listed = list(&star, &s, window);
        assert!(
            listed.len() > 5 && listed.iter().all(|g| g.onset() >= birth),
            "{} glitches",
            listed.len()
        );
        let every = ConstantRate::new(model.small_glitch_rate());
        let (mut rejected, mut beyond) = (0, 0);
        for bin in k - 300..k + 300 {
            for j in 0..6 {
                let id = s.event_id(EventBin::new(bin).unwrap(), j);
                let found = listed.iter().find(|g| g.id() == id);
                assert_eq!(
                    glitch(&star, &s, id).as_ref(),
                    found,
                    "bin {bin}, number {j}"
                );
                match (found, bins().event(&s, id, &every)) {
                    (None, Some(_)) => rejected += 1,
                    (None, None) => beyond += 1,
                    (Some(_), _) => {}
                }
            }
        }
        assert!(
            rejected > 5 && beyond > 3_000,
            "{rejected} rejected, {beyond} beyond"
        );
        let word = listed[0].id().word();
        for foreign in [
            event_id(body(9).into(), tags::STAR_GLITCH, word.bin(), word.number()),
            event_id(
                body(2).into(),
                event_tags::SELF_TEST,
                word.bin(),
                word.number(),
            ),
            event_id(
                body(2).into(),
                tags::STAR_GLITCH_CYCLE,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(glitch(&star, &s, foreign), None);
        }
        assert_eq!(glitch(&magnetar(), &s, listed[0].id()), None);
    }

    /// `large_glitch` regenerates every listed large glitch; a newborn Vela-like star lists no
    /// cycle before its birth, whose regeneration is `None` too.
    #[test]
    fn a_cycle_regenerates_its_large_glitch_and_none_falls_before_the_birth() {
        let (marked, _) = marked_and_unmarked();
        let series = large(marked);
        let star = vela();
        let listed = list_large(&star, marked, span(-60.0, 60.0));
        assert!((30..=45).contains(&listed.len()), "{}", listed.len());
        for g in &listed {
            assert_eq!(
                large_glitch(&star, SEED, marked, g.cycle()).as_ref(),
                Some(g)
            );
            assert_eq!(
                g.id(),
                series.event_id(EventBin::new(g.cycle()).unwrap(), 0)
            );
            assert!((g.fractional_size() * VELA_FREQUENCY_HZ / g.size_hz() - 1.0).abs() < 0.01);
        }
        assert!(listed.windows(2).all(|p| p[1].cycle() == p[0].cycle() + 1));
        // The median draws born 500 years after the epoch: τ_c 1.3 × 10⁴ years at birth.
        let young = pulsar(0.0, 0.0, -500.0);
        let cycles = GlitchCycle::of(&young, SEED, marked).unwrap();
        assert_eq!(*cycles.series(), series);
        let birth = cycles.model().reference();
        let around = list_large(&young, marked, span(400.0, 700.0));
        assert!(
            !around.is_empty() && around.iter().all(|g| g.onset() >= birth),
            "{around:?} against a birth at {birth}"
        );
        let before = around[0].cycle() - 1;
        assert_eq!(large_glitch(&young, SEED, marked, before), None);
        assert_same_bits(cycles.frequency_offset_hz_at(after(birth, -1.0)), 0.0);
    }

    /// The window and cut sets of the partition tests: two centuries, cut at instants of each
    /// kind's own events and at bin edges.
    fn partition_case<E>(
        listing: impl Fn(TimeWindow) -> Vec<E>,
        onset: impl Fn(&E) -> UniverseTime,
    ) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let whole = TimeWindow::new(
            at(-100 * 31_557_600 - 1_234, 5),
            at(100 * 31_557_600 + 77, 0),
        )
        .unwrap();
        let events = listing(whole);
        assert!(events.len() > 30, "{} events", events.len());
        let cuts = vec![
            vec![at(0, 0)],
            vec![
                at(-BIN_SECONDS, 0),
                at(-BIN_SECONDS, 0),
                at(BIN_SECONDS, 1),
                at(40 * BIN_SECONDS + 999, 3),
            ],
            vec![onset(&events[10]), after(onset(&events[11]), 1e-9)],
            (-4..5)
                .map(|d| at(d * 20 * 31_557_600 + 3_600 * d, 0))
                .collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test through the public functions, for both kinds: a listing over a
    /// window is the union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let star = crab();
        let s = small(body(3));
        let listing = |w: TimeWindow| list(&star, &s, w);
        let (whole, cut_sets) = partition_case(listing, Glitch::onset);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
        let (marked, _) = marked_and_unmarked();
        let vela = vela();
        let listing = |w: TimeWindow| list_large(&vela, marked, w);
        let (whole, cut_sets) = partition_case(listing, LargeGlitch::onset);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
    }

    /// The pieces listed on threads of their own give the same glitches. Not on wasm32-wasip1,
    /// which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let star = crab();
        let s = small(body(3));
        let listing = |w: TimeWindow| list(&star, &s, w);
        let (whole, cut_sets) = partition_case(listing, Glitch::onset);
        let glitches = listing(whole);
        let (marked, _) = marked_and_unmarked();
        let vela = vela();
        let listing_large = |w: TimeWindow| list_large(&vela, marked, w);
        let large_glitches = listing_large(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let (threaded, threaded_large): (Vec<Vec<Glitch>>, Vec<Vec<LargeGlitch>>) =
                std::thread::scope(|scope| {
                    let handles: Vec<_> = pieces
                        .iter()
                        .map(|&w| scope.spawn(move || (listing(w), listing_large(w))))
                        .collect();
                    handles.into_iter().map(|h| h.join().unwrap()).unzip()
                });
            assert_eq!(threaded.concat(), glitches);
            assert_eq!(threaded_large.concat(), large_glitches);
        }
    }

    /// `glitches_active_at` gives exactly the listed glitches whose offset is not yet 0 at t.
    #[test]
    fn glitches_active_at_gives_the_glitches_still_recovering_then() {
        let star = crab();
        let s = small(body(4));
        let listed = list(&star, &s, span(-101.0, 12.0));
        let mut seen = 0;
        for i in 0..200 {
            let t = after(UniverseTime::EPOCH, f64::from(i) * 0.05 * YEAR + 17.0);
            let expected: Vec<Glitch> = listed
                .iter()
                .copied()
                .filter(|g| g.is_active_at(t))
                .collect();
            let got = active(&star, &s, t);
            assert_eq!(got, expected, "at {t}");
            seen += got.len();
        }
        assert!(seen > 200 * 30, "{seen} active glitches seen");
    }

    /// The parent's no-history test for this kind: the glitches active at +900 years and the
    /// summed offset there, asked cold, are the same after asking about 100 earlier times.
    #[test]
    fn the_glitches_at_900_years_need_no_history() {
        let (marked, _) = marked_and_unmarked();
        let t = years(900.0);
        for make in [crab as fn() -> StarModel, vela] {
            let star = make();
            let cold = active(&star, &small(marked), t);
            let cold_offset = frequency_offset_hz_at(&star, SEED, marked, t);
            let mut lcg = Lcg::new(0x0928_b900);
            for _ in 0..100 {
                let earlier = after(ClockWindow::START, lcg.next_f64() * 1.899e3 * YEAR);
                assert!(earlier < t);
                let _ = active(&star, &small(marked), earlier);
                let _ = frequency_offset_hz_at(&star, SEED, marked, earlier);
                let _ = list_large(
                    &star,
                    marked,
                    TimeWindow::new(earlier, after(earlier, 10.0 * YEAR)).unwrap(),
                );
            }
            assert_eq!(active(&star, &small(marked), t), cold);
            assert_same_bits(frequency_offset_hz_at(&star, SEED, marked, t), cold_offset);
            assert!(
                !cold.is_empty() && cold_offset > 0.0,
                "{} active, offset {cold_offset:e}",
                cold.len()
            );
        }
    }

    /// The sawtooth jumps by each glitch's Δν at the listed time (± 1 s), ends each cycle at
    /// −⟨Δν⟩ ÷ 2, falls within a cycle, and has zero mean over cycles; the sizes follow the
    /// truncated log-normal.
    #[test]
    fn the_sawtooth_jumps_by_each_step_and_has_zero_mean_over_cycles() {
        let (marked, _) = marked_and_unmarked();
        let star = vela();
        let cycles = GlitchCycle::of(&star, SEED, marked).unwrap();
        let listed = list_large(&star, marked, span(-1_500.0, 1_500.0));
        assert!(listed.len() > 800, "{}", listed.len());
        let half = 0.5 * LARGE_MEAN_SIZE_HZ;
        for pair in listed.windows(2) {
            let (g, next) = (pair[0], pair[1]);
            let before = cycles.frequency_offset_hz_at(after(g.onset(), -1.0));
            let at_glitch = cycles.frequency_offset_hz_at(after(g.onset(), 1.0));
            assert!(
                (at_glitch - before - g.size_hz()).abs() < 1e-6 * g.size_hz(),
                "{g:?}"
            );
            assert!(
                (before + half).abs() < 1e-6 * LARGE_MEAN_SIZE_HZ,
                "{before}"
            );
            let middle = after(
                g.onset(),
                0.5 * next
                    .onset()
                    .checked_since(g.onset())
                    .unwrap()
                    .as_seconds_f64(),
            );
            let mid = cycles.frequency_offset_hz_at(middle);
            let end = cycles.frequency_offset_hz_at(after(next.onset(), -1.0));
            assert!(
                mid < at_glitch && mid > end,
                "{at_glitch:e}, {mid:e}, {end:e}"
            );
        }
        // The mean over cycles: each cycle's (Δνₙ − ⟨Δν⟩) ÷ 2 averages to zero.
        let n = real(listed.len());
        let sizes: Vec<f64> = listed.iter().map(LargeGlitch::size_hz).collect();
        let mean = sizes.iter().sum::<f64>() / n;
        let sd = (sizes.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0)).sqrt();
        assert!(
            (mean - LARGE_MEAN_SIZE_HZ).abs() < 4.0 * sd / n.sqrt(),
            "{mean:e}"
        );
        // And over time: the offset at 20,000 instants of the 3,000 years.
        let mut lcg = Lcg::new(0x5a77_0074);
        let samples: Vec<f64> = (0..20_000)
            .map(|_| cycles.frequency_offset_hz_at(years(-1_500.0 + 3_000.0 * lcg.next_f64())))
            .collect();
        let time_mean = samples.iter().sum::<f64>() / 20_000.0;
        // Samples within one cycle share its size, so the error is the cycles', about sd ÷ 2 ÷ √n.
        assert!(
            time_mean.abs() < 4.0 * 0.5 * sd / n.sqrt() + 0.05 * half,
            "{time_mean:e}"
        );
        // The sizes against the truncated log-normal.
        let edge = phi(-LARGE_SIZE_TRUNCATION);
        let kept = phi(LARGE_SIZE_TRUNCATION) - edge;
        let mut sorted = sizes.clone();
        let ks = ks_one_sample(&mut sorted, |dnu| {
            let z = (math::log10(dnu * 1e6) - LARGE_LOG_SIZE_MEAN) / LARGE_LOG_SIZE_SIGMA;
            ((normal_cdf(z) - edge) / kept).clamp(0.0, 1.0)
        });
        assert_p_value("large glitch sizes", ks.p_value, ALPHA);
        eprintln!(
            "{} large glitches over 3,000 years: mean size {:.3} μHz (law {:.3}), time mean of the \
             offset {:.4} μHz",
            listed.len(),
            mean * 1e6,
            LARGE_MEAN_SIZE_HZ * 1e6,
            time_mean * 1e6
        );
    }

    /// The Crab-like sizes listed over a long window follow the power law (K–S).
    #[test]
    fn the_crab_like_sizes_follow_the_power_law() {
        let star = crab();
        let mut sizes: Vec<f64> = list(&star, &small(body(5)), span(-500.0, 500.0))
            .iter()
            .map(Glitch::fractional_size)
            .collect();
        assert!(sizes.len() > 300, "{}", sizes.len());
        let power = 1.0 - CRAB_SIZE_INDEX;
        let (low, high) = (
            math::powf(MIN_FRACTIONAL_SIZE, power),
            math::powf(MAX_FRACTIONAL_SIZE, power),
        );
        let ks = ks_one_sample(&mut sizes, |x| (math::powf(x, power) - low) / (high - low));
        assert_p_value("Crab-like sizes", ks.p_value, ALPHA);
    }

    /// The mark: the share of marked bodies is 0.648 within a binomial interval; it depends on
    /// the seed and the body alone, the same for every time, window and reference.
    #[test]
    fn the_mark_is_a_fixed_property_of_the_body_at_the_vela_like_share() {
        let n = 20_000_u64;
        let marked = (0..n)
            .filter(|&i| is_vela_like_marked(SEED, body(i)))
            .count();
        let marked = u64::try_from(marked).unwrap();
        let total = real64(n);
        let fit = chi_square_gof(
            &[marked, n - marked],
            &[total * VELA_LIKE_SHARE, total * (1.0 - VELA_LIKE_SHARE)],
        );
        assert_p_value("the Vela-like share", fit.p_value, ALPHA);
        // The same mark on two stars of different references and on any window or instant.
        let (vela, young) = (vela(), pulsar(0.0, 0.0, -500.0));
        let mut differs = 0;
        for i in 0..64 {
            let b = body(i);
            let mark = is_vela_like_marked(SEED, b);
            assert_eq!(GlitchCycle::of(&vela, SEED, b).is_some(), mark);
            assert_eq!(GlitchCycle::of(&young, SEED, b).is_some(), mark);
            let open = Stream::open(SEED, STAR_NS_GLITCH_CLASS, ObjectKey::from(b))
                .decide(Threshold::from_probability(VELA_LIKE_SHARE));
            assert_eq!(open, mark);
            let other = Seed::new(SEED.get() ^ 1);
            differs += usize::from(is_vela_like_marked(other, b) != mark);
        }
        assert!(differs > 10, "{differs}");
    }

    /// The phase's fraction at the reference is uniform over Vela-like stars (K–S), so that they
    /// do not all glitch together at the epoch; and the intervals' scatter is what the slope bound
    /// allows.
    #[test]
    fn the_phase_at_the_reference_is_uniform_over_stars() {
        let star = vela();
        let mut fractions = Vec::new();
        let mut intervals = Vec::new();
        for i in 0..4_000 {
            let b = body(i);
            let Some(cycles) = GlitchCycle::of(&star, SEED, b) else {
                continue;
            };
            let (_, fraction) = cycles.noise().cycle_at(
                cycles.series(),
                &cycles.clock(),
                cycles.model().reference(),
            );
            fractions.push(fraction);
            if i < 300 {
                let listed = list_large(&star, b, span(-150.0, 150.0));
                intervals.extend(listed.windows(2).map(|p| {
                    p[1].onset()
                        .checked_since(p[0].onset())
                        .unwrap()
                        .as_julian_years_f64()
                }));
            }
        }
        assert!(fractions.len() > 2_400, "{}", fractions.len());
        let ks = ks_one_sample(&mut fractions, |f| f);
        assert_p_value("the phase's fraction at the reference", ks.p_value, ALPHA);
        let n = real(intervals.len());
        let mean = intervals.iter().sum::<f64>() / n;
        let sd = (intervals
            .iter()
            .map(|x| (x - mean) * (x - mean))
            .sum::<f64>()
            / (n - 1.0))
            .sqrt();
        let noise = GlitchCycle::of(&star, SEED, marked_and_unmarked().0)
            .unwrap()
            .noise();
        let slope = MonotonePhase::SLOPE_FACTOR * noise.amplitude_cycles()
            / f64::from(noise.lattice_cycles());
        assert!(slope <= MonotonePhase::MAX_SLOPE && slope > 0.89, "{slope}");
        eprintln!(
            "{} intervals: mean {mean:.4} yr, sd {sd:.4} yr, coefficient of variation {:.4} \
             (Vela's 0.37); noise a {}, l {}, J {}, slope bound {slope:.4}, noise bound {:.1} \
             cycles",
            intervals.len(),
            sd / mean,
            noise.amplitude_cycles(),
            noise.lattice_cycles(),
            noise.octaves(),
            noise.noise_bound_cycles(),
        );
        assert!((0.05..0.37).contains(&(sd / mean)), "{}", sd / mean);
    }

    /// The sanity checks against the sources' own figures: the Crab, a Vela-like star at Vela's
    /// parameters, and the period's checks, each printed for the record.
    #[test]
    fn the_sanity_checks_hold_against_the_sources() {
        let (crab_nu, crab_spin_down) = CRAB;
        let crab_rate = per_year(small_glitch_rate(crab_spin_down));
        assert!((crab_rate - 0.512).abs() < 0.003, "{crab_rate}");
        // Fuentes et al.'s bin 16, 25 small glitches in 46 years; Espinoza et al.'s Table 4,
        // 0.6(1); Melatos et al.'s Table 5, λ = 0.91 (0.57–1.3).
        assert!((0.4..0.75).contains(&crab_rate), "{crab_rate}");
        let crab_mean = (1.0 - CRAB_SIZE_INDEX) / (2.0 - CRAB_SIZE_INDEX)
            * (math::powf(MAX_FRACTIONAL_SIZE, 2.0 - CRAB_SIZE_INDEX)
                - math::powf(MIN_FRACTIONAL_SIZE, 2.0 - CRAB_SIZE_INDEX))
            / (math::powf(MAX_FRACTIONAL_SIZE, 1.0 - CRAB_SIZE_INDEX)
                - math::powf(MIN_FRACTIONAL_SIZE, 1.0 - CRAB_SIZE_INDEX));
        let crab_activity = crab_rate / YEAR * crab_nu * crab_mean;
        // Fuentes et al. (section 5.1): 1.1(5) × 10⁻¹⁴ Hz s⁻¹.
        assert!(
            (crab_activity / 1.35e-14 - 1.0).abs() < 0.01,
            "{crab_activity:e}"
        );
        assert!((0.6e-14..1.6e-14).contains(&(crab_activity * 0.9)));
        // A Vela-like star at Vela's parameters: both kinds' rate against Vela's 17 glitches in
        // 37.84 years (Melatos et al., Tables 1 and 2), 4 of them small.
        let vela_small = per_year(small_glitch_rate(VELA_SPIN_DOWN_HZ_PER_S));
        let vela_large = 1.0 / VELA_PERIOD_YEARS;
        let observed_span = (53_960.0 - 40_140.0) / 365.25;
        let observed = 17.0 / observed_span;
        assert!(((vela_small + vela_large) / observed - 1.0).abs() < 0.1);
        // Its activity, ⟨Δν⟩ ÷ P over |ν̇|, against Basu et al.'s 1.8 ± 0.3% and Vela's own.
        let activity = LARGE_MEAN_SIZE_HZ / (VELA_PERIOD_YEARS * YEAR) / VELA_SPIN_DOWN_HZ_PER_S;
        let own: f64 = VELA_GLITCHES
            .iter()
            .map(|&(_, x)| x * 1e-9 * VELA_FREQUENCY_HZ)
            .sum::<f64>()
            / (observed_span * YEAR)
            / VELA_SPIN_DOWN_HZ_PER_S;
        assert!((activity - 0.0163).abs() < 5e-5, "{activity}");
        assert!(
            (activity - 0.018).abs() < 0.003,
            "{activity} against Basu et al.'s 0.018"
        );
        eprintln!(
            "Crab: rate {crab_rate:.4} a year (Fuentes bin 16: {:.3}; Espinoza 0.6(1); Melatos \
             0.91, 0.57–1.3), activity {crab_activity:.3e} Hz/s (Fuentes 1.1(5)e-14); Vela-like at \
             Vela: {vela_small:.4} + {vela_large:.4} = {:.4} a year against {observed:.4} (17 in \
             {observed_span:.2} yr; small {:.4} against 4 in it, {:.4}), activity {activity:.4} \
             (Basu 0.018 ± 0.003; Vela's own {own:.4})",
            25.0 / 46.0,
            vela_small + vela_large,
            vela_small,
            4.0 / observed_span,
        );
    }

    /// Writes one star's lines of the golden: its pulsar at the epoch, its model, its glitches of
    /// both kinds in `window` with their offsets, and its summed offset across the window.
    fn pin_star(w: &mut GoldenWriter, star: &StarModel, window: TimeWindow, body: BodyId) {
        let Some(model) = GlitchModel::of(star) else {
            let state = star.pulsar_at(UniverseTime::EPOCH).unwrap();
            w.f64("period at the epoch (s)", state.period().value());
            w.f64("field at the epoch (G)", state.field().value());
            w.line(&format!("magnetar at the epoch: {}", state.is_magnetar()));
            w.line("no glitch model: a magnetar glitches in neither kind");
            return;
        };
        let state = model.pulsar();
        w.line(&format!("reference {}", model.reference()));
        w.f64("period at the reference (s)", state.period().value());
        w.f64("field at the reference (G)", state.field().value());
        w.line(&format!(
            "magnetar at the reference: {}",
            state.is_magnetar()
        ));
        w.f64("frequency (Hz)", model.frequency_hz());
        w.f64("spin-down (Hz/s)", model.spin_down_hz_per_s());
        w.f64(
            "characteristic age (yr)",
            model.characteristic_age().value(),
        );
        w.f64("Crab-like rate (per s)", model.small_glitch_rate().value());
        let (low, high) = model.fractional_size_bounds();
        w.line(&format!(
            "size law: index {}, x from {low:e} to {high:e}",
            model.size_index()
        ));
        let probes = [
            ("1 s", 1.0),
            ("30 d", 30.0 * 86_400.0),
            ("10 yr", 10.0 * YEAR),
            ("99 yr", 99.0 * YEAR),
        ];
        for g in list(star, &small(body), window) {
            w.line(&format!("{} at {}", g.id(), g.onset()));
            w.f64("  x", g.fractional_size());
            w.f64("  size (Hz)", g.size_hz());
            for (label, seconds) in probes {
                w.f64(
                    &format!("  offset {label} after (Hz)"),
                    g.frequency_offset_hz_at(after(g.onset(), seconds)),
                );
            }
        }
        if let Some(cycles) = GlitchCycle::of(star, SEED, body) {
            w.line(&format!("Vela-like: period {}", cycles.period()));
            let noise = cycles.noise();
            w.line(&format!(
                "noise: a {}, l {}, J {}",
                noise.amplitude_cycles(),
                noise.lattice_cycles(),
                noise.octaves()
            ));
            for g in list_large(star, body, window) {
                w.line(&format!("{} cycle {} at {}", g.id(), g.cycle(), g.onset()));
                w.f64("  size (Hz)", g.size_hz());
                w.f64("  dnu / nu", g.fractional_size());
                for (label, seconds) in [("1 s", 1.0), ("1 yr", YEAR)] {
                    w.f64(
                        &format!("  sawtooth {label} after (Hz)"),
                        cycles.frequency_offset_hz_at(after(g.onset(), seconds)),
                    );
                }
            }
        } else {
            w.line("not Vela-like");
        }
        for k in 0..=6 {
            let t = after(window.start(), f64::from(k) / 6.0 * 30.0 * YEAR);
            w.f64(
                &format!("summed offset at {t} (Hz)"),
                frequency_offset_hz_at(star, SEED, body, t),
            );
        }
    }

    /// Pinned: the marks of 64 bodies; six pulsars at stated draws, one born after the epoch,
    /// their reference instants, rates, periods and size laws, and each glitch's ID, onset, size
    /// and offset at a few instants over a few decades, with the summed offset of each star across
    /// them.
    #[test]
    fn glitches_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let (marked, _) = marked_and_unmarked();
        w.line(&format!(
            "# every star is body {marked} at seed {:#x}",
            SEED.get()
        ));
        w.f64("lifetime of the 15 Msun progenitor (yr)", lifetime());
        let marks: String = (0..64)
            .map(|i| {
                if is_vela_like_marked(SEED, body(i)) {
                    '1'
                } else {
                    '0'
                }
            })
            .collect();
        w.line(&format!(
            "Vela-like marks of bodies 0-63 of systems n << 13: {marks}"
        ));
        let cases: [(&str, StarModel, TimeWindow); 6] = [
            (
                "a young Crab-like pulsar above the nu-dot limit: P0 draw -2.35, field draw -0.6, \
                 born 970 yr before the epoch, over 30 years",
                crab(),
                span(0.0, 30.0),
            ),
            (
                "a Vela-like pulsar with the mark: P0 draw -2.3, field draw -0.7, born 11,000 yr \
                 before the epoch, over 30 years",
                vela(),
                span(0.0, 30.0),
            ),
            (
                "a pulsar of tau_c about 10^6 yr: the median draws, born 630,000 yr before the \
                 epoch, over 300 years",
                old(),
                span(-150.0, 150.0),
            ),
            (
                "a high-field pulsar, not a magnetar at the reference: P0 draw 0, field draw 1.75, \
                 born 1,000 yr before the epoch, over 30 years",
                high_field(),
                span(0.0, 30.0),
            ),
            (
                "a magnetar: P0 draw 0, field draw 2.6, born 3,000 yr before the epoch",
                magnetar(),
                span(0.0, 30.0),
            ),
            (
                "a Vela-like pulsar born after the epoch: the median draws, born 500 yr after it, \
                 over 30 years from 10 years before its birth",
                pulsar(0.0, 0.0, -500.0),
                span(490.0, 520.0),
            ),
        ];
        for (name, star, window) in cases {
            w.line("");
            w.line(&format!("# {name}"));
            pin_star(&mut w, &star, window, marked);
        }
        golden!("stellar/glitches", w.as_str());
    }

    /// One half decade of |ν̇| in the slow test's tallies.
    #[derive(Debug, Default, Clone, Copy)]
    struct HalfDecade {
        stars: u32,
        spin_down: f64,
        crab_rate: f64,
        total_rate: f64,
        crab_activity: f64,
        total_activity: f64,
        listed: u32,
        listed_size: f64,
    }

    /// The slow test's tallies over its sampled neutron stars.
    #[derive(Debug, Default)]
    struct Tally {
        neutron_stars: u32,
        glitching: u32,
        magnetars: u32,
        expected: f64,
        counted: u64,
        cut: u64,
        marked: u64,
        cycles_expected: f64,
        cycles_counted: u64,
        cycles_bound: f64,
        deviations: Vec<f64>,
        decades: [HalfDecade; 24],
    }

    impl Tally {
        /// The half decade of |ν̇| = `spin_down`, from 10⁻¹⁸ Hz s⁻¹.
        fn decade(&mut self, spin_down: f64) -> &mut HalfDecade {
            let k = ((math::log10(spin_down) + 18.0) * 2.0)
                .floor()
                .clamp(0.0, 23.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "an integer clamped to 0–23"
            )]
            let k = k as usize;
            &mut self.decades[k]
        }

        /// Adds `star`, a neutron star at the epoch, whose body is `body`, over `window` of the
        /// universe of `seed`.
        fn add(&mut self, star: &StarModel, seed: Seed, body: BodyId, window: TimeWindow) {
            let width = window
                .end()
                .checked_since(window.start())
                .unwrap()
                .as_seconds_f64();
            self.neutron_stars += 1;
            let state = star.pulsar_at(UniverseTime::EPOCH).unwrap();
            let p = state.period().value();
            let spin_down = state.period_derivative() / (p * p);
            let Some(model) = GlitchModel::of(star) else {
                if state.is_magnetar() {
                    self.magnetars += 1;
                } else {
                    let d = self.decade(spin_down);
                    d.stars += 1;
                    d.spin_down += spin_down;
                }
                return;
            };
            self.glitching += 1;
            assert_eq!(model.reference(), UniverseTime::EPOCH);
            let rate = model.small_glitch_rate().value();
            self.expected += rate * width;
            let small_series = EventSeries::new(seed, tags::STAR_GLITCH, body.into());
            let listed = list(star, &small_series, window);
            self.counted += u64::try_from(listed.len()).unwrap();
            let activity = rate * model.frequency_hz() * crab_mean_fractional_size();
            let d = self.decade(spin_down);
            d.stars += 1;
            d.spin_down += spin_down;
            d.crab_rate += rate;
            d.total_rate += rate;
            d.crab_activity += activity;
            d.total_activity += activity;
            d.listed += u32::try_from(listed.len()).unwrap();
            d.listed_size += listed.iter().map(Glitch::size_hz).sum::<f64>();
            if !model.meets_vela_like_cuts() {
                return;
            }
            self.cut += 1;
            let mark = is_vela_like_marked(seed, body);
            self.marked += u64::from(mark);
            let cycles = GlitchCycle::of(star, seed, body);
            assert_eq!(cycles.is_some(), mark);
            let Some(cycles) = cycles else {
                return;
            };
            let period = cycles.period().as_seconds_f64();
            let mut large_listed = Vec::new();
            large_glitches_in(star, seed, body, window, &mut large_listed);
            self.cycles_counted += u64::try_from(large_listed.len()).unwrap();
            self.cycles_expected += width / period;
            self.cycles_bound += 2.0 * cycles.noise().noise_bound_cycles() + 1.0;
            self.deviations
                .push(real(large_listed.len()) - width / period);
            let d = self.decade(spin_down);
            d.total_rate += 1.0 / period;
            d.total_activity += LARGE_MEAN_SIZE_HZ / period;
        }

        /// Prints the rates and activities per half decade of |ν̇| against the sources', and
        /// returns the slope of log rate against log |ν̇| over the half decades that glitch.
        fn report_half_decades(&self) -> f64 {
            let mut points = Vec::new();
            for (k, d) in (0_u32..).zip(&self.decades) {
                if d.stars == 0 {
                    continue;
                }
                let low = f64::from(k) / 2.0 - 18.0;
                let stars = f64::from(d.stars);
                let mean_spin_down = d.spin_down / stars;
                let espinoza = ESPINOZA_RATES
                    .iter()
                    .find(|r| (r.0 - 15.0 - math::log10(mean_spin_down)).abs() < 0.26)
                    .map(|r| r.1);
                let crab_size = if d.crab_rate > 0.0 {
                    d.crab_activity / d.crab_rate
                } else {
                    0.0
                };
                if d.total_rate > 0.0 {
                    points.push((
                        math::log10(mean_spin_down),
                        math::log10(d.total_rate / stars * YEAR),
                    ));
                }
                let listed_mean = if d.listed > 0 {
                    d.listed_size / f64::from(d.listed)
                } else {
                    0.0
                };
                eprintln!(
                    "log|nudot| {low:.1}–{:.1}: {} stars, mean {mean_spin_down:.3e}; rate a year: \
                     Crab-like {:.4}, total {:.4} (Espinoza {espinoza:?}); activity / |nudot|: \
                     Crab-like {:.5}, total {:.5}; Crab-like mean size {:.4e} uHz; listed {} with \
                     mean {:.4e} uHz",
                    low + 0.5,
                    d.stars,
                    d.crab_rate / stars * YEAR,
                    d.total_rate / stars * YEAR,
                    d.crab_activity / d.spin_down,
                    d.total_activity / d.spin_down,
                    crab_size * 1e6,
                    d.listed,
                    listed_mean * 1e6,
                );
            }
            let m = real(points.len());
            let (mx, my) = (
                points.iter().map(|p| p.0).sum::<f64>() / m,
                points.iter().map(|p| p.1).sum::<f64>() / m,
            );
            points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>()
                / points.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum::<f64>()
        }
    }

    /// The Crab-like law's mean x = Δν ÷ ν, in closed form.
    fn crab_mean_fractional_size() -> f64 {
        let a = CRAB_SIZE_INDEX;
        (1.0 - a) / (2.0 - a)
            * (math::powf(MAX_FRACTIONAL_SIZE, 2.0 - a) - math::powf(MIN_FRACTIONAL_SIZE, 2.0 - a))
            / (math::powf(MAX_FRACTIONAL_SIZE, 1.0 - a) - math::powf(MIN_FRACTIONAL_SIZE, 1.0 - a))
    }

    /// Adds to `tally` the neutron stars among the sample inputs `indices` of band E (8–150 M☉) of
    /// the young thin disc at the solar circle under `seed`: each is evolved once to find its
    /// lifetime, and one that leaves a neutron star is rebuilt at its lifetime plus `max_age` times
    /// a uniform from `lcg` (a steady birth rate over the last `max_age` years), and listed over 30
    /// years from the epoch.
    fn sample_neutron_stars(
        seed: u64,
        indices: core::ops::Range<u64>,
        max_age: f64,
        lcg: &mut Lcg,
        tally: &mut Tally,
    ) {
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like()).unwrap();
        let position = reference_position(Population::YoungThinDisc);
        let component = components_of(&galaxy, Population::YoungThinDisc, position)[0].0;
        let sampling = Sampling {
            component,
            position,
            band: Some(MassBand::E),
        };
        for i in indices {
            let inputs = sample_inputs(&galaxy, &sampling, seed, i);
            let build = |age: f64| {
                StarModel::new(
                    inputs.initial_mass,
                    inputs.composition,
                    inputs.draws.clone(),
                    Years::new(age),
                )
                .unwrap()
            };
            let first = build(1e9);
            if first.neutron_star().is_none() {
                continue;
            }
            let star = build(first.lifetime().unwrap().value() + max_age * lcg.next_f64());
            tally.add(&star, Seed::new(seed), inputs.body, span(0.0, 30.0));
        }
    }

    /// Asserts that `t`'s Crab-like glitches number the rate integral within a Poisson interval,
    /// and its Vela-like cycles the window over P within twice the noise's bound per star (and
    /// their mean deviation zero within four standard errors), and that the marked share among the
    /// stars that meet the cuts is the Vela-like share (χ²).
    fn check_counts(name: &str, t: &Tally) {
        eprintln!(
            "{name}: {} neutron stars, {} glitching, {} magnetars; Crab-like over 30 years: \
             expected {:.1}, counted {}, interval {:?} at alpha {ALPHA:e}",
            t.neutron_stars,
            t.glitching,
            t.magnetars,
            t.expected,
            t.counted,
            poisson_interval(t.expected, ALPHA)
        );
        assert_poisson_count(
            &format!("Crab-like glitches of {name} in 30 years"),
            t.counted,
            t.expected,
            ALPHA,
        );
        let difference = real64(t.cycles_counted) - t.cycles_expected;
        let n = real(t.deviations.len());
        let mean_dev = t.deviations.iter().sum::<f64>() / n;
        let sd_dev = (t
            .deviations
            .iter()
            .map(|x| (x - mean_dev) * (x - mean_dev))
            .sum::<f64>()
            / (n - 1.0))
            .sqrt();
        let cut = real64(t.cut);
        eprintln!(
            "{name}: {} stars meet the Vela-like cuts, {} marked ({:.4}); {} cycling: {} cycles \
             against {:.1}, bound {:.0}; per star {mean_dev:.4} ± {sd_dev:.4}",
            t.cut,
            t.marked,
            real64(t.marked) / cut,
            t.deviations.len(),
            t.cycles_counted,
            t.cycles_expected,
            t.cycles_bound,
        );
        assert!(difference.abs() <= t.cycles_bound, "{difference}");
        assert!(
            mean_dev.abs() < 4.0 * sd_dev / n.sqrt() + 0.05,
            "{mean_dev} ± {sd_dev}"
        );
        let share = chi_square_gof(
            &[t.marked, t.cut - t.marked],
            &[cut * VELA_LIKE_SHARE, cut * (1.0 - VELA_LIKE_SHARE)],
        );
        assert_p_value(
            &format!("the Vela-like share of {name}"),
            share.p_value,
            ALPHA,
        );
    }

    /// The parent's statistical test for this kind, over 10⁴ massive stars whose neutron stars'
    /// ages at the epoch are uniform over 0–10⁷ years: the Crab-like glitches listed in 30 years
    /// number the rate integral within a Poisson interval, and the Vela-like cycles number the
    /// window over P; and the population's rates and activities per half decade of |ν̇| are
    /// printed against the sources'. Few of those stars are Vela-like, so 3,000 more, whose
    /// neutron stars' ages are uniform over 0–10⁵ years, check the cycles and the marked share too.
    ///
    /// The stars are T31's sample inputs ([`sample_neutron_stars`]), with their draws and
    /// composition.
    #[test]
    #[ignore = "slow: 1.3 × 10⁴ massive stars, two tracks each (28 s under slow-test)"]
    fn the_mean_counts_over_10_000_massive_stars_match_the_rate_integrals() {
        let seed = 0x0628_b000_0000_5eed_u64;
        let mut lcg = Lcg::new(0x0628_b0a6);
        let mut steady = Tally::default();
        sample_neutron_stars(seed, 0..10_000, 1e7, &mut lcg, &mut steady);
        assert!(steady.glitching > 1_000, "{}", steady.glitching);
        check_counts("10^4 stars of ages to 10^7 years", &steady);
        let mut young = Tally::default();
        sample_neutron_stars(seed, 10_000..13_000, 1e5, &mut lcg, &mut young);
        assert!(young.deviations.len() > 300, "{}", young.deviations.len());
        check_counts("3,000 stars of ages to 10^5 years", &young);
        let slope = steady.report_half_decades();
        eprintln!("total rate against |nudot| over 0–10^7 years: slope {slope:.3}");
        let slope = young.report_half_decades();
        eprintln!("total rate against |nudot| over 0–10^5 years: slope {slope:.3}");
        for (log, time, count, sum, nudot) in ESPINOZA_TABLE_4 {
            eprintln!(
                "Espinoza et al. Table 4 at log<|nudot|> {log} (1e-15): mean size {:.4e} uHz, \
                 nudot_glitch {nudot} as printed, {:.4} from its sum over its time",
                sum / f64::from(count),
                sum * 1e-6 / (time * YEAR) * 1e15,
            );
        }
    }
}
