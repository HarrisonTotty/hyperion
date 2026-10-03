//! The colour and photopic flux of a star from its effective temperature and gravity (rendering
//! plan R06, Design note 6), read from the fitted table [`tables::star_colour`].
//!
//! The table holds, for model spectra on two grids (stars that are not white dwarfs, and white
//! dwarfs), the linear Rec. 709 chroma of the spectrum at unit luminance, the photopic illuminance
//! of a star of V = 0 with that spectrum relative to Allen's 2.54 µlx, and its scotopic-to-photopic
//! ratio ρ. [`star_colour`] interpolates it bilinearly in log₁₀ `T_eff` and log₁₀ g within the grid
//! the star's kind selects, clamping at the grid's edges: a star hotter or cooler, or of higher or
//! lower gravity, than any node takes the edge's row. The gravity is the star's own, from its mass
//! and radius ([`surface_gravity`]), not a luminosity class.
//!
//! The model sets and their ranges are the table's header's; in brief, PHOENIX-ACES below
//! 3,500 K, ATLAS9 to 27,000 K, TLUSTY OSTAR2002 to 55,000 K, TMAP to 100,000 K, and Koester's
//! DA spectra for white dwarfs, with a blackbody beyond 100,000 K.
//!
//! [`tables::star_colour`]: crate::tables::star_colour

use crate::math;
use crate::tables::star_colour::{
    LUMINANCE_RGB, NORMAL, NORMAL_BAKE, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF,
    WHITE_DWARF_BAKE, WHITE_DWARF_LOG_G, WHITE_DWARF_LOG_TEFF,
};

pub use crate::tables::star_colour::CAMERA_ETA_SUN;

/// The number of wavelengths of a bake spectrum: R08's fifteen.
pub const BAKE_WAVELENGTH_COUNT: usize = 15;

/// The centres of the bake spectrum's bins, nm: fifteen bins 25.33 nm wide from 380 to 760 nm,
/// 392.67 + 25.33 k, mirroring R08's `BAKE_WAVELENGTHS_NM` (R08's Design note 5), which
/// `packages/protocol/fixtures/bake_wavelengths_nm.json` pins for both.
pub const BAKE_WAVELENGTHS_NM: [f64; BAKE_WAVELENGTH_COUNT] = [
    392.666_666_666_666_7,
    418.0,
    443.333_333_333_333_3,
    468.666_666_666_666_7,
    494.0,
    519.333_333_333_333_3,
    544.666_666_666_666_7,
    570.0,
    595.333_333_333_333_3,
    620.666_666_666_666_7,
    646.0,
    671.333_333_333_333_3,
    696.666_666_666_666_7,
    722.0,
    747.333_333_333_333_3,
];
use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};
use crate::units::{Kelvin, SolarMasses, SolarRadii};

/// The columns of the fitted colour table's rows (`tables::star_colour`).
mod column {
    /// Linear Rec. 709 red at unit luminance.
    pub const R: usize = 0;
    /// Linear Rec. 709 green at unit luminance.
    pub const G: usize = 1;
    /// The photopic illuminance of a V = 0 star of this spectrum over 2.54 µlx.
    pub const LUX_PER_V0: usize = 2;
    /// The scotopic-to-photopic ratio.
    pub const SP_RATIO: usize = 3;
    /// The camera band term, mag.
    pub const CAMERA_BAND_MAG: usize = 4;
    /// `A_c ÷ A_V` for red, green and blue.
    pub const EXTINCTION: [usize; 3] = [5, 6, 7];
}

/// Which grid of model atmospheres a star's colour is read from: chosen by the star's kind
/// (Design note 6).
///
/// [`MainSequence`](Self::MainSequence) and [`Giant`](Self::Giant) read the same grid, which spans
/// log g 0–6, since the star's own gravity places it there; the two are kept apart because the
/// limb-darkening table (R06.T4) and later tables may not share one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AtmosphereGrid {
    /// A star on or before the main sequence, or any living star of dwarf gravity.
    MainSequence,
    /// A giant or supergiant, or any evolved living star.
    Giant,
    /// A white dwarf: Koester's DA spectra, log g 6.5–9.5.
    WhiteDwarf,
}

