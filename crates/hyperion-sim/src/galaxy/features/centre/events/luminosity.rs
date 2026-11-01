//! The central black hole's luminosity (plan 09, P09.T30.b): its quiescence, its flares and the
//! t^(−5⁄3) tails of its disruptions in the source horizon.
//!
//! [`luminosity_at`] is, in the unabsorbed 2–10 keV band of the flares (Neilsen et al. 2013, ApJ
//! 774, 42),
//!
//! > L(t) = L_q + Σ flares running at t + Σ tails of the horizon's disrupted victims before t.
//!
//! - **Quiescence**, [`QUIESCENT_LUMINOSITY_WATTS`]: Sgr A*'s, the one Neilsen et al. measure their
//!   flares above, the same for every black hole.
//! - **Flares**: P09.T30.a's, each [`CentreFlare::luminosity_watts_at`](super::CentreFlare::luminosity_watts_at)
//!   above the quiescence.
//! - **Tails** ([`DisruptionTail`]): the debris of a disrupted victim falls back from its
//!   fallback time `t_fb` (Stone and Metzger 2016, MNRAS 455, 859, §3, equation 23) on at
//!   `Ṁ = (M★ ÷ 3 t_fb) (Δt ÷ t_fb)^(−5⁄3)` (Stone, Sari and Loeb 2013, MNRAS 435, 1809,
//!   equations 49–50),
//!   and radiates `ε Ṁ c²`, at most the black hole's Eddington luminosity, of which
//!   [`XRAY_SHARE`], one part in 15.8, is in the band. The efficiency ε is η = 0.1 at every rate
//!   until version 25; from it, it falls with the fallback rate (below).
//!
//! A tail falls as t^(−5⁄3) for ever: no source gives it an end. The light echoes around Sgr A*
//! measure no tail. They record short outbursts of about 10³⁹ erg/s a century or two ago, "a
//! flare of Sgr A* fading about 100 years ago", and a mean below 10³⁶ erg/s since (Ponti et al.
//! 2010, ApJ 714, 732: "an upper limit of 10³⁶ erg s⁻¹ to the mean luminosity of Sgr A* in the
//! last 60–90 years"; Clavel et al. 2013, A&A 558, A32: outbursts "of a few-year time scale").
//! That limit is what the tails are tested against, at the epoch over seeds. A Sun-like star's
//! tail is the module's closed-form check: under η = 0.1 it gives 4.5 × 10³⁸ erg/s a thousand
//! years on, and 2.1 × 10³⁵ erg/s 10⁵ years on, so the horizon's tails keep the centre far above
//! its quiescence in every galaxy (plan 09's Risks, "Findings of P09.T30.b", 2).
//!
//! # The efficiency (P09.T30.c, held until version 25)
//!
//! η = 0.1 is the efficiency of super-Eddington accretion (Stone and Metzger 2016, §3), and a
//! tail is below a tenth of Eddington's rate from about five years on. Below about `α² Ṁ_Edd` the
//! flow is hot and radiates inefficiently (Narayan and Yi 1995, ApJ 452, 710; Yuan and Narayan
//! 2014, ARA&A 52, 529, §2). From `CENTRE_QUIESCENCE_VERSION` a tail radiates at
//! `radiative_efficiency` of its fallback rate, Xie and Yuan's (2012, MNRAS 427, 1580) fit for a
//! hot flow at δ = 0.5, rising to η at 0.6% of Eddington's rate, where it meets Xie and Yuan's
//! two-phase flow (about 8%, near a thin disc's). A Sun-like tail is unchanged for its first 25 years, gives 9.6 × 10³⁷ erg/s a thousand years on,
//! and falls to the quiescence about 4 × 10⁴ years on, where η = 0.1 takes 1.15 × 10⁶. The law
//! takes these approximations, which the research lane `research-p09-t30b` sized over P09.T30.b's
//! 200 seeds at the epoch (where the law gives a median of 9.6 × 10³⁴ erg/s, and 149 seeds within
//! Ponti et al.'s limit):
//!
//! - `Ṁ_net`, the rate at the black hole that Xie and Yuan's ε is per, is taken as the fallback
//!   rate, which errs bright: an outflow from the debris's circularisation radius would lower it,
//!   and the median, by up to about 4×.
//! - Each tail is taken at its own rate, which errs faint: ε at the tails' summed rate would raise
//!   the median about 2×.
//! - [`XRAY_SHARE`] is extrapolated below about 10⁻⁶ of Eddington's luminosity and errs bright
//!   there (its documentation).
//! - Xie and Yuan's models are of a 10 M☉ black hole; "for supermassive black holes, we find that
//!   the results are similar".
//!
//! # The look-back
//!
//! A tail never ends, so its look-back is the whole source horizon: [`TAIL_LOOK_BACK_BINS`] bins
//! of P09.T30.a's 1,024 years reach the horizon's first bin from any time in the clock window. The
//! tails are those of the disruptions inside the horizon, which have victims; a disruption before
//! it has none and adds no tail. The victims are listed once ([`CentreLuminosityModel`]), so a
//! luminosity is the flares' day of look-back and a sum over the horizon's few tens of victims.

use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::CentreModel;
use crate::galaxy::rules::GalaxyRules;
use crate::math;
use crate::time::UniverseTime;
use crate::units::consts::{GM_SUN, SOLAR_MASS_KG, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::{Seconds, SolarMasses, SolarRadii};
use crate::{GENERATOR_VERSION, GeneratorVersion};

use super::disruptions::BuildTidalDisruptionModelError;
use super::flares::{CentreFlareModel, flares_active_at};
use super::victims::{BuildCentreVictimsError, CentreVictims};

/// Sgr A*'s quiescent luminosity, unabsorbed 2–10 keV, W: 3.6 × 10²⁶ W (3.6 × 10³³ erg/s; Nowak
/// et al. 2012, ApJ 759, 95, Table 1, at 8 kpc), the quiescence Neilsen et al. (2013, ApJ 774, 42,
/// §4) measure the flares above ("Lq = 3.6 × 10³³ erg s⁻¹"). Baganoff et al. (2003, ApJ 591, 891,
/// §5.3) give 2.4 (+3.0 −0.6) × 10³³ erg/s, "known only to within a factor of two". Part of the
/// generator version.
pub const QUIESCENT_LUMINOSITY_WATTS: f64 = 3.6e26;

/// η, the share of the infalling rest mass a tail's accretion radiates: 0.1, which simulations
/// "suggest … is typical" (Stone and Metzger 2016, §3). Part of the generator version.
///
/// Stone and Metzger defend it for super-Eddington accretion. Until version 25 a tail radiates
/// at it whatever its rate; from `CENTRE_QUIESCENCE_VERSION` only from 6.2265 × 10⁻³ of
/// Eddington's rate up (`radiative_efficiency`).
pub const RADIATIVE_EFFICIENCY: f64 = 0.1;

/// The generator version from which a tail's radiative efficiency falls with its fallback rate
/// (plan 09, P09.T30.c; the research lane `research-p09-t30b`'s advice, §2, as main ruled it on
/// 2026-10-29): 25, a galaxy-side hold (decision-galaxy-25-train §1).
///
/// It is the hold of one call site, a `match` on [`CentreQuiescenceRules`] read from the galaxy's
/// rules, in the light a tail radiates at a fallback rate, which
/// [`DisruptionTail::bolometric_watts_at`] and [`DisruptionTail::peak_luminosity_watts`] share.
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`CentreQuiescenceRules::ConstantEfficiency`], which is the code of version 23 verbatim, so
/// every generated value keeps its bits; the tests reach the new arm through a galaxy built under
/// `GalaxyRules::every_25_hold()`. The bump to 25 deletes this hold, [`CentreQuiescenceRules`]
/// with its `ConstantEfficiency` arm, the galaxy's `centre_quiescence` field and its tripwire
/// (`the_centre_s_quiescence_is_held_only_until_the_25_bump`).
pub(crate) const CENTRE_QUIESCENCE_VERSION: GeneratorVersion = GeneratorVersion::new(25);

