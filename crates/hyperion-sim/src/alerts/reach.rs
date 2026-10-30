//! Reach and flux: how far a sensor can see a class, and how bright an event is at the observer
//! (plan 12, P12.T5.a; Design notes 5 and 14).
//!
//! # Reach
//!
//! [`class_reach`] is the distance at which the class's brightest event in its best band falls to
//! the sensor's detection limit with no extinction, `d = √(L ÷ 4πF)`, capped at the root cube's
//! diagonal ([`ROOT_CUBE_DIAGONAL`]), beyond which no host lies (Design note 5). It is an upper
//! bound: the walk takes the hosts within it, and each event's flux at the observer decides. The
//! brightest event is the registry's bound ([`PeakBound`]):
//!
//! - a constant bound gives the closed form;
//! - a bound that needs the galaxy (the centre's tails, a share of its black hole's Eddington
//!   luminosity) or the host (a local event's) has no bound here, so it reaches the diagonal: the
//!   centre is one host, and local events are not walked by reach;
//! - a kind whose source is not built, or with no light in the band, reaches nothing.
//!
//! The local kinds ([`Route::Local`]: dwarf novae, X-ray bursts, flares) add nothing to their
//! classes' reach: they come with the range query (P12.T5.e), and dwarf novae and X-ray bursts are
//! excluded from distant scans by reach (P12.T5.c).
//!
//! # Flux
//!
//! An event of luminosity L in a band, seen from a distance d, gives the observer
//! [`flux`] = L ÷ 4πd² × T, with the band's transmission T ([`transmission`]): 10^(−0.4 `A_band`)
//! in a photometric band, from plan 07's [`Sightline::in_band`], and `e^(−τ_X)` in X-rays
//! ([`xray_optical_depth`]). The distance is the one at emission, from the host's apparent
//! position to the observer, and the line of sight is the same segment
//! ([`AlertContext::sightline`]).
//!
//! # X-rays
//!
//! Photoelectric absorption at a few keV is by the metals, in gas and grains, whose column the dust
//! traces, so the X-ray depth reads `A_V`: `τ_X` = [`XRAY_CROSS_SECTION_CM2`] × `A_V` ×
//! [`HYDROGEN_COLUMN_PER_MAG`], the cross-section per hydrogen atom times the solar-equivalent
//! hydrogen column of the dust. That column follows metallicity as plan 07's dust does, and it
//! leaves out the hot corona's ionised gas, which [`Sightline::hydrogen_column`] counts and which
//! absorbs almost nothing (Design note 14). Hydrogen and helium's share of the cross-section does
//! not follow metallicity (MM83's figure of it was not read), so in metal-poor gas the depth reads
//! somewhat low. `τ_X` is 1 at `A_V` = 12.43 mag. The X-ray band is one cross-section at one
//! energy, 2 keV, the softest of the 2–10 keV band the centre's luminosities are given in, so it
//! under-counts the band's flux behind a thick column (plan 12's Risks, "P12.T5.a as built"); a
//! console that wants soft and hard bands replaces the constant by a function of energy (plan 12's
//! Risks, "The X-ray band").

use std::f64::consts::PI;

use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::gas::ccm::HYDROGEN_COLUMN_PER_MAG;
use crate::galaxy::gas::extinction::Sightline;
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Magnitudes, Metres, Watts, WattsPerSquareMetre};

use super::registry::{PeakBound, Route, TransientKind, TransientRow};
use super::{AlertBand, BandLimits, SensorHorizon};

#[cfg(doc)]
use super::AlertContext;

/// The root cube's diagonal: 2 × 65,536 ly × √3 = 227,023.363 ly
/// ([`ROOT_HALF_WIDTH_LY`](crate::coords::ROOT_HALF_WIDTH_LY)), the farthest any two points of the
/// cube lie apart and so the cap on a reach (Design note 5).
pub const ROOT_CUBE_DIAGONAL: LightYears = LightYears::new(227_023.363_449_667_87);

/// Morrison and McCammon's (1983, ApJ 270, 119, Table 2) fit of the photoelectric cross-section
/// per hydrogen atom on 1.840–2.471 keV, σ(E) = (c₀ + c₁E + c₂E²) E⁻³ × 10⁻²⁴ cm², E in keV.
const MM83_C0: f64 = 202.7;
const MM83_C1: f64 = 104.7;
const MM83_C2: f64 = -17.0;

