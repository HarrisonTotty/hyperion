//! The smooth gas: three exponential layers and the hot corona (plan 07, Design note 4).
//!
//! Each layer is a radial factor times `exp(−|z| ÷ h)`, and its amplitude follows from the mass
//! budget: plan 02's gas mass is shared as `1 − f_w − f_c`, `f_w` and `f_c` between the neutral
//! disc, the warm ionised layer and the central molecular disc, and each amplitude is that share
//! divided by 1.4 `m_H` times the layer's volume integral. The corona is outside the budget — a halo
//! component given by its density alone, an additive floor that carries no dust.
//!
//! The neutral and warm layers' radial factor `exp(−R_m ÷ R − R ÷ R_g)` is the form McMillan (2017,
//! MNRAS 465, 76) fits the Milky Way's gas discs with: a hole inside the bar, and a peak at
//! `√(R_m R_g)` near the bar's end. It has no elementary radial integral, so the mass normalisation
//! takes it numerically, once per galaxy, through [`quad::gl_log_panels`] — the rule that
//! substitutes `u = ln R` itself, where [`gl_panels`](quad::gl_panels) integrates in `R` — over
//! eight log-spaced panels of 32 nodes each from 1 ly to 20 `R_g`. The molecular disc's integral is
//! the elementary `4π R_c² h_c`. The panel scheme and the node count are part of the generator
//! version, as everything in [`quad`](crate::galaxy::quad) is.
//!
//! Lanes are not here: they are a factor on the neutral layer alone, and because that factor
//! averages exactly 1 around a circle (Design note 6) every density this module returns is the
//! azimuthal mean of the field at that radius — which is what Design note 11's pressure reads.
//!
//! Because the layers are exponential in height, every vertical column here is a closed form and a
//! column across a bounded interval of height is a difference of two of them. The gas keeps the
//! plain exponential that the stellar discs gave up: the 2026-09-21 ruling that each disc is cored
//! in height was ruled on 2026-09-22 to bind the stellar age cohorts alone, provisionally, to be
//! revisited when plan 09 is written (plan 07's Risks).
//!
//! Radii and heights at this interface are bare `f64` light-years, densities bare `f64` hydrogen
//! nuclei per cubic centimetre and columns bare `f64` nuclei per square centimetre, as the module
//! doc of [`gas`](super) says; the field's facade wraps them in newtypes.

use crate::galaxy::gas::params::GasParams;
use crate::galaxy::quad;
use crate::math;

use super::{CENTIMETRES_PER_LIGHT_YEAR, SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY};

/// Panels of the radial mass integral, log-spaced in `R` (Design note 4).
const RADIAL_PANELS: u32 = 8;

/// The radial mass integral's panel edges: one more than [`RADIAL_PANELS`].
const RADIAL_EDGES: usize = 9;

const _: () = assert!(
    RADIAL_PANELS == 8,
    "radial_edges returns nine edges, which is eight panels"
);

/// The inner edge of the radial mass integral, ly.
///
/// Below one light-year the neutral and warm layers' `exp(−R_m ÷ R)` underflows — the smallest
/// `R_m` the generator draws is 8,000 ly, and `exp(−8_000)` is 0 in `f64` — and the molecular
/// disc's own `R exp(−R ÷ R_c)` contributes `(1 ly ÷ R_c)² ÷ 2` of its integral, under 10⁻⁵ at the
/// smallest `R_c` drawn. The edge cannot be 0: the rule integrates in `ln R`.
const RADIAL_INNER_LY: f64 = 1.0;

/// The outer edge of the radial mass integral, in units of the radial scale length `R_g`.
///
/// The tail beyond 20 `R_g` holds `21 e⁻²⁰` of the integral, 4 × 10⁻⁸, far below the 2% the mass
/// budget is tested to and below the quadrature's own error.
const RADIAL_OUTER_SCALES: f64 = 20.0;

/// One of the three exponential layers the gas disc is made of (Design note 4).
///
/// The corona is not one of them: it is outside the mass budget, has no vertical profile, takes no
/// lanes and carries no dust, so nothing that walks the layers wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GasLayer {
    /// The neutral disc: most of the mass, the thinnest layer, the one the lanes gather and the one
    /// a 21 cm column counts.
    Neutral,
    /// The warm ionised layer: thick, and filling the inner galaxy more evenly than the neutral gas
    /// does, which is why it takes half the hole scale.
    Warm,
    /// The central molecular disc, the diffuse part of the central molecular zone (Design note 5).
    Molecular,
}

