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
//! A colour is the star's own light. [`StarColour::reddened`] gives it through dust (R06.T9.e;
//! decided 2026-10-06, `decision-r06-t9b-band.md`, item 3): each display channel, the photopic and
//! the scotopic light and the camera's band dimmed by its own `A ÷ A_V`, from the companion table
//! [`tables::star_colour_reddening`], row for row the colour table's. It is the one function the
//! band, the limit map's glare and the server's wire read for reddening.
//!
//! [`tables::star_colour`]: crate::tables::star_colour
//! [`tables::star_colour_reddening`]: crate::tables::star_colour_reddening

use crate::math;
use crate::tables::star_colour::{
    LUMINANCE_RGB, NORMAL, NORMAL_BAKE, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF,
    WHITE_DWARF_BAKE, WHITE_DWARF_LOG_G, WHITE_DWARF_LOG_TEFF,
};
use crate::tables::star_colour_reddening::{NORMAL_REDDENING, WHITE_DWARF_REDDENING};

pub use crate::tables::star_colour::CAMERA_ETA_SUN;
pub use crate::tables::star_colour_reddening::CAMERA_REDDENING_A_V;

// The reddening table is row for row the colour table's: both are fitted on the same grids.
const _: () = assert!(
    NORMAL_REDDENING.len() == NORMAL.len() && WHITE_DWARF_REDDENING.len() == WHITE_DWARF.len()
);

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
use crate::units::{Kelvin, Magnitudes, SolarMasses, SolarRadii};

/// The Sun's effective temperature, K (IAU 2015 Resolution B3's nominal value): with
/// [`SUN_LOG_G`], the colour table's solar point, where [`CAMERA_ETA_SUN`] is taken.
pub const SUN_TEFF_K: f64 = 5_772.0;

/// The Sun's surface gravity, log₁₀ g (cgs), from the IAU 2015 nominal GM☉ and R☉
/// ([`surface_gravity`] of one solar mass and radius, to three decimals).
pub const SUN_LOG_G: f64 = 4.438;

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

/// The columns of the reddening table's rows (`tables::star_colour_reddening`).
mod reddening_column {
    /// `A_P ÷ A_V`, the photopic light's.
    pub const PHOTOPIC: usize = 0;
    /// `A_S ÷ A_V`, the scotopic light's.
    pub const SCOTOPIC: usize = 1;
    /// The camera's `A_cam ÷ A_V` at `A_V` → 0 and at `CAMERA_REDDENING_A_V`.
    pub const CAMERA: [usize; 2] = [2, 3];
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
    photopic_extinction_ratio: f64,
    scotopic_extinction_ratio: f64,
    camera_extinction_ratio: [f64; 2],
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

    /// The photopic light's extinction relative to V, `A_P ÷ A_V` at `R_V` = 3.1: plan 07's law at
    /// the effective wavelength of the spectrum under the CIE 1924 V(λ), over the V band's (about
    /// 0.985 for the Sun, R06.T9.e).
    #[must_use]
    pub const fn photopic_extinction_ratio(&self) -> f64 {
        self.photopic_extinction_ratio
    }

    /// The scotopic light's extinction relative to V, `A_S ÷ A_V`, likewise under the CIE 1951
    /// V′(λ) (about 1.12 for the Sun).
    #[must_use]
    pub const fn scotopic_extinction_ratio(&self) -> f64 {
        self.scotopic_extinction_ratio
    }

    /// The default camera's extinction relative to V, `A_cam ÷ A_V`, at `A_V` → 0 and at
    /// [`CAMERA_REDDENING_A_V`]: each band's own integral over the same dust (Design note 18's
    /// sensor), since the camera's band is too broad for one effective wavelength.
    #[must_use]
    pub const fn camera_extinction_ratio(&self) -> [f64; 2] {
        self.camera_extinction_ratio
    }

