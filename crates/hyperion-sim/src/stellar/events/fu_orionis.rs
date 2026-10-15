//! FU Orionis outbursts: the accretion outbursts of young stars, the second single-star event kind
//! (plan 06, P06.T28.d).
//!
//! # Who has outbursts
//!
//! Every star from 0.1 M☉ ([`MIN_INITIAL_MASS`]), the stars with a track, while it is young:
//!
//! - **as a protostar**, from the onset of collapse to `t_p` = 0.5 Myr
//!   ([`PROTOSTAR_DURATION`](crate::stellar::premain::PROTOSTAR_DURATION)), the span over which its
//!   track's phase is [`Phase::Protostar`](crate::stellar::Phase) at every mass (above about 6 M☉
//!   the protostar hands over straight to the main sequence at `t_p`). Both classes of outburst, at
//!   every mass up to 150 M☉, held at the Eddington luminosity: massive protostars are observed to
//!   burst (S255IR NIRS3, Caratti o Garatti et al. 2017, Nature Physics 13, 276; NGC 6334I-MM1,
//!   Hunter et al. 2017, ApJ 837, L29 and 2021, ApJ 912, L17; G358.93−0.03-MM1, Stecklum et al.
//!   2021, A&A 646, A161).
//! - **with its disc**, from `t_p` while the star's protostellar disc lasts, to `t_d` =
//!   [`disc_lifetime`] of its initial mass and its own disc-lifetime rank (the one disc of ruling
//!   33, which plan 14 and the T Tauri class read too), for stars under 2 M☉ only
//!   ([`DISC_PHASE_MASS_LIMIT`], the T Tauri stars): outbursts of the FU Ori class only.
//!
//! Not modelled, each recorded as a deferred finding in plan 06 (P06.T28.d): objects below 0.1 M☉,
//! which follow P06.T13's cooling fits and have no protostar phase; the disc-phase outbursts of
//! Herbig Ae/Be stars, from 2 M☉; EX Lupi-type outbursts; and V1647 Ori-type events of Class II
//! stars.
//!
//! # The rate
//!
//! One kind, on the one series `star.ev.fu_orionis`, in two amplitude classes ([`OutburstClass`]),
//! the "FU Ori event" and the "V1647 Ori event" of Fischer et al. (2023, Protostars and Planets
//! VII, Table 1), from three components ([`OutburstComponent`]). Their rates, by phase:
//!
//! | phase | ages | FU Ori class | V1647 Ori class | total |
//! |---|---|---|---|---|
//! | Class 0 | 0 to `t_c` | 1/5,000 yr | 1/480 yr | 1/438 yr |
//! | Class I | `t_c` to `t_p` | 1/5,000 yr | 1/2,671 yr | 1/1,741 yr |
//! | disc, under 2 M☉ | `t_p` to `t_d` | 1/112,000 yr | none | 1/112,000 yr |
//!
//! and none before the onset of collapse, after the disc, or in the disc phase from 2 M☉. `t_c` =
//! `t_p` (1 − 1/√2) = 146,447 years ([`CLASS_0_END`]) is T15's boundary between Class 0 and Class
//! I ([`protostar_class`](crate::stellar::premain::protostar_class)). The protostellar totals are
//! Zakri et al.'s (2022, ApJ 924, L23, Table 3) medians; the V1647 Ori rates are those totals less
//! the FU Ori rate, since Zakri et al.'s bursts of 2 magnitudes or more include FU Ori events
//! proper. The protostellar FU Ori rate is the brainstorm's one per 5,000 years "for the youngest
//! protostars", within Contreras Peña et al.'s (2019, MNRAS 486, 4590) Class I estimates; the
//! disc's is their Class II recurrence time (section 6, equations 8–9). Each constant's
//! documentation gives its source.
//!
//! The steps between phases are smooth: with smoothsteps S = 3s² − 2s³ of width w = 0.05 Myr
//! ([`STEP_WIDTH`]) **centred** on `t_c`, `t_p` and `t_d`, and C = 1 − S,
//!
//! ```text
//! r(a) = C_p [r_FU + r_V0 C_c + r_VI S_c] + r_d S_p C_d [m < 2 M☉],
//! ```
//!
//! so that each star's expected count is the sharp model's. A disc that ends before `t_p` gives no
//! disc term, and the protostellar rates hold through `t_p` whatever the disc. r is non-increasing
//! in age from the onset of collapse.
//!
//! # Time
//!
//! Outbursts are Poisson bins of [`BIN_SECONDS`] = 500 Julian years that look back
//! [`LOOK_BACK_BINS`] = one bin (plan 06, design note 15); a bin's mean is at most 500 ÷ 438 =
//! 1.14, far under 64. The model turns the onset of collapse and the three step centres into clock
//! times once, the epoch plus (age − the age at the epoch) × 31,557,600 s, as [`StarModel`]'s mass
//! breaks do, and forms the rate at t from the whole nanoseconds between t and each, so no age of
//! gigayears is read at t (design note 23). The rate is a function of the age, the mass and the
//! disc rank alone, so it holds at any t, beyond the clock window too. Since it never rises with
//! age, a bin's bound is the rate at the bin's first instant at which the star exists, padded by
//! 2⁻⁴⁰ of itself against rounding.
//!
//! # Marks
//!
//! Each outburst's marks come from its own stream, [`BinEvent::marks`], from word 0, so marks never
//! move a time. The layout is fixed:
//!
//! - **word 0:** the component, picked by one integer [`Mark`](crate::rng::Mark) among the rate's
//!   three terms in proportion to their values at the onset, which for a marked Poisson process is
//!   exact;
//! - **word 1:** the drawn peak factor F, log-uniform in the component's range
//!   ([`FU_ORI_PROTOSTELLAR_PEAK_FACTOR`], [`FU_ORI_DISC_PEAK_FACTOR`],
//!   [`V1647_ORI_PEAK_FACTOR`]);
//! - **word 2:** the rise, log-uniform in the class's range ([`FU_ORI_RISE_YEARS`],
//!   [`V1647_ORI_RISE_YEARS`]);
//! - **word 3:** for the FU Ori class the fading rate ḟ, log-uniform in
//!   [`FU_ORI_FADING_MAGNITUDES_PER_YEAR`], and a decay of min(2.5 log₁₀ F ÷ ḟ, 500 years − rise)
//!   from the drawn F, so that an FU Ori outburst whose decay would outlast the bin is shortened
//!   to 500 years less its rise; for the V1647 Ori class the decay, log-uniform in
//!   [`V1647_ORI_DECAY_YEARS`].
//!
//! Each log-uniform mark is lo × (hi ÷ lo)^u from one `uniform()`, and the marks are independent.
//! Times come from the marks alone, never from the track. Scatter added later takes words 4, 5, …
//! in a fixed order.
//!
//! # The light curve
//!
//! An outburst's luminosity factor is `F_eff^x`: x rises linearly in time from 0 at the onset to 1
//! at the peak, and falls linearly to 0 at the end, so the light curve is linear in bolometric
//! magnitude through the rise and through the decay, as FU Ori and BBW 76 fade at constant rates
//! in magnitudes (Audard et al. 2014, Protostars and Planets VI, section 2.3.1). Plan 06's ruling
//! fixes the decay's shape; the rise's, linear in magnitude too, is this module's choice. Both ends
//! are exactly 1, with no rescaling ([`Outburst::luminosity_factor_at`]).
//!
//! # The Eddington cap
//!
//! The peak factor is the star's bolometric luminosity at the peak over its quiescent one,
//! 1 + ΔL ÷ L, the quiescent luminosity of a protostar including T15.a's accretion luminosity,
//! G m ṁ ÷ R. It is held at the Eddington luminosity: `F_eff` = max(1, min(F, 1 ÷ Γ)), with Γ the
//! electron-scattering Eddington factor at the star's initial surface hydrogen X = 0.76 − 3Z,
//! Γₑ(X = 0) × (1 + X), the construction of `stellar::classify::peculiar`, read from
//! [`StarModel::state_at`] once per outburst, at the earlier of its onset and +H (beyond +H a
//! living star's track is not built). The cap scales the light curve only, never the times. A star
//! with no state by then, one that forms after +H (no star of the galaxy, whose ages start at −H),
//! is not capped.
//!
//! # Overlapping outbursts
//!
//! Poisson bins are memoryless, so an outburst can start while another runs: in Class 0, at one
//! per 438 years, with mean durations of about 13.5 years for the V1647 Ori class and about 100
//! years for the FU Ori class (21 years over both), about 5% of outbursts start while another runs
//! and about 9% overlap another one way or the other; in Class I, 2.5% and 5%. No refractory rule
//! is added. P06.T28.g composes overlapping factors.
//!
//! An event may change what a console reads, never what the star's track is (design note 14):
//! nothing here changes the star.

use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
    from_nanos_saturating, tags, to_nanos,
};
use crate::math;
use crate::stellar::draws::UnitUniform;
use crate::stellar::premain::{PROTOSTAR_YEARS, disc_lifetime};
use crate::stellar::sse::MIN_INITIAL_MASS;
use crate::stellar::sse::wind;
use crate::stellar::system::StarModel;
use crate::stellar::{Composition, StarState};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Megayears, SolarMasses, Years};

/// Seconds in one Julian year, as an integer.
const SECONDS_PER_JULIAN_YEAR_I64: i64 = 31_557_600;

/// The length of an outburst bin, s: 500 Julian years, 15,778,800,000 s (plan 06, P06.T28.d).
pub const BIN_SECONDS: i64 = 500 * SECONDS_PER_JULIAN_YEAR_I64;

/// How many whole bins [`outbursts_active_at`] looks back: one, 500 years, at least as long as any
/// outburst ([`MAX_DURATION`]).
pub const LOOK_BACK_BINS: u32 = 1;

/// The longest an outburst may last, from its onset to its end: one bin, 500 Julian years, the
/// look-back, so that [`outbursts_active_at`] finds every outburst still running.
///
/// The rise and the decay add to at most this by construction: an FU Ori-class decay of
/// 2.5 log₁₀ F ÷ ḟ that would pass it is shortened to 500 years less the rise (see "Marks" in the
/// [module documentation](self)). An outburst that starts in the bin before the look-back's first
/// ends before the bin that holds the instant asked about begins.
pub const MAX_DURATION: Span = Span::from_seconds(BIN_SECONDS);

/// The rate of FU Ori-class outbursts of a protostar, Class 0 and Class I alike, per Julian year:
/// one per 5,000 years.
///
/// The brainstorm's "one per 5,000 years for the youngest protostars" (plan 06, design note 22),
/// between Contreras Peña, Naylor and Morrell's (2019, MNRAS 486, 4590) Class I recurrence times:
/// 3.0 (+12.7, −1.9) and 6.0 (+25.3, −3.9) kyr from Scholz et al.'s (2013) samples A and B (their
/// section 6.1.1) and 13 (+15, −6) kyr from their own Class I contamination (section 7.4), and at
/// the short end of the 5–50 kyr between outbursts that Audard et al. (2014, Protostars and Planets
/// VI, section 4.1) quote from Scholz et al. (2013, MNRAS 430, 2910).
pub const FU_ORI_PROTOSTELLAR_PER_YEAR: f64 = 1.0 / 5_000.0;

/// The rate of outbursts of both classes of a Class 0 protostar, per Julian year: one per 438
/// years.
///
/// Zakri et al. (2022, ApJ 924, L23, Table 3, the 95% intervals under the logarithmic prior): the
/// median interval between bursts of 2 magnitudes or more at 3.6 or 4.5 µm of Orion's Class 0
/// protostars, 438 years (161–1,884), from three bursts among 92 of them over a 13-year baseline
/// (their section 4.1). These are mid-infrared bursts, of which Fischer et al. (2023, section 5.2)
/// warn that the methods "potentially lead to much higher measured rates of outbursts for
/// protostars".
pub const CLASS_0_PER_YEAR: f64 = 1.0 / 438.0;

/// The rate of outbursts of both classes of a Class I protostar, per Julian year: one per 1,741
/// years.
///
/// Zakri et al. (2022, Table 3), the median for Orion's Class I and flat-spectrum protostars,
/// 1,741 years (527–12,011), from two bursts among 227 (section 4.1), one of them V2775 Ori.
pub const CLASS_I_PER_YEAR: f64 = 1.0 / 1_741.0;

/// The rate of V1647 Ori-class outbursts of a Class 0 protostar, per Julian year: one per 480
/// years, [`CLASS_0_PER_YEAR`] less [`FU_ORI_PROTOSTELLAR_PER_YEAR`].
///
/// Zakri et al.'s (2022) bursts include FU Ori events proper (V2775 Ori, which their section 3
/// calls "one of the lowest amplitude FU Ori outbursts known"), so the V1647 Ori class takes the
/// rest of their total.
pub const V1647_ORI_CLASS_0_PER_YEAR: f64 = CLASS_0_PER_YEAR - FU_ORI_PROTOSTELLAR_PER_YEAR;