impl GasLayer {
    /// Every layer, in the order [`SmoothGas`]'s own arrays are indexed in.
    pub const ALL: [Self; 3] = [Self::Neutral, Self::Warm, Self::Molecular];

    /// Its index into those arrays.
    const fn index(self) -> usize {
        match self {
            Self::Neutral => 0,
            Self::Warm => 1,
            Self::Molecular => 2,
        }
    }
}

/// The smooth gas of one galaxy: the three layers' shapes and amplitudes, and the corona's density.
///
/// It is a pure function of [`GasParams`] and is built once per galaxy, because the two radial mass
/// integrals are quadratures; every method on it is a closed form. Lanes and the log-normal noise
/// are applied on top of it by the field that owns it.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::gas::params::GasParams;
/// use hyperion_sim::galaxy::gas::smooth::{GasLayer, SmoothGas};
///
/// let gas = SmoothGas::new(&GasParams::milky_way_like());
/// // The measured neutral density in the plane at the Sun's radius, hydrogen nuclei per cm³.
/// let neutral = gas.plane_density(GasLayer::Neutral, 26_000.0);
/// assert!((0.6..=0.9).contains(&neutral));
/// // A layer's whole vertical column is 2 h times that, in nuclei per cm².
/// assert!(gas.column(GasLayer::Neutral, 26_000.0) > 1e20);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothGas {
    /// The hole scale `R_m` of the neutral layer, ly. The warm layer takes half of it.
    hole_scale: f64,
    /// The radial scale length `R_g` of the neutral and warm layers, ly.
    radial_scale: f64,
    /// The molecular disc's radial scale length `R_c`, ly.
    molecular_length: f64,
    /// Each layer's scale height, ly, indexed by [`GasLayer::index`].
    heights: [f64; 3],
    /// Each layer's radial amplitude, cm⁻³: the mid-plane density its radial factor multiplies.
    ///
    /// For the molecular disc, whose radial factor is 1 at the centre, this is the central density;
    /// for the other two the factor vanishes at the centre and peaks at `√(R_m R_g)`.
    amplitudes: [f64; 3],
    /// The corona's density `n_cor`, cm⁻³.
    corona_density: f64,
}

impl SmoothGas {
    /// The smooth field of `params`, each layer normalised to its share of the gas mass.
    ///
    /// # Panics
    ///
    /// If a scale length, a scale height or the gas mass is not positive and finite. [`GasParams`]
    /// produces none of those: every one of them is a positive draw or a positive constant.
    #[must_use]
    pub fn new(params: &GasParams) -> Self {
        let hole_scale = params.hole_scale().value();
        let radial_scale = params.radial_scale().value();
        let molecular = params.molecular_disc();
        let molecular_length = molecular.length().value();
        let heights = [
            params.neutral_height().value(),
            params.warm_height().value(),
            molecular.height().value(),
        ];
        for length in [hole_scale, radial_scale, molecular_length] {
            assert!(length > 0.0, "a gas scale length of {length} ly");
        }
        for height in heights {
            assert!(height > 0.0, "a gas scale height of {height} ly");
        }
        let mass = params.gas_mass().value();
        assert!(mass > 0.0 && mass.is_finite(), "a gas mass of {mass} M☉");
        // Each layer's volume integral is 2π ∫ R f(R) dR × 2h, the vertical integral of
        // exp(−|z| ÷ h) over the whole line being 2h. The molecular disc's radial moment is the
        // elementary R_c², which makes its volume the 4π R_c² h_c of Design note 4.
        let moments = [
            radial_moment(hole_scale, radial_scale),
            radial_moment(0.5 * hole_scale, radial_scale),
            molecular_length * molecular_length,
        ];
        let shares = [
            params.neutral_fraction(),
            params.warm_fraction(),
            molecular.fraction(),
        ];
        let amplitudes = core::array::from_fn(|i| {
            let volume = core::f64::consts::TAU * moments[i] * 2.0 * heights[i];
            shares[i] * mass / (SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY * volume)
        });
        Self {
            hole_scale,
            radial_scale,
            molecular_length,
            heights,
            amplitudes,
            corona_density: params.corona_density().value(),
        }
    }

    /// The layer's scale height, ly.
    #[must_use]
    pub fn height(&self, layer: GasLayer) -> f64 {
        self.heights[layer.index()]
    }

    /// The radius `√(R_m R_g)` at which the neutral layer's radial factor peaks, ly: near the bar's
    /// end, where the arms and their lanes start (McMillan 2017).
    #[must_use]
    pub fn neutral_peak_radius(&self) -> f64 {
        (self.hole_scale * self.radial_scale).sqrt()
    }

