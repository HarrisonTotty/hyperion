//! Reach and flux: how far a sensor can see a class, and how bright an event is at the observer
//! (plan 12, P12.T5.a and P12.T5.a.2; Design notes 5 and 14).
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
//! traces, so the X-ray depth reads `A_V`: the column is `N_H` = `A_V` ×
//! [`HYDROGEN_COLUMN_PER_MAG`], the solar-equivalent hydrogen column of the dust. That column
//! follows metallicity as plan 07's dust does, and it leaves out the hot corona's ionised gas,
//! which [`Sightline::hydrogen_column`] counts and which absorbs almost nothing (Design note 14).
//!
//! The band is 2–10 keV, the band the centre's luminosities are given in. Its transmission is the
//! share of a source's light in the band that passes, T = ∫ `F_E` e^(−σ(E) `N_H`) dE ÷ ∫ `F_E` dE,
//! and [`xray_optical_depth`] is the band's effective depth, −ln T. Its parts:
//!
//! - **The cross-section.** σ(E) is Morrison and McCammon's (1983, ApJ 270, 119, Table 2)
//!   photoelectric cross-section per hydrogen atom, plus Thomson scattering by 1.2 electrons per
//!   hydrogen atom, since a photon scattered out of a point source's beam is lost to it. Wilms,
//!   Allen and McCray (2000, ApJ 542, 914, §3.3) hold that "Thomson scattering should always be
//!   included in the modeling process when NH ≳ 10²² cm⁻²". σ falls steeply across the band:
//!   σ(5 keV) is 0.082 of σ(2 keV) and σ(10 keV) 0.022, and the iron K edge at 7.111 keV lifts it
//!   1.87 times.
//! - **The source.** A power law of photon index Γ = 2, flat in ν`F_ν`, so that `F_E` ∝ E⁻¹ and the
//!   band's weights are pure geometry in ln E. It is the centre's flares' own spectrum: "We assume
//!   that all flares have the same Γ = 2 power law X-ray spectrum" (Neilsen et al. 2013, ApJ 774,
//!   42, §3). The spectrum matters far less than the energy: at `A_V` = 30 mag, photoelectric
//!   only, T is 0.715 for Γ = 1.5 and 0.595 for Γ = 2.5, against 0.657 for Γ = 2.
//! - **The rule.** A 3-point Gauss–Legendre rule in ln E on each of MM83's six intervals from 2 to
//!   10 keV, 18 fixed nodes, split at the table's breaks, which are absorption edges. It matches a
//!   converged integral to 5 × 10⁻⁵ at every `A_V` from 0.1 to 200 mag; the same 18 nodes in one
//!   rule across the band err by about 4 × 10⁻³ and converge only slowly across the iron edge. The
//!   beam hardens as the column grows (the energy
//!   whose depth is the band's climbs from 3.48 keV at `A_V` = 1 mag to 4.31 keV at 80), so no one
//!   effective energy holds the band across columns.
//!
//! The band's depth is 1 at `A_V` = 88.47 mag (`N_H` = 1.655 × 10²³ cm⁻²); without the scattering
//! it would be at 112.31 mag. It is 0.223 at `A_V` = 12.43 mag, where the 2 keV cross-section alone
//! would reach 1. At low columns it is about ⟨σ⟩ `N_H`, with ⟨σ⟩ = 1.0775 × 10⁻²³ cm², of which
//! 9.977 × 10⁻²⁴ is photoelectric. The electron scattering, 7.4% of ⟨σ⟩, and hydrogen and helium's
//! own absorption (whose share in MM83's figures was not read) do not follow metallicity, so in
//! metal-poor gas the depth reads somewhat low. A console that wants soft and hard bands, or a
//! source's own spectrum, replaces the nodes (plan 12's Risks, "The X-ray band").

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

/// One row of Morrison and McCammon's (1983) Table 2: their fit of the photoelectric cross-section
/// per hydrogen atom on one interval of energy.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "MM83's rows are the stored nodes' source record, which a test recomputes to the bit"
    )
)]
struct Mm83Row {
    /// The interval's lower end, keV.
    from_kev: f64,
    /// The interval's upper end, keV.
    to_kev: f64,
    /// `c₀`, `c₁` and `c₂` of σ(E) = (`c₀` + `c₁`E + `c₂`E²) E⁻³ × 10⁻²⁴ cm², E in keV.
    coefficients: [f64; 3],
}