/// The rate of V1647 Ori-class outbursts of a Class I protostar, per Julian year: one per 2,671
/// years, [`CLASS_I_PER_YEAR`] less [`FU_ORI_PROTOSTELLAR_PER_YEAR`].
pub const V1647_ORI_CLASS_I_PER_YEAR: f64 = CLASS_I_PER_YEAR - FU_ORI_PROTOSTELLAR_PER_YEAR;

/// The rate of FU Ori-class outbursts of a star with its disc after the protostar phase, per
/// Julian year: one per 112,000 years.
///
/// Contreras Peña, Naylor and Morrell (2019, section 6, equations 8–9): τ = 112 (+68, −38) kyr at
/// 68% confidence with the 1/τ prior, from 6 outbursts among 14,077 Class II young stellar objects
/// over a mean baseline of 55.6 years (131 kyr without the prior, equation 7; 100–130 kyr allowing
/// for contamination, section 7.4). Their outburst "rises by more than four magnitudes on a
/// timescale of around a decade or less, and then remains bright for decades" (section 6). The
/// brainstorm's "one per 112,000 later".
pub const FU_ORI_DISC_PER_YEAR: f64 = 1.0 / 112_000.0;

/// `t_c`, the age at which a protostar passes from Class 0 to Class I: `t_p` (1 − 1/√2) = 146,447
/// years from the onset of collapse.
///
/// It is T15's boundary ([`protostar_class`](crate::stellar::premain::protostar_class)): with its
/// growth law the envelope still to fall in stops outweighing the star there, at Dunham et al.'s
/// (2014, Protostars and Planets VI, Table 3) Class 0 lifetime of 0.15–0.16 Myr.
pub const CLASS_0_END: Years =
    Years::new(PROTOSTAR_YEARS * (1.0 - core::f64::consts::FRAC_1_SQRT_2));

/// w, the width of each smooth step of the rate, 0.05 Myr: from 25,000 years before each step's
/// centre to 25,000 years after (plan 06, P06.T28.d).
pub const STEP_WIDTH: Years = Years::new(50_000.0);

/// The mass from which a star has no disc-phase outbursts: 2 M☉, the top of the T Tauri stars
/// (P06.T24.b's class). Above it a star's disc is a Herbig Ae/Be star's, whose outbursts are not
/// modelled.
pub const DISC_PHASE_MASS_LIMIT: SolarMasses = SolarMasses::new(2.0);

/// The range of the drawn peak factor of an FU Ori-class outburst of a protostar, log-uniform:
/// 5–50 (plan 06's ruling of 2026-10-15).
///
/// Fischer et al. (2023, Table 1) give an outburst "typically ∼2.5–6 mag (factors of tens)", and
/// their section 4.1.2 the recent FU Ori objects' "only 10–50 L☉" against the twentieth century's
/// "several 100 L☉". Audard et al. (2014, section 2.7) give V2775 Ori's rise as 4.5 to about
/// 51 L☉ and HBC 722's as 0.6–0.85 to 4–12 L☉, factors of 11 and 5–20 (their Table 1 gives 2–4.5
/// to 22–28 L☉ and 0.7–0.85 to 8.7–12 L☉, factors of 4.9–14 and 10–17). The floor is sourced; the
/// top is not a measured bolometric ratio: its 4.25 magnitudes lie under the classical FU Ori
/// objects' optical rises of about 5 magnitudes (Audard et al. 2014, section 2.3.1) and within
/// Table 1's ∼2.5–6 mag.
pub const FU_ORI_PROTOSTELLAR_PEAK_FACTOR: (f64, f64) = (5.0, 50.0);

/// The range of the drawn peak factor of an FU Ori-class outburst in the disc phase, log-uniform:
/// 5–300 (plan 06's ruling of 2026-10-15).
///
/// Classical FU Ori objects, T Tauri stars before their outbursts (V1057 Cyg had a classical T
/// Tauri spectrum before its 6-magnitude rise in V in 1969, Szabó et al. 2021, ApJ 917, 80, section
/// 1), reach 100–300 L☉ in outburst (Audard et al. 2014, section 2.2), with accretion rising from
/// about 10⁻⁷ M☉ a year in the T Tauri state to 10⁻⁴ in the FU Ori state (Hartmann and Kenyon
/// 1996, ARA&A 34, 207, abstract). The top, 6.2 magnitudes, follows such optical amplitudes; no
/// source read gives a bolometric ratio above about 20 (HBC 722).
pub const FU_ORI_DISC_PEAK_FACTOR: (f64, f64) = (5.0, 300.0);

/// The range of the rise of an FU Ori-class outburst, Julian years, log-uniform: 0.25–25 (plan 06's
/// ruling of 2026-10-15).
///
/// Rises run from months (HBC 722) to years (V900 Mon); FU Ori and V1057 Cyg brightened "within
/// about a year" and V1515 Cyg over two decades (Fischer et al. 2023, section 4.1.2, after Hartmann
/// and Kenyon 1996), or for about 30 years from the 1940s (Szabó et al. 2022, ApJ 936, 64, section
/// 3.1), beyond this range.
pub const FU_ORI_RISE_YEARS: (f64, f64) = (0.25, 25.0);

/// The range of the fading rate ḟ of an FU Ori-class outburst, magnitudes per Julian year,
/// log-uniform: 0.012–0.13 (plan 06's ruling of 2026-10-15).
///
/// FU Ori fades at 14 mmag a year and BBW 76 at about 23 in V (Audard et al. 2014, section 2.3.1),
/// while V1057 Cyg and V1515 Cyg "faded by 2–4 mag in just a few decades" (Fischer et al. 2023,
/// section 4.1.2), up to about 0.1 magnitudes a year; V1057 Cyg is the fastest-fading FU Ori object
/// known (Szabó et al. 2021, abstract). The sources' rates are optical; the light curve takes them
/// as bolometric.
pub const FU_ORI_FADING_MAGNITUDES_PER_YEAR: (f64, f64) = (0.012, 0.13);

/// The range of the drawn peak factor of a V1647 Ori-class outburst, log-uniform: 3–15 (plan 06's
/// ruling of 2026-10-15).
///
/// Zakri et al.'s (2022, section 3.2) three protostellar bursts brightened by factors of 3.7, 5.4
/// ("likely an underestimate") and 9.7; V1647 Ori itself went from 3.5–5.6 to 34–44 L☉ (Audard et
/// al. 2014, Table 1), a factor of 6–13. Fischer et al.'s (2023) Table 1 gives the class no
/// figures: "intermediate between those of EX Lup bursts and FU Ori outbursts".
pub const V1647_ORI_PEAK_FACTOR: (f64, f64) = (3.0, 15.0);

/// The range of the rise of a V1647 Ori-class outburst, Julian years, log-uniform: 0.25–10 (plan
/// 06's ruling of 2026-10-15).
///
/// Unsourced: no source read gives these events' rise times (Fischer et al.'s 2023 Table 1 row
/// gives the class no figures, and Zakri et al. 2022 give the bursts' durations, not their rises),
/// so the range is the ruling's own, recorded as a deferred finding in plan 06 (P06.T28.d).
pub const V1647_ORI_RISE_YEARS: (f64, f64) = (0.25, 10.0);

/// The range of the decay of a V1647 Ori-class outburst, Julian years, log-uniform: 5–20 (plan 06's
/// ruling of 2026-10-15).
///
/// HOPS 383's burst lasted about 15 years, and Zakri et al.'s (2022, section 3.1) other two at
/// least 9; V899 Mon, V346 Nor, V960 Mon and OO Ser, which "faded monotonically back … over about 5
/// years", "may be more appropriately considered as V1647 Ori type sources" (Fischer et al. 2023,
/// section 4.1.2). V1647 Ori's own 2003 event lasted 2.5 years in all (Audard et al. 2014, Table
/// 1), under this range.
pub const V1647_ORI_DECAY_YEARS: (f64, f64) = (5.0, 20.0);

/// The relative padding of a bin's bound over the rate at its first instant: 2⁻⁴⁰, far above the
/// few units in the last place by which rounding can make the rate rise within a bin, and far below
/// any rate's change.
const BOUND_PADDING: f64 = 1.0 / 1_099_511_627_776.0;

/// The step width in nanoseconds: 50,000 Julian years, 50,000 × 31,557,600 × 10⁹ ns.
const STEP_WIDTH_NANOS: i128 = 1_577_880_000_000_000_000_000;

/// [`STEP_WIDTH_NANOS`] as an `f64`, exactly: 1.57788 × 10²¹ = 2¹⁸ × 5¹⁶ × 39,447, whose odd part,
/// 6.02 × 10¹⁵, is below 2⁵³.
const STEP_WIDTH_NANOS_F64: f64 = 1.577_88e21;

/// An outburst's amplitude class, after the "FU Ori event" and "V1647 Ori event" of Fischer et al.
/// (2023, Protostars and Planets VII, Table 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutburstClass {
    /// An FU Ori event: an outburst "typically ∼2.5–6 mag (factors of tens), taking months to years
    /// to develop" (Table 1's "Outburst" row), with "long decay timescales similar to human
    /// lifetimes" (its FU Ori row); here a peak factor of 5–50 or 5–300, a rise of 0.25–25 years
    /// and a decay of up to 500 years less the rise (plan 06's ruling of 2026-10-15).
    FuOri,
    /// A V1647 Ori event, "intermediate between those of EX Lup bursts and FU Ori outbursts"
    /// (Table 1, which gives the class no figures); here a peak factor of 3–15, a rise of 0.25–10
    /// years and a decay of 5–20 years (plan 06's ruling of 2026-10-15; Zakri et al. 2022, ApJ 924,
    /// L23, sections 3.1–3.2).
    V1647Ori,
}

/// Which of the rate's three terms an outburst is from (see "The rate" in the
/// [module documentation](self)); it sets the class and the range of the peak factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutburstComponent {
    /// An FU Ori-class outburst of a protostar, at [`FU_ORI_PROTOSTELLAR_PER_YEAR`].
    ProtostellarFuOri,
    /// A V1647 Ori-class outburst of a protostar, at [`V1647_ORI_CLASS_0_PER_YEAR`] in Class 0 and
    /// [`V1647_ORI_CLASS_I_PER_YEAR`] in Class I.
    V1647Ori,
    /// An FU Ori-class outburst of a star under 2 M☉ with its disc, at [`FU_ORI_DISC_PER_YEAR`].
    DiscFuOri,
}

impl OutburstComponent {
    /// The three components in the rate's order, which word 0's pick follows.
    pub const ALL: [Self; 3] = [Self::ProtostellarFuOri, Self::V1647Ori, Self::DiscFuOri];

    /// The component's amplitude class.
    #[must_use]
    pub const fn class(self) -> OutburstClass {
        match self {
            Self::ProtostellarFuOri | Self::DiscFuOri => OutburstClass::FuOri,
            Self::V1647Ori => OutburstClass::V1647Ori,
        }
    }

    /// The range of the drawn peak factor of the component's outbursts, log-uniform.
    #[must_use]
    pub const fn peak_factor_range(self) -> (f64, f64) {
        match self {
            Self::ProtostellarFuOri => FU_ORI_PROTOSTELLAR_PEAK_FACTOR,
            Self::V1647Ori => V1647_ORI_PEAK_FACTOR,
            Self::DiscFuOri => FU_ORI_DISC_PEAK_FACTOR,
        }
    }

    /// The range of the rise of the component's outbursts, Julian years, log-uniform.
    #[must_use]
    pub const fn rise_range_years(self) -> (f64, f64) {
        match self.class() {
            OutburstClass::FuOri => FU_ORI_RISE_YEARS,
            OutburstClass::V1647Ori => V1647_ORI_RISE_YEARS,
        }
    }
}

/// The three smooth steps of the rate at one instant or age: for each of the Class 0 end, the
/// protostar's end and the disc's end, S rising from 0 to 1 across the step and C = 1 − S falling,
/// each computed from its own side so that it stays accurate where it is small.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Steps {
    s: [f64; 3],
    c: [f64; 3],
}

/// The smoothstep 3s² − 2s³ of `s`, held to 0 below 0 and 1 above 1.
#[must_use]
fn smoothstep(s: f64) -> f64 {
    if s <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        s * s * (3.0 - 2.0 * s)
    }
}