/// A star's colour as the sky draws it (Design note 6).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct StarColour {
    red_green: [f64; 2],
    lux_per_v0: f64,
    sp_ratio: f64,
    camera_band_mag: f64,
    extinction_ratio: [f64; 3],
    bake_spectrum: [f64; BAKE_WAVELENGTH_COUNT],
}

impl StarColour {
    /// The linear Rec. 709 red and green of the star's light at unit luminance, as the wire
    /// carries them; blue is [`blue`](Self::blue). [`red_green`](Self::red_green) holds them
    /// unrounded.
    #[must_use]
    pub fn chroma(&self) -> [f32; 2] {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the chroma is a display value of order one, and f32 is its declared type"
        )]
        self.red_green.map(|c| c as f32)
    }

    /// The linear Rec. 709 red and green at unit luminance, unrounded.
    #[must_use]
    pub const fn red_green(&self) -> [f64; 2] {
        self.red_green
    }

    /// The linear Rec. 709 blue at unit luminance, from the red, the green and the Rec. 709
    /// luminance weights the table carries.
    #[must_use]
    pub fn blue(&self) -> f64 {
        let [yr, yg, yb] = LUMINANCE_RGB;
        (1.0 - yr * self.red_green[0] - yg * self.red_green[1]) / yb
    }

    /// The photopic illuminance of a star of this colour at V = 0, over 2.54 µlx: about one,
    /// since the V band and V(λ) nearly coincide.
    #[must_use]
    pub const fn lux_per_v0(&self) -> f64 {
        self.lux_per_v0
    }

    /// The scotopic-to-photopic ratio ρ of the star's light (Design note 3): about 2.3 for the
    /// Sun, more for hotter stars.
    #[must_use]
    pub const fn sp_ratio(&self) -> f64 {
        self.sp_ratio
    }

    /// The camera band term, mag: −2.5 log₁₀(η ÷ η☉), with η the default sensor's electrons per
    /// V-band photon for this spectrum and η☉ [`CAMERA_ETA_SUN`] the Sun's (Design notes 6 and
    /// 18). Zero for the Sun, positive (fainter to the camera) for hot stars, down to about −3 for
    /// the coolest dwarfs, whose light the silicon sees in the near infrared.
    #[must_use]
    pub const fn camera_band_mag(&self) -> f64 {
        self.camera_band_mag
    }

    /// Each display channel's extinction relative to V, `A_c ÷ A_V` at `R_V` = 3.1, as red, green and
    /// blue: plan 07's law at the channel's effective wavelength for this spectrum.
    #[must_use]
    pub const fn extinction_ratio(&self) -> [f64; 3] {
        self.extinction_ratio
    }

    /// The spectrum's mean spectral irradiance over each bin of [`BAKE_WAVELENGTHS_NM`], W m⁻²
    /// nm⁻¹ per lux of photopic illuminance (R08's ask): multiplied by a star's illuminance in lux,
    /// it is the star's spectrum in those bins.
    #[must_use]
    pub const fn bake_spectrum(&self) -> [f64; BAKE_WAVELENGTH_COUNT] {
        self.bake_spectrum
    }
}