impl Mm83Row {
    /// σ at `kev`, cm².
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "MM83's rows are the stored nodes' source record, which a test recomputes to the bit"
        )
    )]
    fn cross_section_cm2(&self, kev: f64) -> f64 {
        let [c0, c1, c2] = self.coefficients;
        (c0 + c1 * kev + c2 * kev * kev) / (kev * kev * kev) * 1e-24
    }
}

/// The X-ray band, keV: 2–10, the band the centre's flares and tails are given in (Design note
/// 14).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "MM83's rows are the stored nodes' source record, which a test recomputes to the bit"
    )
)]
const XRAY_BAND_KEV: (f64, f64) = (2.0, 10.0);

/// The rows of Morrison and McCammon's (1983, ApJ 270, 119, Table 2) fit that cover the X-ray band,
/// 1.840–10 keV, broken at the K edges of sulphur, argon, calcium, iron and nickel.
///
/// The fit is for neutral atomic gas of Anders and Ebihara's (1982) abundances, with helium at
/// 0.10 by number (their Table 1), every element in the gas phase; grains, which the fit leaves
/// out (their §III), would lower it slightly above 1 keV. σ(2 keV) is 4.30125 × 10⁻²³ cm². Every
/// row from 1.303 to 10 keV was read at source on 2026-10-30 (plan 12's Risks, "P12.T5.a.2's
/// citations, checked at source"). It is the cross-section with which Predehl and Schmitt (1995,
/// A&A 293, 889) measured `N_H` ÷ `A_V` = 1.79 × 10²¹ cm⁻² mag⁻¹ from X-ray haloes (their
/// abstract; their use of it was not read, but Zhu et al. 2017's Table 3 lists Anders and
/// Ebihara's abundances for them), 4% under plan 07's [`HYDROGEN_COLUMN_PER_MAG`], which the depth
/// uses so that the X-ray band follows the dust's metallicity as the photometric bands do; Güver
/// and Özel (2009, MNRAS 400, 2050) give 2.21 × 10²¹. Wilms, Allen and McCray (2000) take
/// interstellar abundances 20–30% lower in metals (their §2.3), which move σ by up to 30% (their
/// Fig. 2); their calibrations of `N_H` ÷ `A_V` differ too, so a move to them changes the
/// cross-section and the ratio together. Part of the generator version (Design note 14).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "MM83's rows are the stored nodes' source record, which a test recomputes to the bit"
    )
)]
const MM83_ROWS: [Mm83Row; 6] = [
    Mm83Row {
        from_kev: 1.840,
        to_kev: 2.471,
        coefficients: [202.7, 104.7, -17.0],
    },
    Mm83Row {
        from_kev: 2.471,
        to_kev: 3.210,
        coefficients: [342.7, 18.7, 0.0],
    },
    Mm83Row {
        from_kev: 3.210,
        to_kev: 4.038,
        coefficients: [352.2, 18.7, 0.0],
    },
    Mm83Row {
        from_kev: 4.038,
        to_kev: 7.111,
        coefficients: [433.9, -2.4, 0.75],
    },
    Mm83Row {
        from_kev: 7.111,
        to_kev: 8.331,
        coefficients: [629.0, 30.9, 0.0],
    },
    Mm83Row {
        from_kev: 8.331,
        to_kev: 10.0,
        coefficients: [701.2, 25.2, 0.0],
    },
];

/// The Thomson cross-section, cm²: 6.652 458 732 1 × 10⁻²⁵ (CODATA 2018).
const THOMSON_CROSS_SECTION_CM2: f64 = 6.652_458_732_1e-25;

/// The cross-section per hydrogen atom for electron scattering, cm²: Thomson's for 1.2 electrons,
/// hydrogen's one and two for each of MM83's 0.10 helium atoms (the metals' few electrons are left
/// out), 7.983 × 10⁻²⁵, 0.84 of MM83's photoelectric σ at 10 keV. Part of the generator version
/// (Design note 14).
const ELECTRON_SCATTERING_CM2: f64 = 1.2 * THOMSON_CROSS_SECTION_CM2;