/// `nanos` as a share of the step width, `nanos ÷ w`.
#[must_use]
fn share_of_step(nanos: i128) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a time inside a step of 1.6e21 ns; the f64 rounds it monotonically"
    )]
    let x = nanos as f64;
    x / STEP_WIDTH_NANOS_F64
}

/// A step's (S, C) at `nanos_from_centre` nanoseconds after its centre.
#[must_use]
fn step_at_nanos(nanos_from_centre: i128) -> (f64, f64) {
    let half = STEP_WIDTH_NANOS / 2;
    let (rising, falling) = (nanos_from_centre + half, half - nanos_from_centre);
    if falling <= 0 {
        (1.0, 0.0)
    } else if rising <= 0 {
        (0.0, 1.0)
    } else {
        (
            smoothstep(share_of_step(rising)),
            smoothstep(share_of_step(falling)),
        )
    }
}

/// A step's (S, C) at `years_from_centre` Julian years after its centre.
#[must_use]
fn step_at_years(years_from_centre: f64) -> (f64, f64) {
    let half = STEP_WIDTH.value() / 2.0;
    (
        smoothstep((years_from_centre + half) / STEP_WIDTH.value()),
        smoothstep((half - years_from_centre) / STEP_WIDTH.value()),
    )
}

/// The rate of FU Orionis outbursts of a star as a closed form of its age, from its initial mass
/// and its disc's lifetime: no track and no clock (see "The rate" in the
/// [module documentation](self)).
///
/// [`FuOrionisModel`] holds one for its star; the galaxy-wide counts build one per sampled star
/// without its track.
///
/// # Examples
///
/// A solar-mass star bursts about once in 438 years in Class 0, once in 1,741 in Class I, and once
/// in 112,000 while its disc lasts:
///
/// ```
/// use hyperion_sim::events::EventsPerSecond;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::stellar::events::fu_orionis::FuOrionisRates;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let rates = FuOrionisRates::new(SolarMasses::new(1.0), UnitUniform::HALF).ok_or("a star")?;
/// let once_in = |age: f64| 1.0 / (rates.rate_at_age(Years::new(age)).value() * 31_557_600.0);
/// assert!((once_in(5e4) - 438.0).abs() < 1e-6);
/// assert!((once_in(3e5) - 1_741.0).abs() < 1e-6);
/// // The median disc lasts 1.73 Myr.
/// assert!((once_in(1e6) - 112_000.0).abs() < 1e-3);
/// assert_eq!(rates.rate_at_age(Years::new(3e6)), EventsPerSecond::ZERO);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FuOrionisRates {
    disc_lifetime: Megayears,
    disc_phase: bool,
}

impl FuOrionisRates {
    /// The rate law of a star of `initial_mass` whose disc-lifetime rank is `disc_rank` (its
    /// [`StarDraws::disc_lifetime`](crate::stellar::draws::StarDraws::disc_lifetime)), or `None`
    /// below 0.1 M☉ ([`MIN_INITIAL_MASS`]), where no outbursts are modelled.
    #[must_use]
    pub fn new(initial_mass: SolarMasses, disc_rank: UnitUniform) -> Option<Self> {
        (initial_mass >= MIN_INITIAL_MASS).then(|| Self {
            disc_lifetime: disc_lifetime(initial_mass, disc_rank),
            disc_phase: initial_mass < DISC_PHASE_MASS_LIMIT,
        })
    }

    /// `t_d`, the lifetime of the star's protostellar disc ([`disc_lifetime`]): 0.3–15 Myr from the
    /// onset of collapse.
    #[must_use]
    pub const fn disc_lifetime(&self) -> Megayears {
        self.disc_lifetime
    }

    /// Whether the star has disc-phase outbursts: whether it is under 2 M☉
    /// ([`DISC_PHASE_MASS_LIMIT`]).
    #[must_use]
    pub const fn has_disc_phase(&self) -> bool {
        self.disc_phase
    }

    /// `t_d` in years.
    #[must_use]
    fn disc_years(&self) -> f64 {
        self.disc_lifetime.value() * 1e6
    }

    /// The age from which the rate is zero: the end of the protostar's step, or of the disc's for a
    /// star with a disc phase whose disc outlasts it.
    #[must_use]
    pub fn last_age(&self) -> Years {
        let half = STEP_WIDTH.value() / 2.0;
        let protostar = PROTOSTAR_YEARS + half;
        Years::new(if self.disc_phase {
            protostar.max(self.disc_years() + half)
        } else {
            protostar
        })
    }

    /// The steps at `age_years` (Julian years since the onset of collapse).
    #[must_use]
    fn steps_at_age(&self, age_years: f64) -> Steps {
        let (s_c, c_c) = step_at_years(age_years - CLASS_0_END.value());
        let (s_p, c_p) = step_at_years(age_years - PROTOSTAR_YEARS);
        let (s_d, c_d) = step_at_years(age_years - self.disc_years());
        Steps {
            s: [s_c, s_p, s_d],
            c: [c_c, c_p, c_d],
        }
    }

    /// The three components' rates per Julian year at `steps`, in [`OutburstComponent::ALL`]'s
    /// order.
    #[must_use]
    fn components_per_year(&self, steps: Steps) -> [f64; 3] {
        let ([s_c, s_p, _], [c_c, c_p, c_d]) = (steps.s, steps.c);
        let fu = c_p * FU_ORI_PROTOSTELLAR_PER_YEAR;
        let v1647 = c_p * (V1647_ORI_CLASS_0_PER_YEAR * c_c + V1647_ORI_CLASS_I_PER_YEAR * s_c);
        let disc = if self.disc_phase {
            FU_ORI_DISC_PER_YEAR * s_p * c_d
        } else {
            0.0
        };
        [fu, v1647, disc]
    }

    /// The components' rates at `age`, in [`OutburstComponent::ALL`]'s order, per second: none
    /// before the onset of collapse.
    #[must_use]
    fn components_at_age(&self, age: Years) -> [f64; 3] {
        if age.value() > 0.0 {
            per_second(self.components_per_year(self.steps_at_age(age.value())))
        } else {
            [0.0; 3]
        }
    }

    /// The rate of outbursts of both classes at `age`, Julian years since the onset of collapse:
    /// r(a) of the [module documentation](self), zero at and before the onset.
    #[must_use]
    pub fn rate_at_age(&self, age: Years) -> EventsPerSecond {
        EventsPerSecond::new(total(self.components_at_age(age)))
    }

    /// The rate of `component`'s outbursts at `age`, Julian years since the onset of collapse.
    #[must_use]
    pub fn component_rate_at_age(
        &self,
        component: OutburstComponent,
        age: Years,
    ) -> EventsPerSecond {
        EventsPerSecond::new(self.components_at_age(age)[index_of(component)])
    }
}

/// The index of `component` in [`OutburstComponent::ALL`].
#[must_use]
const fn index_of(component: OutburstComponent) -> usize {
    match component {
        OutburstComponent::ProtostellarFuOri => 0,
        OutburstComponent::V1647Ori => 1,
        OutburstComponent::DiscFuOri => 2,
    }
}

/// Rates per Julian year as rates per second.
#[must_use]
fn per_second(per_year: [f64; 3]) -> [f64; 3] {
    per_year.map(|r| r / SECONDS_PER_JULIAN_YEAR)
}

/// The three components' sum, in their order: the rate.
#[must_use]
fn total(components: [f64; 3]) -> f64 {
    components[0] + components[1] + components[2]
}

/// The clock time, as nanoseconds since the epoch, at which a star whose age at the epoch is
/// `age_at_epoch_years` Julian years is `age_years` old: the epoch plus (age − age at the epoch) ×
/// 31,557,600 s, floored to the nanosecond ([`Span::from_seconds_f64`]), as [`StarModel`]'s mass
/// breaks place an age; held to the clock's ends where it lies beyond them.
#[must_use]
fn clock_nanos_of_age(age_at_epoch_years: f64, age_years: f64) -> i128 {
    let seconds = (age_years - age_at_epoch_years) * SECONDS_PER_JULIAN_YEAR;
    match Span::from_seconds_f64(seconds).and_then(|span| UniverseTime::EPOCH.checked_add(span)) {
        Some(t) => to_nanos(t),
        None if seconds < 0.0 => to_nanos(from_nanos_saturating(i128::MIN)),
        None => to_nanos(from_nanos_saturating(i128::MAX)),
    }
}

/// The outbursts of one star: its rate law and its clock (plan 06, P06.T28.d; see the
/// [module documentation](self)).
///
/// It is the [`RateModel`] of the star's `star.ev.fu_orionis` series. It holds the star's onset of
/// collapse and its three step centres as clock times, turned once from ages, so that the rate at
/// t reads only the nanoseconds between t and each.
///
/// # Examples
///
/// A Class 0 protostar's steps: it becomes a Class I object about 46,000 years after the epoch and
/// stops accreting 400,000 years after it.
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::fu_orionis::FuOrionisModel;
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let star = StarModel::new(SolarMasses::new(0.5), Composition::SOLAR, StarDraws::median(), Years::new(1e5))?;
/// let model = FuOrionisModel::of(&star).ok_or("a star from 0.1 M☉ has outbursts")?;
/// let years_from_epoch = |t: UniverseTime| t.since_epoch().as_julian_years_f64();
/// assert!((years_from_epoch(model.formation()) + 1e5).abs() < 1e-3);
/// assert!((years_from_epoch(model.class_0_end()) - 46_446.6).abs() < 0.1);
/// assert!((years_from_epoch(model.protostar_end()) - 4e5).abs() < 1e-3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FuOrionisModel<'a> {
    star: &'a StarModel,
    rates: FuOrionisRates,
    /// The onset of collapse, nanoseconds since the epoch.
    formation: i128,
    /// The centres of the Class 0, protostar and disc steps, nanoseconds since the epoch.
    centres: [i128; 3],
    /// The first instant from which the rate is zero, nanoseconds since the epoch.
    last: i128,
}

impl<'a> FuOrionisModel<'a> {
    /// The outbursts of `star`, or `None` for an object below 0.1 M☉, which follows P06.T13's
    /// cooling fits and has no protostar phase. Every other star has a model; its rate is zero
    /// outside its young ages.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let rates = FuOrionisRates::new(star.initial_mass(), star.draws().disc_lifetime())?;
        let epoch_age = star.age_at_epoch().value();
        let at = |age: f64| clock_nanos_of_age(epoch_age, age);
        Some(Self {
            star,
            rates,
            formation: at(0.0),
            centres: [
                at(CLASS_0_END.value()),
                at(PROTOSTAR_YEARS),
                at(rates.disc_years()),
            ],
            last: at(rates.last_age().value()),
        })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The star's rate law in age.
    #[must_use]
    pub const fn rates(&self) -> &FuOrionisRates {
        &self.rates
    }

    /// `t_d`, the lifetime of the star's protostellar disc: 0.3–15 Myr from the onset of collapse.
    #[must_use]
    pub const fn disc_lifetime(&self) -> Megayears {
        self.rates.disc_lifetime
    }

    /// The onset of collapse, age zero, as a clock time: the star exists after it.
    #[must_use]
    pub fn formation(&self) -> UniverseTime {
        from_nanos_saturating(self.formation)
    }

    /// The centre of the step from Class 0 to Class I, at age `t_c` ([`CLASS_0_END`]), as a clock
    /// time.
    #[must_use]
    pub fn class_0_end(&self) -> UniverseTime {
        from_nanos_saturating(self.centres[0])
    }

    /// The centre of the step at the protostar's end, at age `t_p`
    /// ([`PROTOSTAR_DURATION`](crate::stellar::premain::PROTOSTAR_DURATION)), as a clock time.
    #[must_use]
    pub fn protostar_end(&self) -> UniverseTime {
        from_nanos_saturating(self.centres[1])
    }

    /// The centre of the step at the disc's end, at age `t_d`
    /// ([`disc_lifetime`](Self::disc_lifetime)), as a clock time. For a star from 2 M☉ it closes
    /// nothing.
    #[must_use]
    pub fn disc_end(&self) -> UniverseTime {
        from_nanos_saturating(self.centres[2])
    }

    /// The first instant from which the star has no outbursts: the end of the last step that
    /// closes a phase, at [`FuOrionisRates::last_age`].
    #[must_use]
    pub fn last_instant(&self) -> UniverseTime {
        from_nanos_saturating(self.last)
    }

    /// The rate of outbursts at `age`: [`FuOrionisRates::rate_at_age`].
    #[must_use]
    pub fn rate_at_age(&self, age: Years) -> EventsPerSecond {
        self.rates.rate_at_age(age)
    }

