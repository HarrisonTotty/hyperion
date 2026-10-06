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
//! # Reddening
//!
//! A colour is the star's own light. [`StarColour::reddened`] gives it through dust (R06.T9.e;
//! decided 2026-10-06, `decision-r06-t9b-band.md`, item 3 and its addendum), from four companion
//! tables row for row the colour table's: [`tables::star_colour_reddening`] and the three of
//! [`REDDENING_A_V_NODES`] (`star_colour_reddening_av02_05`, `_av10_15` and `_av20_30`). It is
//! the one function the band, the limit map's glare and the server's wire read for reddening.
//!
//! - **One A<sub>V</sub>.** Every A<sub>V</sub> here is plan 07's sightline quantity: the law's
//!   normalisation at x = 1.82 µm⁻¹ (CCM's 0.549 µm) times the dust column, the only extinction the
//!   sim computes. Each
//!   column is a band's broadband extinction over it for the row's own spectrum.
//! - **The bands.** Each Rec. 709 colour-matching function c̄ (the matrix applied to x̄, ȳ and z̄)
//!   is split into its positive and negative parts, c̄ = c̄⁺ − c̄⁻, each a positive band: six parts.
//!   With them, the V band (Bessell and Murphy's photonic V), the scotopic V′(λ) and the default
//!   camera's QE λ: nine bands. A band X of weights w is dimmed by its transmission T(A) = ∫S w
//!   10<sup>−0.4 ℓ A</sup> ÷ ∫S w, ℓ plan 07's law, and its secant is k(A) = −2.5 log₁₀ T(A) ÷ A.
//! - **What the tables hold.** Each part's value at unit luminance, ∫S c̄± ÷ ∫S ȳ; each band's
//!   first moment of ℓ, its secant's limit as A → 0; and each band's secant at every node of
//!   [`REDDENING_A_V_NODES`]. A secant is piecewise linear in A through its moment and the nodes,
//!   and held beyond the last, 30.
//! - **What passes.** Each channel is c(A) = c⁺ 10<sup>−0.4 k⁺(A) A</sup> − c⁻ 10<sup>−0.4 k⁻(A)
//!   A</sup>, and the photopic light the channels' luminance, Σ Y<sub>c</sub> c(A) (`LUMINANCE_RGB`),
//!   since ȳ is exactly that luminance of r̄, ḡ and b̄. The colour of unit luminance is c(A) over
//!   it, lifted into gamut by T3's own rule ([`lift_into_gamut`]); a strongly reddened channel can
//!   go negative, as blue does past A<sub>V</sub> 7–8 for the Sun's light.
//!
//! [`tables::star_colour`]: crate::tables::star_colour
//! [`tables::star_colour_reddening`]: crate::tables::star_colour_reddening

use crate::math;
use crate::tables::star_colour::{
    LUMINANCE_RGB, NORMAL, NORMAL_BAKE, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF,
    WHITE_DWARF_BAKE, WHITE_DWARF_LOG_G, WHITE_DWARF_LOG_TEFF,
};
use crate::tables::star_colour_reddening::{NORMAL_REDDENING, WHITE_DWARF_REDDENING};
use crate::tables::{
    star_colour_reddening_av02_05 as av02_05, star_colour_reddening_av10_15 as av10_15,
    star_colour_reddening_av20_30 as av20_30,
};

pub use crate::tables::star_colour::CAMERA_ETA_SUN;

/// The extinctions at which the reddening tables hold each band's secant, mag of plan 07's
/// sightline A<sub>V</sub> (decided 2026-10-06, `decision-r06-t9b-band.md`, addendum item 2): the
/// secants are piecewise linear in A<sub>V</sub> through the moment at 0 and these, and held beyond
/// the last. A listed star lies behind at most about A<sub>V</sub> 21 − DM (no cut exceeds
/// [`MAX_CUT_V`](crate::sky::eye::MAX_CUT_V) and no star is brighter than about M<sub>V</sub> −10),
/// so 30 covers every listable star. `hyperion-fit` reads this constant, and its manifests record
/// it.
pub const REDDENING_A_V_NODES: [f64; REDDENING_NODE_COUNT] = [2.0, 5.0, 10.0, 15.0, 20.0, 30.0];

/// The number of [`REDDENING_A_V_NODES`].
pub const REDDENING_NODE_COUNT: usize = 6;