/// One node of the X-ray band's rule (module documentation).
#[derive(Debug, Clone, Copy)]
struct BandNode {
    /// The photon energy, keV.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "MM83's rows are the stored nodes' source record, which a test recomputes to the bit"
        )
    )]
    energy_kev: f64,
    /// MM83's photoelectric cross-section per hydrogen atom at it, cm².
    photoelectric_cm2: f64,
    /// Its share of a Γ = 2 source's light in the band: its Gauss weight times its interval's
    /// half-width in ln E, over ln 5, the band's width in ln E.
    weight: f64,
}

/// The X-ray band's 18 nodes, three to each of MM83's intervals between 2 and 10 keV in order:
/// abscissae 0 and ±√(3 ÷ 5) about each interval's middle in ln E, with Gauss weights 8 ÷ 9 and
/// 5 ÷ 9. Stored at full precision, and recomputed from [`MM83_ROWS`], the band and the rule to
/// the bit by a test. Part of the generator version (Design note 14).
const XRAY_BAND_NODES: [BandNode; 18] = [
    BandNode {
        energy_kev: 2.048_239_921_266_574,
        photoelectric_cm2: 4.024_586_311_784_488e-23,
        weight: 0.036_499_241_410_916_226,
    },
    BandNode {
        energy_kev: 2.223_060_952_830_578_7,
        photoelectric_cm2: 3.198_877_977_037_274_5e-23,
        weight: 0.058_398_786_257_465_95,
    },
    BandNode {
        energy_kev: 2.412_803_279_873_583_7,
        photoelectric_cm2: 2.536_965_947_538_362_2e-23,
        weight: 0.036_499_241_410_916_226,
    },
    BandNode {
        energy_kev: 2.544_950_229_167_251_4,
        photoelectric_cm2: 2.367_828_138_384_427_8e-23,
        weight: 0.045_158_624_793_100_93,
    },
    BandNode {
        energy_kev: 2.816_364_678_091_244_6,
        photoelectric_cm2: 1.769_834_568_727_895_4e-23,
        weight: 0.072_253_799_668_961_48,
    },
    BandNode {
        energy_kev: 3.116_724_998_820_684_4,
        photoelectric_cm2: 1.324_433_767_226_261_7e-23,
        weight: 0.045_158_624_793_100_93,
    },
    BandNode {
        energy_kev: 3.294_101_864_674_752_6,
        photoelectric_cm2: 1.157_654_866_912_049_2e-23,
        weight: 0.039_606_405_609_724_79,
    },
    BandNode {
        energy_kev: 3.600_274_989_497_330_5,
        photoelectric_cm2: 8.989_819_519_079_599e-24,
        weight: 0.063_370_248_975_559_66,
    },
    BandNode {
        energy_kev: 3.934_905_638_165_448_6,
        photoelectric_cm2: 6.988_517_891_769_793e-24,
        weight: 0.039_606_405_609_724_79,
    },
    BandNode {
        energy_kev: 4.303_921_759_345_932,
        photoelectric_cm2: 5.487_172_528_050_363e-24,
        weight: 0.097_669_254_008_701_08,
    },
    BandNode {
        energy_kev: 5.358_564_919_826_949,
        photoelectric_cm2: 2.876_348_430_949_380_2e-24,
        weight: 0.156_270_806_413_921_71,
    },
    BandNode {
        energy_kev: 6.671_640_333_063_05,
        photoelectric_cm2: 1.519_636_425_002_907_3e-24,
        weight: 0.097_669_254_008_701_08,
    },
    BandNode {
        energy_kev: 7.239_036_605_696_263,
        photoelectric_cm2: 2.247_744_439_620_473e-24,
        weight: 0.027_328_487_871_464_805,
    },
    BandNode {
        energy_kev: 7.696_865_660_773_872,
        photoelectric_cm2: 1.901_051_102_325_887_7e-24,
        weight: 0.043_725_580_594_343_68,
    },
    BandNode {
        energy_kev: 8.183_649_873_159_059,
        photoelectric_cm2: 1.609_036_152_652_923_4e-24,
        weight: 0.027_328_487_871_464_805,
    },
    BandNode {
        energy_kev: 8.504_224_167_742_93,
        photoelectric_cm2: 1.488_529_031_979_271_1e-24,
        weight: 0.031_515_764_083_870_024,
    },
    BandNode {
        energy_kev: 9.127_431_182_978_048,
        photoelectric_cm2: 1.224_623_304_392_597e-24,
        weight: 0.050_425_222_534_192_02,
    },
    BandNode {
        energy_kev: 9.796_308_088_396_854,
        photoelectric_cm2: 1.008_443_819_266_103_6e-24,
        weight: 0.031_515_764_083_870_024,
    },
];