/// The efficiency a tail radiates at (the hold of [`CENTRE_QUIESCENCE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CentreQuiescenceRules {
    /// Version 24's and before: [`RADIATIVE_EFFICIENCY`] at every rate, so a tail's light falls
    /// as its fallback rate, t^(−5⁄3).
    ConstantEfficiency,
    /// Version 25's: [`radiative_efficiency`] of the fallback rate, Xie and Yuan's hot flow below
    /// 6.2265 × 10⁻³ of Eddington's rate and [`RADIATIVE_EFFICIENCY`] above.
    HotFlow,
}

/// Xie and Yuan's (2012, MNRAS 427, 1580, Table 1, equation 11) fit to a hot accretion flow's
/// radiative efficiency, `ε = ε₀ (Ṁ_net ÷ Ṁ_c)^a`, as `(ε₀, a)` for its three ranges of rate, from
/// the lowest: (1.58, 0.65), (0.055, 0.076) and (0.17, 1.12), at their δ = 0.5 (the share of the
/// turbulent heating that goes to the electrons), viscosity α = 0.1 and outflow index s = 0.4.
/// Their table bounds the ranges at `Ṁ_net ÷ Ṁ_Edd` = 2.9 × 10⁻⁵, 3.3 × 10⁻³ and 5.3 × 10⁻³ (their
/// `Ṁ_cr,LHAF`); [`radiative_efficiency`] switches where adjacent ranges meet instead. Yuan and
/// Narayan (2014, ARA&A 52, 529, §2.2.2) put δ "probably … in the range 0.1−0.5"; 0.5 is Sgr A*'s
/// (Yuan et al. 2003), and the brighter end. Part of the generator version.
pub(crate) const HOT_FLOW_EFFICIENCY: [(f64, f64); 3] =
    [(1.58, 0.65), (0.055, 0.076), (0.17, 1.12)];

/// Xie and Yuan's normalising rate `Ṁ_c` in Eddington rates: 10⁻² (their equation 11). Part of the
/// generator version.
pub(crate) const HOT_FLOW_RATE_SCALE: f64 = 1e-2;

/// Xie and Yuan's Eddington accretion rate per watt of Eddington luminosity, kg/J: `10 ÷ c²`, so
/// that `Ṁ_Edd = 10 L_Edd ÷ c²`, `L_Edd ÷ η c²` at η = 0.1 (Xie and Yuan 2012, with their Table 1;
/// Stone and Metzger 2016, equation 24), 6.01 × 10²¹ kg/s (0.0955 M☉/yr) at the Milky Way's
/// 4.30 × 10⁶ M☉.
///
/// Their fit is in rates of this Eddington rate: one of `L_Edd ÷ c²` would move each range by
/// 10^a.
const EDDINGTON_RATE_PER_WATT: f64 = 10.0 / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);

/// The radiative efficiency of a tail's accretion at `eddington_ratio`, its rate in Eddington
/// rates (`Ṁ ÷ Ṁ_Edd`, with Xie and Yuan's `Ṁ_Edd = 10 L_Edd ÷ c²`), from version 25
/// ([`CENTRE_QUIESCENCE_VERSION`]).
///
/// It is `min(η, max(min(ε₁, ε₂), ε₃))` over the three ranges of [`HOT_FLOW_EFFICIENCY`], each
/// `εᵢ = ε₀ (ṁ ÷ 10⁻²)^a`: ε₁ below ṁ = 2.8802 × 10⁻⁵, where it meets ε₂; ε₂ up to 3.3929 × 10⁻³,
/// where it meets ε₃; ε₃ up to 6.2265 × 10⁻³, where it reaches [`RADIATIVE_EFFICIENCY`]; and η
/// above. So it is continuous and non-decreasing in the rate, and a tail's light falls
/// monotonically after its fallback time. Xie and Yuan "adjusted [the bounds] after the fitting so
/// that a continuous fitting function is achieved"; the published bounds, 2.9 × 10⁻⁵ and
/// 3.3 × 10⁻³, are within 1% and 3% of where the ranges meet, and taken as printed would leave a 3%
/// dip at 3.3 × 10⁻³. Their table stops at 5.3 × 10⁻³ (ε = 0.083, `Ṁ_cr,LHAF`); above it their flow
/// has two phases at about 8%, "close to or even slightly larger than that of the standard thin
/// disk", so the top range is extended to η, which moves a tail by at most about 20% over a few
/// years. The switch to η then sits at 0.6% of Eddington's luminosity, a factor 2–6 below where
/// X-ray binaries turn from soft to hard, "at about 1–4% of the Eddington rate" (Maccarone 2003,
/// A&A 409, 697).
///
/// `eddington_ratio` is at least 0, as a fallback rate is; ε is then in [0, η], and 0 at 0.
///
/// At EHT's accretion rate for Sgr A*, 5.2–9.5 × 10⁻⁹ M☉/yr, it gives 1.8–4.8 × 10³⁵ erg/s
/// bolometric, 2–4 times below the 6.8–9.2 × 10³⁵ of their abstract (Event Horizon Telescope
/// Collaboration 2022, ApJL 930, L16, Paper V), an efficiency of 6.0–8.9 × 10⁻⁴ against their
/// (1.3–3.0) × 10⁻³ (§6). Their models are MAD flows about a spinning black hole, where Xie and
/// Yuan's are about a Schwarzschild one, and their rate, ṁ ≈ 10⁻⁷, is some 300–500 times below
/// the top of Xie and Yuan's lowest, open-ended range.
#[must_use]
pub(crate) fn radiative_efficiency(eddington_ratio: f64) -> f64 {
    let x = eddington_ratio / HOT_FLOW_RATE_SCALE;
    let [low, middle, high] = HOT_FLOW_EFFICIENCY.map(|(e0, a)| e0 * math::powf(x, a));
    low.min(middle).max(high).min(RADIATIVE_EFFICIENCY)
}

