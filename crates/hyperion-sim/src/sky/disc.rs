//! The discs of the observer's own stars (rendering plan R06, Design note 16): their limb darkening,
//! read from the fitted table [`tables::limb_darkening`].
//!
//! The power-2 law, I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Hestroffer 1997, A&A 327, 199; Maxted 2018, A&A
//! 616, A39), with c and α in Johnson B, V and R for the display's blue, green and red, from
//! Claret and Southworth (2022, 2023) for stars and Claret et al. (2020) for white dwarfs. The
//! table is interpolated bilinearly in log₁₀ `T_eff` and log₁₀ g within the grid the star's kind
//! selects and clamped at its edges: at 50,000 K for O stars, at the least gravity tabulated for
//! hot giants, at 2,300 K below, and at 100,000 K for white dwarfs.
//!
//! [`tables::limb_darkening`]: crate::tables::limb_darkening

use crate::galaxy::Galaxy;
use crate::math;
use crate::sky::colour::{AtmosphereGrid, StarColour, star_colour};
use crate::stellar::multiplicity::StarIndex;
use crate::stellar::photometry::{
    absolute_bolometric_magnitude, absolute_magnitude_v, bolometric_correction_v,
};
use crate::stellar::system::SystemStars;
use crate::stellar::{Phase, StarState};
use crate::tables::limb_darkening::{
    NORMAL, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF, WHITE_DWARF_LOG_G, WHITE_DWARF_LOG_TEFF,
};
use crate::tables::star_colour::LUMINANCE_RGB;
use crate::time::UniverseTime;
use crate::units::consts::{METRES_PER_PARSEC, SOLAR_RADIUS_M};
use crate::units::{CandelasPerSquareMetre, Kelvin, Magnitudes, Metres, Radians};

/// One row of the fitted limb-darkening table: c and α in B, V and R.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct LimbRow {
    /// c in B.
    pub c_b: f64,
    /// α in B.
    pub alpha_b: f64,
    /// c in V.
    pub c_v: f64,
    /// α in V.
    pub alpha_v: f64,
    /// c in R.
    pub c_r: f64,
    /// α in R.
    pub alpha_r: f64,
}

/// The power-2 limb-darkening law of one band: I(μ) ÷ I(1) = 1 − c (1 − μ^α).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct PowerTwo {
    c: f64,
    alpha: f64,
}

impl PowerTwo {
    /// The law with coefficients `c` and `alpha` (the catalogues' g and h).
    #[must_use]
    pub const fn new(c: f64, alpha: f64) -> Self {
        Self { c, alpha }
    }

    /// The coefficient c: one less the limb's intensity at μ = 0, relative to the centre's.
    #[must_use]
    pub const fn c(&self) -> f64 {
        self.c
    }

    /// The exponent α.
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// The intensity at `mu` (the cosine of the angle from the surface normal, clamped to 0–1)
    /// relative to the centre's.
    #[must_use]
    pub fn intensity(&self, mu: f64) -> f64 {
        1.0 - self.c * (1.0 - math::powf(mu.clamp(0.0, 1.0), self.alpha))
    }

    /// The disc's mean intensity relative to the centre's, ∫ I(μ) 2μ dμ ÷ I(1) = 1 − c α ÷
    /// (α + 2): 0.799 for the Sun in V.
    #[must_use]
    pub fn disc_average(&self) -> f64 {
        1.0 - self.c * self.alpha / (self.alpha + 2.0)
    }
}