/// The X-ray band's effective optical depth along `sightline`, −ln T, T the share of a Γ = 2
/// source's 2–10 keV light that passes (module documentation; Design note 14).
///
/// T = Σₖ wₖ e^(−(σₖ + `σ_e`) `N_H`) ÷ Σₖ wₖ over the band's 18 nodes, each with MM83's
/// photoelectric σₖ and the electron scattering `σ_e`, at the column `N_H` = `A_V` ×
/// [`HYDROGEN_COLUMN_PER_MAG`]; dividing by the weights' own sum makes a clear line's depth 0
/// exactly. The depth is 1 at `A_V` = 88.47 mag and 0.223 at 12.43 mag, and about ⟨σ⟩ `N_H` with
/// ⟨σ⟩ = 1.0775 × 10⁻²³ cm² at low columns. It reads the line's dust, never its hydrogen column,
/// so the hot gas of the corona and of a superbubble adds nothing to it: that gas's free electrons
/// would scatter, but at a depth of 8 × 10⁻⁴ per 10²¹ cm⁻² of ionised column, which is left out. It
/// costs 18 exponentials, summed in a fixed order.
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
/// // X-rays of 2–10 keV pass where visual light does not.
/// assert!(xray_optical_depth(&line) < 0.1 * line.a_v().value());
/// let visual = transmission(&line, AlertBand::Photometric(Band::V));
/// assert!(transmission(&line, AlertBand::XRay) > visual);
/// ```
#[must_use]
pub fn xray_optical_depth(sightline: &Sightline) -> f64 {
    xray_depth_of(sightline.a_v())
}