/// A tail's bolometric luminosity over its unabsorbed 2–10 keV luminosity: 15.8, the median
/// bolometric correction of low-luminosity active nuclei below a tenth of Eddington (Ho 2009, ApJ
/// 699, 626, §3: "sources with Lbol/LEdd ≲ 0.1 have a median CX = 15.8, with an interquartile
/// range of 9.6"). Part of the generator version.
///
/// It is extrapolated below about 10⁻⁶ of Eddington's luminosity, where it errs bright: Sgr A*
/// itself, near 10⁻⁹, has `C_X` of about 190–380 (EHT 2022's bolometric luminosity over Baganoff
/// et al.'s 2003 and Nowak et al.'s 2012 quiescence), its light "predominately at radio to
/// submillimeter" (Genzel et al. 2010, Rev. Mod. Phys. 82, 3121, §7). A share that falls with the
/// rate needs a sourced law (plan 09's Risks, "P09.T30.c as built", a gap).
pub const BOLOMETRIC_CORRECTION: f64 = 15.8;

/// The share of a tail's luminosity in the unabsorbed 2–10 keV band, `1 ÷ 15.8`
/// ([`BOLOMETRIC_CORRECTION`]).
pub const XRAY_SHARE: f64 = 1.0 / BOLOMETRIC_CORRECTION;

/// How many 1,024-year bins before its own a tail's look-back takes, 257: from any time in the
/// clock window back to the source horizon's first bin, bin −257, which +H's bin 0 needs
/// (module documentation). [`luminosity_at`] sums the listed victims, which are those bins'
/// disruptions; plan 12's registry row (P12.T5.a) reads it as the tails' duration, as it reads
/// the flares' [`LOOK_BACK_BINS`](super::flares::LOOK_BACK_BINS).
pub const TAIL_LOOK_BACK_BINS: u32 = 257;

/// The Eddington luminosity of a black hole of `mass`, W: `4π G M m_p c ÷ σ_T` for ionised
/// hydrogen, 1.257 × 10³¹ W (1.257 × 10³⁸ erg/s) per solar mass with the IAU 2015 nominal `GM☉`
/// and CODATA 2018's `m_p` and `σ_T`; 5.41 × 10³⁷ W at the Milky Way's 4.30 × 10⁶ M☉. Stone and
/// Metzger (2016, after equation 24) take `L_Edd ≃ 1.5 × 10⁴⁶ M₈ erg/s`, some 19% higher, as for an
/// opacity of about 0.33 cm² g⁻¹.
#[must_use]
pub fn eddington_luminosity_watts(mass: SolarMasses) -> f64 {
    EDDINGTON_WATTS_PER_SOLAR_MASS * mass.value()
}

/// The proton's mass, kg (CODATA 2018).
const PROTON_MASS_KG: f64 = 1.672_621_923_69e-27;

/// The Thomson cross-section, m² (CODATA 2018).
const THOMSON_CROSS_SECTION_M2: f64 = 6.652_458_732_1e-29;

/// `4π GM☉ m_p c ÷ σ_T`, W per solar mass.
const EDDINGTON_WATTS_PER_SOLAR_MASS: f64 =
    4.0 * core::f64::consts::PI * GM_SUN * PROTON_MASS_KG * SPEED_OF_LIGHT
        / THOMSON_CROSS_SECTION_M2;

/// The time after a disruption at which its most bound debris returns to pericentre, s:
/// `t_fb = (π ÷ √2) R★^{3⁄2} M• ÷ (M★ √(G M•))`, the period of the debris bound by the black
/// hole's tide across the star at its tidal radius, `G M• R★ ÷ r_t²`, frozen in there whatever
/// the penetration (Stone, Sari and Loeb 2013, equation 3): 3.54 × 10⁶ s M₆^½ m★⁻¹ r★^{3⁄2},
/// Stone and Metzger's (2016, §3, equation 23) and Stone, Sari and Loeb's (2013, equation 49 with
/// n = 0, k = 1) 3.5 × 10⁶ s, 41 days at 10⁶ M☉ and 85 days at the Milky Way's black hole for a
/// Sun-like star.
#[must_use]
pub fn fallback_time(
    black_hole_mass: SolarMasses,
    star_mass: SolarMasses,
    star_radius: SolarRadii,
) -> Seconds {
    let r = star_radius.value() * SOLAR_RADIUS_M;
    let gm = GM_SUN * black_hole_mass.value();
    Seconds::new(
        core::f64::consts::PI / core::f64::consts::SQRT_2 * r * r.sqrt() * black_hole_mass.value()
            / (star_mass.value() * gm.sqrt()),
    )
}

/// A disrupted victim's tail: the light of its debris falling back (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisruptionTail {
    disruption: UniverseTime,
    fallback: Seconds,
    /// `M★ ÷ 3 t_fb`, kg/s: the fallback rate at `t_fb`.
    peak_rate_kg_per_s: f64,
    /// The black hole's Eddington luminosity, W.
    eddington_watts: f64,
    /// The efficiency it radiates at, its galaxy's (P09.T30.c's hold).
    rules: CentreQuiescenceRules,
}

impl DisruptionTail {
    /// The tail of a disruption at `disruption` of a star of present mass `star_mass` and radius
    /// `star_radius` by a black hole of `black_hole_mass`, radiating as the generator version's
    /// galaxies' tails do.
    #[must_use]
    pub fn new(
        disruption: UniverseTime,
        black_hole_mass: SolarMasses,
        star_mass: SolarMasses,
        star_radius: SolarRadii,
    ) -> Self {
        Self::new_with_rules(
            disruption,
            black_hole_mass,
            star_mass,
            star_radius,
            GalaxyRules::of_version(GENERATOR_VERSION).centre_quiescence,
        )
    }

    /// [`new`](Self::new), radiating under `rules`, a galaxy's (P09.T30.c's hold).
    #[must_use]
    pub(crate) fn new_with_rules(
        disruption: UniverseTime,
        black_hole_mass: SolarMasses,
        star_mass: SolarMasses,
        star_radius: SolarRadii,
        rules: CentreQuiescenceRules,
    ) -> Self {
        let fallback = fallback_time(black_hole_mass, star_mass, star_radius);
        Self {
            disruption,
            fallback,
            peak_rate_kg_per_s: star_mass.value() * SOLAR_MASS_KG / (3.0 * fallback.value()),
            eddington_watts: eddington_luminosity_watts(black_hole_mass),
            rules,
        }
    }

    /// The instant of the disruption.
    #[must_use]
    pub const fn disruption(&self) -> UniverseTime {
        self.disruption
    }

    /// The fallback time `t_fb` ([`fallback_time`]): the tail starts this long after the
    /// disruption.
    #[must_use]
    pub const fn fallback_time(&self) -> Seconds {
        self.fallback
    }