/// The power-2 laws of a star of `teff` and `log_g` (log₁₀ g, cgs) in B, V and R, for the
/// display's blue, green and red, interpolated bilinearly in log₁₀ `T_eff` and log₁₀ g within
/// `grid` and clamped at its edges.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::colour::AtmosphereGrid;
/// use hyperion_sim::sky::disc::limb_coefficients;
/// use hyperion_sim::units::Kelvin;
///
/// // The Sun's disc in V keeps about 80% of its central intensity on average.
/// let [_, v, _] = limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
/// assert!((v.disc_average() - 0.799).abs() < 0.005);
/// // The limb is darker in blue than in red.
/// let [b, _, r] = limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
/// assert!(b.intensity(0.1) < r.intensity(0.1));
/// ```
#[must_use]
pub fn limb_coefficients(teff: Kelvin, log_g: f64, grid: AtmosphereGrid) -> [PowerTwo; 3] {
    let (log_teff_nodes, log_g_nodes, rows): (&[f64], &[f64], &[LimbRow]) = match grid {
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => {
            (&NORMAL_LOG_TEFF, &NORMAL_LOG_G, &NORMAL)
        }
        AtmosphereGrid::WhiteDwarf => (&WHITE_DWARF_LOG_TEFF, &WHITE_DWARF_LOG_G, &WHITE_DWARF),
    };
    let (ti, tf) = bracket(log_teff_nodes, math::log10(teff.value()));
    let (gi, gf) = bracket(log_g_nodes, log_g);
    let width = log_g_nodes.len();
    let at = |a: usize, b: usize| rows[a * width + b];
    let mix = |field: fn(&LimbRow) -> f64| {
        let low = field(&at(ti, gi)) * (1.0 - gf) + field(&at(ti, gi + 1)) * gf;
        let high = field(&at(ti + 1, gi)) * (1.0 - gf) + field(&at(ti + 1, gi + 1)) * gf;
        low * (1.0 - tf) + high * tf
    };
    [
        PowerTwo::new(mix(|r| r.c_b), mix(|r| r.alpha_b)),
        PowerTwo::new(mix(|r| r.c_v), mix(|r| r.alpha_v)),
        PowerTwo::new(mix(|r| r.c_r), mix(|r| r.alpha_r)),
    ]
}

/// The illuminance of a star of V = 0 outside an atmosphere, lx: Allen's 2.54 µlx (Allen 1973,
/// *Astrophysical Quantities*, 3rd ed., p. 197; Crumey 2014, MNRAS 442, 2600, §1.3), the zero point
/// the colour table's `lux_per_v0` is a ratio to.
pub const V0_ILLUMINANCE_LX: f64 = 2.54e-6;

/// One of the observer's own stars as a disc (Design note 16): its size, its surface brightness per
/// display channel and its limb darkening, in the order B, V, R for the display's b, g, r.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HostDisc {
    star: StarIndex,
    radius: Metres,
    teff: Kelvin,
    log_g: f64,
    mean_luminance: [CandelasPerSquareMetre; 3],
    central_luminance: [CandelasPerSquareMetre; 3],
    limb: [PowerTwo; 3],
    colour: StarColour,
}

impl HostDisc {
    /// The star's index in its system.
    #[must_use]
    pub const fn star(&self) -> StarIndex {
        self.star
    }

    /// The star's radius.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// The star's effective temperature.
    #[must_use]
    pub const fn teff(&self) -> Kelvin {
        self.teff
    }

    /// The star's surface gravity, log₁₀ g in cm s⁻².
    #[must_use]
    pub const fn log_g(&self) -> f64 {
        self.log_g
    }

    /// The disc's mean luminance per channel at the surface, cd m⁻², B, V, R: the photopic mean
    /// L̄ times the star's linear Rec. 709 blue, green and red at unit luminance, so that the
    /// channels' Rec. 709 luminance is L̄ and the illuminance at distance d is π L̄ (R ÷ d)².
    #[must_use]
    pub const fn mean_luminance(&self) -> [CandelasPerSquareMetre; 3] {
        self.mean_luminance
    }

    /// The disc's central luminance per channel, cd m⁻², B, V, R: the mean over each channel's
    /// disc average, I(1) = L̄ ÷ (1 − c α ÷ (α + 2)).
    #[must_use]
    pub const fn central_luminance(&self) -> [CandelasPerSquareMetre; 3] {
        self.central_luminance
    }

    /// The limb-darkening laws, B, V, R.
    #[must_use]
    pub const fn limb(&self) -> [PowerTwo; 3] {
        self.limb
    }

    /// The star's colour row.
    #[must_use]
    pub const fn colour(&self) -> StarColour {
        self.colour
    }
}