/// The X-ray band's depth at visual extinction `a_v`.
#[must_use]
fn xray_depth_of(a_v: Magnitudes) -> f64 {
    let column = a_v.value() * HYDROGEN_COLUMN_PER_MAG;
    let (weights, passed) = XRAY_BAND_NODES
        .iter()
        .fold((0.0, 0.0), |(weights, passed), node| {
            let depth = (node.photoelectric_cm2 + ELECTRON_SCATTERING_CM2) * column;
            (
                weights + node.weight,
                passed + node.weight * math::exp(-depth),
            )
        });
    math::ln(weights / passed)
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
/// can be generated. A walked class's bounds are constants (a test holds them), so only the
/// centre's victims, whose bound needs the galaxy, reach the diagonal whatever the sensor's limits;
/// a walked class reaches it only when its bound is bright enough for them.
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
        // The luminous blue variables' giant eruptions are built, though their class's process is
        // not: the class reaches as far as its eruptions do.
        assert_eq!(
            class_reach(ClassId::LUMINOUS_BLUE_VARIABLE, &both),
            kind_reach(TransientKind::GiantEruption, &both)
        );
        assert!(class_reach(ClassId::LUMINOUS_BLUE_VARIABLE, &both) > LightYears::ZERO);
        for class in [
            ClassId::STELLAR_MERGER,
            ClassId::NEUTRON_STAR_MERGER,
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

    /// The band's nodes recomputed from MM83's rows, the band's ends and the 3-point rule.
    fn recomputed_nodes() -> Vec<BandNode> {
        let (low, high) = XRAY_BAND_KEV;
        let width = math::ln(high / low);
        let offset = (3.0_f64 / 5.0).sqrt();
        MM83_ROWS
            .iter()
            .flat_map(|row| {
                let from = math::ln(row.from_kev.max(low));
                let to = math::ln(row.to_kev.min(high));
                let (middle, half) = (f64::midpoint(from, to), (to - from) / 2.0);
                [(-offset, 5.0 / 9.0), (0.0, 8.0 / 9.0), (offset, 5.0 / 9.0)].map(|(x, gauss)| {
                    let energy_kev = math::exp(middle + half * x);
                    BandNode {
                        energy_kev,
                        photoelectric_cm2: row.cross_section_cm2(energy_kev),
                        weight: gauss * half / width,
                    }
                })
            })
            .collect()
    }

    /// The stored nodes are the rule's over MM83's rows to the bit, the rows cover the band without
    /// a gap, the weights sum to 1, and the figures the documentation quotes hold: σ(2 keV), the
    /// fall of σ across the band and the iron edge's step.
    #[test]
    fn the_band_nodes_are_the_rule_over_mm83s_rows() {
        let nodes = recomputed_nodes();
        assert_eq!(nodes.len(), XRAY_BAND_NODES.len());
        for (stored, recomputed) in XRAY_BAND_NODES.iter().zip(&nodes) {
            assert_same_bits(stored.energy_kev, recomputed.energy_kev);
            assert_same_bits(stored.photoelectric_cm2, recomputed.photoelectric_cm2);
            assert_same_bits(stored.weight, recomputed.weight);
        }
        for pair in MM83_ROWS.windows(2) {
            assert_same_bits(pair[0].to_kev, pair[1].from_kev);
        }
        let (low, high) = XRAY_BAND_KEV;
        assert!(MM83_ROWS[0].from_kev < low && MM83_ROWS[0].to_kev > low);
        assert_same_bits(MM83_ROWS[5].to_kev, high);
        let weights = XRAY_BAND_NODES
            .iter()
            .fold(0.0, |sum, node| sum + node.weight);
        assert!((weights - 1.0).abs() < 1e-15, "{weights}");
        // The advice's six figures of the first and last nodes.
        let (first, last) = (XRAY_BAND_NODES[0], XRAY_BAND_NODES[17]);
        assert!((first.energy_kev / 2.048_240 - 1.0).abs() < 1e-6);
        assert!((first.photoelectric_cm2 / 4.024_586e-23 - 1.0).abs() < 1e-6);
        assert!((last.energy_kev / 9.796_308 - 1.0).abs() < 1e-6);
        assert!((last.weight / 0.031_515_764 - 1.0).abs() < 1e-8);

        let sigma = |kev: f64| {
            let row = MM83_ROWS
                .iter()
                .find(|row| row.from_kev <= kev && kev < row.to_kev)
                .unwrap_or(&MM83_ROWS[5]);
            row.cross_section_cm2(kev)
        };
        let at_2 = sigma(2.0);
        assert!((at_2 / 4.301_25e-23 - 1.0).abs() < 1e-6, "{at_2:e}");
        assert!((sigma(5.0) / at_2 - 0.082).abs() < 5e-4);
        assert!((sigma(10.0) / at_2 - 0.022).abs() < 5e-4);
        let edge = MM83_ROWS[4].cross_section_cm2(7.111) / MM83_ROWS[3].cross_section_cm2(7.111);
        assert!((edge - 1.87).abs() < 5e-3, "{edge}");
        // The scattering is 0.84 of MM83's σ at 10 keV.
        assert!((ELECTRON_SCATTERING_CM2 / 7.983e-25 - 1.0).abs() < 1e-4);
        let top = MM83_ROWS[5].cross_section_cm2(high);
        assert!(
            (ELECTRON_SCATTERING_CM2 / top - 0.84).abs() < 5e-3,
            "{top:e}"
        );
    }

    /// The band's transmission at `a_v` by a converged rule: Simpson's in ln E, 2,000 steps on each
    /// of MM83's intervals in the band.
    fn converged_transmission(a_v: f64) -> f64 {
        let (low, high) = XRAY_BAND_KEV;
        let column = a_v * HYDROGEN_COLUMN_PER_MAG;
        let steps = 2_000_u32;
        let passed = MM83_ROWS.iter().fold(0.0, |sum, row| {
            let from = math::ln(row.from_kev.max(low));
            let to = math::ln(row.to_kev.min(high));
            let step = (to - from) / f64::from(steps);
            let simpson = (0..=steps).fold(0.0, |total, i| {
                let kev = math::exp(math::mul_add(step, f64::from(i), from));
                let factor = if i == 0 || i == steps {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                let depth = (row.cross_section_cm2(kev) + ELECTRON_SCATTERING_CM2) * column;
                total + factor * math::exp(-depth)
            });
            sum + simpson * step / 3.0
        });
        passed / math::ln(high / low)
    }

    /// The 18-node transmission is the converged integral's to 5 × 10⁻⁵ from 0.1 to 200 mag, and
    /// both are pinned to six figures at 1, 12.43, 30 and 50 mag.
    #[test]
    fn the_band_transmission_matches_a_converged_integral() {
        let band = |a_v| math::exp(-xray_depth_of(Magnitudes::new(a_v)));
        for (a_v, pinned, converged) in [
            (1.0, 0.980_239, 0.980_239),
            (12.43, 0.800_478, 0.800_478),
            (30.0, 0.628_037, 0.628_038),
            (50.0, 0.506_926, 0.506_927),
        ] {
            let (nodes, reference) = (band(a_v), converged_transmission(a_v));
            assert!((nodes - pinned).abs() < 5e-7, "{a_v} mag: {nodes}");
            assert!(
                (reference - converged).abs() < 5e-7,
                "{a_v} mag: {reference}"
            );
        }
        for a_v in [0.1, 1.0, 3.0, 10.0, 12.43, 30.0, 50.0, 80.0, 120.0, 200.0] {
            let (nodes, reference) = (band(a_v), converged_transmission(a_v));
            assert!(
                (nodes - reference).abs() < 5e-5,
                "{a_v} mag: {nodes} against {reference}"
            );
        }
    }

    /// The X-ray band's depth is 1 at the `A_V` its documentation states, 88.47 mag, and at 112.31
    /// mag without the electron scattering; it is 0.223 at 12.43 mag, ⟨σ⟩ `N_H` at low columns, 0
    /// on a clear line, and the line's `A_V` through it.
    #[test]
    fn the_xray_depth_is_unity_at_its_stated_visual_extinction() {
        assert!((xray_depth_of(Magnitudes::new(88.47)) - 1.0).abs() < 1e-4);
        assert!((88.47 * HYDROGEN_COLUMN_PER_MAG / 1.655e23 - 1.0).abs() < 1e-3);
        let photoelectric = |a_v: f64| {
            let column = a_v * HYDROGEN_COLUMN_PER_MAG;
            let passed = XRAY_BAND_NODES.iter().fold(0.0, |sum, node| {
                sum + node.weight * math::exp(-node.photoelectric_cm2 * column)
            });
            -math::ln(passed)
        };
        assert!((photoelectric(112.31) - 1.0).abs() < 1e-4);
        assert!((xray_depth_of(Magnitudes::new(12.43)) - 0.223).abs() < 5e-4);
        let thin = 1e-3;
        let mean = xray_depth_of(Magnitudes::new(thin)) / (thin * HYDROGEN_COLUMN_PER_MAG);
        assert!((mean / 1.0775e-23 - 1.0).abs() < 1e-4, "{mean:e}");
        let absorbed = mean - ELECTRON_SCATTERING_CM2;
        assert!((absorbed / 9.977e-24 - 1.0).abs() < 1e-4, "{absorbed:e}");
        assert!((ELECTRON_SCATTERING_CM2 / mean - 0.074).abs() < 5e-4);
        assert_same_bits(xray_depth_of(Magnitudes::ZERO), 0.0);
        // The depth's bits, so that a rewrite of its arithmetic cannot move them unseen.
        for (a_v, depth) in [
            (1.0, 0.019_958_823_100_035_057),
            (12.43, 0.222_546_314_279_996_58),
            (88.47, 0.999_966_861_511_217_4),
        ] {
            assert_same_bits(xray_depth_of(Magnitudes::new(a_v)), depth);
        }
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
