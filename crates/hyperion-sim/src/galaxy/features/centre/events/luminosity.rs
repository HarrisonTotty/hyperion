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
//!   fallback time `t_fb` on at `Ṁ = (M★ ÷ 3 t_fb) (Δt ÷ t_fb)^(−5⁄3)` (Stone, Sari and Loeb 2013,
//!   MNRAS 435, 1809, equations 49–50; Stone and Metzger 2016, MNRAS 455, 859, §3, equation 23),
//!   and radiates `η Ṁ c²` with η = 0.1, at most the black hole's Eddington luminosity, of which
//!   [`XRAY_SHARE`], one part in 15.8, is in the band.
//!
//! A Sun-like star disrupted by the Milky Way's black hole gives 4.5 × 10³⁸ erg/s a thousand
//! years on, in the plan's 0.3–3 × 10³⁹ erg/s (Ponti et al. 2010, ApJ 714, 732: about 1.5 × 10³⁹
//! erg/s from Sgr A* to light the echoes, fading about a century ago; Clavel et al. 2013, A&A 558,
//! A32, §4.1: "at least of the order of 10³⁹ erg s⁻¹" in 2–10 keV). A tail falls as t^(−5⁄3)
//! for ever, so the horizon's tails together keep the centre far above its quiescence: a
//! Sun-like tail still gives 2.1 × 10³⁵ erg/s 10⁵ years on, while η = 0.1 is the efficiency of
//! super-Eddington accretion (Stone and Metzger 2016, §3) and a tail is under a tenth of
//! Eddington's from about five years on (plan 09's Risks, "P09.T30.b as built", a finding for
//! the owner's ruling).
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
use crate::math;
use crate::time::UniverseTime;
use crate::units::consts::{GM_SUN, SOLAR_MASS_KG, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::{Seconds, SolarMasses, SolarRadii};

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
pub const RADIATIVE_EFFICIENCY: f64 = 0.1;

/// A tail's bolometric luminosity over its unabsorbed 2–10 keV luminosity: 15.8, the median
/// bolometric correction of low-luminosity active nuclei (Ho 2009, ApJ 699, 626: "median CX =
/// Lbol/LX = 15.8"). Part of the generator version.
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
/// Metzger (2016, equation 24) take a solar-composition opacity, 0.34 cm² g⁻¹, some 17% higher.
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
}