    /// The layer's radial factor at cylindrical radius `r ≥ 0` (ly), dimensionless and at most 1.
    ///
    /// It is 0 at the centre for the neutral and warm layers, whose hole divides by `r`: at `r = 0`
    /// the exponent is `−∞` and the factor underflows to 0 rather than becoming a NaN.
    fn radial_factor(&self, layer: GasLayer, r: f64) -> f64 {
        match layer {
            GasLayer::Neutral => math::exp(-self.hole_scale / r - r / self.radial_scale),
            GasLayer::Warm => math::exp(-0.5 * self.hole_scale / r - r / self.radial_scale),
            GasLayer::Molecular => math::exp(-r / self.molecular_length),
        }
    }

    /// The layer's mid-plane density at cylindrical radius `r ≥ 0` (ly), cm⁻³.
    ///
    /// This is the azimuthal mean of the layer at that radius: the lane factor of Design note 6
    /// averages exactly 1 around every circle, so it moves no mean.
    #[must_use]
    pub fn plane_density(&self, layer: GasLayer, r: f64) -> f64 {
        self.amplitudes[layer.index()] * self.radial_factor(layer, r)
    }

    /// The layer's density at cylindrical radius `r ≥ 0` and height `z`, both ly, in cm⁻³.
    #[must_use]
    pub fn density(&self, layer: GasLayer, r: f64, z: f64) -> f64 {
        self.plane_density(layer, r) * math::exp(-z.abs() / self.height(layer))
    }

    /// The three layers together at `(r, z)` in ly, cm⁻³: `n_disc` of Design note 4, without lanes
    /// and without noise.
    #[must_use]
    pub fn disc(&self, r: f64, z: f64) -> f64 {
        GasLayer::ALL
            .iter()
            .map(|&layer| self.density(layer, r, z))
            .sum()
    }

    /// The corona's density `n_cor`, cm⁻³: the same everywhere, and dust-free, because grains do
    /// not survive in it (Design note 10).
    #[must_use]
    pub fn corona_density(&self) -> f64 {
        self.corona_density
    }

    /// The mean field at `(r, z)` in ly, cm⁻³: the three layers plus the corona.
    #[must_use]
    pub fn mean_density(&self, r: f64, z: f64) -> f64 {
        self.disc(r, z) + self.corona_density
    }

    /// The azimuthal mean of the disc gas in the plane at radius `r` (ly), cm⁻³: the `n̄_disc(R, 0)`
    /// that Design note 11's hydrostatic pressure is proportional to.
    ///
    /// It is the lane-free mid-plane sum, because the lane factor averages exactly 1 around the
    /// circle of radius `r`, which is what keeps lanes out of the pressure.
    #[must_use]
    pub fn plane_disc_mean(&self, r: f64) -> f64 {
        self.disc(r, 0.0)
    }

    /// The layer's whole vertical column at radius `r` (ly), in nuclei per cm²: `2 h` times its
    /// mid-plane density.
    #[must_use]
    pub fn column(&self, layer: GasLayer, r: f64) -> f64 {
        2.0 * self.height(layer) * self.plane_density(layer, r) * CENTIMETRES_PER_LIGHT_YEAR
    }

    /// The layer's column between the heights `z_lo ≤ z_hi` (ly) at radius `r` (ly), in nuclei per
    /// cm². The interval need not lie on one side of the plane.
    ///
    /// Where the answer is a difference of two vertical integrals the difference is clamped at 0 by
    /// comparison and never by `f64::max`, which may return either of two inputs that compare
    /// equal and so can leak `−0.0` into a column (plan 02's `map::across_pixel` documents the
    /// idiom).
    #[must_use]
    pub fn column_between(&self, layer: GasLayer, r: f64, z_lo: f64, z_hi: f64) -> f64 {
        debug_assert!(z_lo <= z_hi, "a column from {z_lo} ly down to {z_hi} ly");
        let height = self.height(layer);
        let thickness = across_heights(|z| from_plane(height, z), z_lo, z_hi);
        thickness * self.plane_density(layer, r) * CENTIMETRES_PER_LIGHT_YEAR
    }

    /// The whole vertical column of disc gas at radius `r` (ly), in nuclei per cm².
    #[must_use]
    pub fn disc_column(&self, r: f64) -> f64 {
        GasLayer::ALL
            .iter()
            .map(|&layer| self.column(layer, r))
            .sum()
    }