    /// The rate at which the debris falls back at `t`, kg/s: 0 before `t_fb` after the disruption,
    /// `(M★ ÷ 3 t_fb) (Δt ÷ t_fb)^(−5⁄3)` from it, half the star's mass in all (Stone, Sari and
    /// Loeb 2013, equation 50, for debris spread evenly in energy).
    #[must_use]
    pub fn fallback_rate_kg_per_s_at(&self, t: UniverseTime) -> f64 {
        let Some(since) = t.checked_since(self.disruption) else {
            return 0.0;
        };
        let x = since.as_seconds_f64() / self.fallback.value();
        if x < 1.0 {
            return 0.0;
        }
        self.peak_rate_kg_per_s * math::powf(x, -5.0 / 3.0)
    }

    /// Its luminosity at `t` over every band, W: `ε Ṁ c²`, at most the black hole's Eddington
    /// luminosity, with ε = η = 0.1 until version 25 and `radiative_efficiency` of the rate
    /// from it (module documentation).
    #[must_use]
    pub fn bolometric_watts_at(&self, t: UniverseTime) -> f64 {
        let accreted = self.accreted_watts(self.fallback_rate_kg_per_s_at(t));
        accreted.min(self.eddington_watts)
    }

    /// Its luminosity at `t` in the unabsorbed 2–10 keV band, W: [`XRAY_SHARE`] of the
    /// bolometric.
    #[must_use]
    pub fn luminosity_watts_at(&self, t: UniverseTime) -> f64 {
        XRAY_SHARE * self.bolometric_watts_at(t)
    }

    /// The most it ever gives in the band, W: [`XRAY_SHARE`] of its light at `t_fb`, which is the
    /// Eddington luminosity when the fallback is above Eddington's rate there.
    ///
    /// The light falls monotonically from `t_fb` under either efficiency, since
    /// `radiative_efficiency` does not decrease with the rate. At the Milky Way's black hole every
    /// victim peaks above the hot flow's switch to η (the least about 0.61 Eddington rates), so its
    /// peak is the same under either. At a black hole near 10⁸ M☉ a star of several solar radii
    /// can peak below the switch (a 1 M☉, 10 R☉ star from 7.7 × 10⁷ M☉), where the falling
    /// efficiency lowers it.
    #[must_use]
    pub fn peak_luminosity_watts(&self) -> f64 {
        XRAY_SHARE * self.peak_bolometric_watts()
    }

    /// The most it ever gives over every band, W: its light at `t_fb`, of which
    /// [`peak_luminosity_watts`](Self::peak_luminosity_watts) is the 2–10 keV share.
    #[must_use]
    pub fn peak_bolometric_watts(&self) -> f64 {
        self.eddington_watts
            .min(self.accreted_watts(self.peak_rate_kg_per_s))
    }

    /// The light of accretion at `rate_kg_per_s` before Eddington's cap, W: `ε Ṁ c²` with the
    /// efficiency of the tail's rules (P09.T30.c's hold, its one call site).
    #[must_use]
    fn accreted_watts(&self, rate_kg_per_s: f64) -> f64 {
        match self.rules {
            CentreQuiescenceRules::ConstantEfficiency => {
                RADIATIVE_EFFICIENCY * rate_kg_per_s * SPEED_OF_LIGHT * SPEED_OF_LIGHT
            }
            CentreQuiescenceRules::HotFlow => {
                let eddington_rate_kg_per_s = EDDINGTON_RATE_PER_WATT * self.eddington_watts;
                radiative_efficiency(rate_kg_per_s / eddington_rate_kg_per_s)
                    * rate_kg_per_s
                    * SPEED_OF_LIGHT
                    * SPEED_OF_LIGHT
            }
        }
    }
}

/// The black hole's luminosity at one time, by part, W, unabsorbed 2–10 keV (module
/// documentation).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct CentreLuminosity {
    /// W, as the other two.
    quiescent: f64,
    flares: f64,
    tails: f64,
}

impl CentreLuminosity {
    /// The quiescence, W.
    #[must_use]
    pub const fn quiescent_watts(&self) -> f64 {
        self.quiescent
    }

    /// The flares running, W.
    #[must_use]
    pub const fn flares_watts(&self) -> f64 {
        self.flares
    }

    /// The disruptions' tails, W.
    #[must_use]
    pub const fn tails_watts(&self) -> f64 {
        self.tails
    }

    /// The whole, W: the quiescence, then the flares, then the tails, added in that order.
    #[must_use]
    pub fn total_watts(&self) -> f64 {
        self.quiescent + self.flares + self.tails
    }
}

/// What the black hole's luminosity reads: its flares and its disruptions' victims (module
/// documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct CentreLuminosityModel {
    flares: CentreFlareModel,
    victims: CentreVictims,
}

impl CentreLuminosityModel {
    /// The luminosity of `galaxy`'s black hole, whose centre is `centre`.
    ///
    /// # Errors
    ///
    /// As [`CentreVictims::new`], which no galaxy the builder allows reaches.
    pub fn new(
        galaxy: &Galaxy,
        centre: &CentreModel,
    ) -> Result<Self, BuildTidalDisruptionModelError> {
        Ok(Self {
            flares: CentreFlareModel::of(galaxy),
            victims: CentreVictims::new(galaxy, centre)?,
        })
    }

    /// The luminosity of `galaxy`'s black hole, from the centre the galaxy holds.
    ///
    /// # Errors
    ///
    /// As [`CentreVictims::of`], which no galaxy the builder allows reaches.
    pub fn of(galaxy: &Galaxy) -> Result<Self, BuildCentreVictimsError> {
        Ok(Self {
            flares: CentreFlareModel::of(galaxy),
            victims: CentreVictims::of(galaxy)?,
        })
    }

    /// Its flares.
    #[must_use]
    pub const fn flares(&self) -> &CentreFlareModel {
        &self.flares
    }

    /// Its disruptions' victims.
    #[must_use]
    pub const fn victims(&self) -> &CentreVictims {
        &self.victims
    }
}