    /// The rate of `component`'s outbursts at `age`: [`FuOrionisRates::component_rate_at_age`].
    #[must_use]
    pub fn component_rate_at_age(
        &self,
        component: OutburstComponent,
        age: Years,
    ) -> EventsPerSecond {
        self.rates.component_rate_at_age(component, age)
    }

    /// The rate of `component`'s outbursts at the clock time `t`, from the clock's own steps, as
    /// the series is thinned.
    #[must_use]
    pub fn component_rate_at(
        &self,
        component: OutburstComponent,
        t: UniverseTime,
    ) -> EventsPerSecond {
        EventsPerSecond::new(self.components_at(to_nanos(t))[index_of(component)])
    }

    /// The steps at `t` nanoseconds since the epoch.
    #[must_use]
    fn steps_at(&self, t: i128) -> Steps {
        let (s_c, c_c) = step_at_nanos(t - self.centres[0]);
        let (s_p, c_p) = step_at_nanos(t - self.centres[1]);
        let (s_d, c_d) = step_at_nanos(t - self.centres[2]);
        Steps {
            s: [s_c, s_p, s_d],
            c: [c_c, c_p, c_d],
        }
    }

    /// The components' rates per second at `t` nanoseconds since the epoch: none at and before the
    /// onset of collapse, and none from the last instant.
    #[must_use]
    fn components_at(&self, t: i128) -> [f64; 3] {
        if t <= self.formation || t >= self.last {
            return [0.0; 3];
        }
        per_second(self.rates.components_per_year(self.steps_at(t)))
    }

    /// The outburst of `event`, an event of this model's series: its component from word 0 of its
    /// marks, its peak factor, rise and decay from words 1–3, and its cap from the star's state.
    #[must_use]
    fn outburst_of(&self, event: BinEvent) -> Outburst {
        let id = event.id();
        let onset = event.time();
        let mut marks = event.into_marks();
        let components = self.components_at(to_nanos(onset));
        let component = marks
            .mark()
            .pick_weighted(&components, total(components))
            .map(|i| OutburstComponent::ALL[i])
            .expect("an accepted onset has a positive rate, and the bound is the weights' own sum");
        let drawn = log_uniform(component.peak_factor_range(), marks.uniform());
        let rise = years_span(log_uniform(component.rise_range_years(), marks.uniform()));
        let third = marks.uniform();
        let (decay, fading_magnitudes_per_year) = match component.class() {
            OutburstClass::FuOri => {
                let fading = log_uniform(FU_ORI_FADING_MAGNITUDES_PER_YEAR, third);
                (fu_ori_decay(drawn, fading, rise), Some(fading))
            }
            OutburstClass::V1647Ori => {
                (years_span(log_uniform(V1647_ORI_DECAY_YEARS, third)), None)
            }
        };
        let duration = rise
            .checked_add(decay)
            .expect("a rise and a decay of under 500 years each add without overflow");
        debug_assert!(duration <= MAX_DURATION, "an outburst lasts {duration}");
        let peak_factor = self
            .star
            .state_at(onset.min(ClockWindow::END))
            .map_or(drawn, |state| {
                capped(drawn, &state, self.star.composition())
            });
        Outburst {
            id,
            component,
            onset,
            rise,
            decay,
            duration,
            drawn_peak_factor: drawn,
            peak_factor,
            fading_magnitudes_per_year,
        }
    }
}

impl RateModel for FuOrionisModel<'_> {
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
        let (start, end) = (to_nanos(bin.start()), to_nanos(bin.end()));
        if end <= self.formation || start >= self.last {
            return EventsPerSecond::ZERO;
        }
        // The rate never rises with age after the onset of collapse, so its value at the bin's
        // first instant after the onset bounds it; the rate formula without the onset's cut gives
        // the protostar's first rate there.
        let first = start.max(self.formation);
        let at_first = total(per_second(
            self.rates.components_per_year(self.steps_at(first)),
        ));
        EventsPerSecond::new(at_first + at_first * BOUND_PADDING)
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        EventsPerSecond::new(total(self.components_at(to_nanos(t))))
    }
}

/// `lo` × (`hi` ÷ `lo`)^`u`: log-uniform between `lo` and `hi` for a uniform `u` in [0, 1).
#[must_use]
fn log_uniform((lo, hi): (f64, f64), u: f64) -> f64 {
    lo * math::powf(hi / lo, u)
}

/// The decay of an FU Ori-class outburst of drawn peak factor `drawn`, fading at
/// `fading_magnitudes_per_year`, after a rise of `rise`: 2.5 log₁₀ F ÷ ḟ, cut to 500 years less
/// the rise.
#[must_use]
fn fu_ori_decay(drawn: f64, fading_magnitudes_per_year: f64, rise: Span) -> Span {
    let left = MAX_DURATION
        .checked_sub(rise)
        .expect("a rise of at most 25 years leaves most of the bin");
    years_span(2.5 * math::log10(drawn) / fading_magnitudes_per_year).min(left)
}

/// `years` Julian years as a span, floored to the nanosecond.
#[must_use]
fn years_span(years: f64) -> Span {
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .expect("a mark of at most a few centuries is a finite span")
}

/// The drawn peak factor `drawn` held at the Eddington luminosity of a star in `state`:
/// max(1, min(F, 1 ÷ Γ)), Γ = Γₑ(X = 0) × (1 + X) at X = 0.76 − 3Z (see "The Eddington cap" in the
/// [module documentation](self)).
#[must_use]
fn capped(drawn: f64, state: &StarState, composition: &Composition) -> f64 {
    let x = 0.76 - 3.0 * composition.z_fit().value();
    let gamma = wind::eddington_factor(state.luminosity(), state.mass()) * (1.0 + x);
    drawn.min(1.0 / gamma).max(1.0)
}

/// The clock time `t` plus `span`, held to the clock's end.
#[must_use]
fn saturating_add(t: UniverseTime, span: Span) -> UniverseTime {
    t.checked_add(span)
        .unwrap_or_else(|| from_nanos_saturating(i128::MAX))
}

/// One FU Orionis outburst: its ID, component and class, onset, rise and decay, and peak factor,
/// and its light curve.
///
/// Its luminosity factor is 1 before the onset and from the end, and in between rises to its peak
/// factor and falls back, linearly in magnitude
/// ([`luminosity_factor_at`](Self::luminosity_factor_at)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Outburst {
    id: EventId,
    component: OutburstComponent,
    onset: UniverseTime,
    rise: Span,
    decay: Span,
    duration: Span,
    drawn_peak_factor: f64,
    peak_factor: f64,
    fading_magnitudes_per_year: Option<f64>,
}

impl Outburst {
    /// The outburst's ID, from which [`outburst`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The rate's term the outburst is from, word 0 of its marks.
    #[must_use]
    pub const fn component(&self) -> OutburstComponent {
        self.component
    }

    /// The outburst's amplitude class.
    #[must_use]
    pub const fn class(&self) -> OutburstClass {
        self.component.class()
    }

    /// The onset, where the factor starts to rise from 1.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The peak, the onset plus the rise, where the factor is [`peak_factor`](Self::peak_factor).
    #[must_use]
    pub fn peak(&self) -> UniverseTime {
        saturating_add(self.onset, self.rise)
    }

    /// The first instant after the outburst, its onset plus its duration (held to the clock's end).
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        saturating_add(self.onset, self.duration)
    }

    /// The rise, from the onset to the peak: 0.25–25 years for the FU Ori class, 0.25–10 years for
    /// the V1647 Ori class, floored to the nanosecond.
    #[must_use]
    pub const fn rise(&self) -> Span {
        self.rise
    }

    /// The decay, from the peak to the end: min(2.5 log₁₀ F ÷ ḟ, 500 years − rise) for the FU Ori
    /// class, 5–20 years for the V1647 Ori class, floored to the nanosecond.
    #[must_use]
    pub const fn decay(&self) -> Span {
        self.decay
    }

    /// The duration, the rise plus the decay: at most [`MAX_DURATION`], 500 years.
    #[must_use]
    pub const fn duration(&self) -> Span {
        self.duration
    }

    /// F, the peak factor as drawn, before the Eddington cap: 5–50 (protostellar) or 5–300 (disc)
    /// for the FU Ori class, 3–15 for the V1647 Ori class.
    #[must_use]
    pub const fn drawn_peak_factor(&self) -> f64 {
        self.drawn_peak_factor
    }

    /// The peak factor, the star's bolometric luminosity at the peak over its quiescent one,
    /// 1 + ΔL ÷ L: the drawn factor held at the Eddington luminosity, `F_eff` = max(1, min(F,
    /// 1 ÷ Γ)), at least 1.
    #[must_use]
    pub const fn peak_factor(&self) -> f64 {
        self.peak_factor
    }

    /// ḟ, the drawn fading rate of an FU Ori-class outburst, bolometric magnitudes per Julian year
    /// (0.012–0.13), from which its decay follows; `None` for the V1647 Ori class, whose decay is
    /// drawn. Where neither the cap nor the 500-year limit binds, the light curve fades at it.
    #[must_use]
    pub const fn fading_magnitudes_per_year(&self) -> Option<f64> {
        self.fading_magnitudes_per_year
    }

    /// Whether the outburst is running at `t`: from its onset, inclusive, to its end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.end()
    }

    /// The star's bolometric luminosity at `t` over its quiescent one, as this outburst makes it.
    ///
    /// It is 1 before the onset and from the end, and in between `F_eff`^x, with x = (t − onset) ÷
    /// rise through the rise and x = (end − t) ÷ decay through the decay: linear in magnitude
    /// through both, 1 exactly at the onset and the end, and the peak factor exactly at the peak.
    /// It is continuous in `t`. P06.T28.g composes it into the star's summary.
    #[must_use]
    pub fn luminosity_factor_at(&self, t: UniverseTime) -> f64 {
        let Some(elapsed) = t.checked_since(self.onset) else {
            return 1.0;
        };
        if elapsed.is_negative() || elapsed >= self.duration {
            return 1.0;
        }
        let x = if elapsed < self.rise {
            elapsed.as_seconds_f64() / self.rise.as_seconds_f64()
        } else {
            // Under the duration, so the difference is positive and cannot overflow.
            let left = self
                .duration
                .checked_sub(elapsed)
                .map_or(0.0, Span::as_seconds_f64);
            left / self.decay.as_seconds_f64()
        };
        if x >= 1.0 {
            self.peak_factor
        } else {
            math::powf(self.peak_factor, x)
        }
    }
}

/// The outburst bins: 500 years long, looking back one.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("500 years lie between 16 s and 2^44 s")
}

/// `window` cut to the span in which `model`'s star can have outbursts, from its onset of collapse
/// to its last instant, or `None` where they do not meet. Outside that span every bin's bound is
/// zero, so the cut drops nothing and spares the empty bins.
#[must_use]
fn active_part(model: &FuOrionisModel<'_>, window: TimeWindow) -> Option<TimeWindow> {
    let start = window.start().max(model.formation());
    let end = window.end().min(model.last_instant());
    (start < end).then(|| TimeWindow::new(start, end).expect("start before end"))
}

/// Appends to `out` the FU Orionis outbursts of `star` with onsets in `window`, sorted by onset and
/// then by ID.
///
/// An object below 0.1 M☉ ([`FuOrionisModel::of`]) has none. `series` is the star's
/// `star.ev.fu_orionis` series, `EventSeries::new(seed, tags::STAR_FU_ORIONIS, body.into())`:
/// [`StarModel`] holds no seed or ID, so it comes in beside the star, as for the flares. Every bin
/// the window touches is generated whole, so a listing over a window is the union of the listings
/// over any partition of it, in any order of asking. `out` is only appended to; nothing in it is
/// read.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FU_ORIONIS`].
///
/// # Examples
///
/// A Class I protostar's outbursts over the source horizon, the 264,144 years up to +H that an
/// observer can see, and one of them regenerated from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::fu_orionis::{OutburstClass, outburst, outbursts_in};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::{ClockWindow, SourceHorizon};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A solar-mass protostar 400,000 years after its onset of collapse: Class I at the epoch, and
/// // Class 0 at the far end of the horizon.
/// let star = StarModel::new(SolarMasses::new(1.0), Composition::SOLAR, StarDraws::median(), Years::new(4e5))?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = EventSeries::new(Seed::new(7), tags::STAR_FU_ORIONIS, body.into());
/// let horizon = TimeWindow::new(SourceHorizon::START, ClockWindow::END)?;
///
/// let mut outbursts = Vec::new();
/// outbursts_in(&star, &series, horizon, &mut outbursts);
/// // About 170 at one per 1,741 years in Class I and one per 438 in Class 0.
/// assert!((110..230).contains(&outbursts.len()));
/// let brightest = outbursts
///     .iter()
///     .filter(|o| o.class() == OutburstClass::FuOri)
///     .max_by(|a, b| a.peak_factor().total_cmp(&b.peak_factor()))
///     .ok_or("dozens of FU Ori-class outbursts")?;
/// assert_eq!(outburst(&star, &series, brightest.id()).as_ref(), Some(brightest));
/// assert!(brightest.peak_factor() > 5.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn outbursts_in(
    star: &StarModel,
    series: &EventSeries,
    window: TimeWindow,
    out: &mut Vec<Outburst>,
) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FU_ORIONIS,
        "a star's FU Orionis outbursts are its star.ev.fu_orionis series"
    );
    let Some(model) = FuOrionisModel::of(star) else {
        return;
    };
    let Some(window) = active_part(&model, window) else {
        return;
    };
    let mut events = Vec::new();
    bins().events_in(series, window, &model, &mut events);
    out.extend(events.into_iter().map(|event| model.outburst_of(event)));
}