    /// The disc gas's column between the heights `z_lo ≤ z_hi` (ly) at radius `r` (ly), in nuclei
    /// per cm².
    #[must_use]
    pub fn disc_column_between(&self, r: f64, z_lo: f64, z_hi: f64) -> f64 {
        GasLayer::ALL
            .iter()
            .map(|&layer| self.column_between(layer, r, z_lo, z_hi))
            .sum()
    }
}

/// `∫₀^∞ R exp(−hole ÷ R − R ÷ scale) dR`, ly², by Design note 4's fixed quadrature.
///
/// The integral has no elementary form. It is `2 hole scale K₂(2 √(hole ÷ scale))` in the modified
/// Bessel function of the second kind, which nothing else in the sim needs, so the mass
/// normalisation integrates it numerically instead — once per galaxy, over the log-spaced panels of
/// [`radial_edges`], which hold it to about 10⁻⁷ across the whole range of drawn parameters (unit
/// tests).
fn radial_moment(hole: f64, scale: f64) -> f64 {
    let edges = radial_edges(scale);
    quad::gl_log_panels(|r| r * math::exp(-hole / r - r / scale), &edges)
}

/// The panel edges of [`radial_moment`]: [`RADIAL_PANELS`] panels log-spaced from
/// [`RADIAL_INNER_LY`] to `RADIAL_OUTER_SCALES × scale`.
fn radial_edges(scale: f64) -> [f64; RADIAL_EDGES] {
    let step = math::ln(RADIAL_OUTER_SCALES * scale / RADIAL_INNER_LY) / f64::from(RADIAL_PANELS);
    core::array::from_fn(|k| {
        let k = u32::try_from(k).expect("nine panel edges are indexed by a u32");
        RADIAL_INNER_LY * math::exp(step * f64::from(k))
    })
}

/// `∫₀^{|z|} exp(−z′ ÷ h) dz′ = h (1 − exp(−|z| ÷ h))`, ly: an exponential layer's column from the
/// plane to `z`, per unit of mid-plane density.
fn from_plane(height: f64, z: f64) -> f64 {
    -height * math::exp_m1(-z.abs() / height)
}