/// The colour of a star of effective temperature `teff` and gravity `log_g` (log₁₀ g, cgs) on
/// `grid`, interpolated bilinearly in log₁₀ `T_eff` and log₁₀ g and clamped at the grid's edges.
///
/// A temperature or gravity that is not a number takes the grid's lowest node, so that a broken
/// state can never index past the table; callers pass finite values.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::colour::{AtmosphereGrid, star_colour, surface_gravity};
/// use hyperion_sim::units::{Kelvin, SolarMasses, SolarRadii};
///
/// // The Sun: white to the eye, slightly warm in Rec. 709.
/// let log_g = surface_gravity(SolarMasses::new(1.0), SolarRadii::new(1.0));
/// let sun = star_colour(Kelvin::new(5_772.0), log_g, AtmosphereGrid::MainSequence);
/// let [r, g] = sun.chroma();
/// assert!(r > g && f64::from(g) > sun.blue());
/// assert!((sun.lux_per_v0() - 1.0).abs() < 0.1);
/// ```
#[must_use]
pub fn star_colour(teff: Kelvin, log_g: f64, grid: AtmosphereGrid) -> StarColour {
    type Bake = [f64; BAKE_WAVELENGTH_COUNT];
    type Grid<'a> = (&'a [f64], &'a [f64], &'a [[f64; 8]], &'a [Bake]);
    let (log_teff_nodes, log_g_nodes, rows, bakes): Grid<'static> = match grid {
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => {
            (&NORMAL_LOG_TEFF, &NORMAL_LOG_G, &NORMAL, &NORMAL_BAKE)
        }
        AtmosphereGrid::WhiteDwarf => (
            &WHITE_DWARF_LOG_TEFF,
            &WHITE_DWARF_LOG_G,
            &WHITE_DWARF,
            &WHITE_DWARF_BAKE,
        ),
    };
    let (ti, tf) = bracket(log_teff_nodes, math::log10(teff.value()));
    let (gi, gf) = bracket(log_g_nodes, log_g);
    let width = log_g_nodes.len();
    let at = |a: usize, b: usize| rows[a * width + b];
    let mix = |k: usize| {
        let low = at(ti, gi)[k] * (1.0 - gf) + at(ti, gi + 1)[k] * gf;
        let high = at(ti + 1, gi)[k] * (1.0 - gf) + at(ti + 1, gi + 1)[k] * gf;
        low * (1.0 - tf) + high * tf
    };
    let red_green = [mix(column::R), mix(column::G)];
    let bake_at = |a: usize, b: usize| bakes[a * width + b];
    let bake_spectrum = std::array::from_fn(|k| {
        let low = bake_at(ti, gi)[k] * (1.0 - gf) + bake_at(ti, gi + 1)[k] * gf;
        let high = bake_at(ti + 1, gi)[k] * (1.0 - gf) + bake_at(ti + 1, gi + 1)[k] * gf;
        low * (1.0 - tf) + high * tf
    });
    StarColour {
        red_green,
        lux_per_v0: mix(column::LUX_PER_V0),
        sp_ratio: mix(column::SP_RATIO),
        camera_band_mag: mix(column::CAMERA_BAND_MAG),
        extinction_ratio: column::EXTINCTION.map(mix),
        bake_spectrum,
    }
}

/// The interval of rising `nodes` that holds `x` and the fraction along it, clamped to the first
/// or last interval's end; a NaN takes the first node.
#[must_use]
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 2;
    let i = nodes
        .partition_point(|&node| node <= x)
        .saturating_sub(1)
        .min(last);
    let t = (x - nodes[i]) / (nodes[i + 1] - nodes[i]);
    (i, if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) })
}