// The reddening tables are row for row the colour table's: all are fitted on the same grids.
const _: () = assert!(
    NORMAL_REDDENING.len() == NORMAL.len()
        && WHITE_DWARF_REDDENING.len() == WHITE_DWARF.len()
        && av02_05::NORMAL_SECANTS.len() == NORMAL.len()
        && av02_05::WHITE_DWARF_SECANTS.len() == WHITE_DWARF.len()
        && av10_15::NORMAL_SECANTS.len() == NORMAL.len()
        && av10_15::WHITE_DWARF_SECANTS.len() == WHITE_DWARF.len()
        && av20_30::NORMAL_SECANTS.len() == NORMAL.len()
        && av20_30::WHITE_DWARF_SECANTS.len() == WHITE_DWARF.len()
        && NORMAL.len() < 1 << 16
        && WHITE_DWARF.len() < 1 << 16
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

/// The reddening tables' bands, in their column order (the [module](self) documentation): the
/// six parts of the Rec. 709 colour-matching functions, then V, the scotopic and the camera.
pub(crate) mod band {
    /// r̄⁺, r̄⁻, ḡ⁺, ḡ⁻, b̄⁺ and b̄⁻: part `2c` is channel c's positive part, `2c + 1` its negative.
    pub const PARTS: usize = 6;
    /// Bessell and Murphy's photonic V.
    pub const V: usize = 6;
    /// The CIE 1951 V′(λ).
    pub const SCOTOPIC: usize = 7;
    /// The default camera's QE λ.
    pub const CAMERA: usize = 8;
    /// Every band.
    pub const COUNT: usize = 9;
}

/// The columns of `tables::star_colour_reddening`'s rows: the parts' values at unit luminance,
/// then the bands' moments.
mod reddening_column {
    /// The first of the six parts' values.
    pub const PARTS: usize = 0;
    /// The first of the nine bands' moments.
    pub const MOMENTS: usize = 6;
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

/// One of the table's two grids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
enum Grid {
    /// Stars that are not white dwarfs.
    #[default]
    Normal,
    /// White dwarfs.
    WhiteDwarf,
}

/// Where a colour was read from the table: its grid, the lower corner of its cell and the
/// fractions along the cell, so that [`StarColour::reddening`] reads the reddening tables as
/// [`star_colour`] read the colour table, without a star holding all of their columns.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
struct TablePoint {
    grid: Grid,
    teff_node: u16,
    log_g_node: u16,
    along_teff: f64,
    along_log_g: f64,
}

impl TablePoint {
    /// The bilinear mix of column `k` of `rows`, one row a node of the point's grid, as
    /// [`star_colour`] mixes the colour table.
    #[must_use]
    fn mix<const N: usize>(self, rows: &[[f64; N]], k: usize) -> f64 {
        let width = match self.grid {
            Grid::Normal => NORMAL_LOG_G.len(),
            Grid::WhiteDwarf => WHITE_DWARF_LOG_G.len(),
        };
        let (ti, gi) = (usize::from(self.teff_node), usize::from(self.log_g_node));
        let (tf, gf) = (self.along_teff, self.along_log_g);
        let at = |a: usize, b: usize| rows[a * width + b][k];
        let low = at(ti, gi) * (1.0 - gf) + at(ti, gi + 1) * gf;
        let high = at(ti + 1, gi) * (1.0 - gf) + at(ti + 1, gi + 1) * gf;
        low * (1.0 - tf) + high * tf
    }
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
    table: TablePoint,
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
    /// blue: plan 07's law at the effective wavelength of the channel's positive lobe for this
    /// spectrum over the law at the V band's effective wavelength (R06.T3.c), not over the
    /// sightline's `A_V` that the reddening tables take. [`reddened`](Self::reddened) does not read it: it takes each channel's
    /// positive and negative parts exactly (R06.T9.e, addendum item 1), which this ratio's
    /// reddened colour excess falls some 19% short of.
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

    /// The colour's reddening curves, read from the reddening tables at the colour's place in its
    /// grid: what [`reddened`](Self::reddened) dims the light by. A caller that reddens one
    /// colour through many dusts, as the band does a node at a time, reads them once.
    #[must_use]
    pub fn reddening(&self) -> Reddening {
        let at = self.table;
        let (zero, nodes): (&[[f64; 15]], [&[[f64; 18]]; 3]) = match at.grid {
            Grid::Normal => (
                &NORMAL_REDDENING,
                [
                    &av02_05::NORMAL_SECANTS,
                    &av10_15::NORMAL_SECANTS,
                    &av20_30::NORMAL_SECANTS,
                ],
            ),
            Grid::WhiteDwarf => (
                &WHITE_DWARF_REDDENING,
                [
                    &av02_05::WHITE_DWARF_SECANTS,
                    &av10_15::WHITE_DWARF_SECANTS,
                    &av20_30::WHITE_DWARF_SECANTS,
                ],
            ),
        };
        let secants = std::array::from_fn(|b| {
            std::array::from_fn(|n| {
                if n == 0 {
                    at.mix(zero, reddening_column::MOMENTS + b)
                } else {
                    // Each node file holds two nodes, nine bands each.
                    let (file, second) = ((n - 1) / 2, (n - 1) % 2);
                    at.mix(nodes[file], second * band::COUNT + b)
                }
            })
        });
        Reddening {
            red_green: self.red_green,
            sp_ratio: self.sp_ratio,
            camera_band_mag: self.camera_band_mag,
            parts: std::array::from_fn(|p| at.mix(zero, reddening_column::PARTS + p)),
            secants,
        }
    }

    /// The star's light through dust of plan 07's sightline A<sub>V</sub> `a_v` (R06.T9.e;
    /// decided 2026-10-06, `decision-r06-t9b-band.md`, item 3 and its addendum): what each band
    /// keeps, the colour and S/P ratio of what passes, its own V band's extinction and its camera
    /// band term ([`Reddening::through`] of [`reddening`](Self::reddening)).
    ///
    /// At `A_V` 2 a Sun-like star's eye colour offset falls by about 0.27 mag (in a dark, scotopic
    /// sky's limit), and its camera term by about 0.36.
    ///
    /// `a_v` is a finite extinction. No dust, or a profile's rounding a hair below zero, leaves
    /// the light as it is: every value is then the colour's own, bit for bit.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::sky::colour::{AtmosphereGrid, star_colour};
    /// use hyperion_sim::units::Magnitudes;
    /// use hyperion_sim::units::Kelvin;
    ///
    /// let sun = star_colour(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
    /// // Behind a magnitude of dust the Sun's light is redder and less scotopic.
    /// let behind = sun.reddened(Magnitudes::new(1.0));
    /// let [red, green, blue] = behind.transmission();
    /// assert!(blue < green && green < red);
    /// assert!(behind.sp_ratio() < sun.sp_ratio());
    /// // Its own V band is dimmed by about the sightline's A_V.
    /// assert!((behind.v_extinction().value() - 1.0).abs() < 0.01);
    /// // Silicon sees the dust less than V does: the star is brighter to it relative to V.
    /// assert!(behind.camera_band_mag() < sun.camera_band_mag());
    /// ```
    #[must_use]
    pub fn reddened(&self, a_v: Magnitudes) -> Reddened {
        if a_v.value() <= 0.0 {
            return Reddened::unreddened(self.red_green, self.sp_ratio, self.camera_band_mag);
        }
        self.reddening().through(a_v)
    }
}

/// A colour's reddening curves ([`StarColour::reddening`]): each part of the Rec. 709
/// colour-matching functions at unit luminance, and each of the nine bands' secants at
/// A<sub>V</sub> → 0 and at each node of [`REDDENING_A_V_NODES`] (the [module](self)
/// documentation), with the colour's own chroma, ρ and camera term.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Reddening {
    red_green: [f64; 2],
    sp_ratio: f64,
    camera_band_mag: f64,
    parts: [f64; band::PARTS],
    secants: [[f64; REDDENING_NODE_COUNT + 1]; band::COUNT],
}

impl Reddening {
    /// The light through dust of plan 07's sightline A<sub>V</sub> `a_v`, as
    /// [`StarColour::reddened`] gives it.
    ///
    /// Each band's secant is piecewise linear in A<sub>V</sub> through its moment at 0 and the
    /// nodes, and held beyond the last. Each channel is its two parts, each dimmed by its own
    /// secant; the photopic light is the channels' Rec. 709 luminance; the colour of unit
    /// luminance is the channels over it, lifted into gamut ([`lift_into_gamut`]); ρ is ρ
    /// t<sub>S</sub> ÷ t<sub>P</sub>; the V extinction is k<sub>V</sub>(A) A; and the camera term is
    /// c + (k<sub>cam</sub>(A) − k<sub>V</sub>(A)) A, relative to the star's V after its own
    /// extinction.
    ///
    /// Beyond the last node the photopic light is held as a band of its own: its secant at the
    /// node, the parts' there, and the colour of unit luminance the node's. Each part's own held
    /// secant would let the slowest part outlast the others, and their luminance, a few parts in
    /// 10¹¹ of the light at `A_V` 30, falls below zero from about 81 on some rows. The V, scotopic
    /// and camera secants are held as they are.
    ///
    /// # Panics
    ///
    /// In debug builds, if `a_v` is not finite.
    #[must_use]
    pub fn through(&self, a_v: Magnitudes) -> Reddened {
        let a = a_v.value();
        debug_assert!(a.is_finite(), "an extinction of {a} mag");
        if a <= 0.0 {
            return Reddened::unreddened(self.red_green, self.sp_ratio, self.camera_band_mag);
        }
        let last = REDDENING_A_V_NODES[REDDENING_NODE_COUNT - 1];
        let secant = self.secants_at(a);
        // The parts are taken at the last node beyond it, their light then dimmed on as one band.
        let parts_at = a.min(last);
        let through = |b: usize| math::exp10(-0.4 * secant[b] * parts_at);
        let p = self.parts;
        let unreddened = [p[0] - p[1], p[2] - p[3], p[4] - p[5]];
        let mut channels: [f64; 3] =
            std::array::from_fn(|c| p[2 * c] * through(2 * c) - p[2 * c + 1] * through(2 * c + 1));
        let [yr, yg, yb] = LUMINANCE_RGB;
        let luminance = yr * channels[0] + yg * channels[1] + yb * channels[2];
        let luminance_unreddened = yr * unreddened[0] + yg * unreddened[1] + yb * unreddened[2];
        // The parts' luminance is ∫S ȳ T ÷ ∫S ȳ, positive to the last node on every row (the
        // tests), so what passes always has a colour.
        let [red, green, _] = lift_into_gamut(channels.map(|c| c / luminance));
        let mut photopic = luminance / luminance_unreddened;
        if a > last {
            let held = math::exp10(math::log10(photopic) * (a / last));
            channels = channels.map(|c| c * (held / photopic));
            photopic = held;
        }
        let scotopic = math::exp10(-0.4 * secant[band::SCOTOPIC] * a);
        Reddened {
            transmission: std::array::from_fn(|c| channels[c] / unreddened[c]),
            photopic_transmission: photopic,
            scotopic_transmission: scotopic,
            red_green: [red, green],
            sp_ratio: self.sp_ratio * scotopic / photopic,
            camera_band_mag: self.camera_band_mag + (secant[band::CAMERA] - secant[band::V]) * a,
            v_extinction: Magnitudes::new(secant[band::V] * a),
        }
    }

    /// Every band's secant at `a` mag: piecewise linear through the moment at 0 and the nodes,
    /// the moment's below 0 and the last node's beyond it.
    #[must_use]
    fn secants_at(&self, a: f64) -> [f64; band::COUNT] {
        let last = REDDENING_NODE_COUNT;
        let node = |n: usize| {
            if n == 0 {
                0.0
            } else {
                REDDENING_A_V_NODES[n - 1]
            }
        };
        // The interval [node(i), node(i + 1)] holding a, and the fraction along it.
        let i = REDDENING_A_V_NODES
            .partition_point(|&x| x <= a)
            .min(last - 1);
        let f = ((a - node(i)) / (node(i + 1) - node(i))).clamp(0.0, 1.0);
        std::array::from_fn(|b| self.secants[b][i] * (1.0 - f) + self.secants[b][i + 1] * f)
    }

    /// Band `b`'s secant at `a` mag ([`band`]), for the tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn secant(&self, b: usize, a: f64) -> f64 {
        self.secants_at(a)[b]
    }

    /// The photopic light's extinction over A<sub>V</sub> as A<sub>V</sub> → 0, `A_P ÷ A_V`: the
    /// parts' moments weighted by their luminance, for the tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn photopic_ratio_at_zero(&self) -> f64 {
        let p = self.parts;
        let [yr, yg, yb] = LUMINANCE_RGB;
        let weights = [yr, -yr, yg, -yg, yb, -yb];
        let (moment, luminance) = (0..band::PARTS).fold((0.0, 0.0), |(m, l), k| {
            (
                m + weights[k] * p[k] * self.secants[k][0],
                l + weights[k] * p[k],
            )
        });
        moment / luminance
    }

    /// The parts' values at unit luminance, for the tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn parts(&self) -> [f64; band::PARTS] {
        self.parts
    }

    /// These curves behind grey dust: every band dimmed as plan 07's sightline A<sub>V</sub>, as
    /// the band dimmed its light before R06.T9.e, for the tests' unreddened march.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_grey_dust(mut self) -> Self {
        self.secants = [[1.0; REDDENING_NODE_COUNT + 1]; band::COUNT];
        self
    }
}