/// `∫_{z_lo}^{z_hi}` of a layer whose column from the plane is `from_plane_to`, per unit of
/// mid-plane density, for `z_lo ≤ z_hi`.
///
/// An interval on one side of the plane is a difference of two values of a function that never
/// falls, so it is clamped at 0 with `if difference > 0.0`, which lets through neither a
/// rounded-negative value nor `−0.0`; an interval that straddles the plane is a sum of two
/// non-negative columns and needs no clamp.
fn across_heights(from_plane_to: impl Fn(f64) -> f64, z_lo: f64, z_hi: f64) -> f64 {
    let clamped = |difference: f64| if difference > 0.0 { difference } else { 0.0 };
    if z_lo >= 0.0 {
        clamped(from_plane_to(z_hi) - from_plane_to(z_lo))
    } else if z_hi <= 0.0 {
        clamped(from_plane_to(z_lo) - from_plane_to(z_hi))
    } else {
        from_plane_to(z_lo) + from_plane_to(z_hi)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::coords::ROOT_HALF_WIDTH_LY;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;

    /// The Sun's radius, where the brainstorm's measured densities apply.
    const SUN_RADIUS_LY: f64 = 26_000.0;

    /// Seeds of the sweeps, in the form `gas::params`'s own tests use.
    fn seeds(count: u64) -> impl Iterator<Item = Seed> {
        (0..count).map(|n| Seed::new(0x0700_5eed_0000_0000 | n))
    }

    /// The gas parameters of `seed`'s own galaxy, so that a sweep sees the whole range of `R_g`,
    /// `R_m` and the nuclear disc's length as well as of this plan's own draws.
    ///
    /// Building a galaxy's parameters costs tens of milliseconds, so a sweep of thousands of seeds
    /// draws this plan's own parameters against one galaxy and only tens against their own, which is
    /// how `gas::params`'s and plan 02's own fast sweeps are split.
    fn drawn(seed: Seed) -> GasParams {
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        GasParams::from_galaxy(seed, &galaxy)
    }

    #[track_caller]
    fn assert_relative(what: &str, actual: f64, expected: f64, tolerance: f64) {
        let error = ((actual - expected) / expected).abs();
        assert!(
            error <= tolerance,
            "{what}: {actual} is {error:e} from {expected}, over {tolerance:e}"
        );
    }

    #[track_caller]
    fn assert_within(what: &str, value: f64, low: f64, high: f64) {
        assert!(
            (low..=high).contains(&value),
            "{what} = {value} lies outside [{low}, {high}]"
        );
    }

    /// `∫₀^∞ R exp(−hole ÷ R − R ÷ scale) dR` by a midpoint sum in `ln R` over 200,000 points from
    /// 10⁻³ ly to 80 scale lengths: an independent reference for [`radial_moment`], sharing none of
    /// its nodes and neither its rule nor its panel scheme.
    fn reference_radial_moment(hole: f64, scale: f64) -> f64 {
        let points = 200_000_u32;
        let (lo, hi) = (1e-3, 80.0 * scale);
        let step = math::ln(hi / lo) / f64::from(points);
        let mut total = 0.0;
        for i in 0..points {
            let r = lo * math::exp(step * (f64::from(i) + 0.5));
            total += r * r * math::exp(-hole / r - r / scale) * step;
        }
        total
    }

    /// Each layer's mass in M☉ by a midpoint sum of `1.4 m_H n` in `ln R` and `ln |z|` over its
    /// whole support, and the part of each inside the root cube.
    ///
    /// The sum shares no node with Design note 4's normalisation quadrature and is taken in the
    /// field's own coordinates, `2π ∫ R ∫ n dz dR`, doubled for the two sides of the plane. Its grid
    /// closes the fixture's mass to a few parts in a million (measured by
    /// [`the_fixtures_mass_closes_and_names_its_share_outside_the_cube`]), so the 2% the plan asks
    /// of the mass budget tests the normalisation and not the reference.
    ///
    /// The root cube is a cube, not a cylinder: a circle of radius `R` between the cube's
    /// half-width `a` and its face diagonal `a √2` lies inside the cube over `arccos(a ÷ R)` of each
    /// octant's quarter-turn, which is the weight the in-cube sum carries.
    fn brute_force_masses(gas: &SmoothGas) -> ([f64; 3], [f64; 3]) {
        let half_width = f64::from(ROOT_HALF_WIDTH_LY);
        let (radii, heights) = (400_u32, 400_u32);
        let (r_lo, r_hi) = (0.25, 40.0 * gas.radial_scale);
        let (z_lo, z_hi) = (1e-3, 1e6);
        let r_step = math::ln(r_hi / r_lo) / f64::from(radii);
        let z_step = math::ln(z_hi / z_lo) / f64::from(heights);
        let mut whole = [0.0; 3];
        let mut in_cube = [0.0; 3];
        for i in 0..radii {
            let r = r_lo * math::exp(r_step * (f64::from(i) + 0.5));
            // `R dR` in `u = ln R` is `R² du`.
            let weight = r * r * r_step;
            let inside = if r <= half_width {
                1.0
            } else if r >= half_width * core::f64::consts::SQRT_2 {
                0.0
            } else {
                math::acos(half_width / r) / core::f64::consts::FRAC_PI_4
            };
            for layer in GasLayer::ALL {
                let mut column = 0.0;
                let mut column_in_cube = 0.0;
                for j in 0..heights {
                    let z = z_lo * math::exp(z_step * (f64::from(j) + 0.5));
                    let slab = z * z_step * gas.density(layer, r, z);
                    column += slab;
                    if z <= half_width {
                        column_in_cube += slab;
                    }
                }
                whole[layer.index()] += weight * column;
                in_cube[layer.index()] += weight * column_in_cube * inside;
            }
        }
        let scale = 2.0 * core::f64::consts::TAU * SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY;
        (whole.map(|m| m * scale), in_cube.map(|m| m * scale))
    }

    /// Design note 4's normalisation against a brute-force integral of the field itself, for the
    /// Milky Way fixture and twenty drawn galaxies: each layer carries its share of plan 02's gas
    /// mass to 2%, and so does the disc as a whole.
    ///
    /// The plan asked for the integral "over the cube". It is taken over the layers' own support
    /// instead, because a fifth of the gas can lie outside the root cube at the largest scale
    /// lengths the generator draws, while the mass the normalisation divides by is the whole of it;
    /// the in-cube share is a figure this test reports rather than one it folds into the budget.
    #[test]
    fn each_layer_carries_its_share_of_the_gas_mass() {
        let mut cases = vec![("milky_way".to_owned(), GasParams::milky_way_like())];
        cases.extend(seeds(20).map(|seed| (format!("{seed}"), drawn(seed))));
        for (name, params) in cases {
            let gas = SmoothGas::new(&params);
            let (whole, in_cube) = brute_force_masses(&gas);
            let shares = [
                params.neutral_fraction(),
                params.warm_fraction(),
                params.molecular_disc().fraction(),
            ];
            for layer in GasLayer::ALL {
                assert_relative(
                    &format!("{name} {layer:?} mass"),
                    whole[layer.index()],
                    shares[layer.index()] * params.gas_mass().value(),
                    0.02,
                );
            }
            let total: f64 = whole.iter().sum();
            assert_relative(
                &format!("{name} gas mass"),
                total,
                params.gas_mass().value(),
                0.02,
            );
            // The neutral layer reaches well past the cube at the largest scale lengths; the
            // smallest share seen is about 0.8, and a share above 1 would mean a weight above one.
            let inside: f64 = in_cube.iter().sum();
            assert_within(&format!("{name} in-cube share"), inside / total, 0.70, 1.0);
        }
    }

    /// The fixture's own figures: the mass closing to a few parts in a million, which is what makes
    /// the 2% budget above a test of the code, and 5.6% of its gas outside the root cube.
    ///
    /// Plan 07's Risks estimated 11% from the cube's inscribed cylinder of radius 65,536 ly. The
    /// cube's corners reach 92,681 ly, and counting them halves the figure.
    #[test]
    fn the_fixtures_mass_closes_and_names_its_share_outside_the_cube() {
        let params = GasParams::milky_way_like();
        let gas = SmoothGas::new(&params);
        let (whole, in_cube) = brute_force_masses(&gas);
        let total: f64 = whole.iter().sum();
        let inside: f64 = in_cube.iter().sum();
        assert_relative("fixture gas mass", total, params.gas_mass().value(), 1e-4);
        assert_relative("fixture in-cube share", inside / total, 0.944, 0.01);
    }

    /// The radial moment's quadrature against an independent midpoint sum, at the extremes of the
    /// drawn ranges of `R_m` and `R_g` as well as at the fixture's values.
    #[test]
    fn the_radial_moment_quadrature_holds_over_the_drawn_ranges() {
        // `R_m` is 0.8–1.2 of a bar half-length of 10,000–18,000 ly, and the warm layer halves it;
        // `R_g` is 1.5–2 of a thin scale length of 7,000–11,500 ly.
        for hole in [4_000.0, 8_000.0, 16_000.0, 21_600.0] {
            for scale in [10_500.0, 14_840.0, 23_000.0] {
                assert_relative(
                    &format!("moment({hole}, {scale})"),
                    radial_moment(hole, scale),
                    reference_radial_moment(hole, scale),
                    1e-6,
                );
            }
        }
    }

    /// The measured interstellar medium at the Sun's radius and at the centre, for the Milky Way
    /// fixture (Design note 4 and the brainstorm's "Dust and gas").
    #[test]
    fn the_fixture_matches_the_measured_medium_in_the_plane() {
        let gas = SmoothGas::new(&GasParams::milky_way_like());
        assert_within(
            "neutral density at the Sun",
            gas.plane_density(GasLayer::Neutral, SUN_RADIUS_LY),
            0.6,
            0.9,
        );
        assert_within(
            "warm ionised density at the Sun",
            gas.plane_density(GasLayer::Warm, SUN_RADIUS_LY),
            0.025,
            0.035,
        );
        assert_within(
            "molecular density at the centre",
            gas.plane_density(GasLayer::Molecular, 0.0),
            20.0,
            80.0,
        );
    }

    /// The hole: inside an eighth of `R_m` the neutral layer holds under 1% of its peak density, for
    /// every seed, and nothing inside the hole rises above its edge (Design note 4).
    #[test]
    fn the_hole_empties_the_neutral_layer_inside_the_bar() {
        let mut worst = 0.0_f64;
        let mut check = |params: &GasParams| {
            let gas = SmoothGas::new(params);
            let peak = gas.plane_density(GasLayer::Neutral, gas.neutral_peak_radius());
            let hole_radius = params.hole_scale().value() / 8.0;
            let edge = gas.plane_density(GasLayer::Neutral, hole_radius);
            let ratio = edge / peak;
            assert!(ratio < 0.01, "the hole holds {ratio} of the peak density");
            for step in 1..8_u32 {
                let r = hole_radius * f64::from(step) / 8.0;
                let inside = gas.plane_density(GasLayer::Neutral, r);
                assert!(
                    inside <= edge,
                    "{inside} at {r} ly is above the hole's edge"
                );
            }
            worst = worst.max(ratio);
        };
        let fixture = GalaxyParams::milky_way_like();
        for seed in seeds(2_000) {
            check(&GasParams::from_galaxy(seed, &fixture));
        }
        for seed in seeds(32) {
            check(&drawn(seed));
        }
        // The edge of the hole is not itself empty, so the bound above is a bound and not an
        // underflow.
        assert!(
            worst > 0.0,
            "the neutral layer underflows at the hole's edge"
        );
    }

    /// The corona is its drawn density everywhere: the mean field is the disc plus that constant at
    /// every radius and height, and it is the whole of what is left where the disc has faded.
    #[test]
    fn the_corona_is_its_density_everywhere() {
        for params in seeds(8).map(drawn) {
            let gas = SmoothGas::new(&params);
            assert_same_bits(gas.corona_density(), params.corona_density().value());
            for r in [0.0, 1.0, 290.0, SUN_RADIUS_LY, 60_000.0, 120_000.0] {
                for z in [0.0, -25.0, 400.0, -3_000.0, 60_000.0] {
                    let mean = gas.mean_density(r, z);
                    assert_relative(
                        &format!("the corona at ({r}, {z})"),
                        mean - gas.disc(r, z),
                        gas.corona_density(),
                        1e-8,
                    );
                    assert!(
                        mean >= gas.corona_density(),
                        "the field falls below the corona at ({r}, {z})"
                    );
                }
            }
            // Inside the disc the corona is a floor and not the whole answer; far outside it — past
            // a thousand scale heights, where every layer has underflowed — it is the whole answer.
            for (r, z) in [(290.0, 0.0), (SUN_RADIUS_LY, 0.0), (SUN_RADIUS_LY, 400.0)] {
                assert!(
                    gas.mean_density(r, z) > gas.corona_density(),
                    "the disc adds nothing at ({r}, {z})"
                );
            }
            let far = gas.mean_density(400_000.0, 400_000.0);
            assert_same_bits(far, gas.corona_density());
        }
    }

    /// The azimuthal mean in the plane is the lane-free mid-plane sum, bit for bit, which is what
    /// Design note 11's pressure reads.
    #[test]
    fn the_plane_mean_is_the_mid_plane_disc_density() {
        let gas = SmoothGas::new(&GasParams::milky_way_like());
        for r in [100.0, 1_000.0, SUN_RADIUS_LY, 50_000.0] {
            assert_same_bits(gas.plane_disc_mean(r), gas.disc(r, 0.0));
        }
    }

    /// Every closed-form column against a midpoint sum of the density it integrates, and the whole
    /// column against `2 h` times the mid-plane density.
    #[test]
    fn the_vertical_columns_are_the_integrals_of_the_density() {
        let gas = SmoothGas::new(&GasParams::milky_way_like());
        for layer in GasLayer::ALL {
            let height = gas.height(layer);
            for r in [290.0, 8_000.0, SUN_RADIUS_LY] {
                let column = gas.column(layer, r);
                assert_relative(
                    "the whole column",
                    column,
                    2.0 * height * gas.plane_density(layer, r) * CENTIMETRES_PER_LIGHT_YEAR,
                    0.0,
                );
                // A midpoint sum over ±40 scale heights, past which the exponential has nothing
                // left: e⁻⁴⁰ is 4 × 10⁻¹⁸.
                let (points, span) = (40_000_u32, 40.0 * height);
                let step = 2.0 * span / f64::from(points);
                let mut sum = 0.0;
                for i in 0..points {
                    let z = -span + (f64::from(i) + 0.5) * step;
                    sum += step * gas.density(layer, r, z);
                }
                assert_relative(
                    "the column against a midpoint sum",
                    column,
                    sum * CENTIMETRES_PER_LIGHT_YEAR,
                    1e-6,
                );
            }
        }
    }

    /// A column across an interval: the pieces of a partition add to the whole column, `z → −z`
    /// leaves each piece alone bit for bit, and an interval that carries nothing gives a positive
    /// zero.
    #[test]
    fn a_column_across_an_interval_partitions_the_whole_column() {
        let gas = SmoothGas::new(&GasParams::milky_way_like());
        let r = SUN_RADIUS_LY;
        // The outermost edges are far enough out that every layer's vertical integral has
        // saturated, so the partition covers the whole line and not 20 scale heights of it.
        let edges = [
            -1e6, -60_000.0, -3_000.0, -400.0, 0.0, 17.0, 400.0, 9_000.0, 60_000.0, 1e6,
        ];
        for layer in GasLayer::ALL {
            let pieces: f64 = edges
                .windows(2)
                .map(|edge| gas.column_between(layer, r, edge[0], edge[1]))
                .sum();
            assert_relative("the partition", pieces, gas.column(layer, r), 1e-12);
            for edge in edges.windows(2) {
                assert_same_bits(
                    gas.column_between(layer, r, -edge[1], -edge[0]),
                    gas.column_between(layer, r, edge[0], edge[1]),
                );
            }
            // A zero-width interval, and one far out where both vertical integrals have saturated:
            // the clamp returns a positive zero and never `−0.0`, which is why it is a comparison
            // and not `f64::max`.
            assert_same_bits(gas.column_between(layer, r, 0.0, 0.0), 0.0);
            assert_same_bits(gas.column_between(layer, r, 1e9, 1e9), 0.0);
            assert_same_bits(gas.column_between(layer, r, -1e9, -1e9), 0.0);
            assert_same_bits(gas.column_between(layer, r, 1e6, 1.000_001e6), 0.0);
        }
        // The disc's column is the three layers', across an interval as well as whole.
        let layers: f64 = GasLayer::ALL.iter().map(|&l| gas.column(l, r)).sum();
        assert_relative("the disc column", gas.disc_column(r), layers, 0.0);
        let piece = gas.disc_column_between(r, -400.0, 400.0);
        assert!(piece > 0.0 && piece < gas.disc_column(r));
    }

    /// The layers' shapes: each falls by `e` over its scale height and is even in `z`, and the
    /// neutral layer's radial factor peaks at `√(R_m R_g)`.
    #[test]
    fn each_layer_falls_by_e_over_its_scale_height() {
        for params in seeds(8).map(drawn) {
            let gas = SmoothGas::new(&params);
            let peak = gas.neutral_peak_radius();
            assert_relative(
                "the neutral peak",
                peak,
                (params.hole_scale().value() * params.radial_scale().value()).sqrt(),
                0.0,
            );
            for layer in GasLayer::ALL {
                let height = gas.height(layer);
                let at_plane = gas.plane_density(layer, peak);
                assert_relative(
                    "one scale height up",
                    gas.density(layer, peak, height),
                    at_plane / core::f64::consts::E,
                    1e-12,
                );
                assert_same_bits(
                    gas.density(layer, peak, -height),
                    gas.density(layer, peak, height),
                );
            }
            for step in [-4_000.0, -400.0, 400.0, 4_000.0] {
                let away = gas.plane_density(GasLayer::Neutral, peak + step);
                assert!(
                    away <= gas.plane_density(GasLayer::Neutral, peak),
                    "the neutral layer rises {step} ly from its peak"
                );
            }
        }
    }

    /// The radial factors at the centre: the two layers with a hole vanish there rather than
    /// dividing by zero into a NaN, and the molecular disc is at its amplitude.
    #[test]
    fn the_hole_vanishes_at_the_centre_without_a_nan() {
        let gas = SmoothGas::new(&GasParams::milky_way_like());
        assert_same_bits(gas.plane_density(GasLayer::Neutral, 0.0), 0.0);
        assert_same_bits(gas.plane_density(GasLayer::Warm, 0.0), 0.0);
        assert!(gas.plane_density(GasLayer::Molecular, 0.0) > 0.0);
        assert_same_bits(
            gas.disc(0.0, 0.0),
            gas.plane_density(GasLayer::Molecular, 0.0),
        );
    }

    /// The panel edges: nine of them, log-spaced, from one light-year to twenty scale lengths.
    #[test]
    fn the_radial_panels_are_log_spaced_from_one_light_year() {
        let edges = radial_edges(14_840.0);
        assert_eq!(edges.len(), RADIAL_EDGES);
        assert_same_bits(edges[0], RADIAL_INNER_LY);
        assert_relative(
            "the outer edge",
            edges[RADIAL_EDGES - 1],
            RADIAL_OUTER_SCALES * 14_840.0,
            1e-14,
        );
        for triple in edges.windows(3) {
            assert_relative(
                "the log spacing",
                triple[1] / triple[0],
                triple[2] / triple[1],
                1e-14,
            );
        }
    }

    /// The field is a pure function of its parameters: two fields built from one [`GasParams`] agree
    /// bit for bit, and different parameters differ.
    #[test]
    fn the_smooth_field_is_a_pure_function_of_its_parameters() {
        let params = drawn(Seed::new(0x0700_5eed_0000_0011));
        let once = SmoothGas::new(&params);
        assert_eq!(once, SmoothGas::new(&params));
        assert_ne!(once, SmoothGas::new(&GasParams::milky_way_like()));
        for r in [1.0, 290.0, SUN_RADIUS_LY] {
            assert_same_bits(once.disc(r, 30.0), SmoothGas::new(&params).disc(r, 30.0));
        }
    }
}