/// The grid a star's tables are read from: white dwarfs their own, stars before or on the main
/// sequence the dwarfs', every other living star the giants'.
fn grid_of(phase: Phase) -> AtmosphereGrid {
    match phase {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            AtmosphereGrid::WhiteDwarf
        }
        Phase::Protostar | Phase::PreMainSequence | Phase::MainSequence => {
            AtmosphereGrid::MainSequence
        }
        Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => AtmosphereGrid::Giant,
    }
}

/// A star's absolute V magnitude for its disc: plan 06's for a living star; for a white dwarf,
/// which plan 06 leaves without one until ask A4, the bolometric magnitude less the dwarfs'
/// correction at its temperature (`photometry`'s own caution: up to about 0.6 mag off at 4,000 K).
fn disc_absolute_v(state: &StarState) -> Option<Magnitudes> {
    match grid_of(state.phase()) {
        AtmosphereGrid::WhiteDwarf => Some(
            absolute_bolometric_magnitude(state.luminosity())?
                - bolometric_correction_v(state.effective_temperature())?,
        ),
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => absolute_magnitude_v(state),
    }
}

/// The disc of each of `stars`' stars that shines at time `t` (Design note 16): its radius,
/// `T_eff` and log g from plan 06's state ([`SystemStars::state_at`], the pair-evolved state), its
/// colour, the photopic mean luminance per channel from its V flux and radius, and the central
/// luminance from the mean and each channel's disc average.
///
/// A star with no V magnitude (a neutron star, a black hole, a merged-away star, a substellar
/// object, or a living star outside the corrections' table) has no disc and is left out. The
/// galaxy is not read today; it is the argument the census's discs will need.
///
/// The photopic mean follows from flux conservation, E = π L̄ (R ÷ d)² for a sphere: a star of
/// absolute V `M` lights 2.54 µlx × `lux_per_v0` × 10^(−0.4 M) at 10 pc, so L̄ = that × (10 pc)² ÷
/// (π R²).
#[must_use]
pub fn host_discs(_galaxy: &Galaxy, stars: &SystemStars, t: UniverseTime) -> Vec<HostDisc> {
    let Some(state) = stars.state_at(t) else {
        return Vec::new();
    };
    let ten_parsecs = 10.0 * METRES_PER_PARSEC;
    state
        .stars()
        .iter()
        .enumerate()
        .filter_map(|(i, star)| {
            let m_v = disc_absolute_v(star)?;
            let log_g = star.surface_gravity()?.value();
            let radius = star.radius().value() * SOLAR_RADIUS_M;
            let teff = star.effective_temperature();
            let grid = grid_of(star.phase());
            let colour = star_colour(teff, log_g, grid);
            let limb = limb_coefficients(teff, log_g, grid);
            let at_ten_parsecs =
                V0_ILLUMINANCE_LX * colour.lux_per_v0() * math::exp10(-0.4 * m_v.value());
            let mean = at_ten_parsecs * ten_parsecs * ten_parsecs
                / (std::f64::consts::PI * radius * radius);
            let [r, g] = colour.chroma();
            let channels = [colour.blue(), f64::from(g), f64::from(r)];
            let mean_luminance = channels.map(|c| CandelasPerSquareMetre::new(mean * c));
            let central_luminance = std::array::from_fn(|c| {
                CandelasPerSquareMetre::new(mean_luminance[c].value() / limb[c].disc_average())
            });
            Some(HostDisc {
                star: StarIndex::from_body(u8::try_from(i).ok()?)?,
                radius: Metres::new(radius),
                teff,
                log_g,
                mean_luminance,
                central_luminance,
                limb,
                colour,
            })
        })
        .collect()
}

/// The angular radius of a sphere of `radius` at `distance` from its centre, asin(R ÷ d); a
/// quarter turn from inside the sphere.
#[must_use]
pub fn angular_radius(radius: Metres, distance: Metres) -> Radians {
    Radians::new(math::asin((radius.value() / distance.value()).min(1.0)))
}

/// The Rec. 709 luminance of a disc's channels, B, V, R, cd m⁻².
#[must_use]
pub fn channel_luminance(channels: [CandelasPerSquareMetre; 3]) -> f64 {
    let [yr, yg, yb] = LUMINANCE_RGB;
    yb * channels[0].value() + yg * channels[1].value() + yr * channels[2].value()
}