impl DisruptionTail {
    /// The tail of a disruption at `disruption` of a star of present mass `star_mass` and radius
    /// `star_radius` by a black hole of `black_hole_mass`.
    #[must_use]
    pub fn new(
        disruption: UniverseTime,
        black_hole_mass: SolarMasses,
        star_mass: SolarMasses,
        star_radius: SolarRadii,
    ) -> Self {
        let fallback = fallback_time(black_hole_mass, star_mass, star_radius);
        Self {
            disruption,
            fallback,
            peak_rate_kg_per_s: star_mass.value() * SOLAR_MASS_KG / (3.0 * fallback.value()),
            eddington_watts: eddington_luminosity_watts(black_hole_mass),
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

    /// Its luminosity at `t` over every band, W: `η Ṁ c²`, at most the black hole's Eddington
    /// luminosity.
    #[must_use]
    pub fn bolometric_watts_at(&self, t: UniverseTime) -> f64 {
        let accreted = RADIATIVE_EFFICIENCY
            * self.fallback_rate_kg_per_s_at(t)
            * SPEED_OF_LIGHT
            * SPEED_OF_LIGHT;
        accreted.min(self.eddington_watts)
    }

    /// Its luminosity at `t` in the unabsorbed 2–10 keV band, W: [`XRAY_SHARE`] of the
    /// bolometric.
    #[must_use]
    pub fn luminosity_watts_at(&self, t: UniverseTime) -> f64 {
        XRAY_SHARE * self.bolometric_watts_at(t)
    }

    /// The most it ever gives in the band, W: [`XRAY_SHARE`] of the Eddington luminosity, at
    /// `t_fb` when the fallback is above Eddington's rate there.
    #[must_use]
    pub fn peak_luminosity_watts(&self) -> f64 {
        XRAY_SHARE
            * self.eddington_watts.min(
                RADIATIVE_EFFICIENCY * self.peak_rate_kg_per_s * SPEED_OF_LIGHT * SPEED_OF_LIGHT,
            )
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
mod tests {
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
    use crate::time::{ClockWindow, SourceHorizon, Span};

    /// Watts in an erg per second.
    const WATTS_PER_ERG_S: f64 = 1e-7;

    fn fixture() -> CentreLuminosityModel {
        CentreLuminosityModel::new(milky_way_galaxy(), milky_way_centre()).unwrap()
    }

    fn years_after(t: UniverseTime, years: i64) -> UniverseTime {
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

    /// P09.T30.b: a thousand years after a Sun-like star's disruption by the Milky Way's black hole
    /// its tail gives 0.3–3 × 10³⁹ erg/s (Ponti et al. 2010) in 2–10 keV; its debris carries half
    /// the star back; it is dark before its fallback time and at most its share of Eddington's at
    /// it. The fixture's own victims, and those of 200 seeds, are printed for the plan's records.
    #[test]
    fn a_thousand_years_after_a_disruption_its_tail_gives_ponti_s_luminosity() {
        let bh = milky_way_galaxy().params().black_hole().mass();
        let sun = DisruptionTail::new(
            UniverseTime::EPOCH,
            bh,
            SolarMasses::new(1.0),
            SolarRadii::new(1.0),
        );
        let at = |years: i64| sun.luminosity_watts_at(years_after(UniverseTime::EPOCH, years));
        let thousand = at(1_000) / WATTS_PER_ERG_S;
        eprintln!(
            "a Sun-like victim's tail: {thousand:.4e} erg/s at 1,000 yr, {:.4e} at 100 yr, {:.4e} \
             at 10,000 yr; fallback time {:.2} d",
            at(100) / WATTS_PER_ERG_S,
            at(10_000) / WATTS_PER_ERG_S,
            sun.fallback_time().value() / 86_400.0
        );
        assert!((0.3e39..=3e39).contains(&thousand), "{thousand:e} erg/s");
        // t^(−5⁄3): ten times later, 10^(−5⁄3) as bright.
        assert!((at(10_000) / at(1_000) / math::powf(10.0, -5.0 / 3.0) - 1.0).abs() < 1e-9);
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
        let rate = sun.fallback_rate_kg_per_s_at(first);
        let returned = rate * sun.fallback_time().value() * 1.5 / SOLAR_MASS_KG;
        assert!((returned - 0.5).abs() < 1e-9, "{returned}");

        // The drawn victims, mostly M dwarfs, fall below the bracket, as `M★^⅓ R★`: pinned as they
        // stand (plan 09's Risks, "P09.T30.b as built", a finding).
        let report = |label: &str, victims: &[&TidalDisruptionVictim]| -> (f64, f64) {
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
                 {inside} in 0.3-3e39",
                l.len(),
                l[0],
                l[l.len() / 2],
                l[l.len() - 1]
            );
            #[expect(
                clippy::cast_precision_loss,
                reason = "counts of a few thousand tails, exact in f64"
            )]
            let share = inside as f64 / l.len() as f64;
            (l[l.len() / 2], share)
        };
        let model = fixture();
        report(
            "the fixture",
            &model.victims().victims().iter().collect::<Vec<_>>(),
        );
        let swept: Vec<CentreVictims> = (0..200_u64)
            .map(|i| {
                CentreVictims::from_parts(
                    Seed::new(0x0930_b000_0000_0000 | i),
                    bh,
                    milky_way_centre(),
                    milky_way_galaxy().mass_function(),
                )
                .unwrap()
            })
            .collect();
        let (median, share) = report(
            "200 seeds",
            &swept
                .iter()
                .flat_map(CentreVictims::victims)
                .collect::<Vec<_>>(),
        );
        // As measured: a median of 8.20 × 10³⁷ erg/s, and 538 of 5,524 tails (9.7%) in the
        // bracket.
        assert!((7e37..1e38).contains(&median), "{median:e} erg/s");
        assert!((0.08..0.12).contains(&share), "{share}");
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

    /// Pinned: the constants' closed forms, a Sun-like tail at five times, and the fixture's
    /// luminosity by part at instants across the clock window and after its victims.
    #[test]
    fn centre_luminosity_is_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let bh = milky_way_galaxy().params().black_hole().mass();
        w.f64(
            "Eddington luminosity per solar mass (W)",
            eddington_luminosity_watts(SolarMasses::new(1.0)),
        );
        w.f64(
            "fallback time of a Sun-like star at the fixture's black hole (s)",
            fallback_time(bh, SolarMasses::new(1.0), SolarRadii::new(1.0)).value(),
        );
        let sun = DisruptionTail::new(
            UniverseTime::EPOCH,
            bh,
            SolarMasses::new(1.0),
            SolarRadii::new(1.0),
        );
        for years in [1, 10, 100, 1_000, 100_000] {
            w.f64(
                &format!("a Sun-like tail at {years} yr (W)"),
                sun.luminosity_watts_at(years_after(UniverseTime::EPOCH, years)),
            );
        }
        let model = fixture();
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
            w.line(&format!("at {t}"));
            w.f64("  flares (W)", l.flares_watts());
            w.f64("  tails (W)", l.tails_watts());
            w.f64("  total (W)", l.total_watts());
        }
        golden!("galaxy/features/centre_luminosity", w.as_str());
    }
}