/// The surface gravity of a star of `mass` and `radius`: log₁₀ g with g in cm s⁻², g = G M ÷ R²,
/// with the IAU 2015 nominal GM☉ and R☉ (Resolution B3); the Sun's is 4.438.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::colour::surface_gravity;
/// use hyperion_sim::units::{SolarMasses, SolarRadii};
///
/// let sun = surface_gravity(SolarMasses::new(1.0), SolarRadii::new(1.0));
/// assert!((sun - 4.438).abs() < 0.001);
/// // A red giant of 1.2 M☉ and 40 R☉.
/// let giant = surface_gravity(SolarMasses::new(1.2), SolarRadii::new(40.0));
/// assert!((giant - 1.313).abs() < 0.001);
/// ```
#[must_use]
pub fn surface_gravity(mass: SolarMasses, radius: SolarRadii) -> f64 {
    let r = radius.value() * SOLAR_RADIUS_M;
    // m s⁻² to cm s⁻².
    math::log10(GM_SUN * mass.value() / (r * r) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colour(teff: f64, log_g: f64, grid: AtmosphereGrid) -> StarColour {
        star_colour(Kelvin::new(teff), log_g, grid)
    }

    fn rgb(c: &StarColour) -> [f64; 3] {
        [f64::from(c.chroma()[0]), f64::from(c.chroma()[1]), c.blue()]
    }

    #[test]
    fn a_node_is_read_back_exactly() {
        let n = NORMAL_LOG_G.len();
        for (i, &log_teff) in NORMAL_LOG_TEFF.iter().enumerate() {
            for (j, &log_g) in NORMAL_LOG_G.iter().enumerate() {
                let c = colour(math::exp10(log_teff), log_g, AtmosphereGrid::Giant);
                let row = NORMAL[i * n + j];
                let (lux, rho) = (row[column::LUX_PER_V0], row[column::SP_RATIO]);
                assert!((c.lux_per_v0() - lux).abs() < 1e-9 * lux);
                assert!((c.sp_ratio() - rho).abs() < 1e-9 * rho);
            }
        }
    }

    #[test]
    fn the_grids_are_clamped_at_their_edges() {
        for grid in [AtmosphereGrid::MainSequence, AtmosphereGrid::WhiteDwarf] {
            assert_eq!(colour(1_000.0, 4.5, grid), colour(1_500.0, 4.5, grid));
            assert_eq!(colour(1.0e6, 8.0, grid), colour(2.0e6, 8.0, grid));
            assert_eq!(colour(10_000.0, -3.0, grid), colour(10_000.0, -1.0, grid));
            assert_eq!(colour(10_000.0, 12.0, grid), colour(10_000.0, 11.0, grid));
            let nan = colour(f64::NAN, f64::NAN, grid);
            assert!(nan.lux_per_v0().is_finite() && nan.sp_ratio().is_finite());
        }
    }

    #[test]
    fn colour_runs_from_red_to_blue_and_rho_rises_with_temperature() {
        let mut previous: Option<StarColour> = None;
        for teff in [
            2_400.0, 3_000.0, 4_000.0, 5_772.0, 8_000.0, 15_000.0, 40_000.0,
        ] {
            let c = colour(teff, 4.5, AtmosphereGrid::MainSequence);
            let [r, _, b] = rgb(&c);
            if let Some(p) = previous {
                let [pr, _, pb] = rgb(&p);
                assert!(r / b < pr / pb, "{teff} K is not bluer");
                assert!(c.sp_ratio() > p.sp_ratio(), "{teff} K: ρ {}", c.sp_ratio());
            }
            previous = Some(c);
        }
    }

    #[test]
    fn every_row_is_in_gamut_at_unit_luminance() {
        let [yr, yg, yb] = LUMINANCE_RGB;
        assert!((yr + yg + yb - 1.0).abs() < 1e-6);
        for row in NORMAL.iter().chain(&WHITE_DWARF) {
            let (r, g) = (row[column::R], row[column::G]);
            let b = (1.0 - yr * r - yg * g) / yb;
            assert!(r >= -1e-6 && g >= -1e-6 && b >= -1e-6, "{row:?}");
            assert!((0.5..2.0).contains(&row[column::LUX_PER_V0]), "{row:?}");
            assert!((0.2..5.0).contains(&row[column::SP_RATIO]), "{row:?}");
        }
    }

    /// A star of V = 0 lights 2.54 µlx within a tenth of a magnitude from O5 to M6 on the main
    /// sequence: from 0 for the hot stars to +0.10 mag for K5–M2, as Pickles' (1998) own spectra
    /// give (0.000 at O5 V, 0.100 at K5 V, 0.093 at M2 V), since the V band and V(λ) differ
    /// little. The brainstorm's "within 0.08 mag" was corrected to this from the measurement
    /// (ruled 2026-10-02, R06's Risks).
    #[test]
    fn lux_per_v0_is_one_within_a_tenth_of_a_magnitude_from_o5_to_m6() {
        // T_eff and log g of O5 V to M6 V, approximately: after Mamajek's table of 2022.04.16 and
        // Martins et al. (2005) for O5 V, with gravities from the types' masses and radii.
        for (teff, log_g) in [
            (41_500.0, 3.9),
            (30_000.0, 4.0),
            (9_700.0, 4.3),
            (5_772.0, 4.44),
            (4_400.0, 4.6),
            (3_550.0, 4.8),
            (2_850.0, 5.1),
        ] {
            let c = colour(teff, log_g, AtmosphereGrid::MainSequence);
            let mag = 2.5 * math::log10(c.lux_per_v0());
            assert!(mag.abs() < 0.11, "{teff} K: {mag:.3} mag");
        }
    }

    /// The Sun's camera band term is zero by construction, η☉ is decision-camera-eta's 3.0 ± 0.1,
    /// and every row's term fits the wire's 1/32 mag steps (−4.0 to +3.97).
    #[test]
    fn the_camera_term_is_relative_to_the_sun() {
        let sun = colour(5_772.0, 4.438, AtmosphereGrid::MainSequence);
        assert!(
            sun.camera_band_mag().abs() < 1e-9,
            "{}",
            sun.camera_band_mag()
        );
        assert!((2.9..3.15).contains(&CAMERA_ETA_SUN), "{CAMERA_ETA_SUN}");
        for row in NORMAL.iter().chain(&WHITE_DWARF) {
            assert!(
                (-4.0..=3.97).contains(&row[column::CAMERA_BAND_MAG]),
                "{row:?}"
            );
        }
        // Hot stars are fainter to silicon than to V, cool ones brighter.
        assert!(colour(30_000.0, 4.0, AtmosphereGrid::MainSequence).camera_band_mag() > 0.0);
        assert!(colour(3_000.0, 5.0, AtmosphereGrid::MainSequence).camera_band_mag() < -1.0);
    }

    /// Reddening is strongest in blue and weakest in red, about 1.25, 1.03 and 0.91 of `A_V` for a
    /// Sun-like star (Cardelli et al. 1989 at the channels' effective wavelengths).
    #[test]
    fn the_extinction_ratios_order_blue_green_red() {
        for teff in [3_000.0, 5_772.0, 20_000.0] {
            let [r, g, b] = colour(teff, 4.5, AtmosphereGrid::MainSequence).extinction_ratio();
            assert!(b > g && g > r, "{teff} K: {r} {g} {b}");
            assert!(
                (0.6..0.95).contains(&r) && (0.9..1.15).contains(&g) && (1.1..1.5).contains(&b)
            );
        }
    }

    /// `BAKE_WAVELENGTHS_NM` is the cross-language fixture's, which R08's constant reads too (the
    /// fit's own tests check that every row's bake spectrum holds 1 lx).
    #[test]
    fn the_bake_wavelengths_are_r08s_fixture() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/bake_wavelengths_nm.json"
        ));
        let list = fixture
            .split("\"wavelengths_nm\":")
            .nth(1)
            .and_then(|rest| rest.split('[').nth(1))
            .and_then(|rest| rest.split(']').next())
            .expect("the fixture lists wavelengths_nm");
        let values: Vec<f64> = list
            .split(',')
            .map(|v| v.trim().parse().expect("a wavelength is a number"))
            .collect();
        assert_eq!(values.len(), BAKE_WAVELENGTH_COUNT);
        for (a, b) in values.iter().zip(BAKE_WAVELENGTHS_NM) {
            assert!((a - b).abs() < 1e-9, "{a} against {b}");
        }
    }

    /// `CAMERA_ETA_SUN` is the cross-language fixture's, which the client's default camera reads.
    #[test]
    fn camera_eta_sun_is_the_fixtures() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/camera_eta_sun.json"
        ));
        let value: f64 = fixture
            .split("\"camera_eta_sun\":")
            .nth(1)
            .and_then(|rest| rest.split(['\n', '}']).next())
            .and_then(|v| v.trim().trim_end_matches(',').parse().ok())
            .expect("the fixture holds camera_eta_sun");
        assert!(
            (value - CAMERA_ETA_SUN).abs() < 1e-12,
            "{value} against {CAMERA_ETA_SUN}"
        );
    }

    #[test]
    fn a_white_dwarf_reads_its_own_grid() {
        let wd = colour(10_000.0, 8.0, AtmosphereGrid::WhiteDwarf);
        let dwarf = colour(10_000.0, 8.0, AtmosphereGrid::MainSequence);
        assert_ne!(wd, dwarf);
    }

    #[test]
    fn surface_gravity_is_g_m_over_r_squared() {
        let g = surface_gravity(SolarMasses::new(0.6), SolarRadii::new(0.0125));
        // 0.6 M☉ in 0.0125 R☉, a typical white dwarf: 10^8.022 cm s⁻².
        assert!((g - 8.0224).abs() < 0.001, "{g}");
    }
}