/// A star's light through dust ([`StarColour::reddened`]): what each band keeps, and the colour,
/// S/P ratio, V extinction and camera band term of what is left.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Reddened {
    transmission: [f64; 3],
    photopic_transmission: f64,
    scotopic_transmission: f64,
    red_green: [f64; 2],
    sp_ratio: f64,
    camera_band_mag: f64,
    v_extinction: Magnitudes,
}

impl Reddened {
    /// The light of a colour of chroma `red_green`, ρ `sp_ratio` and camera term `camera_band_mag`
    /// through no dust: its own values, every transmission one.
    #[must_use]
    const fn unreddened(red_green: [f64; 2], sp_ratio: f64, camera_band_mag: f64) -> Self {
        Self {
            transmission: [1.0; 3],
            photopic_transmission: 1.0,
            scotopic_transmission: 1.0,
            red_green,
            sp_ratio,
            camera_band_mag,
            v_extinction: Magnitudes::ZERO,
        }
    }

    /// The fraction of each linear Rec. 709 channel that passes, red, green and blue: each
    /// channel's signed value through the dust over its value without it, before the lift into
    /// gamut. It is what the band dims each channel of its light by at the solar row's colour,
    /// which is in gamut; a channel that changes sign, as blue does past `A_V` 7–8 for the Sun's
    /// light, passes a negative fraction. For a colour lifted into gamut (the coolest dwarfs) the
    /// ratio of its signed channels is not its lifted colour's; read
    /// [`red_green`](Self::red_green) for that.
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
    /// colour's [`red_green`](StarColour::red_green) are: its channels over their luminance,
    /// lifted into gamut by T3's rule ([`lift_into_gamut`]).
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