/// The X-ray band's energy, keV: 2 (Design note 14).
const XRAY_ENERGY_KEV: f64 = 2.0;

/// `σ_X`, the photoelectric cross-section per hydrogen atom at 2 keV, cm²: 4.301 × 10⁻²³.
///
/// Morrison and McCammon's (1983, ApJ 270, 119, Table 2) fit on 1.840–2.471 keV, (202.7 + 104.7 E −
/// 17.0 E²) E⁻³ × 10⁻²⁴ cm², which is 344.1 ÷ 8 × 10⁻²⁴ at E = 2 keV: neutral atomic gas of Anders
/// and Ebihara's (1982) abundances, with helium at 0.10 by number (their Table 1), every element in
/// the gas phase; grains, which the fit leaves out (their §III), would lower it slightly above 1
/// keV. It is the cross-section with which Predehl and Schmitt (1995, A&A 293, 889) measured `N_H`
/// ÷ `A_V` = 1.79 × 10²¹ cm⁻² mag⁻¹ from X-ray haloes (their abstract; their use of it was not
/// read, but Zhu et al. 2017's Table 3 lists Anders and Ebihara's abundances for them), 4% under
/// plan 07's [`HYDROGEN_COLUMN_PER_MAG`], which the depth uses so that the X-ray band follows the
/// dust's metallicity as the photometric bands do; Güver and Özel (2009, MNRAS 400, 2050) give 2.21
/// × 10²¹. With it `τ_X` = 1 at `N_H` = 2.325 × 10²² cm⁻², `A_V` = 12.43 mag. Wilms, Allen and
/// McCray (2000, ApJ 542, 914), the other candidate the plan names, take lower interstellar
/// abundances and so a smaller cross-section. Part of the generator version (Design note 14).
pub const XRAY_CROSS_SECTION_CM2: f64 =
    (MM83_C0 + MM83_C1 * XRAY_ENERGY_KEV + MM83_C2 * XRAY_ENERGY_KEV * XRAY_ENERGY_KEV)
        / (XRAY_ENERGY_KEV * XRAY_ENERGY_KEV * XRAY_ENERGY_KEV)
        * 1e-24;

/// The X-ray band's optical depth along `sightline`, `σ_X` × `A_V` × `N_H` ÷ `A_V` (module
/// documentation; Design note 14): 1 at `A_V` = 12.43 mag.
///
/// It reads the line's dust, never its hydrogen column, so the hot gas of the corona and of a
/// superbubble adds nothing to it.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{AlertBand, transmission, xray_optical_depth};
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, sightline};
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let at = |ly| GalacticPosition::from_light_years(ly).expect("in the cube");
/// let mut cache = NoiseCache::with_capacity(4_096);
/// // Ten thousand light-years towards the centre, through the plane.
/// let (sun, star) = (at([0.0, 26_000.0, 20.0]), at([0.0, 16_000.0, 20.0]));
/// let line =
///     sightline(galaxy.gas(), &sun, &star, NoiseMode::Mean, Quality::Full, &[], &mut cache);
/// // X-rays at 2 keV pass where visual light does not.
/// assert!(xray_optical_depth(&line) < 0.1 * line.a_v().value());
/// let visual = transmission(&line, AlertBand::Photometric(Band::V));
/// assert!(transmission(&line, AlertBand::XRay) > visual);
/// ```
#[must_use]
pub fn xray_optical_depth(sightline: &Sightline) -> f64 {
    xray_depth_of(sightline.a_v())
}

/// `τ_X` at visual extinction `a_v`.
#[must_use]
fn xray_depth_of(a_v: Magnitudes) -> f64 {
    XRAY_CROSS_SECTION_CM2 * a_v.value() * HYDROGEN_COLUMN_PER_MAG
}

/// The share of a band's light that `sightline` lets through: 10^(−0.4 `A_band`) in a photometric
/// band ([`Sightline::in_band`]; 1 in radio) and `e^(−τ_X)` in X-rays ([`xray_optical_depth`]).
#[must_use]
pub fn transmission(sightline: &Sightline, band: AlertBand) -> f64 {
    match band {
        AlertBand::Photometric(band) => math::exp10(-0.4 * sightline.in_band(band).value()),
        AlertBand::XRay => math::exp(-xray_optical_depth(sightline)),
    }
}