    /// The star's light through dust that dims its V by `a_v` (R06.T9.e; decided 2026-10-06,
    /// `decision-r06-t9b-band.md`, item 3).
    ///
    /// Each band is dimmed by its own ratio: a transmission t = 10<sup>−0.4 k `A_V`</sup> for each
    /// display channel ([`extinction_ratio`](Self::extinction_ratio)) and for the photopic and the
    /// scotopic light; the reddened ρ is ρ t<sub>S</sub> ÷ t<sub>P</sub>; and the camera band term
    /// becomes c − 2.5 log₁₀(t<sub>cam</sub> ÷ t<sub>V</sub>) with t<sub>V</sub> =
    /// 10<sup>−0.4 `A_V`</sup>, which is c − (1 − k<sub>cam</sub>) `A_V`, k<sub>cam</sub> taken as
    /// linear in `A_V` through [`camera_extinction_ratio`](Self::camera_extinction_ratio)'s two
    /// columns. At `A_V` 2 a Sun-like star's eye colour offset falls by about 0.27 mag (in a dark,
    /// scotopic sky's limit), and its camera term by about 0.36.
    ///
    /// Beyond [`CAMERA_REDDENING_A_V`] the line is extrapolated, as the ruling leaves it: for the
    /// Sun's light it is off the integral by −0.07 mag at `A_V` 5 and −0.74 at 10, and the camera's
    /// extinction it implies peaks near `A_V` 16 (R06's Risks, "Deviations in T9.e, as built").
    ///
    /// `a_v` is a finite extinction, 0 or more (a profile's rounding a hair below zero dims
    /// nothing to speak of); at 0 every value is the colour's own, bit for bit.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::sky::colour::{AtmosphereGrid, star_colour};
    /// use hyperion_sim::units::{Kelvin, Magnitudes};
    ///
    /// let sun = star_colour(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
    /// // Behind a magnitude of dust the Sun's light is redder and less scotopic.
    /// let behind = sun.reddened(Magnitudes::new(1.0));
    /// let [red, green, blue] = behind.transmission();
    /// assert!(blue < green && green < red);
    /// assert!(behind.sp_ratio() < sun.sp_ratio());
    /// // Silicon sees the dust less than V does: the star is brighter to it relative to V.
    /// assert!(behind.camera_band_mag() < sun.camera_band_mag());
    /// ```
    #[must_use]
    pub fn reddened(&self, a_v: Magnitudes) -> Reddened {
        let a = a_v.value();
        debug_assert!(a.is_finite(), "an extinction of {a} mag");
        let through = |ratio: f64| math::exp10(-0.4 * ratio * a);
        let photopic = through(self.photopic_extinction_ratio);
        let scotopic = through(self.scotopic_extinction_ratio);
        let [at_zero, at_reference] = self.camera_extinction_ratio;
        let camera_ratio = at_zero + (at_reference - at_zero) * (a / CAMERA_REDDENING_A_V);
        let transmission = self.extinction_ratio.map(through);
        let [t_red, t_green, t_blue] = transmission;
        let [red, green] = self.red_green;
        let [yr, yg, _] = LUMINANCE_RGB;
        // The Rec. 709 luminance of what passes, per unit of the colour's: Y_r r t_r + Y_g g t_g
        // + Y_b b t_b with Y_b b = 1 − Y_r r − Y_g g, written so that it is one exactly where
        // every transmission is.
        let luminance = yr * red * (t_red - t_blue) + yg * green * (t_green - t_blue) + t_blue;
        Reddened {
            transmission,
            photopic_transmission: photopic,
            scotopic_transmission: scotopic,
            red_green: [red * t_red / luminance, green * t_green / luminance],
            sp_ratio: self.sp_ratio * scotopic / photopic,
            camera_band_mag: self.camera_band_mag - (a - camera_ratio * a),
        }
    }
}

#[cfg(test)]
impl StarColour {
    /// This colour behind grey dust: every band dimmed as V is, as the band dimmed its light
    /// before R06.T9.e, for the tests' unreddened march.
    #[must_use]
    pub(crate) const fn with_grey_dust(mut self) -> Self {
        self.extinction_ratio = [1.0; 3];
        self.photopic_extinction_ratio = 1.0;
        self.scotopic_extinction_ratio = 1.0;
        self.camera_extinction_ratio = [1.0; 2];
        self
    }
}