    /// The camera band term of the light that passes, mag, relative to its V after its own V
    /// extinction ([`v_extinction`](Self::v_extinction)), as [`StarColour::camera_band_mag`] is to
    /// V.
    #[must_use]
    pub const fn camera_band_mag(&self) -> f64 {
        self.camera_band_mag
    }

    /// The extinction of the star's own V band, v★(A) A, mag: about 1.004 A<sub>V</sub> for the
    /// Sun's light as A<sub>V</sub> → 0 and 0.977 A<sub>V</sub> at 10, since the sightline's
    /// A<sub>V</sub> is the law at 0.549 µm (`decision-r06-t9b-band.md`, addendum item 4). A
    /// star's V is M<sub>V</sub> + DM plus this, which the census cuts from R06.T8.k.
    #[must_use]
    pub const fn v_extinction(&self) -> Magnitudes {
        self.v_extinction
    }
}

/// Linear Rec. 709 `rgb` of unit luminance, brought into gamut by the colour table's own rule
/// (R06.T3.a, `hyperion-fit`'s `Observer::unit_rgb`): mixed with white of the same luminance (r =
/// g = b = 1) by the least amount that lifts its lowest channel to zero, Walker's (1996) rule of
/// adding white, so the hue is kept and no channel is clipped on its own. A colour in gamut is
/// returned as it is. The fit holds the two to the same bits on every row of the table.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::colour::lift_into_gamut;
///
/// // A deep red of unit luminance with a negative blue: lifted towards white until blue is zero.
/// let [r, g, b] = lift_into_gamut([2.5, 0.8, -1.0]);
/// assert!(b.abs() < 1e-12 && r > g && g > b);
/// // In gamut, untouched.
/// assert_eq!(lift_into_gamut([1.2, 0.9, 1.0]), [1.2, 0.9, 1.0]);
/// ```
#[must_use]
pub fn lift_into_gamut(rgb: [f64; 3]) -> [f64; 3] {
    let lowest = rgb[0].min(rgb[1]).min(rgb[2]);
    if lowest >= 0.0 {
        rgb
    } else {
        let t = 1.0 / (1.0 - lowest);
        rgb.map(|c| 1.0 + t * (c - 1.0))
    }
}