/// The flux of `luminosity` at `distance` with nothing in the way, L ÷ 4πd²; `None` at no
/// distance, where a point source has no flux.
#[must_use]
pub fn inverse_square_flux(luminosity: Watts, distance: Metres) -> Option<WattsPerSquareMetre> {
    let d = distance.value();
    (d > 0.0).then(|| WattsPerSquareMetre::new(luminosity.value() / (4.0 * PI * d * d)))
}

/// The flux at the observer in `band` of an event of `luminosity` in it, at `distance` through
/// `sightline`: L ÷ 4πd² × [`transmission`] (module documentation); `None` at no distance.
///
/// `distance` and `sightline` are the event's at emission, from the host's apparent position to
/// the observer ([`AlertContext::sightline`]). Where a source's luminosity is not built (plan 11's
/// X-ray luminosities, P11.T9) the caller passes it in.
///
/// # Examples
///
/// A nova at the Eddington luminosity of a solar-mass white dwarf, 1.26 × 10³¹ W, 1,000 ly away
/// through one magnitude of visual extinction:
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, flux, inverse_square_flux};
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::extinction::Sightline;
/// use hyperion_sim::units::{LightYears, Metres, Watts};
///
/// let nova = Watts::new(1.257e31);
/// let d = Metres::from(LightYears::new(1_000.0));
/// let bare = inverse_square_flux(nova, d).ok_or("a distance")?;
/// assert!((bare.value() / 1.1176e-8 - 1.0).abs() < 1e-3); // W m⁻²
/// // Through nothing the flux is the bare one.
/// let v = AlertBand::Photometric(Band::V);
/// let clear = flux(nova, d, &Sightline::ZERO, v).ok_or("a distance")?;
/// assert_eq!(clear, bare);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn flux(
    luminosity: Watts,
    distance: Metres,
    sightline: &Sightline,
    band: AlertBand,
) -> Option<WattsPerSquareMetre> {
    inverse_square_flux(luminosity, distance).map(|bare| bare * transmission(sightline, band))
}

/// The distance at which `luminosity` falls to the detection limit F of `limits` with nothing in
/// the way, `√(L ÷ 4πF)`, capped at [`ROOT_CUBE_DIAGONAL`]: no distance for no luminosity.
#[must_use]
pub fn reach(luminosity: Watts, limits: BandLimits) -> LightYears {
    let area = luminosity.value() / (4.0 * PI * limits.detect().value());
    if area.is_nan() || area <= 0.0 {
        return LightYears::ZERO;
    }
    let ly = area.sqrt() / METRES_PER_LIGHT_YEAR;
    if ly < ROOT_CUBE_DIAGONAL.value() {
        LightYears::new(ly)
    } else {
        ROOT_CUBE_DIAGONAL
    }
}

/// How far `sensor` can see an event of `kind`: the largest [`reach`] over the sensor's bands of
/// the kind's bound in each against the band's detection limit (module documentation).
///
/// A bound that needs the galaxy or the host reaches the diagonal; a band with no bound reaches
/// nothing.
#[must_use]
pub fn kind_reach(kind: TransientKind, sensor: &SensorHorizon) -> LightYears {
    row_reach(kind.row(), sensor)
}

/// How far `sensor` can see the class's brightest event in its best band, with no extinction
/// (Design note 5): the largest [`kind_reach`] of the kinds whose hosts are the class's and which
/// are not local.
///
/// It is zero for a class none of whose kinds is built, which is true while no event of the class
/// can be generated.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, BandLimits, SensorHorizon, class_reach};
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let w = WattsPerSquareMetre::new;
/// let visual = BandLimits::new(w(1e-6), w(1e-5))?;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), visual)])?;
/// // A Type Ia's bolometric peak falls to a microwatt per square metre at about 31,000 ly; a core
/// // collapse's shock breakout reaches the whole cube.
/// let ia = class_reach(ClassId::TYPE_IA, &sensor).value();
/// assert!((30_000.0..32_000.0).contains(&ia));
/// assert!(class_reach(ClassId::CORE_COLLAPSE, &sensor).value() > 227_000.0);
/// // Plan 11's classes have no light curve yet.
/// assert_eq!(class_reach(ClassId::ACCRETING_WHITE_DWARF, &sensor).value(), 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn class_reach(class: ClassId, sensor: &SensorHorizon) -> LightYears {
    class_reach_over(
        TransientKind::ALL.into_iter().map(TransientKind::row),
        class,
        sensor,
    )
}