/// The black hole's luminosity at `t`, by part, unabsorbed 2–10 keV (module documentation).
///
/// A pure function of the model and `t`: the flares running at `t` in onset order, then the
/// tails of the horizon's disrupted victims up to `t` in list order.
///
/// # Examples
///
/// The model lists the horizon's victims once; each luminosity then reads the flares' day and
/// the tails:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::events::luminosity::{
///     CentreLuminosityModel, QUIESCENT_LUMINOSITY_WATTS, luminosity_at,
/// };
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::from_params(Seed::new(5), GalaxyParams::milky_way_like())?;
/// let model = CentreLuminosityModel::of(&galaxy)?;
/// let now = luminosity_at(&model, UniverseTime::EPOCH);
/// // Never below the quiescence, and in erg/s for a console.
/// assert!(now.total_watts() >= QUIESCENT_LUMINOSITY_WATTS);
/// let erg_per_s = now.total_watts() * 1e7;
/// assert!(erg_per_s.is_finite());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn luminosity_at(model: &CentreLuminosityModel, t: UniverseTime) -> CentreLuminosity {
    let mut running = Vec::new();
    flares_active_at(&model.flares, t, &mut running);
    let flares_watts = running
        .iter()
        .fold(0.0, |sum, f| sum + f.luminosity_watts_at(t));
    let tails_watts = model
        .victims
        .victims()
        .iter()
        .take_while(|v| v.time() <= t)
        .filter_map(|v| v.tail())
        .fold(0.0, |sum, tail| sum + tail.luminosity_watts_at(t));
    CentreLuminosity {
        quiescent: QUIESCENT_LUMINOSITY_WATTS,
        flares: flares_watts,
        tails: tails_watts,
    }
}