/// The colour of the Sun's light: the table at [`SUN_TEFF_K`] and [`SUN_LOG_G`], the point of
/// [`CAMERA_ETA_SUN`], whose reddening curves dim every node of the band (R06.T9.e).
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
/// # Panics
///
/// Never: each grid has fewer than 2¹⁶ rows, which a constant assertion holds, so its node indices
/// fit the colour's `u16`s.
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
    type Rows<'a> = (&'a [f64], &'a [f64], &'a [[f64; 8]], &'a [Bake], Grid);
    let (log_teff_nodes, log_g_nodes, rows, bakes, table): Rows<'static> = match grid {
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => (
            &NORMAL_LOG_TEFF,
            &NORMAL_LOG_G,
            &NORMAL,
            &NORMAL_BAKE,
            Grid::Normal,
        ),
        AtmosphereGrid::WhiteDwarf => (
            &WHITE_DWARF_LOG_TEFF,
            &WHITE_DWARF_LOG_G,
            &WHITE_DWARF,
            &WHITE_DWARF_BAKE,
            Grid::WhiteDwarf,
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
    let node = |i: usize| u16::try_from(i).expect("a grid has fewer than 2¹⁶ nodes (asserted)");
    StarColour {
        red_green,
        lux_per_v0: mix(column::LUX_PER_V0),
        sp_ratio: mix(column::SP_RATIO),
        camera_band_mag: mix(column::CAMERA_BAND_MAG),
        extinction_ratio: column::EXTINCTION.map(mix),
        bake_spectrum,
        table: TablePoint {
            grid: table,
            teff_node: node(ti),
            log_g_node: node(gi),
            along_teff: tf,
            along_log_g: gf,
        },
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

    /// The blue of a reddened colour of unit luminance.
    fn blue_of(r: &Reddened) -> f64 {
        let [yr, yg, yb] = LUMINANCE_RGB;
        let [red, green] = r.red_green();
        (1.0 - yr * red - yg * green) / yb
    }

    /// Every node of both grids: its grid, its row of the tables, and its colour read back.
    fn node_colours() -> Vec<(Grid, usize, StarColour)> {
        let mut out = Vec::new();
        for (grid, log_teff, log_g, atmosphere) in [
            (
                Grid::Normal,
                &NORMAL_LOG_TEFF[..],
                &NORMAL_LOG_G[..],
                AtmosphereGrid::Giant,
            ),
            (
                Grid::WhiteDwarf,
                &WHITE_DWARF_LOG_TEFF[..],
                &WHITE_DWARF_LOG_G[..],
                AtmosphereGrid::WhiteDwarf,
            ),
        ] {
            for (i, &t) in log_teff.iter().enumerate() {
                for (j, &g) in log_g.iter().enumerate() {
                    out.push((
                        grid,
                        i * log_g.len() + j,
                        colour(math::exp10(t), g, atmosphere),
                    ));
                }
            }
        }
        out
    }

    /// The reddening tables' columns of row `row` of `grid`: the parts' values and, per band, the
    /// moment and the secant at each node.
    fn table_columns(grid: Grid, row: usize) -> ([f64; 6], [[f64; 7]; 9]) {
        let (zero, nodes): (&[[f64; 15]], [&[[f64; 18]]; 3]) = match grid {
            Grid::Normal => (
                &NORMAL_REDDENING,
                [
                    &av02_05::NORMAL_SECANTS,
                    &av10_15::NORMAL_SECANTS,
                    &av20_30::NORMAL_SECANTS,
                ],
            ),
            Grid::WhiteDwarf => (
                &WHITE_DWARF_REDDENING,
                [
                    &av02_05::WHITE_DWARF_SECANTS,
                    &av10_15::WHITE_DWARF_SECANTS,
                    &av20_30::WHITE_DWARF_SECANTS,
                ],
            ),
        };
        let parts = std::array::from_fn(|p| zero[row][p]);
        let secants = std::array::from_fn(|b| {
            std::array::from_fn(|n| match n {
                0 => zero[row][6 + b],
                _ => nodes[(n - 1) / 2][row][((n - 1) % 2) * 9 + b],
            })
        });
        (parts, secants)
    }

    #[test]
    fn a_node_is_read_back_exactly() {
        for (grid, row, c) in node_colours() {
            let table = match grid {
                Grid::Normal => NORMAL[row],
                Grid::WhiteDwarf => WHITE_DWARF[row],
            };
            let (lux, rho) = (table[column::LUX_PER_V0], table[column::SP_RATIO]);
            assert!((c.lux_per_v0() - lux).abs() < 1e-9 * lux);
            assert!((c.sp_ratio() - rho).abs() < 1e-9 * rho);
            let reddening = c.reddening();
            let (parts, secants) = table_columns(grid, row);
            for (got, want) in reddening.parts().into_iter().zip(parts) {
                assert!((got - want).abs() < 1e-9 * want.abs().max(1e-6));
            }
            for (b, curve) in secants.iter().enumerate() {
                assert!((reddening.secant(b, 0.0) - curve[0]).abs() < 1e-9 * curve[0].abs());
            }
        }
    }

    /// Test 1 of the addendum: at each node `reddened` reproduces the node's columns, on every
    /// row of both grids, to their digits (the rows' own values read back, to 10⁻⁹ relative):
    /// each band's secant, the scotopic transmission it gives, the V extinction, the camera term
    /// and the photopic light the parts' luminance.
    #[test]
    fn at_each_node_reddened_reproduces_the_nodes_columns() {
        let [yr, yg, yb] = LUMINANCE_RGB;
        for (grid, row, c) in node_colours() {
            let reddening = c.reddening();
            let (parts, secants) = table_columns(grid, row);
            for (n, &a) in REDDENING_A_V_NODES.iter().enumerate() {
                let column = |b: usize| secants[b][n + 1];
                for b in 0..band::COUNT {
                    let got = reddening.secant(b, a);
                    assert!(
                        (got - column(b)).abs() <= 1e-9 * column(b).abs(),
                        "{grid:?} row {row}, band {b} at A_V {a}: {got} against {}",
                        column(b)
                    );
                }
                let r = c.reddened(Magnitudes::new(a));
                let close = |got: f64, want: f64| (got - want).abs() <= 1e-9 * want.abs();
                assert!(close(
                    r.scotopic_transmission(),
                    math::exp10(-0.4 * column(band::SCOTOPIC) * a)
                ));
                assert!(close(r.v_extinction().value(), column(band::V) * a));
                assert!(
                    (r.camera_band_mag()
                        - (c.camera_band_mag() + (column(band::CAMERA) - column(band::V)) * a))
                        .abs()
                        < 1e-9
                );
                let channel = |k: usize| {
                    parts[2 * k] * math::exp10(-0.4 * column(2 * k) * a)
                        - parts[2 * k + 1] * math::exp10(-0.4 * column(2 * k + 1) * a)
                };
                let luminance = |c: [f64; 3]| yr * c[0] + yg * c[1] + yb * c[2];
                let photopic = luminance([channel(0), channel(1), channel(2)])
                    / luminance([
                        parts[0] - parts[1],
                        parts[2] - parts[3],
                        parts[4] - parts[5],
                    ]);
                assert!(
                    close(r.photopic_transmission(), photopic),
                    "{grid:?} row {row} at A_V {a}"
                );
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

    /// `reddened` at no dust, or a rounding below it, is the colour: every transmission one, the
    /// V extinction zero, and the chroma, ρ and camera term the colour's own, bit for bit.
    #[test]
    fn reddened_by_no_dust_is_the_colour() {
        for c in sampled_colours() {
            for a_v in [
                Magnitudes::ZERO,
                Magnitudes::new(-0.0),
                Magnitudes::new(-1e-9),
            ] {
                let r = c.reddened(a_v);
                assert_eq!(r.transmission().map(bits), [bits(1.0); 3], "{c:?}");
                assert_eq!(bits(r.photopic_transmission()), bits(1.0), "{c:?}");
                assert_eq!(bits(r.scotopic_transmission()), bits(1.0), "{c:?}");
                assert_eq!(bits(r.v_extinction().value()), bits(0.0), "{c:?}");
                assert_eq!(r.red_green().map(bits), c.red_green().map(bits), "{c:?}");
                assert_eq!(bits(r.sp_ratio()), bits(c.sp_ratio()), "{c:?}");
                assert_eq!(
                    bits(r.camera_band_mag()),
                    bits(c.camera_band_mag()),
                    "{c:?}"
                );
                assert_eq!(r, c.reddening().through(a_v), "{c:?}");
            }
        }
    }

    /// Test 4 of the addendum, the sim's half: the lift of each row's parts at unit luminance,
    /// c⁺(0) − c⁻(0), is the colour table's chroma within 10⁻⁶, on every row of both grids (the
    /// fit holds the lift to `unit_rgb` bit for bit), so the reddened colour leaves the colour
    /// continuously.
    #[test]
    fn the_lift_of_each_rows_parts_is_its_chroma() {
        let mut worst: f64 = 0.0;
        for (grid, row, _) in node_colours() {
            let (parts, _) = table_columns(grid, row);
            let table = match grid {
                Grid::Normal => NORMAL[row],
                Grid::WhiteDwarf => WHITE_DWARF[row],
            };
            let lifted = lift_into_gamut([
                parts[0] - parts[1],
                parts[2] - parts[3],
                parts[4] - parts[5],
            ]);
            for k in [column::R, column::G] {
                let d = (lifted[k] - table[k]).abs();
                worst = worst.max(d);
                assert!(d < 1e-6, "{grid:?} row {row}: {lifted:?} against {table:?}");
            }
        }
        eprintln!("the lift of the parts against the chroma: within {worst:.2e}");
    }

    /// The lift is T3's: a colour in gamut is untouched, and one out of it is mixed with white at
    /// its luminance until its lowest channel is zero, every channel moved by the same factor.
    #[test]
    fn the_lift_mixes_with_white_at_the_same_luminance() {
        let [yr, yg, yb] = LUMINANCE_RGB;
        let inside = [1.3, 0.9, 0.7];
        assert_eq!(lift_into_gamut(inside).map(bits), inside.map(bits));
        let outside = [2.0, 0.95, (1.0 - yr * 2.0 - yg * 0.95) / yb];
        assert!(outside[2] < 0.0);
        let lifted = lift_into_gamut(outside);
        assert!(lifted[2].abs() < 1e-12, "{lifted:?}");
        let y = yr * lifted[0] + yg * lifted[1] + yb * lifted[2];
        assert!((y - 1.0).abs() < 1e-12, "{y}");
        let t: Vec<f64> = (0..3)
            .map(|i| (lifted[i] - 1.0) / (outside[i] - 1.0))
            .collect();
        assert!((t[0] - t[1]).abs() < 1e-12 && (t[1] - t[2]).abs() < 1e-12);
    }

    /// Dust takes blue light first and red light last, and the scotopic light, bluer than the
    /// photopic, before it; so what passes is dimmer, in gamut at unit luminance, and of lower ρ
    /// at every dust, and redder to `A_V` 12 (red over green rising, blue over green falling or
    /// held at a lifted zero). Beyond, the light is the spectrum's far red, which T3's lift takes
    /// to green zero with a little blue, as it does monochromatic red. The channels' own
    /// transmissions order blue, green, red for the solar row the band reads, to the node at 5.
    /// (The camera term need not fall: a hot star's light is bluer under silicon than under V.)
    #[test]
    fn dust_reddens_the_light_and_lowers_its_rho() {
        let [yr, yg, yb] = LUMINANCE_RGB;
        for c in sampled_colours().into_iter().skip(1) {
            let mut previous = c.reddened(Magnitudes::ZERO);
            for a_v in [0.1, 1.0, 2.0, 5.0, 8.0, 12.0, 17.0, 25.0, 30.0, 40.0] {
                let r = c.reddened(Magnitudes::new(a_v));
                assert!(
                    r.scotopic_transmission() < r.photopic_transmission()
                        && r.photopic_transmission() < previous.photopic_transmission(),
                    "{c:?} at {a_v}: {r:?}"
                );
                let [red_now, green_now] = r.red_green();
                let [red_then, green_then] = previous.red_green();
                if a_v <= 12.0 {
                    assert!(
                        red_now / green_now > red_then / green_then,
                        "{c:?} at {a_v}"
                    );
                    assert!(blue_of(&r) / green_now <= blue_of(&previous) / green_then + 1e-12);
                }
                let y = yr * red_now + yg * green_now + yb * blue_of(&r);
                assert!((y - 1.0).abs() < 1e-12, "{c:?} at {a_v}: {y}");
                assert!(
                    [red_now, green_now, blue_of(&r)]
                        .iter()
                        .all(|&x| x >= -1e-12),
                    "{c:?} at {a_v}"
                );
                assert!(r.sp_ratio() < previous.sp_ratio(), "{c:?} at {a_v}");
                previous = r;
            }
        }
        let sun = solar_colour();
        for a_v in [0.1, 1.0, 2.0, 5.0] {
            let [red, green, blue] = sun.reddened(Magnitudes::new(a_v)).transmission();
            assert!(
                blue < green && green < red && red < 1.0,
                "the Sun at {a_v}: {red} {green} {blue}"
            );
        }
    }

    /// Test 6 of the addendum: every band's transmission falls with the dust on every row of
    /// both grids, between the nodes as at them and beyond the last.
    #[test]
    fn every_bands_transmission_falls_with_the_dust() {
        for (grid, row, c) in node_colours() {
            let reddening = c.reddening();
            for b in 0..band::COUNT {
                let mut depth = 0.0;
                for step in 1..=800 {
                    let a = f64::from(step) * 0.05;
                    let now = reddening.secant(b, a) * a;
                    assert!(
                        now > depth,
                        "{grid:?} row {row}, band {b}: the optical depth {now} at A_V {a} against \
                         {depth}"
                    );
                    depth = now;
                }
            }
        }
    }

    /// What passes keeps a photopic light that falls with the dust, and a finite colour and ρ, on
    /// every row of both grids from `A_V` 0.5 to 100: the parts' luminance stays positive to the
    /// last node, and beyond it the photopic light is held as one band.
    #[test]
    fn the_light_that_passes_keeps_a_colour_at_every_dust() {
        for (grid, row, c) in node_colours() {
            let reddening = c.reddening();
            let mut previous = 1.0;
            for step in 1..=200 {
                let a = f64::from(step) * 0.5;
                let r = reddening.through(Magnitudes::new(a));
                assert!(
                    r.photopic_transmission() > 0.0
                        && r.photopic_transmission() < previous
                        && r.red_green().iter().all(|v| v.is_finite())
                        && r.sp_ratio() > 0.0,
                    "{grid:?} row {row} at A_V {a}: {r:?}"
                );
                previous = r.photopic_transmission();
            }
        }
    }

    /// Test 7 of the addendum: beyond 30 each band keeps the 30 node's secant. The photopic light
    /// keeps its own, the parts' at 30, and the colour of unit luminance is the 30 node's.
    #[test]
    fn beyond_thirty_the_last_nodes_secants_hold() {
        for c in sampled_colours() {
            let reddening = c.reddening();
            for b in 0..band::COUNT {
                assert_eq!(
                    bits(reddening.secant(b, 40.0)),
                    bits(reddening.secant(b, 30.0)),
                    "band {b}"
                );
            }
            let (at_30, at_40) = (
                c.reddened(Magnitudes::new(30.0)),
                c.reddened(Magnitudes::new(40.0)),
            );
            assert_eq!(
                bits(at_40.v_extinction().value()),
                bits(reddening.secant(band::V, 30.0) * 40.0)
            );
            assert_eq!(
                bits(at_40.scotopic_transmission()),
                bits(math::exp10(
                    -0.4 * reddening.secant(band::SCOTOPIC, 30.0) * 40.0
                ))
            );
            assert_eq!(at_40.red_green().map(bits), at_30.red_green().map(bits));
            let photopic_secant =
                |r: &Reddened, a: f64| -2.5 * math::log10(r.photopic_transmission()) / a;
            assert!(
                (photopic_secant(&at_40, 40.0) - photopic_secant(&at_30, 30.0)).abs() < 1e-12,
                "{c:?}"
            );
        }
    }

    /// The node files say which nodes they hold, and they are the sim's.
    #[test]
    fn the_node_files_hold_the_sims_nodes() {
        let held: Vec<f64> = [av02_05::A_V_NODES, av10_15::A_V_NODES, av20_30::A_V_NODES].concat();
        assert_eq!(
            held.iter().map(|&a| bits(a)).collect::<Vec<_>>(),
            REDDENING_A_V_NODES.map(bits)
        );
    }

    /// Test 8 of the addendum: the solar row's ratios lie in the ruling's ranges, as
    /// `A_V` → 0 (`decision-r06-t9b-band.md`, item 3 as corrected by the addendum, items 3 and 4):
    /// `A_S ÷ A_V − A_P ÷ A_V` 0.11–0.15, `A_P ÷ A_V` 0.97–1.01, `A_cam ÷ A_V` 0.84–0.92 and V's own
    /// 0.99–1.02.
    #[test]
    fn the_solar_rows_ratios_are_the_rulings() {
        let sun = solar_colour();
        assert_eq!(sun, colour(5_772.0, 4.438, AtmosphereGrid::MainSequence));
        let reddening = sun.reddening();
        let photopic = reddening.photopic_ratio_at_zero();
        let scotopic = reddening.secant(band::SCOTOPIC, 0.0);
        let camera = reddening.secant(band::CAMERA, 0.0);
        let v = reddening.secant(band::V, 0.0);
        eprintln!(
            "the solar row as A_V → 0: A_P ÷ A_V {photopic:.4}, A_S ÷ A_V {scotopic:.4}, \
             A_cam ÷ A_V {camera:.4}, V's own {v:.4}"
        );
        assert!((0.97..=1.01).contains(&photopic), "A_P ÷ A_V {photopic}");
        assert!(
            (0.11..=0.15).contains(&(scotopic - photopic)),
            "A_S ÷ A_V {scotopic} less A_P ÷ A_V {photopic}"
        );
        assert!((0.84..=0.92).contains(&camera), "A_cam ÷ A_V {camera}");
        assert!((0.99..=1.02).contains(&v), "V's {v}");
        // The dust takes the blue first, so what is left sits redder, where silicon sees less of
        // it.
        assert!(reddening.secant(band::CAMERA, 2.0) < camera);
    }

    /// Test 3 of the addendum: the Sun's light behind a magnitude of dust has blue 0.689 ± 0.003 at
    /// unit luminance, and behind two 0.500 ± 0.005: the direct integrals of the solar row's own
    /// spectrum, 0.6893 and 0.5003 (the fit's between-the-nodes test), where T3.c's ratios gave 0.725.
    /// The addendum's 0.685 and 0.496 are the 5,750 K, log g 4.5 ATLAS9 node's alone (0.6847 and
    /// 0.4967 from the tables), 0.004 below these (R06's Risks, "Deviations in T9.e, as built"); a
    /// 5,772 K blackbody gives 0.669 and 0.482.
    #[test]
    fn the_suns_blue_behind_dust_is_its_reddened_spectrums() {
        let sun = solar_colour();
        let one = blue_of(&sun.reddened(Magnitudes::new(1.0)));
        let two = blue_of(&sun.reddened(Magnitudes::new(2.0)));
        eprintln!("the Sun's blue at unit luminance: {one:.4} at A_V 1, {two:.4} at 2");
        assert!((one - 0.689).abs() <= 0.003, "at A_V 1: {one}");
        assert!((two - 0.500).abs() <= 0.005, "at A_V 2: {two}");
        // T3.c's ratios, each channel dimmed at its positive lobe's effective wavelength, fall
        // short of the excess by some 19%: 0.725.
        let [yr, yg, yb] = LUMINANCE_RGB;
        let t = sun.extinction_ratio().map(|k| math::exp10(-0.4 * k));
        let [red, green] = sun.red_green();
        let luminance = yr * red * t[0] + yg * green * t[1] + yb * sun.blue() * t[2];
        let ratios = sun.blue() * t[2] / luminance;
        assert!((ratios - 0.725).abs() < 0.005, "T3.c's ratios: {ratios}");
    }

    /// At `A_V` 2 the Sun's eye colour offset (against a dark, scotopic sky) falls by about 0.27
    /// mag, 2.5 log₁₀(t<sub>S</sub> ÷ t<sub>P</sub>), and its camera term by about 0.36, (k<sub>cam</sub>
    /// − k<sub>V</sub>) A<sub>V</sub> (`decision-r06-t9b-band.md`, item 3 as corrected); an M
    /// dwarf's camera term by more.
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
        let expected =
            2.5 * math::log10(behind.scotopic_transmission() / behind.photopic_transmission());
        assert!((eye - expected).abs() < 1e-9, "{eye} against {expected}");
        assert!((eye + 0.27).abs() < 0.03, "the eye offset moves by {eye}");
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
        assert_ne!(wd.reddening(), dwarf.reddening());
    }

    #[test]
    fn surface_gravity_is_g_m_over_r_squared() {
        let g = surface_gravity(SolarMasses::new(0.6), SolarRadii::new(0.0125));
        // 0.6 M☉ in 0.0125 R☉, a typical white dwarf: 10^8.022 cm s⁻².
        assert!((g - 8.0224).abs() < 0.001, "{g}");
    }
}