/// [`class_reach`] over `rows`.
#[must_use]
fn class_reach_over<'r>(
    rows: impl IntoIterator<Item = &'r TransientRow>,
    class: ClassId,
    sensor: &SensorHorizon,
) -> LightYears {
    rows.into_iter()
        .filter(|row| row.classes().contains(class) && row.route() != Route::Local)
        .map(|row| row_reach(row, sensor))
        .fold(LightYears::ZERO, farther)
}

/// [`kind_reach`] of `row`.
#[must_use]
fn row_reach(row: &TransientRow, sensor: &SensorHorizon) -> LightYears {
    sensor
        .bands()
        .map(|(band, limits)| match row.peak_bound(band) {
            None => LightYears::ZERO,
            Some(PeakBound::Watts(luminosity)) => reach(luminosity, limits),
            Some(PeakBound::EddingtonShare(_) | PeakBound::PerHost) => ROOT_CUBE_DIAGONAL,
        })
        .fold(LightYears::ZERO, farther)
}

/// The longer of two reaches.
#[must_use]
fn farther(a: LightYears, b: LightYears) -> LightYears {
    if b > a { b } else { a }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::alerts::{AlertContext, BandLimits, CORE_COLLAPSE_PEAK, TYPE_IA_PEAK};
    use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
    use crate::galaxy::Galaxy;
    use crate::galaxy::features::centre::events::flares as centre_flares;
    use crate::galaxy::features::centre::events::luminosity::eddington_luminosity_watts;
    use crate::galaxy::gas::ccm::Band;
    use crate::galaxy::gas::extinction::{NoiseMode, SIGHTLINE_QUALITY, sightline};
    use crate::galaxy::gas::modifiers::{GasModifier, NoModifiers};
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::units::{HydrogenPerCm3, SolarMasses};

    const V: AlertBand = AlertBand::Photometric(Band::V);

    fn w(value: f64) -> WattsPerSquareMetre {
        WattsPerSquareMetre::new(value)
    }

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("inside the root cube")
    }

    fn sensor(bands: &[(AlertBand, f64)]) -> SensorHorizon {
        SensorHorizon::new(
            bands
                .iter()
                .map(|&(band, detect)| (band, BandLimits::new(w(detect), w(detect)).unwrap())),
        )
        .unwrap()
    }

    #[test]
    fn the_diagonal_is_the_root_cubes() {
        let edge = f64::from(2 * ROOT_HALF_WIDTH_LY);
        assert_same_bits(ROOT_CUBE_DIAGONAL.value(), edge * 3.0_f64.sqrt());
    }

    /// Reach grows monotonically as the limit falls, follows `√(L ÷ 4πF)` below the cap and caps at
    /// the diagonal: `class_reach` for both supernova classes in V, and `kind_reach` for the
    /// centre's flares, which have no class, in X-rays.
    #[test]
    fn reach_grows_as_the_limit_falls_and_caps_at_the_diagonal() {
        /// What a case's reach is of.
        #[derive(Debug, Clone, Copy)]
        enum Of {
            Class(ClassId),
            Kind(TransientKind),
        }
        let flares = Watts::new(centre_flares::MAX_PEAK_LUMINOSITY_WATTS);
        let cases = [
            (Of::Class(ClassId::CORE_COLLAPSE), V, CORE_COLLAPSE_PEAK),
            (Of::Class(ClassId::TYPE_IA), V, TYPE_IA_PEAK),
            (
                Of::Kind(TransientKind::CentralFlare),
                AlertBand::XRay,
                flares,
            ),
        ];
        for (of, band, peak) in cases {
            let mut last = LightYears::ZERO;
            let mut capped = false;
            // Limits from 1 W m⁻² down to 10⁻³⁰, half a decade a step.
            for step in 0..=60 {
                let limit = math::exp10(-f64::from(step) / 2.0);
                let sensor = sensor(&[(band, limit)]);
                let reached = match of {
                    Of::Class(class) => class_reach(class, &sensor),
                    Of::Kind(kind) => kind_reach(kind, &sensor),
                };
                assert!(
                    reached >= last,
                    "{of:?} at {limit:e}: {reached:?} < {last:?}"
                );
                let closed = (peak.value() / (4.0 * PI * limit)).sqrt() / METRES_PER_LIGHT_YEAR;
                if closed < ROOT_CUBE_DIAGONAL.value() {
                    assert!((reached.value() / closed - 1.0).abs() < 1e-14, "{of:?}");
                } else {
                    assert_same_bits(reached.value(), ROOT_CUBE_DIAGONAL.value());
                    capped = true;
                }
                last = reached;
            }
            assert!(capped, "{of:?} reaches the diagonal at the faintest limit");
        }
    }

    /// A sensor's best band sets the reach; a band with no bound reaches nothing, a bound that
    /// needs the galaxy reaches the diagonal, and an unbuilt class reaches nothing.
    #[test]
    fn the_best_band_sets_the_reach() {
        let both = sensor(&[(V, 1e-9), (AlertBand::XRay, 1e-15)]);
        assert_eq!(
            kind_reach(TransientKind::CentralFlare, &both),
            kind_reach(
                TransientKind::CentralFlare,
                &sensor(&[(AlertBand::XRay, 1e-15)])
            )
        );
        assert_eq!(
            kind_reach(TransientKind::TypeIa, &sensor(&[(AlertBand::XRay, 1e-15)])),
            LightYears::ZERO
        );
        assert_eq!(
            class_reach(ClassId::TIDAL_DISRUPTION_VICTIM, &both),
            ROOT_CUBE_DIAGONAL
        );
        for class in [
            ClassId::STELLAR_MERGER,
            ClassId::NEUTRON_STAR_MERGER,
            ClassId::LUMINOUS_BLUE_VARIABLE,
            ClassId::XRAY_BINARY,
            ClassId::ACCRETING_WHITE_DWARF,
            ClassId::ACCRETING_WHITE_DWARF_SLOW,
        ] {
            assert_eq!(class_reach(class, &both), LightYears::ZERO, "{class}");
        }
    }

    /// A local kind adds nothing to its class's reach, however bright: the class's distant kinds
    /// alone set it.
    #[test]
    fn a_local_kind_adds_nothing_to_its_classs_reach() {
        let row = |kind, route, watts| {
            let hosts = crate::alerts::ClassSet::of(ClassId::ACCRETING_WHITE_DWARF);
            TransientRow::for_test(kind, hosts, route, Watts::new(watts))
        };
        let rows = [
            row(TransientKind::Nova, Route::Recurrent, 1e31),
            row(TransientKind::DwarfNova, Route::Local, 1e35),
        ];
        let limits = BandLimits::new(w(1e-12), w(1e-12)).unwrap();
        let visual = sensor(&[(V, 1e-12)]);
        let reached = class_reach_over(&rows, ClassId::ACCRETING_WHITE_DWARF, &visual);
        assert_eq!(reached, reach(Watts::new(1e31), limits));
        assert_eq!(reach(Watts::ZERO, limits), LightYears::ZERO);
        assert_eq!(
            class_reach_over(&rows, ClassId::XRAY_BINARY, &visual),
            LightYears::ZERO
        );
    }

    /// A pinned nova, at the Eddington luminosity of a solar-mass white dwarf, seen from 100, 1,000
    /// and 10,000 ly coreward of it in the plane: the inverse square of the distance times the
    /// transmission of each line, which falls with distance, and X-rays pass more of the dust than
    /// visual light.
    #[test]
    fn a_pinned_novas_flux_at_three_distances() {
        // Classical novae reach their white dwarf's Eddington luminosity, some exceed it
        // ("Eddington novae", "super-Eddington novae": Della Valle and Izzo 2020, A&A Rev. 28, 3):
        // a test value, since plan 11's nova light curves (P11.T9) are not built.
        let nova = Watts::new(eddington_luminosity_watts(SolarMasses::new(1.0)));
        let galaxy = Galaxy::new(Seed::new(42));
        let host = at([0.0, 26_000.0, 20.0]);
        let mut noise = NoiseCache::with_capacity(4_096);
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let mut last = [f64::INFINITY; 2];
        for (ly, bare) in [
            (100.0, 1.1176e-6),
            (1_000.0, 1.1176e-8),
            (10_000.0, 1.1176e-10),
        ] {
            let observer = at([0.0, 26_000.0 - ly, 20.0]);
            let distance = host.distance_to(&observer);
            let line = context.sightline(&galaxy, &host, &observer);
            let unextinguished = inverse_square_flux(nova, distance).unwrap();
            assert!(
                (unextinguished.value() / bare - 1.0).abs() < 1e-3,
                "{unextinguished:?}"
            );
            let visual = flux(nova, distance, &line, V).unwrap();
            let expected =
                unextinguished.value() * math::exp10(-0.4 * line.in_band(Band::V).value());
            assert!((visual.value() / expected - 1.0).abs() < 1e-12, "{ly} ly");
            let xray = flux(nova, distance, &line, AlertBand::XRay).unwrap();
            assert!(xray > visual, "{ly} ly: {xray:?} against {visual:?}");
            assert!(
                visual.value() < last[0] && xray.value() < last[1],
                "{ly} ly"
            );
            assert!(line.a_v().value() > 0.0);
            last = [visual.value(), xray.value()];
        }
        assert_eq!(inverse_square_flux(nova, Metres::ZERO), None);
        assert_eq!(flux(nova, Metres::ZERO, &Sightline::ZERO, V), None);
    }

    /// The X-ray depth is 1 at the `A_V` the documentation states from `σ_X`, 12.43 mag, and is the
    /// line's `A_V` through it.
    #[test]
    fn the_xray_depth_is_unity_at_its_stated_visual_extinction() {
        assert!((xray_depth_of(Magnitudes::new(12.43)) - 1.0).abs() < 5e-4);
        assert!((XRAY_CROSS_SECTION_CM2 / 4.301e-23 - 1.0).abs() < 1e-4);
        let galaxy = Galaxy::new(Seed::new(42));
        let mut cache = NoiseCache::with_capacity(4_096);
        let line = sightline(
            galaxy.gas(),
            &at([0.0, 26_000.0, 20.0]),
            &at([0.0, 16_000.0, 20.0]),
            NoiseMode::Realised,
            SIGHTLINE_QUALITY,
            &[],
            &mut cache,
        );
        assert_same_bits(xray_optical_depth(&line), xray_depth_of(line.a_v()));
        assert_same_bits(transmission(&Sightline::ZERO, AlertBand::XRay), 1.0);
        assert_same_bits(
            transmission(&line, AlertBand::Photometric(Band::Radio)),
            1.0,
        );
    }

    /// Hot gas adds hydrogen to a line but no dust, so it leaves the X-ray depth as it was: the
    /// same superbubble along a line, empty and then filled with coronal gas.
    #[test]
    fn hot_coronal_gas_leaves_the_xray_depth_unchanged() {
        let galaxy = Galaxy::new(Seed::new(42));
        let (a, b) = (at([0.0, 26_000.0, 20.0]), at([0.0, 22_000.0, 20.0]));
        let bubble = |interior| GasModifier::Hole {
            centre: at([0.0, 24_000.0, 20.0]),
            radius: LightYears::new(500.0),
            interior: HydrogenPerCm3::new(interior),
        };
        let mut cache = NoiseCache::with_capacity(4_096);
        let mut line = |interior| {
            sightline(
                galaxy.gas(),
                &a,
                &b,
                NoiseMode::Realised,
                SIGHTLINE_QUALITY,
                &[bubble(interior)],
                &mut cache,
            )
        };
        let (empty, hot) = (line(0.0), line(0.005));
        // The hot gas is there: a thousand light-years of it at 0.005 cm⁻³.
        let added = hot.hydrogen_column().value() - empty.hydrogen_column().value();
        assert!((added / 4.730e18 - 1.0).abs() < 1e-3, "{added:e} cm⁻²");
        assert_same_bits(hot.a_v().value(), empty.a_v().value());
        assert_same_bits(xray_optical_depth(&hot), xray_optical_depth(&empty));
        assert_same_bits(
            transmission(&hot, AlertBand::XRay),
            transmission(&empty, AlertBand::XRay),
        );
        assert!(xray_optical_depth(&hot) > 0.0);
    }
}