/// A star's light through dust ([`StarColour::reddened`]): what each band keeps, and the S/P ratio
/// and camera band term of what is left.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Reddened {
    transmission: [f64; 3],
    photopic_transmission: f64,
    scotopic_transmission: f64,
    red_green: [f64; 2],
    sp_ratio: f64,
    camera_band_mag: f64,
}

impl Reddened {
    /// The fraction of each linear Rec. 709 channel's light that passes, red, green and blue: so
    /// the reddened colour is the colour's red, green and blue times these, at a luminance of the
    /// photopic transmission's times the colour's.
    #[must_use]
    pub const fn transmission(&self) -> [f64; 3] {
        self.transmission
    }

    /// The fraction of the photopic light that passes: the star's photopic illuminance is its
    /// unextinguished one times this.
    #[must_use]
    pub const fn photopic_transmission(&self) -> f64 {
        self.photopic_transmission
    }

    /// The fraction of the scotopic light that passes.
    #[must_use]
    pub const fn scotopic_transmission(&self) -> f64 {
        self.scotopic_transmission
    }

    /// The linear Rec. 709 red and green of the light that passes, at unit luminance, as the
    /// colour's [`red_green`](StarColour::red_green) are: each channel times its transmission,
    /// over the Rec. 709 luminance of the three.
    #[must_use]
    pub const fn red_green(&self) -> [f64; 2] {
        self.red_green
    }

    /// [`red_green`](Self::red_green) as the wire carries a chroma (Design note 17).
    #[must_use]
    pub fn chroma(&self) -> [f32; 2] {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the chroma is a display value of order one, and f32 is its declared type"
        )]
        self.red_green.map(|c| c as f32)
    }

    /// The S/P ratio ρ of the light that passes, ρ t<sub>S</sub> ÷ t<sub>P</sub> (Design note 3),
    /// from which a star's eye colour offset is read.
    #[must_use]
    pub const fn sp_ratio(&self) -> f64 {
        self.sp_ratio
    }

    /// The camera band term of the light that passes, mag, relative to its V after extinction, as
    /// [`StarColour::camera_band_mag`] is to V.
    #[must_use]
    pub const fn camera_band_mag(&self) -> f64 {
        self.camera_band_mag
    }
}