/// The interval of rising `nodes` that holds `x` and the fraction along it, clamped to the first
/// or last interval's end; a NaN takes the first node.
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 2;
    let i = nodes
        .partition_point(|&node| node <= x)
        .saturating_sub(1)
        .min(last);
    let t = (x - nodes[i]) / (nodes[i + 1] - nodes[i]);
    (i, if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun() -> [PowerTwo; 3] {
        limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence)
    }

    /// The solar row in V: c = 0.7837 and α = 0.6893 from Claret and Southworth's (2022) ATLAS
    /// table at 5,772 K and log g 4.44, and a disc average of 0.799 (Design note 16).
    #[test]
    fn the_solar_row_is_claret_and_southworths() {
        let [_, v, _] = sun();
        assert!((v.c() - 0.784).abs() < 0.005, "c {}", v.c());
        assert!((v.alpha() - 0.689).abs() < 0.005, "α {}", v.alpha());
        assert!(
            (v.disc_average() - 0.799).abs() < 0.005,
            "{}",
            v.disc_average()
        );
        assert!((v.intensity(1.0) - 1.0).abs() < 1e-12);
        assert!((v.intensity(0.0) - (1.0 - v.c())).abs() < 1e-12);
    }

    /// At μ = 0.1 the Sun's law in V lies within 0.015 of the observed quadratic fit at 5,522 Å,
    /// I(μ) ÷ I(1) = 0.29462 + 0.98032 μ − 0.27494 μ², which gives 0.390 (Pierce and Slaughter
    /// 1977, Solar Phys. 51, 25, Table III, eq. 10; the source of Cox 2000's polynomial in
    /// *Allen's Astrophysical Quantities*, 4th ed., §14.7). The quadratic overestimates the limb by
    /// about 0.01 against their fifth-degree fits and Neckel and Labs 1994's (0.382 at 550 nm).
    #[test]
    fn the_solar_limb_follows_the_observed_polynomial() {
        let [_, v, _] = sun();
        let mu: f64 = 0.1;
        let observed = 0.294_62 + 0.980_32 * mu - 0.274_94 * mu * mu;
        assert!(
            (v.intensity(mu) - observed).abs() < 0.015,
            "{}",
            v.intensity(mu)
        );
    }

    #[test]
    fn every_clamp_returns_a_finite_row() {
        for (teff, log_g, grid) in [
            (1_000.0, 5.0, AtmosphereGrid::MainSequence),
            (90_000.0, 4.0, AtmosphereGrid::MainSequence),
            (45_000.0, 0.0, AtmosphereGrid::Giant),
            (3_000.0, 7.0, AtmosphereGrid::MainSequence),
            (200_000.0, 8.0, AtmosphereGrid::WhiteDwarf),
            (2_000.0, 10.0, AtmosphereGrid::WhiteDwarf),
            (f64::NAN, f64::NAN, AtmosphereGrid::Giant),
        ] {
            for law in limb_coefficients(Kelvin::new(teff), log_g, grid) {
                assert!(
                    law.c().is_finite() && law.alpha().is_finite(),
                    "{teff} {log_g}"
                );
                assert!((0.0..=1.0).contains(&law.disc_average()), "{teff} {log_g}");
            }
        }
        // Beyond the edge is the edge.
        assert_eq!(
            limb_coefficients(Kelvin::new(90_000.0), 4.5, AtmosphereGrid::MainSequence),
            limb_coefficients(Kelvin::new(50_000.0), 4.5, AtmosphereGrid::MainSequence)
        );
        assert_eq!(
            limb_coefficients(Kelvin::new(200_000.0), 8.0, AtmosphereGrid::WhiteDwarf),
            limb_coefficients(Kelvin::new(100_000.0), 8.0, AtmosphereGrid::WhiteDwarf)
        );
    }

    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::units::consts::METRES_PER_AU;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x0600_d15c_0000_0000),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// The first system near the Sun-like point whose primary at the epoch satisfies `want`.
    fn system_where(
        galaxy: &Galaxy,
        layer: Layer,
        want: impl Fn(&StarState) -> bool,
    ) -> SystemStars {
        let mut cell = Vec::new();
        for x in 0..64 {
            let key = CellKey::new(
                layer,
                [x, 26_000 / i32::try_from(layer.cell_size_ly()).unwrap(), 0],
            )
            .unwrap();
            generate_cell(galaxy, key, &mut cell);
            for record in &cell {
                let stars = SystemStars::generate(galaxy, record);
                if let Some(state) = stars.state_at(UniverseTime::EPOCH)
                    && want(&state.stars()[0])
                {
                    return stars;
                }
            }
        }
        panic!("no such system in 64 cells");
    }

    #[test]
    fn the_sun_from_one_au_subtends_half_a_degree() {
        let rho = angular_radius(Metres::new(SOLAR_RADIUS_M), Metres::new(METRES_PER_AU));
        let diameter = 2.0 * rho.value().to_degrees();
        assert!((diameter - 0.533).abs() < 0.001, "{diameter}");
        let inside = angular_radius(Metres::new(2.0), Metres::new(1.0));
        assert!((inside.value() - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
    }

    /// π L̄ sin²ρ is the illuminance from the star's V through 2.54 µlx within 1% (in V, the
    /// channels' photopic luminance), and the central luminance times each channel's disc average
    /// returns its mean to 10⁻¹².
    #[test]
    fn a_discs_flux_is_its_v_magnitude() {
        let galaxy = galaxy();
        let stars = system_where(&galaxy, Layer::C, |s| s.phase() == Phase::MainSequence);
        let discs = host_discs(&galaxy, &stars, UniverseTime::EPOCH);
        let disc = discs.first().expect("a main-sequence primary has a disc");
        let state = stars.state_at(UniverseTime::EPOCH).unwrap();
        let m_v = absolute_magnitude_v(&state.stars()[0]).unwrap().value();
        let d = 1_000.0 * disc.radius().value();
        let rho = angular_radius(disc.radius(), Metres::new(d)).value();
        let from_disc = std::f64::consts::PI
            * channel_luminance(disc.mean_luminance())
            * math::sin(rho)
            * math::sin(rho);
        let distance_modulus = 5.0 * math::log10(d / (10.0 * METRES_PER_PARSEC));
        let from_v = V0_ILLUMINANCE_LX
            * disc.colour().lux_per_v0()
            * math::exp10(-0.4 * (m_v + distance_modulus));
        assert!(
            (from_disc / from_v - 1.0).abs() < 0.01,
            "{from_disc} against {from_v}"
        );
        for c in 0..3 {
            let back = disc.central_luminance()[c].value() * disc.limb()[c].disc_average();
            let mean = disc.mean_luminance()[c].value();
            assert!(
                (back - mean).abs() <= 1e-12 * mean,
                "{c}: {back} against {mean}"
            );
        }
    }

    #[test]
    fn a_white_dwarf_host_takes_the_white_dwarf_rows() {
        let galaxy = galaxy();
        let stars = system_where(&galaxy, Layer::C, |s| {
            s.phase() == Phase::CarbonOxygenWhiteDwarf
                && s.effective_temperature().value() > 4_000.0
        });
        let discs = host_discs(&galaxy, &stars, UniverseTime::EPOCH);
        let disc = discs.first().expect("a white-dwarf primary has a disc");
        assert_eq!(disc.star(), StarIndex::PRIMARY);
        assert_eq!(
            disc.limb(),
            limb_coefficients(disc.teff(), disc.log_g(), AtmosphereGrid::WhiteDwarf)
        );
        assert_eq!(
            disc.colour(),
            star_colour(disc.teff(), disc.log_g(), AtmosphereGrid::WhiteDwarf)
        );
        assert!(disc.log_g() > 7.0, "{}", disc.log_g());
    }

    #[test]
    fn a_white_dwarf_reads_its_own_grid() {
        let wd = limb_coefficients(Kelvin::new(10_000.0), 8.0, AtmosphereGrid::WhiteDwarf);
        let star = limb_coefficients(Kelvin::new(10_000.0), 8.0, AtmosphereGrid::MainSequence);
        assert_ne!(wd, star);
    }
}