#[cfg(test)]
mod centre_quiescence_hold;

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;

    use super::super::disruptions::BIN_SECONDS;
    use super::super::victims::TidalDisruptionVictim;
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::Seed;
    use crate::events::PoissonBins;
    use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
    use crate::galaxy::params::GalaxyParams;
    use crate::time::{ClockWindow, SourceHorizon, Span};
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

    /// Watts in an erg per second.
    pub(super) const WATTS_PER_ERG_S: f64 = 1e-7;

    fn fixture() -> CentreLuminosityModel {
        CentreLuminosityModel::new(milky_way_galaxy(), milky_way_centre()).unwrap()
    }

    pub(super) fn years_after(t: UniverseTime, years: i64) -> UniverseTime {
        t.checked_add(Span::from_julian_years(years).unwrap())
            .unwrap()
    }

    /// The Eddington luminosity is 1.257 × 10³⁸ erg/s per solar mass, and the fallback time of a
    /// Sun-like star 3.54 × 10⁶ s at 10⁶ M☉ (Stone and Metzger's 3.5 × 10⁶ s), scaling as
    /// `M•^½ M★⁻¹ R★^{3⁄2}`.
    #[test]
    fn the_eddington_luminosity_and_the_fallback_time_are_their_closed_forms() {
        let per_sun = eddington_luminosity_watts(SolarMasses::new(1.0)) / WATTS_PER_ERG_S;
        assert!((per_sun / 1.257e38 - 1.0).abs() < 1e-3, "{per_sun:e} erg/s");
        let t = |m_bh: f64, m: f64, r: f64| {
            fallback_time(
                SolarMasses::new(m_bh),
                SolarMasses::new(m),
                SolarRadii::new(r),
            )
            .value()
        };
        let sun = t(1e6, 1.0, 1.0);
        eprintln!(
            "fallback time of a Sun-like star at 1e6 M_sun: {sun:.6e} s ({:.2} d)",
            sun / 86_400.0
        );
        assert!((sun / 3.54e6 - 1.0).abs() < 2e-3, "{sun:e} s");
        assert!((t(4e6, 1.0, 1.0) / sun - 2.0).abs() < 1e-12);
        assert!((t(1e6, 2.0, 1.0) / sun - 0.5).abs() < 1e-12);
        assert!((t(1e6, 1.0, 4.0) / sun - 8.0).abs() < 1e-12);
    }

    /// The held efficiency, η at every rate, and the hot flow's (P09.T30.c's hold).
    const HELD: CentreQuiescenceRules = CentreQuiescenceRules::ConstantEfficiency;
    const HOT_FLOW: CentreQuiescenceRules = CentreQuiescenceRules::HotFlow;

    /// A Sun-like star's tail at the fixture's black hole, disrupted at the epoch, under `rules`.
    fn sun_like_tail(rules: CentreQuiescenceRules) -> DisruptionTail {
        DisruptionTail::new_with_rules(
            UniverseTime::EPOCH,
            milky_way_galaxy().params().black_hole().mass(),
            SolarMasses::new(1.0),
            SolarRadii::new(1.0),
            rules,
        )
    }

    /// The victims of P09.T30.b's 200-seed sweep, universes `0x0930_b000_0000_0000 | i`, about
    /// the fixture's black hole, centre and mass function, with tails under `rules`.
    pub(super) fn swept_victims(
        rules: CentreQuiescenceRules,
    ) -> impl Iterator<Item = (Seed, CentreVictims)> {
        (0..200_u64).map(move |i| {
            let seed = Seed::new(0x0930_b000_0000_0000 | i);
            let victims = CentreVictims::from_parts_with_rules(
                seed,
                milky_way_galaxy().params().black_hole().mass(),
                milky_way_centre(),
                milky_way_galaxy().mass_function(),
                rules,
            )
            .unwrap();
            (seed, victims)
        })
    }

    /// P09.T30.b's closed forms, without Ponti's bracket (P09.T30.c: the light echoes measure
    /// short outbursts, not a tail a thousand years on). A Sun-like star's tail at the Milky Way's
    /// black hole is dark before its fallback time and its share of Eddington's at it; its debris
    /// falls back as t^(−5⁄3) and carries half the star back; under η its light falls as the rate,
    /// and under the hot flow as `Ṁ^(1 + a)`, on the lowest range from about 600 years on. Its
    /// figures are the research lane's.
    #[test]
    fn a_sun_like_tail_follows_its_closed_forms() {
        let bh = milky_way_galaxy().params().black_hole().mass();
        let eddington_rate =
            10.0 * eddington_luminosity_watts(bh) / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        for rules in [HELD, HOT_FLOW] {
            let sun = sun_like_tail(rules);
            let at = |years: i64| sun.luminosity_watts_at(years_after(UniverseTime::EPOCH, years));
            let rate =
                |years: i64| sun.fallback_rate_kg_per_s_at(years_after(UniverseTime::EPOCH, years));
            let erg_s = |years: i64| at(years) / WATTS_PER_ERG_S;
            eprintln!(
                "{rules:?}: a Sun-like victim's tail: {:.4e} erg/s at 1 yr, {:.4e} at 10, {:.4e} at \
                 100, {:.4e} at 1,000, {:.4e} at 10,000, {:.4e} at 100,000; fallback time {:.2} d",
                erg_s(1),
                erg_s(10),
                erg_s(100),
                erg_s(1_000),
                erg_s(10_000),
                erg_s(100_000),
                sun.fallback_time().value() / 86_400.0
            );
            // The rate falls as t^(−5⁄3): ten times later, 10^(−5⁄3) of it.
            assert!((rate(10_000) / rate(1_000) / math::powf(10.0, -5.0 / 3.0) - 1.0).abs() < 1e-9);
            let before = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(0.99 * sun.fallback_time().value()).unwrap())
                .unwrap();
            assert_eq!(sun.luminosity_watts_at(before), 0.0);
            assert_eq!(
                sun.luminosity_watts_at(years_after(UniverseTime::EPOCH, -1)),
                0.0
            );
            let first = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(sun.fallback_time().value()).unwrap())
                .unwrap();
            let peak = sun.luminosity_watts_at(first);
            assert!(peak <= sun.peak_luminosity_watts() * (1.0 + 1e-12));
            assert!((peak / (XRAY_SHARE * eddington_luminosity_watts(bh)) - 1.0).abs() < 1e-12);
            // The debris falling back from t_fb on is half the star: ∫ (x)^(−5⁄3) dx from 1 is 3⁄2.
            let returned = sun.fallback_rate_kg_per_s_at(first) * sun.fallback_time().value() * 1.5
                / SOLAR_MASS_KG;
            assert!((returned - 0.5).abs() < 1e-9, "{returned}");
            // The research lane's figures (its advice's §2 table), to their three digits.
            let figures: [(i64, f64); 6] = match rules {
                CentreQuiescenceRules::ConstantEfficiency => [
                    (1, 3.42e43),
                    (10, 9.73e41),
                    (100, 2.10e40),
                    (1_000, 4.52e38),
                    (10_000, 9.73e36),
                    (100_000, 2.10e35),
                ],
                CentreQuiescenceRules::HotFlow => [
                    (1, 3.42e43),
                    (10, 9.73e41),
                    (100, 9.33e39),
                    (1_000, 9.59e37),
                    (10_000, 1.71e35),
                    (100_000, 3.03e32),
                ],
            };
            for (years, figure) in figures {
                let l = erg_s(years);
                assert!(
                    (l / figure - 1.0).abs() < 5e-3,
                    "{rules:?} at {years} yr: {l:e} erg/s"
                );
            }
            let fall = at(10_000) / at(1_000);
            match rules {
                // η at every rate: the light is the rate's.
                CentreQuiescenceRules::ConstantEfficiency => {
                    assert!((fall / math::powf(10.0, -5.0 / 3.0) - 1.0).abs() < 1e-9);
                }
                // On the lowest range, 1.32 × 10⁻⁵ Eddington rates at 1,000 yr: ε ∝ Ṁ^0.65.
                CentreQuiescenceRules::HotFlow => {
                    let ratio = rate(1_000) / eddington_rate;
                    assert!((ratio / 1.32e-5 - 1.0).abs() < 5e-3, "{ratio:e}");
                    let (e0, a) = HOT_FLOW_EFFICIENCY[0];
                    let expected = XRAY_SHARE
                        * e0
                        * math::powf(ratio / HOT_FLOW_RATE_SCALE, a)
                        * rate(1_000)
                        * SPEED_OF_LIGHT
                        * SPEED_OF_LIGHT;
                    assert!((at(1_000) / expected - 1.0).abs() < 1e-12);
                    assert!((fall / math::powf(10.0, -5.0 / 3.0 * (1.0 + a)) - 1.0).abs() < 1e-9);
                }
            }
        }
    }

    /// The drawn victims' tails a thousand years on, mostly M dwarfs', whose tails scale as
    /// `M★^⅓ R★` under η: the fixture's printed, and the 200 seeds' pinned as they stand under either
    /// efficiency (plan 09's Risks, "P09.T30.b as built" and "P09.T30.c as built").
    #[test]
    fn the_drawn_tails_a_thousand_years_on_are_pinned_as_they_stand() {
        let report = |label: &str, victims: &[&TidalDisruptionVictim]| -> (usize, f64, usize) {
            let mut l: Vec<f64> = victims
                .iter()
                .filter_map(|v| {
                    v.tail().map(|t| {
                        t.luminosity_watts_at(years_after(v.time(), 1_000)) / WATTS_PER_ERG_S
                    })
                })
                .collect();
            l.sort_by(f64::total_cmp);
            let inside = l.iter().filter(|&&x| (0.3e39..=3e39).contains(&x)).count();
            eprintln!(
                "{label}: {} tails at 1,000 yr: least {:.3e}, median {:.3e}, greatest {:.3e} erg/s; \
                 {inside} in the old 0.3-3e39",
                l.len(),
                l[0],
                l[l.len() / 2],
                l[l.len() - 1]
            );
            (l.len(), l[l.len() / 2], inside)
        };
        let model = fixture();
        report(
            "the fixture, under the version's efficiency",
            &model.victims().victims().iter().collect::<Vec<_>>(),
        );
        // As measured: medians of 8.20 × 10³⁷ and 5.74 × 10³⁶ erg/s, and 538 and 35 of 5,524
        // tails in the old bracket (the research lane's figures).
        for (rules, median, bracket) in [(HELD, 8.20e37, 538), (HOT_FLOW, 5.74e36, 35)] {
            let swept: Vec<CentreVictims> = swept_victims(rules).map(|(_, v)| v).collect();
            let (count, m, inside) = report(
                &format!("200 seeds, {rules:?}"),
                &swept
                    .iter()
                    .flat_map(CentreVictims::victims)
                    .collect::<Vec<_>>(),
            );
            assert_eq!(count, 5_524);
            assert!((m / median - 1.0).abs() < 5e-3, "{rules:?}: {m:e} erg/s");
            assert_eq!(inside, bracket, "{rules:?}");
        }
    }

    /// P09.T30.c: the hot flow's efficiency is each of Xie and Yuan's three ranges between where it
    /// meets its neighbours, 2.8802 × 10⁻⁵ and 3.3929 × 10⁻³ Eddington rates, within 1% and 3% of
    /// their published bounds, and η from where the top range reaches it, 6.2265 × 10⁻³. It is
    /// continuous and non-decreasing, where the bounds as printed would leave a 3% dip; and at EHT's
    /// accretion rate for Sgr A* it gives 1.8–4.8 × 10³⁵ erg/s, 2–4 times below their bolometric
    /// luminosity.
    #[test]
    fn the_hot_flow_s_efficiency_is_xie_and_yuan_s_fit_made_continuous() {
        let range = |i: usize, ratio: f64| {
            let (e0, a) = HOT_FLOW_EFFICIENCY[i];
            e0 * math::powf(ratio / HOT_FLOW_RATE_SCALE, a)
        };
        let meet = |i: usize| {
            let ((e1, a1), (e2, a2)) = (HOT_FLOW_EFFICIENCY[i], HOT_FLOW_EFFICIENCY[i + 1]);
            HOT_FLOW_RATE_SCALE * math::powf(e2 / e1, 1.0 / (a1 - a2))
        };
        let (e3, a3) = HOT_FLOW_EFFICIENCY[2];
        let switches = [
            meet(0),
            meet(1),
            HOT_FLOW_RATE_SCALE * math::powf(RADIATIVE_EFFICIENCY / e3, 1.0 / a3),
        ];
        eprintln!("the hot flow's switches: {switches:?} Eddington rates");
        for (switch, figure) in switches.iter().zip([2.8802e-5, 3.3929e-3, 6.2265e-3]) {
            assert!((switch / figure - 1.0).abs() < 1e-4, "{switch:e}");
        }
        assert!((switches[0] / 2.9e-5 - 1.0).abs() < 0.01);
        assert!((switches[1] / 3.3e-3 - 1.0).abs() < 0.03);
        // Taken at the published bound, the third range would start 2.9% below the second.
        let dip = range(2, 3.3e-3) / range(1, 3.3e-3);
        assert!((0.96..0.98).contains(&dip), "{dip}");

        let close = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-15;
        let ranges = [
            (1e-12, switches[0]),
            (switches[0], switches[1]),
            (switches[1], switches[2]),
        ];
        for (i, (low, high)) in ranges.into_iter().enumerate() {
            for ratio in [low * (1.0 + 1e-9), (low * high).sqrt(), high * (1.0 - 1e-9)] {
                assert!(
                    close(radiative_efficiency(ratio), range(i, ratio)),
                    "{ratio:e}"
                );
            }
        }
        for ratio in [switches[2] * (1.0 + 1e-9), 0.1, 1.0, 15.0, 1e3] {
            assert!(
                close(radiative_efficiency(ratio), RADIATIVE_EFFICIENCY),
                "{ratio:e}"
            );
        }
        for switch in switches {
            let below = radiative_efficiency(switch * (1.0 - 1e-12));
            let above = radiative_efficiency(switch * (1.0 + 1e-12));
            assert!((above / below - 1.0).abs() < 1e-10, "{switch:e}");
        }
        assert_same_bits(radiative_efficiency(0.0), 0.0);
        let grid: Vec<f64> = (0..=140_000_u32)
            .map(|k| math::powf(10.0, -12.0 + f64::from(k) * 1e-4))
            .collect();
        assert!(
            grid.windows(2)
                .all(|w| radiative_efficiency(w[1]) >= radiative_efficiency(w[0])),
            "non-decreasing from 1e-12 to 1e2 Eddington rates"
        );

        // EHT 2022 Paper V's favoured models of Sgr A*: 5.2–9.5 × 10⁻⁹ M☉/yr, and a bolometric
        // 6.8–9.2 × 10³⁵ erg/s.
        let bh = GalaxyParams::milky_way_like().black_hole().mass();
        let eddington_rate = EDDINGTON_RATE_PER_WATT * eddington_luminosity_watts(bh);
        eprintln!(
            "Xie and Yuan's Eddington rate at the fixture's black hole: {eddington_rate:.4e} kg/s, \
             {:.4e} M_sun/yr",
            eddington_rate * SECONDS_PER_JULIAN_YEAR / SOLAR_MASS_KG
        );
        assert!((eddington_rate / 6.01e21 - 1.0).abs() < 5e-3);
        for (msun_per_year, figure, eht) in [(5.2e-9, 1.76e35, 6.8e35), (9.5e-9, 4.77e35, 9.2e35)] {
            let rate = msun_per_year * SOLAR_MASS_KG / SECONDS_PER_JULIAN_YEAR;
            let l = radiative_efficiency(rate / eddington_rate)
                * rate
                * SPEED_OF_LIGHT
                * SPEED_OF_LIGHT
                / WATTS_PER_ERG_S;
            eprintln!(
                "at {msun_per_year:e} M_sun/yr: {l:.4e} erg/s bolometric, {:.2}x below EHT",
                eht / l
            );
            assert!((l / figure - 1.0).abs() < 5e-3, "{l:e} erg/s");
            assert!((1.9..4.0).contains(&(eht / l)));
        }
    }

    /// P09.T30.c: under either efficiency a tail's light falls monotonically from its fallback
    /// time, and [`DisruptionTail::peak_luminosity_watts`] is its light there: for a Sun-like star
    /// at the Milky Way's black hole, whose peak is 15 Eddington rates, the same under both; and for
    /// a 10 R☉ star at 10⁸ M☉, whose peak, 4.2 × 10⁻³ Eddington rates, is below the hot flow's
    /// switch to η, where the falling efficiency lowers it. Under the hot flow a Sun-like tail is
    /// η's while its rate is above the switch, for 24.9 years, and falls to the quiescence
    /// 4.07 × 10⁴ years on, where η takes 1.15 × 10⁶.
    #[test]
    fn a_tail_s_light_falls_from_its_peak_under_either_efficiency() {
        let low_peak = |rules| {
            DisruptionTail::new_with_rules(
                UniverseTime::EPOCH,
                SolarMasses::new(1e8),
                SolarMasses::new(1.0),
                SolarRadii::new(10.0),
                rules,
            )
        };
        let after = |seconds: f64| {
            UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(seconds).unwrap())
                .unwrap()
        };
        for rules in [HELD, HOT_FLOW] {
            for tail in [sun_like_tail(rules), low_peak(rules)] {
                // Just after t_fb, past the nanosecond the instant is floored to.
                let first = tail.fallback_time().value() * (1.0 + 1e-12);
                let peak = tail.luminosity_watts_at(after(first));
                let bound = tail.peak_luminosity_watts();
                assert!(
                    peak <= bound && peak / bound > 1.0 - 1e-9,
                    "{rules:?}: {peak:e} W"
                );
                let mut last = peak;
                // From t_fb to 10⁶ t_fb, 2.3 × 10⁵ yr for the Sun-like star.
                for k in 1..=600_u32 {
                    let l = tail
                        .luminosity_watts_at(after(first * math::powf(10.0, f64::from(k) / 100.0)));
                    assert!(l <= last && l > 0.0, "{rules:?} at {k}");
                    last = l;
                }
            }
        }
        let (sun_held, sun_hot) = (sun_like_tail(HELD), sun_like_tail(HOT_FLOW));
        assert_same_bits(
            sun_hot.peak_luminosity_watts(),
            sun_held.peak_luminosity_watts(),
        );
        let (low_held, low_hot) = (low_peak(HELD), low_peak(HOT_FLOW));
        let lowered = low_hot.peak_luminosity_watts() / low_held.peak_luminosity_watts();
        eprintln!("a 10 R_sun star's peak at 1e8 M_sun under the hot flow: {lowered:.4} of eta's");
        assert!((0.6..0.7).contains(&lowered), "{lowered}");

        let years = |y: i64| years_after(UniverseTime::EPOCH, y);
        for y in [1, 10, 24] {
            assert_same_bits(
                sun_hot.luminosity_watts_at(years(y)),
                sun_held.luminosity_watts_at(years(y)),
            );
        }
        assert!(sun_hot.luminosity_watts_at(years(25)) < sun_held.luminosity_watts_at(years(25)));
        let quiet = |tail: &DisruptionTail| {
            let (mut low, mut high) = (1.0_f64, 1e8_f64);
            for _ in 0..100 {
                let mid = (low * high).sqrt();
                let t = UniverseTime::EPOCH
                    .checked_add(Span::from_seconds_f64(mid * SECONDS_PER_JULIAN_YEAR).unwrap())
                    .unwrap();
                if tail.luminosity_watts_at(t) > QUIESCENT_LUMINOSITY_WATTS {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            high
        };
        let (held_quiet, hot_quiet) = (quiet(&sun_held), quiet(&sun_hot));
        eprintln!(
            "a Sun-like tail falls to the quiescence {held_quiet:.4e} yr on under eta, \
             {hot_quiet:.4e} under the hot flow"
        );
        assert!((held_quiet / 1.146e6 - 1.0).abs() < 1e-3);
        assert!((hot_quiet / 4.068e4 - 1.0).abs() < 1e-3);
    }

    /// The luminosity is the quiescence, the flares running and the tails of the victims disrupted
    /// by then, each part as its own model gives it, at instants across the source horizon.
    #[test]
    fn the_luminosity_is_quiescence_flares_and_tails() {
        let model = fixture();
        let victims = model.victims().victims();
        let mut times: Vec<UniverseTime> = vec![
            SourceHorizon::START,
            ClockWindow::START,
            UniverseTime::EPOCH,
            ClockWindow::END,
        ];
        for v in victims.iter().step_by(4) {
            times.extend([
                v.time(),
                years_after(v.time(), 1),
                years_after(v.time(), 1_000),
            ]);
        }
        for t in times {
            let l = luminosity_at(&model, t);
            assert_eq!(l.quiescent_watts(), QUIESCENT_LUMINOSITY_WATTS);
            let mut running = Vec::new();
            flares_active_at(model.flares(), t, &mut running);
            let flares: f64 = running
                .iter()
                .fold(0.0, |s, f| s + f.luminosity_watts_at(t));
            assert_eq!(l.flares_watts(), flares);
            let tails = victims
                .iter()
                .filter(|v| v.time() <= t)
                .filter_map(|v| v.tail())
                .fold(0.0, |s, tail| s + tail.luminosity_watts_at(t));
            assert_eq!(l.tails_watts(), tails);
            assert_eq!(l.total_watts(), QUIESCENT_LUMINOSITY_WATTS + flares + tails);
        }
        assert_eq!(
            luminosity_at(&model, SourceHorizon::START).tails_watts(),
            0.0
        );
    }

    /// The tails' look-back: from any time of the clock window, the disruptions of the bins
    /// [`TAIL_LOOK_BACK_BINS`] back up to it, inside the source horizon, are the victims disrupted
    /// by then; from +H it reaches the horizon's first bin exactly.
    #[test]
    fn the_tails_look_back_over_the_whole_horizon() {
        let model = fixture();
        let series = model.victims().model().series();
        let rate = *model.victims().model();
        let bins = PoissonBins::new(BIN_SECONDS, TAIL_LOOK_BACK_BINS).unwrap();
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let mut events = Vec::new();
            bins.active_at(series, t, &rate, &mut events);
            let looked: Vec<_> = events
                .iter()
                .filter(|e| SourceHorizon::contains(e.time()))
                .map(crate::events::BinEvent::id)
                .collect();
            let held: Vec<_> = model
                .victims()
                .victims()
                .iter()
                .filter(|v| v.time() <= t)
                .map(|v| v.disruption().id())
                .collect();
            assert_eq!(looked, held, "at {t}");
        }
        let first = bins.bin_of(SourceHorizon::START).unwrap().get();
        assert_eq!(
            bins.bin_of(ClockWindow::END).unwrap().get() - first,
            i64::from(TAIL_LOOK_BACK_BINS)
        );
    }

    /// The luminosity at any instants in any order is the same.
    #[test]
    fn the_luminosity_is_order_independent() {
        let model = fixture();
        let times: Vec<UniverseTime> = (-20..=20)
            .map(|k| years_after(UniverseTime::EPOCH, 50 * k))
            .chain(
                model
                    .victims()
                    .victims()
                    .iter()
                    .map(|v| years_after(v.time(), 500)),
            )
            .collect();
        assert_order_independent(&times, |&t| luminosity_at(&model, t));
        assert_eq!(fixture(), model);
        assert_eq!(
            CentreLuminosityModel::of(milky_way_galaxy()).unwrap(),
            model
        );
    }

    /// One entry of the `centre_luminosity` golden: a line of its own, or a labelled value.
    #[derive(Debug, Clone, PartialEq)]
    pub(super) enum Entry {
        Line(String),
        Value(String, f64),
    }

    /// The `centre_luminosity` golden's entries for `galaxy`, the fixture under its rules: the
    /// constants' closed forms, a Sun-like tail at five times, and the luminosity by part at
    /// instants across the clock window and after its victims.
    pub(super) fn centre_luminosity_entries(galaxy: &Galaxy) -> Vec<Entry> {
        let value = |label: &str, value: f64| Entry::Value(label.to_owned(), value);
        let bh = galaxy.params().black_hole().mass();
        let mut entries = vec![
            value(
                "Eddington luminosity per solar mass (W)",
                eddington_luminosity_watts(SolarMasses::new(1.0)),
            ),
            value(
                "fallback time of a Sun-like star at the fixture's black hole (s)",
                fallback_time(bh, SolarMasses::new(1.0), SolarRadii::new(1.0)).value(),
            ),
        ];
        let sun = DisruptionTail::new_with_rules(
            UniverseTime::EPOCH,
            bh,
            SolarMasses::new(1.0),
            SolarRadii::new(1.0),
            galaxy.rules().centre_quiescence,
        );
        for years in [1, 10, 100, 1_000, 100_000] {
            entries.push(value(
                &format!("a Sun-like tail at {years} yr (W)"),
                sun.luminosity_watts_at(years_after(UniverseTime::EPOCH, years)),
            ));
        }
        let model = CentreLuminosityModel::new(galaxy, milky_way_centre()).unwrap();
        let mut times: Vec<UniverseTime> = (-4..=4)
            .map(|k| years_after(UniverseTime::EPOCH, 250 * k))
            .collect();
        times.extend(
            model
                .victims()
                .victims()
                .iter()
                .step_by(5)
                .map(|v| years_after(v.time(), 1_000)),
        );
        for t in times {
            let l = luminosity_at(&model, t);
            entries.extend([
                Entry::Line(format!("at {t}")),
                value("  flares (W)", l.flares_watts()),
                value("  tails (W)", l.tails_watts()),
                value("  total (W)", l.total_watts()),
            ]);
        }
        entries
    }

    /// Pinned: [`centre_luminosity_entries`] of the fixture under the version's rules.
    #[test]
    fn centre_luminosity_is_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for entry in centre_luminosity_entries(milky_way_galaxy()) {
            match entry {
                Entry::Line(line) => w.line(&line),
                Entry::Value(label, value) => w.f64(&label, value),
            }
        }
        golden!("galaxy/features/centre_luminosity", w.as_str());
    }
}