/// The colour of the Sun's light: the table at [`SUN_TEFF_K`] and [`SUN_LOG_G`], the point of
/// [`CAMERA_ETA_SUN`], whose ratios redden every node of the band (R06.T9.e).
#[must_use]
pub fn solar_colour() -> StarColour {
    star_colour(
        Kelvin::new(SUN_TEFF_K),
        SUN_LOG_G,
        AtmosphereGrid::MainSequence,
    )
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
    type Grid<'a> = (
        &'a [f64],
        &'a [f64],
        &'a [[f64; 8]],
        &'a [Bake],
        &'a [[f64; 4]],
    );
    let (log_teff_nodes, log_g_nodes, rows, bakes, reddening): Grid<'static> = match grid {
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => (
            &NORMAL_LOG_TEFF,
            &NORMAL_LOG_G,
            &NORMAL,
            &NORMAL_BAKE,
            &NORMAL_REDDENING,
        ),
        AtmosphereGrid::WhiteDwarf => (
            &WHITE_DWARF_LOG_TEFF,
            &WHITE_DWARF_LOG_G,
            &WHITE_DWARF,
            &WHITE_DWARF_BAKE,
            &WHITE_DWARF_REDDENING,
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
    let reddening_at = |a: usize, b: usize| reddening[a * width + b];
    let mix_reddening = |k: usize| {
        let low = reddening_at(ti, gi)[k] * (1.0 - gf) + reddening_at(ti, gi + 1)[k] * gf;
        let high = reddening_at(ti + 1, gi)[k] * (1.0 - gf) + reddening_at(ti + 1, gi + 1)[k] * gf;
        low * (1.0 - tf) + high * tf
    };
    StarColour {
        red_green,
        lux_per_v0: mix(column::LUX_PER_V0),
        sp_ratio: mix(column::SP_RATIO),
        camera_band_mag: mix(column::CAMERA_BAND_MAG),
        extinction_ratio: column::EXTINCTION.map(mix),
        bake_spectrum,
        photopic_extinction_ratio: mix_reddening(reddening_column::PHOTOPIC),
        scotopic_extinction_ratio: mix_reddening(reddening_column::SCOTOPIC),
        camera_extinction_ratio: reddening_column::CAMERA.map(mix_reddening),
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
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::sky::eye::{SkyBackground, SpRatio, luminance, star_colour_offset};
    use crate::units::MagnitudesPerArcsec2;

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
                let reddening = NORMAL_REDDENING[i * n + j];
                let read = [
                    c.photopic_extinction_ratio(),
                    c.scotopic_extinction_ratio(),
                    c.camera_extinction_ratio()[0],
                    c.camera_extinction_ratio()[1],
                ];
                for (got, want) in read.into_iter().zip(reddening) {
                    assert!((got - want).abs() < 1e-9 * want, "{got} against {want}");
                }
            }
        }
    }

    /// Every colour of the table, at its nodes and between them, on both grids.
    fn sampled_colours() -> Vec<StarColour> {
        let mut out = vec![StarColour::default()];
        for grid in [AtmosphereGrid::MainSequence, AtmosphereGrid::WhiteDwarf] {
            for teff in [
                2_300.0, 2_870.0, 3_500.0, 4_444.0, 5_772.0, 7_777.0, 12_345.0, 40_000.0, 2.0e5,
            ] {
                for log_g in [0.0, 1.3, 4.438, 5.0, 7.0, 8.2, 9.5] {
                    out.push(colour(teff, log_g, grid));
                }
            }
        }
        out
    }

    /// `reddened` at no dust is the colour: every transmission one, and ρ and the camera term the
    /// colour's own, bit for bit.
    #[test]
    fn reddened_by_no_dust_is_the_colour() {
        for c in sampled_colours() {
            for a_v in [Magnitudes::ZERO, Magnitudes::new(-0.0)] {
                let r = c.reddened(a_v);
                assert_eq!(r.transmission().map(bits), [bits(1.0); 3], "{c:?}");
                assert_eq!(bits(r.photopic_transmission()), bits(1.0), "{c:?}");
                assert_eq!(bits(r.scotopic_transmission()), bits(1.0), "{c:?}");
                assert_eq!(r.red_green().map(bits), c.red_green().map(bits), "{c:?}");
                assert_eq!(bits(r.sp_ratio()), bits(c.sp_ratio()), "{c:?}");
                assert_eq!(
                    bits(r.camera_band_mag()),
                    bits(c.camera_band_mag()),
                    "{c:?}"
                );
            }
        }
    }

    /// Dust takes blue light first and red light last, and the scotopic light, bluer than the
    /// photopic, before it; so the reddened ρ falls with the dust. (The camera term need not: a
    /// hot star's light is bluer under silicon than under V.)
    #[test]
    fn the_transmissions_order_blue_green_red_and_scotopic_below_photopic() {
        for c in sampled_colours().into_iter().skip(1) {
            let mut previous = c.reddened(Magnitudes::ZERO);
            for a_v in [0.1, 1.0, 2.0, 5.0] {
                let r = c.reddened(Magnitudes::new(a_v));
                let [red, green, blue] = r.transmission();
                assert!(
                    blue < green && green < red && red < 1.0,
                    "{c:?} at {a_v}: {r:?}"
                );
                assert!(
                    r.scotopic_transmission() < r.photopic_transmission(),
                    "{c:?} at {a_v}: {r:?}"
                );
                // What passes is redder, and its colour of unit luminance.
                let [red_now, green_now] = r.red_green();
                let [red_then, green_then] = previous.red_green();
                assert!(
                    red_now / green_now > red_then / green_then,
                    "{c:?} at {a_v}"
                );
                let [yr, yg, yb] = LUMINANCE_RGB;
                let blue_now = c.blue() * blue
                    / (yr * c.red_green()[0] * red
                        + yg * c.red_green()[1] * green
                        + yb * c.blue() * blue);
                let y = yr * red_now + yg * green_now + yb * blue_now;
                assert!((y - 1.0).abs() < 1e-12, "{c:?} at {a_v}: {y}");
                assert!(r.sp_ratio() < previous.sp_ratio(), "{c:?} at {a_v}");
                previous = r;
            }
        }
    }

    /// The solar row's ratios lie in the ruling's ranges (`decision-r06-t9b-band.md`, item 3, and
    /// R06.T9.e's tests: 0.97–1.00, 0.11–0.15 and 0.78–0.88). The camera's are its integrals', 0.874
    /// and 0.820, above the ruling's effective-wavelength figures (R06's Risks, "Deviations in T9.e,
    /// as built").
    #[test]
    fn the_solar_rows_ratios_are_the_rulings() {
        let sun = solar_colour();
        assert_eq!(sun, colour(5_772.0, 4.438, AtmosphereGrid::MainSequence));
        let (photopic, scotopic) = (
            sun.photopic_extinction_ratio(),
            sun.scotopic_extinction_ratio(),
        );
        assert!((0.97..=1.00).contains(&photopic), "A_P ÷ A_V {photopic}");
        assert!(
            (0.11..=0.15).contains(&(scotopic - photopic)),
            "A_S ÷ A_V {scotopic} less A_P ÷ A_V {photopic}"
        );
        let [camera_0, camera_2] = sun.camera_extinction_ratio();
        assert!((0.78..=0.88).contains(&camera_0), "A_cam ÷ A_V {camera_0}");
        // The dust takes the blue first, so what is left sits redder, where silicon sees less
        // of it.
        assert!(camera_2 < camera_0, "{camera_2} against {camera_0}");
        assert_eq!(bits(CAMERA_REDDENING_A_V), bits(2.0));
    }

    /// At `A_V` 2 the Sun's eye colour offset (against a dark, scotopic sky) falls by about 0.27
    /// mag, (`A_S` − `A_P`) ÷ `A_V` of it a magnitude, and its camera term by about 0.36 (the ruling's
    /// "about −0.26" and "about −0.4"), an M dwarf's camera term by more.
    #[test]
    fn dust_of_two_magnitudes_moves_the_eye_offset_and_the_camera_term_as_ruled() {
        let dark = SkyBackground::new(
            luminance(MagnitudesPerArcsec2::new(30.0)),
            SpRatio::REFERENCE,
        )
        .expect("a background");
        let offset =
            |rho: f64| star_colour_offset(SpRatio::new(rho).expect("a ratio"), &dark).value();
        let two = Magnitudes::new(2.0);
        let sun = solar_colour();
        let behind = sun.reddened(two);
        let eye = offset(behind.sp_ratio()) - offset(sun.sp_ratio());
        let expected = -2.0 * (sun.scotopic_extinction_ratio() - sun.photopic_extinction_ratio());
        assert!((eye - expected).abs() < 1e-9, "{eye} against {expected}");
        assert!((eye + 0.26).abs() < 0.03, "the eye offset moves by {eye}");
        let camera = behind.camera_band_mag() - sun.camera_band_mag();
        assert!(
            (-0.45..-0.3).contains(&camera),
            "the camera term moves by {camera}"
        );
        let m_dwarf = colour(3_550.0, 4.8, AtmosphereGrid::MainSequence);
        let m_camera = m_dwarf.reddened(two).camera_band_mag() - m_dwarf.camera_band_mag();
        assert!(
            m_camera < camera,
            "an M dwarf's {m_camera} against the Sun's {camera}"
        );
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