/// Appends to `out` the FU Orionis outbursts of `star` running at `t`, sorted by onset and then by
/// ID.
///
/// They are those with an onset from the start of the bin before `t`'s up to `t` whose end is
/// after `t`; every outburst lasts at most the look-back, so none is missed. `series` is as for
/// [`outbursts_in`]. The result depends on `star`, `series` and `t` alone, never on what was asked
/// before.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FU_ORIONIS`].
pub fn outbursts_active_at(
    star: &StarModel,
    series: &EventSeries,
    t: UniverseTime,
    out: &mut Vec<Outburst>,
) {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FU_ORIONIS,
        "a star's FU Orionis outbursts are its star.ev.fu_orionis series"
    );
    let Some(model) = FuOrionisModel::of(star) else {
        return;
    };
    if t <= model.formation() || t >= saturating_add(model.last_instant(), MAX_DURATION) {
        return;
    }
    let mut events = Vec::new();
    bins().active_at(series, t, &model, &mut events);
    out.extend(
        events
            .into_iter()
            .map(|event| model.outburst_of(event))
            .filter(|outburst| outburst.is_active_at(t)),
    );
}

/// The FU Orionis outburst `id` names, regenerated on its own.
///
/// `None` if `id` is not of `series`, names a candidate that was rejected or a number beyond its
/// bin's count, or `star` is an object below 0.1 M☉.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_FU_ORIONIS`].
#[must_use]
pub fn outburst(star: &StarModel, series: &EventSeries, id: EventId) -> Option<Outburst> {
    debug_assert_eq!(
        series.tag(),
        tags::STAR_FU_ORIONIS,
        "a star's FU Orionis outbursts are its star.ev.fu_orionis series"
    );
    let model = FuOrionisModel::of(star)?;
    bins()
        .event(series, id, &model)
        .map(|event| model.outburst_of(event))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample,
        poisson_interval,
    };

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::events::{EventBin, event_id};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::{Galaxy, Population};
    use crate::id::{BodyId, SystemId, event_tags};
    use crate::rng::Seed;
    use crate::stellar::Phase;
    use crate::stellar::draws::{StarDraws, StarDrawsParts};
    use crate::stellar::premain::{ProtostarClass, disc_lifetime_mean, protostar_class};
    use crate::stellar::testing::{Sampling, reference_position, sample_inputs};
    use crate::time::SourceHorizon;
    use crate::units::consts::{GM_SUN, SOLAR_LUMINOSITY_W, SPEED_OF_LIGHT};

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000d);
    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;
    const T_P: f64 = PROTOSTAR_YEARS;

    /// A star of `mass` M☉, `age` years old at the epoch, whose disc-lifetime rank is `rank`, at
    /// the median of its other draws.
    fn star_ranked(mass: f64, age: f64, rank: f64) -> StarModel {
        let draws = StarDraws::from_parts(StarDrawsParts {
            disc_lifetime: UnitUniform::new(rank).unwrap(),
            ..StarDrawsParts::MEDIAN
        });
        StarModel::new(
            SolarMasses::new(mass),
            Composition::SOLAR,
            draws,
            Years::new(age),
        )
        .unwrap()
    }

    /// A star of `mass` M☉, `age` years old at the epoch, at the median draws.
    fn star(mass: f64, age: f64) -> StarModel {
        star_ranked(mass, age, 0.5)
    }

    /// The disc-lifetime rank that gives a star of `mass` M☉ a disc of `myr` Myr: u = 1 − e^(−t/τ).
    fn rank_for(mass: f64, myr: f64) -> f64 {
        -math::exp_m1(-myr / disc_lifetime_mean(SolarMasses::new(mass)).value())
    }

    fn rates_of(mass: f64, rank: f64) -> FuOrionisRates {
        FuOrionisRates::new(SolarMasses::new(mass), UnitUniform::new(rank).unwrap()).unwrap()
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    fn series(index: u16) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_FU_ORIONIS, body(index).into())
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// `years` Julian years from the epoch.
    fn years(years: f64) -> UniverseTime {
        after(UniverseTime::EPOCH, years * YEAR)
    }

    /// The instant `n` nanoseconds from `t`.
    fn shifted(t: UniverseTime, n: i64) -> UniverseTime {
        from_nanos_saturating(to_nanos(t) + i128::from(n))
    }

    /// The bins `from..to` as one window.
    fn bins_window(from: i64, to: i64) -> TimeWindow {
        TimeWindow::new(at(from * BIN_SECONDS, 0), at(to * BIN_SECONDS, 0)).unwrap()
    }

    /// The whole bins of the source horizon, −263,000 to +1,000 years.
    fn horizon() -> TimeWindow {
        let w = bins_window(-526, 2);
        assert!(SourceHorizon::contains(w.start()), "{w:?}");
        assert_eq!(w.end(), ClockWindow::END);
        w
    }

    fn list(star: &StarModel, series: &EventSeries, w: TimeWindow) -> Vec<Outburst> {
        let mut out = Vec::new();
        outbursts_in(star, series, w, &mut out);
        out
    }

    /// The outbursts of `n` series of `star` over `w`, one series after another.
    fn list_many(star: &StarModel, n: u16, w: TimeWindow) -> Vec<Outburst> {
        let mut out = Vec::new();
        for i in 0..n {
            outbursts_in(star, &series(i), w, &mut out);
        }
        out
    }

    fn running(star: &StarModel, series: &EventSeries, t: UniverseTime) -> Vec<Outburst> {
        let mut out = Vec::new();
        outbursts_active_at(star, series, t, &mut out);
        out
    }

    fn per_year(rate: EventsPerSecond) -> f64 {
        rate.value() * YEAR
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    /// Simpson's rule for `integrand` over `[from, to]` in `steps` (even) steps.
    fn simpson(integrand: impl Fn(f64) -> f64, from: f64, to: f64, steps: u32) -> f64 {
        let width = (to - from) / f64::from(steps);
        let mut sum = integrand(from) + integrand(to);
        for i in 1..steps {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * integrand(from + width * f64::from(i));
        }
        sum * width / 3.0
    }

    /// The rate is each phase's total of §3.0 away from the steps, its components split as the
    /// law says, and zero before the onset of collapse, after the disc and, from 2 M☉, in the disc
    /// phase; the clock model's rate is the closed form's.
    #[test]
    fn the_rate_is_each_phases_total_away_from_the_steps_and_zero_outside_them() {
        use OutburstComponent::{DiscFuOri, ProtostellarFuOri, V1647Ori};
        let one = rates_of(1.0, rank_for(1.0, 3.0));
        assert!((one.disc_lifetime().value() - 3.0).abs() < 1e-12);
        let rate = |r: &FuOrionisRates, age: f64| per_year(r.rate_at_age(Years::new(age)));
        let part = |r: &FuOrionisRates, c: OutburstComponent, age: f64| {
            per_year(r.component_rate_at_age(c, Years::new(age)))
        };
        for age in [1.0, 5e4, 1.2e5] {
            assert!(relative(rate(&one, age), 1.0 / 438.0) < 1e-12, "{age}");
            assert!(relative(part(&one, ProtostellarFuOri, age), 1.0 / 5_000.0) < 1e-12);
            assert!(relative(part(&one, V1647Ori, age), 1.0 / 438.0 - 1.0 / 5_000.0) < 1e-12);
            assert!((1.0 / part(&one, V1647Ori, age) - 480.0).abs() < 0.1);
            assert_same_bits(part(&one, DiscFuOri, age), 0.0);
        }
        for age in [1.72e5, 3e5, 4.74e5] {
            assert!(relative(rate(&one, age), 1.0 / 1_741.0) < 1e-12, "{age}");
            assert!(relative(part(&one, ProtostellarFuOri, age), 1.0 / 5_000.0) < 1e-12);
            assert!((1.0 / part(&one, V1647Ori, age) - 2_671.0).abs() < 0.2);
            assert_same_bits(part(&one, DiscFuOri, age), 0.0);
        }
        for age in [5.26e5, 1e6, 2.97e6] {
            assert!(relative(rate(&one, age), 1.0 / 112_000.0) < 1e-12, "{age}");
            assert_same_bits(part(&one, ProtostellarFuOri, age), 0.0);
            assert_same_bits(part(&one, V1647Ori, age), 0.0);
        }
        for age in [-1e3, -1e-9, 0.0, 3.026e6, 1e7, 1.3e10] {
            assert_same_bits(rate(&one, age), 0.0);
        }
        assert!((one.last_age().value() - 3.025e6).abs() < 1e-6);
        // From 2 M☉ the disc phase holds nothing; just below it, the disc's rate.
        for (mass, disc) in [(1.99, true), (2.0, false), (3.0, false), (150.0, false)] {
            let r = rates_of(mass, 0.5);
            assert_eq!(r.has_disc_phase(), disc, "{mass}");
            assert!(relative(rate(&r, 1e5), 1.0 / 438.0) < 1e-12);
            assert!(relative(rate(&r, 3e5), 1.0 / 1_741.0) < 1e-12);
            if !disc {
                assert_same_bits(rate(&r, 5.25e5), 0.0);
                assert_same_bits(r.last_age().value(), T_P + 25_000.0);
            }
        }
        // The clock model gives the closed form's rate in age.
        for (mass, age) in [
            (0.5, 1e5),
            (0.5, CLASS_0_END.value()),
            (1.0, T_P),
            (0.3, 2e6),
        ] {
            let s = star(mass, age);
            let model = FuOrionisModel::of(&s).unwrap();
            for offset in [-2e3, 0.0, 777.0] {
                let clock = per_year(model.rate(years(offset)));
                let closed = rate(model.rates(), age + offset);
                assert!(
                    (clock - closed).abs() <= 1e-9 * closed,
                    "{mass} M☉ at {age} yr + {offset}: {clock} against {closed}"
                );
                for c in OutburstComponent::ALL {
                    let clock = per_year(model.component_rate_at(c, years(offset)));
                    let closed = per_year(model.component_rate_at_age(c, Years::new(age + offset)));
                    assert!((clock - closed).abs() <= 1e-9 * closed.max(1e-300));
                }
            }
        }
    }

    /// Each smooth step of width w integrates to the sharp step's, so that a star's expected count
    /// is the sharp model's: Class 0 to I, the protostar's end into the disc or into nothing, and
    /// the disc's end.
    #[test]
    fn each_smooth_step_integrates_to_the_sharp_steps() {
        let w = STEP_WIDTH.value();
        let check = |r: &FuOrionisRates, centre: f64, before: f64, after: f64| {
            let f = |age: f64| per_year(r.rate_at_age(Years::new(age)));
            let integral = simpson(f, centre - w, centre + w, 4_000);
            let sharp = (before + after) * w;
            assert!(
                (integral - sharp).abs() < 1e-9 * sharp,
                "step at {centre}: {integral} against {sharp}"
            );
        };
        let one = rates_of(1.0, rank_for(1.0, 3.0));
        check(&one, CLASS_0_END.value(), 1.0 / 438.0, 1.0 / 1_741.0);
        check(&one, T_P, 1.0 / 1_741.0, 1.0 / 112_000.0);
        check(&one, 3e6, 1.0 / 112_000.0, 0.0);
        check(&rates_of(3.0, 0.5), T_P, 1.0 / 1_741.0, 0.0);
        // The whole life's: t_c ÷ 438 + (t_p − t_c) ÷ 1,741 + (t_d − t_p) ÷ 112,000.
        let life = simpson(
            |a| per_year(one.rate_at_age(Years::new(a))),
            1e-6,
            3.1e6,
            62_000,
        );
        let sharp = CLASS_0_END.value() / 438.0
            + (T_P - CLASS_0_END.value()) / 1_741.0
            + (3e6 - T_P) / 112_000.0;
        assert!(relative(life, sharp) < 1e-6, "{life} against {sharp}");
    }

    /// The age rules are T15's: a track star is a protostar exactly while its age is below `t_p`,
    /// and Class 0 exactly while it is below `t_c`, at every mass and across the clock window; and
    /// the model's clock times are the ages they stand for.
    #[test]
    fn the_age_rules_agree_with_t15() {
        let times = [
            ClockWindow::START,
            years(-500.0),
            UniverseTime::EPOCH,
            years(500.0),
            ClockWindow::END,
        ];
        let mut seen = [0_u32; 3];
        for mass in [0.1, 0.3, 1.0, 3.0, 8.0, 30.0, 150.0] {
            for age in [5e3, 1e5, 1.46e5, 1.47e5, 3e5, 4.995e5, 5.004e5, 7e5] {
                let s = star(mass, age);
                for t in times {
                    let a = s.age_at(t).value();
                    let state = s.state_at(t).unwrap();
                    let protostar = state.phase() == Phase::Protostar;
                    assert_eq!(protostar, a < T_P, "{mass} M☉ at {a} yr");
                    let class0 = protostar_class(SolarMasses::new(mass), Years::new(a))
                        == Some(ProtostarClass::Class0);
                    assert_eq!(class0, a < CLASS_0_END.value(), "{mass} M☉ at {a} yr");
                    seen[usize::from(protostar) + usize::from(class0)] += 1;
                }
                let model = FuOrionisModel::of(&s).unwrap();
                for (t, expected) in [
                    (model.formation(), 0.0),
                    (model.class_0_end(), CLASS_0_END.value()),
                    (model.protostar_end(), T_P),
                    (model.disc_end(), model.disc_lifetime().value() * 1e6),
                    (model.last_instant(), model.rates().last_age().value()),
                ] {
                    assert!(
                        (s.age_at(t).value() - expected).abs() < 1e-3,
                        "{mass} M☉, {t}"
                    );
                }
            }
        }
        assert!(seen.iter().all(|&n| n > 30), "{seen:?}");
    }

    /// The parent's test: the bound is never exceeded, at many instants of every bin around the
    /// star's onset of collapse and each step, from 0.1 to 150 M☉, with discs of 0.3 Myr, the
    /// median and 15 Myr, and discs that end inside the protostar's step or before it on both
    /// sides of 2 M☉; and a bin's mean is at most 64.
    #[test]
    fn the_bound_holds_across_mass_disc_and_every_step() {
        let bins = bins();
        let mut cases: Vec<(f64, f64)> = Vec::new();
        for mass in [0.1, 0.5, 1.0, 1.9, 2.1, 5.0, 30.0, 150.0] {
            for rank in [1e-9, 0.5, 1.0 - 1e-9] {
                cases.push((mass, rank));
            }
        }
        for mass in [1.5, 3.0] {
            for myr in [0.51, 0.49, 0.35] {
                cases.push((mass, rank_for(mass, myr)));
            }
        }
        let (mut checked, mut floors, mut caps) = (0_u32, 0_u32, 0_u32);
        for (mass, rank) in cases {
            let disc = rates_of(mass, rank).disc_lifetime().value();
            floors += u32::from(disc <= 0.3);
            caps += u32::from(disc >= 15.0);
            for centre in [0.0, CLASS_0_END.value(), T_P, disc * 1e6] {
                // The step's centre 123.4 years after the epoch, inside bin 0.
                let s = star_ranked(mass, centre - 123.4, rank);
                let model = FuOrionisModel::of(&s).unwrap();
                for k in -52..=52 {
                    let window = bins.bin_window(EventBin::new(k).unwrap()).unwrap();
                    let bound = model.bound(window);
                    #[expect(clippy::cast_precision_loss, reason = "a bin of 1.6e10 s")]
                    let mean = bound.value() * BIN_SECONDS as f64;
                    assert!(mean <= PoissonBins::MAX_MEAN_PER_BIN, "{mass} M☉: {mean}");
                    let last = shifted(window.end(), -1);
                    let instants = (0..=40)
                        .map(|i| at(window.start().seconds() + i * (BIN_SECONDS / 40), 0))
                        .chain([shifted(window.start(), 1), last]);
                    for t in instants {
                        assert!(
                            model.rate(t) <= bound,
                            "{mass} M☉, disc {disc} Myr, step at {centre} yr, bin {k}, {t}: \
                             {:?} above {bound:?}",
                            model.rate(t)
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(floors >= 4 && caps >= 4, "{floors} floors, {caps} caps");
        assert!(checked > 300_000, "{checked}");
    }

    /// The binomial check of word 0: the components at an onset are in the ratio of the rate's
    /// terms there, in Class 0 (an FU Ori share of 438 ÷ 5,000) and in Class I (1,741 ÷ 5,000),
    /// and every disc-phase outburst is a disc FU Ori outburst.
    #[test]
    fn the_component_shares_at_an_onset_are_the_rate_terms_ratios() {
        let cases = [
            (
                "Class 0",
                star(0.5, 1e5),
                60,
                bins_window(-180, 0),
                438.0 / 5_000.0,
            ),
            (
                "Class I",
                star(0.5, 4.5e5),
                20,
                bins_window(-500, 0),
                1_741.0 / 5_000.0,
            ),
        ];
        for (name, s, n, w, share) in cases {
            let outbursts = list_many(&s, n, w);
            assert!(
                outbursts
                    .iter()
                    .all(|o| o.component() != OutburstComponent::DiscFuOri),
                "{name}"
            );
            let fu = outbursts
                .iter()
                .filter(|o| o.component() == OutburstComponent::ProtostellarFuOri)
                .count();
            let (fu, total) = (
                u64::try_from(fu).unwrap(),
                u64::try_from(outbursts.len()).unwrap(),
            );
            #[expect(clippy::cast_precision_loss, reason = "a count of thousands")]
            let expected = [total as f64 * share, total as f64 * (1.0 - share)];
            let fit = chi_square_gof(&[fu, total - fu], &expected);
            eprintln!("{name}: {fu} FU Ori of {total}, expected share {share:.4}");
            assert!(total > 2_000, "{name}: {total}");
            assert_p_value(&format!("{name}: the FU Ori share"), fit.p_value, ALPHA);
        }
        let disc = star_ranked(0.5, 2e6, rank_for(0.5, 3.0));
        let outbursts = list_many(&disc, 40, horizon());
        assert!(outbursts.len() > 50, "{}", outbursts.len());
        assert!(
            outbursts
                .iter()
                .all(|o| o.component() == OutburstComponent::DiscFuOri)
        );
    }

    /// A rate model that accepts every candidate of a model's bins: the same bound, and the rate
    /// equal to it.
    struct EveryCandidate<'a>(&'a FuOrionisModel<'a>);

    impl RateModel for EveryCandidate<'_> {
        fn bound(&self, bin: TimeWindow) -> EventsPerSecond {
            self.0.bound(bin)
        }

        fn rate(&self, t: UniverseTime) -> EventsPerSecond {
            let bins = bins();
            self.0
                .bound(bins.bin_window(bins.bin_of(t).unwrap()).unwrap())
        }
    }

    /// `outburst(id)` regenerates every listed outburst around a star's onset of collapse in the
    /// middle of a bin, and returns `None` for a rejected candidate, a number beyond its bin's
    /// count, an ID of another series and an object below 0.1 M☉.
    #[test]
    fn an_id_regenerates_its_outburst_and_names_nothing_else() {
        // A solar-mass star that starts to collapse 373 years before the epoch, in bin −1.
        let young = star(1.0, 373.0);
        let model = FuOrionisModel::of(&young).unwrap();
        let every = EveryCandidate(&model);
        let (mut rejected, mut beyond, mut found) = (0, 0, 0);
        for index in 0..60 {
            let s = series(index);
            let listed = list(&young, &s, bins_window(-2, 2));
            assert!(listed.iter().all(|o| o.onset() > model.formation()));
            for bin in -2..2 {
                for j in 0..=u8::MAX {
                    let id = s.event_id(EventBin::new(bin).unwrap(), j);
                    let expected = listed.iter().find(|o| o.id() == id);
                    assert_eq!(
                        outburst(&young, &s, id).as_ref(),
                        expected,
                        "series {index}, bin {bin}, number {j}"
                    );
                    match (expected, bins().event(&s, id, &every)) {
                        (None, Some(_)) => rejected += 1,
                        (None, None) => beyond += 1,
                        (Some(_), _) => found += 1,
                    }
                }
            }
        }
        assert!(
            rejected > 5 && beyond > 10_000 && found > 100,
            "{rejected} rejected, {beyond} beyond, {found} found"
        );
        let s = series(0);
        let listed = list(&young, &s, bins_window(0, 2));
        let word = listed[0].id().word();
        for foreign in [
            event_id(
                body(9).into(),
                tags::STAR_FU_ORIONIS,
                word.bin(),
                word.number(),
            ),
            event_id(
                body(0).into(),
                event_tags::SELF_TEST,
                word.bin(),
                word.number(),
            ),
        ] {
            assert_eq!(outburst(&young, &s, foreign), None);
        }
        assert_eq!(outburst(&star(0.05, 373.0), &s, listed[0].id()), None);
    }

    /// The outbursts of `component` from `n` series of `star` over `w`.
    fn of_component(
        star: &StarModel,
        n: u16,
        w: TimeWindow,
        component: OutburstComponent,
    ) -> Vec<Outburst> {
        let mut outbursts = list_many(star, n, w);
        outbursts.retain(|o| o.component() == component);
        outbursts
    }

    /// The K–S test of `values` against the log-uniform law on `(lo, hi)`.
    fn assert_log_uniform(what: &str, mut values: Vec<f64>, (lo, hi): (f64, f64)) {
        assert!(values.len() > 200, "{what}: {}", values.len());
        assert!(
            values.iter().all(|&v| (lo..=hi).contains(&v)),
            "{what} outside {lo}–{hi}"
        );
        let span = math::ln(hi / lo);
        let ks = ks_one_sample(&mut values, |x| (math::ln(x / lo) / span).clamp(0.0, 1.0));
        assert_p_value(what, ks.p_value, ALPHA);
    }

    /// Every mark lies in its class's range and follows its log-uniform law (K–S), every outburst
    /// ends within 500 years, and an FU Ori decay is 2.5 log₁₀ F ÷ ḟ, cut to 500 years less the
    /// rise.
    #[test]
    fn every_mark_follows_its_law_and_every_outburst_ends_within_a_bin() {
        let class0 = star(0.5, 1e5);
        let disc = star_ranked(0.5, 2e6, rank_for(0.5, 3.0));
        let samples = [
            (
                OutburstComponent::ProtostellarFuOri,
                of_component(
                    &class0,
                    30,
                    bins_window(-180, 0),
                    OutburstComponent::ProtostellarFuOri,
                ),
            ),
            (
                OutburstComponent::V1647Ori,
                of_component(
                    &class0,
                    4,
                    bins_window(-180, 0),
                    OutburstComponent::V1647Ori,
                ),
            ),
            (
                OutburstComponent::DiscFuOri,
                of_component(&disc, 300, horizon(), OutburstComponent::DiscFuOri),
            ),
        ];
        let mut cut = 0;
        for (component, outbursts) in samples {
            let name = format!("{component:?}");
            assert_log_uniform(
                &format!("{name}: drawn peak factors"),
                outbursts.iter().map(Outburst::drawn_peak_factor).collect(),
                component.peak_factor_range(),
            );
            assert_log_uniform(
                &format!("{name}: rises"),
                outbursts
                    .iter()
                    .map(|o| o.rise().as_julian_years_f64())
                    .collect(),
                component.rise_range_years(),
            );
            for o in &outbursts {
                assert!(o.duration() <= MAX_DURATION, "{o:?}");
                assert_eq!(o.duration(), o.rise().checked_add(o.decay()).unwrap());
                assert_eq!(o.peak(), o.onset().checked_add(o.rise()).unwrap());
                assert_eq!(o.end(), o.onset().checked_add(o.duration()).unwrap());
            }
            match component.class() {
                OutburstClass::FuOri => {
                    assert_log_uniform(
                        &format!("{name}: fading rates"),
                        outbursts
                            .iter()
                            .map(|o| o.fading_magnitudes_per_year().unwrap())
                            .collect(),
                        FU_ORI_FADING_MAGNITUDES_PER_YEAR,
                    );
                    for o in &outbursts {
                        let left = MAX_DURATION.checked_sub(o.rise()).unwrap();
                        let natural = years_span(
                            2.5 * math::log10(o.drawn_peak_factor())
                                / o.fading_magnitudes_per_year().unwrap(),
                        );
                        assert_eq!(o.decay(), natural.min(left), "{o:?}");
                        cut += u32::from(natural > left);
                    }
                }
                OutburstClass::V1647Ori => {
                    assert!(
                        outbursts
                            .iter()
                            .all(|o| o.fading_magnitudes_per_year().is_none())
                    );
                    assert_log_uniform(
                        &format!("{name}: decays"),
                        outbursts
                            .iter()
                            .map(|o| o.decay().as_julian_years_f64())
                            .collect(),
                        V1647_ORI_DECAY_YEARS,
                    );
                }
            }
        }
        // F up to 300 at ḟ down to 0.012 a year passes 500 years, about once in 1,800 disc
        // outbursts; the cut itself, at the corner.
        eprintln!("{cut} decays cut to 500 years less the rise");
        let rise = years_span(20.0);
        let corner = fu_ori_decay(299.0, 0.012, rise);
        assert_eq!(corner, MAX_DURATION.checked_sub(rise).unwrap());
        let uncut = fu_ori_decay(50.0, 0.012, rise);
        assert!((uncut.as_julian_years_f64() - 2.5 * math::log10(50.0) / 0.012).abs() < 1e-6);
    }

    /// The factor is 1 outside an outburst and at its ends, its peak factor at the peak, F^½
    /// half-way through the rise and the decay (linear in magnitude), continuous at the onset, the
    /// peak and the end, and rises, then falls.
    #[test]
    fn the_factor_is_one_outside_an_outburst_and_its_peak_factor_at_the_peak() {
        let protostar = star(0.5, 1e5);
        let outbursts = list_many(&protostar, 4, bins_window(-60, 0));
        let fu = outbursts
            .iter()
            .find(|o| o.class() == OutburstClass::FuOri)
            .unwrap();
        let small = outbursts
            .iter()
            .find(|o| o.class() == OutburstClass::V1647Ori)
            .unwrap();
        for o in [fu, small] {
            let (peak, top) = (o.peak(), o.peak_factor());
            assert_same_bits(o.luminosity_factor_at(shifted(o.onset(), -1)), 1.0);
            assert_same_bits(o.luminosity_factor_at(o.onset()), 1.0);
            assert_same_bits(o.luminosity_factor_at(o.end()), 1.0);
            assert_same_bits(o.luminosity_factor_at(peak), top);
            assert!(o.is_active_at(o.onset()) && !o.is_active_at(o.end()));
            assert!(!o.is_active_at(shifted(o.onset(), -1)));
            // Continuous: a second moves it by a sliver of its excess anywhere.
            let excess = top - 1.0;
            for t in [
                o.onset(),
                shifted(peak, -1_000_000_000),
                peak,
                shifted(o.end(), -1_000_000_000),
            ] {
                let step =
                    (o.luminosity_factor_at(after(t, 1.0)) - o.luminosity_factor_at(t)).abs();
                assert!(step < 1e-5 * excess, "{step} at {t}");
            }
            let half_rise = after(o.onset(), o.rise().as_seconds_f64() / 2.0);
            let half_decay = after(peak, o.decay().as_seconds_f64() / 2.0);
            for t in [half_rise, half_decay] {
                assert!(
                    relative(o.luminosity_factor_at(t), top.sqrt()) < 1e-9,
                    "{t}"
                );
            }
            // Rising to the peak and falling after it, at 2,000 instants.
            let samples = 2_000_u32;
            let (mut last, mut past_peak) = (1.0, false);
            for i in 1..=samples {
                let t = after(
                    o.onset(),
                    o.duration().as_seconds_f64() * f64::from(i) / f64::from(samples),
                );
                let factor = o.luminosity_factor_at(t);
                assert!((1.0..=top).contains(&factor));
                if t <= peak {
                    assert!(factor >= last);
                } else {
                    // The first sample past the peak is compared with the peak itself.
                    if !past_peak {
                        (past_peak, last) = (true, top);
                    }
                    assert!(factor <= last);
                }
                last = factor;
            }
        }
    }

    /// The Eddington luminosity of a star of `mass` M☉ at hydrogen fraction `x`, L☉: 4π c G M ÷ κ
    /// with κ = 0.02 (1 + X) m² kg⁻¹.
    fn eddington_luminosity(mass: f64, x: f64) -> f64 {
        4.0 * core::f64::consts::PI * SPEED_OF_LIGHT * GM_SUN * mass
            / (0.02 * (1.0 + x))
            / SOLAR_LUMINOSITY_W
    }

    /// The cap: a peak factor is at least 1 and at most the drawn one, and the peak luminosity at
    /// most the larger of the quiescent and the Eddington luminosity, over masses to 150 M☉
    /// through the protostar phase; it binds for massive protostars and never at 1 M☉.
    #[test]
    fn the_peak_is_held_at_the_eddington_luminosity() {
        let mut binding: BTreeMap<String, u32> = BTreeMap::new();
        for mass in [0.1, 0.5, 1.0, 3.0, 10.0, 30.0, 60.0, 100.0, 150.0] {
            for age in [2e4, 1e5, 3e5, 4.9e5] {
                let s = star(mass, age);
                let x = 0.76 - 3.0 * s.composition().z_fit().value();
                let outbursts = list_many(&s, 12, bins_window(-20, 0));
                assert!(outbursts.len() > 20, "{mass} M☉ at {age} yr");
                let mut capped = 0_u32;
                for o in &outbursts {
                    let state = s.state_at(o.onset()).unwrap();
                    let (l, m) = (state.luminosity().value(), state.mass().value());
                    let ceiling = l.max(eddington_luminosity(m, x));
                    assert!(o.peak_factor() >= 1.0 && o.peak_factor() <= o.drawn_peak_factor());
                    assert!(
                        o.peak_factor() * l <= ceiling * (1.0 + 1e-12),
                        "{mass} M☉ at {age} yr: {o:?}"
                    );
                    if o.peak_factor() < o.drawn_peak_factor() {
                        capped += 1;
                        assert!(relative(o.peak_factor() * l, ceiling) < 1e-12);
                    }
                }
                let state = s.state_at(UniverseTime::EPOCH).unwrap();
                let limit =
                    eddington_luminosity(state.mass().value(), x) / state.luminosity().value();
                eprintln!(
                    "{mass} M☉ at {age} yr: L {:.4e} L☉, M {:.3} M☉, L_Edd ÷ L {limit:.3}; \
                     {capped} of {} capped",
                    state.luminosity().value(),
                    state.mass().value(),
                    outbursts.len()
                );
                *binding.entry(format!("{mass}")).or_insert(0) += capped;
            }
        }
        assert_eq!(binding["1"], 0, "the cap never binds at 1 M☉");
        assert!(binding["30"] > 0 && binding["100"] > 0, "{binding:?}");
    }

    /// Objects of 0.01–0.099 M☉ have no model and no outbursts; a 3 M☉ star has none from the end
    /// of its protostar's step, half a step width past `t_p`.
    #[test]
    fn substellar_objects_have_none_and_a_3_msun_star_none_past_its_protostar() {
        for mass in [0.01, 0.05, 0.08, 0.099] {
            let s = star(mass, 1e5);
            assert_eq!(FuOrionisModel::of(&s), None, "{mass}");
            assert_eq!(
                FuOrionisRates::new(SolarMasses::new(mass), UnitUniform::HALF),
                None
            );
            assert!(list(&s, &series(0), horizon()).is_empty());
            assert!(running(&s, &series(0), UniverseTime::EPOCH).is_empty());
            let id = series(0).event_id(EventBin::new(0).unwrap(), 0);
            assert_eq!(outburst(&s, &series(0), id), None);
        }
        assert_eq!(
            FuOrionisRates::new(SolarMasses::new(f64::NAN), UnitUniform::HALF),
            None
        );
        // Ages 337,000–601,000 years over the horizon.
        let s = star(3.0, 6e5);
        let model = FuOrionisModel::of(&s).unwrap();
        let outbursts = list_many(&s, 10, horizon());
        assert!(outbursts.len() > 300, "{}", outbursts.len());
        let end = T_P + STEP_WIDTH.value() / 2.0;
        assert!(outbursts.iter().all(|o| s.age_at(o.onset()).value() < end));
        let past = TimeWindow::new(model.last_instant(), ClockWindow::END).unwrap();
        assert!(list_many(&s, 10, past).is_empty());
        assert_same_bits(model.rate(model.last_instant()).value(), 0.0);
        assert_same_bits(model.bound(bins_window(0, 1)).value(), 0.0);
    }

    /// The step width's three forms are one number: 50,000 Julian years, in years, in nanoseconds
    /// and as an `f64` of nanoseconds, so that the clock's steps and the closed form's agree.
    #[test]
    fn the_step_width_is_one_number_in_its_three_forms() {
        let nanos = 50_000 * i128::from(SECONDS_PER_JULIAN_YEAR_I64) * 1_000_000_000;
        assert_eq!(STEP_WIDTH_NANOS, nanos);
        assert_same_bits(STEP_WIDTH.value(), 50_000.0);
        assert_same_bits(STEP_WIDTH_NANOS_F64, STEP_WIDTH.value() * YEAR * 1e9);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the width is exact in f64, checked here"
        )]
        let as_f64 = STEP_WIDTH_NANOS as f64;
        assert_same_bits(as_f64, STEP_WIDTH_NANOS_F64);
    }

    /// Stars whose onset of collapse or last instant falls inside the source horizon: the 0.8 M☉
    /// star whose disc ends at 1 Myr, a 3 M☉ star past its protostar phase, a solar-mass star that
    /// starts to collapse mid-bin, and a protostar that starts within the window.
    fn edge_stars() -> [StarModel; 4] {
        [
            star_ranked(0.8, 1.05e6, rank_for(0.8, 1.0)),
            star(3.0, 6e5),
            star(1.0, 373.0),
            star(0.5, 1e5),
        ]
    }

    /// The listing's cut to the star's active part drops nothing: over the source horizon it gives
    /// exactly the outbursts of every bin the uncut window touches, for stars whose onset of
    /// collapse or last instant lies inside it.
    #[test]
    fn the_cut_to_the_active_part_drops_nothing() {
        let mut seen = 0;
        for s in edge_stars() {
            let model = FuOrionisModel::of(&s).unwrap();
            let inside =
                horizon().contains(model.formation()) || horizon().contains(model.last_instant());
            assert!(inside, "{:?}", model.formation());
            for index in 0..8 {
                let sr = series(index);
                let mut events = Vec::new();
                bins().events_in(&sr, horizon(), &model, &mut events);
                let uncut: Vec<Outburst> =
                    events.into_iter().map(|e| model.outburst_of(e)).collect();
                let listed = list(&s, &sr, horizon());
                assert_eq!(listed, uncut, "series {index}");
                seen += listed.len();
            }
        }
        assert!(seen > 500, "{seen} outbursts");
    }

    /// The partition rule at the active part's edges: cuts at the onset of collapse, a nanosecond
    /// after it, a nanosecond before the last instant and at it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_partitions_at_the_active_parts_edges() {
        for s in edge_stars() {
            let model = FuOrionisModel::of(&s).unwrap();
            let sr = series(5);
            let listing = |w: TimeWindow| list(&s, &sr, w);
            let mut cuts: Vec<UniverseTime> = [
                model.formation(),
                shifted(model.formation(), 1),
                shifted(model.last_instant(), -1),
                model.last_instant(),
            ]
            .into_iter()
            .filter(|&t| horizon().contains(t))
            .collect();
            cuts.sort_unstable();
            assert!(!cuts.is_empty());
            assert_partition_independent(horizon(), &cuts, listing);
            assert_order_independent(&partition(horizon(), &cuts), |w| listing(*w));
        }
    }

    /// The window and cut sets of the partition tests, on a Class 0 protostar.
    fn partition_case(
        listing: impl Fn(TimeWindow) -> Vec<Outburst>,
    ) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let whole =
            TimeWindow::new(at(-60 * BIN_SECONDS - 1_234, 5), at(BIN_SECONDS + 77, 0)).unwrap();
        let outbursts = listing(whole);
        assert!(outbursts.len() > 30, "{} outbursts", outbursts.len());
        let cuts = vec![
            vec![at(0, 0)],
            vec![
                at(-BIN_SECONDS, 0),
                at(-BIN_SECONDS, 0),
                at(BIN_SECONDS / 2, 1),
                at(BIN_SECONDS, 3),
            ],
            vec![outbursts[10].onset(), after(outbursts[11].onset(), 1e-9)],
            (-59..1)
                .map(|k| at(k * BIN_SECONDS + 3_600 * k, 0))
                .collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test through the public function: a listing over a window is the
    /// union of the listings over any partition of it, in any order of asking.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let s = star(0.5, 1e5);
        let sr = series(2);
        let listing = |w: TimeWindow| list(&s, &sr, w);
        let (whole, cut_sets) = partition_case(listing);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
    }

    /// The pieces listed on threads of their own give the same outbursts. Not on wasm32-wasip1,
    /// which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let s = star(0.5, 1e5);
        let sr = series(2);
        let listing = |w: TimeWindow| list(&s, &sr, w);
        let (whole, cut_sets) = partition_case(listing);
        let outbursts = listing(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<Outburst>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), outbursts);
        }
    }

    /// `outbursts_active_at` gives exactly the listed outbursts running at t.
    #[test]
    fn outbursts_active_at_gives_the_outbursts_running_then() {
        let s = star(0.5, 1e5);
        let mut seen = 0;
        for index in 0..4 {
            let sr = series(index);
            let listed = list(&s, &sr, bins_window(-10, 4));
            for i in 0..2_000 {
                let t = at(-8 * BIN_SECONDS + i64::from(i) * 86_400 * 1_004 + 17, 3);
                let expected: Vec<Outburst> = listed
                    .iter()
                    .copied()
                    .filter(|o| o.is_active_at(t))
                    .collect();
                let got = running(&s, &sr, t);
                assert_eq!(got, expected, "series {index} at {t}");
                seen += got.len();
            }
        }
        assert!(seen > 100, "{seen} running outbursts seen");
        // Nothing runs before the onset of collapse or long after the last instant.
        assert!(running(&star(1.0, 1e9), &series(0), UniverseTime::EPOCH).is_empty());
        assert!(running(&star(1.0, -2e3), &series(0), UniverseTime::EPOCH).is_empty());
    }

    /// The parent's no-history test for this kind: the outbursts running at +900 years, asked
    /// cold, are the same after asking about 100 earlier times.
    #[test]
    fn the_outbursts_running_at_900_years_need_no_history() {
        let base = years(900.0);
        let probe = star(0.5, 1e5);
        let (index, t) = (0..64)
            .flat_map(|i| (0..100).map(move |k| (i, after(base, f64::from(k) * YEAR))))
            .find(|&(i, t)| !running(&probe, &series(i), t).is_empty())
            .unwrap();
        let sr = series(index);
        let s = star(0.5, 1e5);
        let cold = running(&s, &sr, t);
        let mut lcg = Lcg::new(0x0928_d900);
        for _ in 0..100 {
            let earlier = after(ClockWindow::START, lcg.next_f64() * 1.899e3 * YEAR);
            assert!(earlier < t);
            let _ = running(&s, &sr, earlier);
            let _ = list(
                &s,
                &sr,
                TimeWindow::new(earlier, after(earlier, 50.0 * YEAR)).unwrap(),
            );
        }
        assert_eq!(running(&s, &sr, t), cold);
        assert!(!cold.is_empty());
    }

    /// Writes `model`'s disc, its clock times, its bound and clock rate about each of them, and
    /// its rates in age at six ages, for the golden.
    fn pin_model(w: &mut GoldenWriter, model: &FuOrionisModel<'_>) {
        w.f64("disc lifetime (Myr)", model.disc_lifetime().value());
        w.line(&format!("disc phase: {}", model.rates().has_disc_phase()));
        w.line(&format!("formation at {}", model.formation()));
        w.line(&format!("class 0 end at {}", model.class_0_end()));
        w.line(&format!("protostar end at {}", model.protostar_end()));
        w.line(&format!("disc end at {}", model.disc_end()));
        w.line(&format!("last instant {}", model.last_instant()));
        // The bound and the clock's rate, which thinning reads: at the bins holding the onset
        // of collapse, each step's centre and the last instant, and a quarter of a step either
        // side of each centre.
        let bins = bins();
        let quarter = STEP_WIDTH.value() * YEAR / 4.0;
        for (label, t) in [
            ("formation", model.formation()),
            ("class 0 end", model.class_0_end()),
            ("protostar end", model.protostar_end()),
            ("disc end", model.disc_end()),
            ("last instant", model.last_instant()),
        ] {
            let bin = bins.bin_window(bins.bin_of(t).unwrap()).unwrap();
            w.f64(
                &format!("bound of the {label} bin (per s)"),
                model.bound(bin).value(),
            );
            for (side, offset) in [("before", -quarter), ("after", quarter)] {
                w.f64(
                    &format!("rate a quarter step {side} the {label} (per s)"),
                    model.rate(after(t, offset)).value(),
                );
            }
        }
        for age in [5e4, CLASS_0_END.value(), 3e5, T_P, 1e6, 2e6] {
            let a = Years::new(age);
            w.f64(
                &format!("rate at {age} yr (per s)"),
                model.rate_at_age(a).value(),
            );
            for c in OutburstComponent::ALL {
                w.f64(
                    &format!("  {c:?} at {age} yr (per s)"),
                    model.component_rate_at_age(c, a).value(),
                );
            }
        }
    }

    /// Pinned: five stars, their step centres, rates and component rates at six ages, and each
    /// outburst's ID, onset, class and component, drawn and capped peak factor, rise, decay,
    /// fading rate and factor at four instants.
    #[test]
    fn fu_orionis_outbursts_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let cases = [
            (
                "a Class 0 protostar, 0.5 M☉ at 0.1 Myr, over the 5,000 years before the epoch",
                star(0.5, 1e5),
                0..1,
                bins_window(-10, 0),
            ),
            (
                "a 1 M☉ star at t_c, its Class 0 step across the 10,000 years before the epoch",
                star(1.0, CLASS_0_END.value()),
                1..2,
                bins_window(-20, 0),
            ),
            (
                "a 0.3 M☉ star at 2 Myr with its disc of 3 Myr, over the source horizon, four \
                 series",
                star_ranked(0.3, 2e6, rank_for(0.3, 3.0)),
                2..6,
                horizon(),
            ),
            (
                "a 0.8 M☉ star at 1.05 Myr whose disc ends at 1 Myr, over the source horizon, six \
                 series",
                star_ranked(0.8, 1.05e6, rank_for(0.8, 1.0)),
                6..12,
                horizon(),
            ),
            (
                "a 30 M☉ protostar at 0.49 Myr, near t_p, over the 20,000 years before the epoch",
                star(30.0, 4.9e5),
                12..13,
                bins_window(-40, 0),
            ),
        ];
        let mut capped = 0;
        for (name, s, indices, window) in cases {
            let model = FuOrionisModel::of(&s).unwrap();
            w.line("");
            w.line(&format!("# {name}"));
            pin_model(&mut w, &model);
            for index in indices {
                for o in list(&s, &series(index), window) {
                    w.line(&format!("{} at {}", o.id(), o.onset()));
                    w.line(&format!(
                        "  class {:?}, component {:?}",
                        o.class(),
                        o.component()
                    ));
                    w.f64("  drawn peak factor", o.drawn_peak_factor());
                    w.f64("  peak factor", o.peak_factor());
                    w.line(&format!("  rise {}", o.rise()));
                    w.line(&format!("  decay {}", o.decay()));
                    if let Some(fading) = o.fading_magnitudes_per_year() {
                        w.f64("  fading rate (mag per yr)", fading);
                    }
                    let instants = [
                        (
                            "half the rise",
                            after(o.onset(), o.rise().as_seconds_f64() / 2.0),
                        ),
                        ("the peak", o.peak()),
                        (
                            "half the decay",
                            after(o.peak(), o.decay().as_seconds_f64() / 2.0),
                        ),
                        ("a day before the end", after(o.end(), -86_400.0)),
                    ];
                    for (label, t) in instants {
                        w.f64(&format!("  factor at {label}"), o.luminosity_factor_at(t));
                    }
                    capped += u32::from(o.peak_factor() < o.drawn_peak_factor());
                }
            }
        }
        assert!(capped > 0, "the 30 M☉ protostar's cap binds");
        golden!("stellar/fu_orionis", w.as_str());
    }

    /// The expected counts of one star's outbursts over `window`, ∫ rate dt, all and of the FU Ori
    /// class, by the midpoint rule on the pieces between the onset of collapse, the steps' ends
    /// and the last instant: one piece on each plateau, where the rate is constant, and pieces of
    /// at most 100 years inside the steps.
    fn expected_counts(model: &FuOrionisModel<'_>, window: TimeWindow) -> (f64, f64) {
        let (lo, hi) = (to_nanos(window.start()), to_nanos(window.end()));
        let half = STEP_WIDTH_NANOS / 2;
        let mut cuts = vec![lo, hi, model.formation, model.last];
        for c in model.centres {
            cuts.extend([c - half, c + half]);
        }
        cuts.retain(|&c| lo <= c && c <= hi);
        cuts.sort_unstable();
        cuts.dedup();
        let in_step = |t: i128| {
            model
                .centres
                .iter()
                .any(|&c| (c - half..c + half).contains(&t))
        };
        let piece = i128::from(100 * SECONDS_PER_JULIAN_YEAR_I64) * 1_000_000_000;
        let (mut all, mut fu) = (0.0, 0.0);
        for pair in cuts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let n = if in_step(a + (b - a) / 2) {
                (b - a) / piece + 1
            } else {
                1
            };
            for k in 0..n {
                let (from, to) = (a + (b - a) * k / n, a + (b - a) * (k + 1) / n);
                let components = model.components_at(from + (to - from) / 2);
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a span of up to 1e22 ns, rounded by 1e-16 of itself"
                )]
                let seconds = (to - from) as f64 / 1e9;
                all += total(components) * seconds;
                fu += (components[0] + components[2]) * seconds;
            }
        }
        (all, fu)
    }

    /// The parent's statistical test for this kind: over 10⁴ stars of the young thin disc's
    /// masses, compositions and draws (T31's `sample_inputs`), at ages from a test-local LCG over
    /// 0–16 Myr, the outbursts listed over the source horizon's whole bins number the rate
    /// integral within a Poisson interval, and the FU Ori-class outbursts number their own.
    #[test]
    #[ignore = "slow: 10⁴ young stars, each with its track (1.1 s under slow-test)"]
    fn the_mean_count_over_10_000_stars_matches_the_rate_integral() {
        const STARS: u16 = 10_000;
        let seed = 0x0628_d000_0000_5eed_u64;
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like()).unwrap();
        let component = galaxy
            .fields()
            .components()
            .iter()
            .position(|c| c.population() == Population::YoungThinDisc)
            .unwrap();
        let sampling = Sampling {
            component,
            position: reference_position(Population::YoungThinDisc),
            band: None,
        };
        let window = horizon();
        let mut lcg = Lcg::new(0x0628_d016);
        let (mut expected, mut expected_fu) = (0.0, 0.0);
        let (mut counted, mut counted_fu) = (0_u64, 0_u64);
        let (mut substellar, mut forming, mut ending, mut discs) = (0_u32, 0_u32, 0_u32, 0_u32);
        for i in 0..STARS {
            let inputs = sample_inputs(&galaxy, &sampling, seed, u64::from(i));
            let age = Years::new(lcg.next_f64() * 1.6e7);
            let s =
                StarModel::new(inputs.initial_mass, inputs.composition, inputs.draws, age).unwrap();
            let Some(model) = FuOrionisModel::of(&s) else {
                substellar += 1;
                assert!(list(&s, &series(i), window).is_empty());
                continue;
            };
            forming += u32::from(window.contains(model.formation()));
            ending += u32::from(window.contains(model.protostar_end()));
            discs += u32::from(model.rates().has_disc_phase() && window.contains(model.disc_end()));
            let (all, fu) = expected_counts(&model, window);
            expected += all;
            expected_fu += fu;
            let outbursts = list(&s, &series(i), window);
            let fu_count = outbursts
                .iter()
                .filter(|o| o.class() == OutburstClass::FuOri)
                .count();
            counted += u64::try_from(outbursts.len()).unwrap();
            counted_fu += u64::try_from(fu_count).unwrap();
        }
        eprintln!(
            "{STARS} stars over the source horizon's 528 bins: expected {expected:.1}, counted \
             {counted}, interval {:?}; FU Ori class expected {expected_fu:.1}, counted \
             {counted_fu}, interval {:?}, at alpha {ALPHA:e}; {substellar} below 0.1 M☉; \
             {forming} form, {ending} end their protostar phase and {discs} their disc inside the \
             window",
            poisson_interval(expected, ALPHA),
            poisson_interval(expected_fu, ALPHA),
        );
        assert!(expected > 500.0 && forming > 50 && ending > 50 && discs > 50);
        assert_poisson_count("outbursts of 10⁴ stars", counted, expected, ALPHA);
        assert_poisson_count(
            "FU Ori-class outbursts of 10⁴ stars",
            counted_fu,
            expected_fu,
            ALPHA,
        );
    }
}
