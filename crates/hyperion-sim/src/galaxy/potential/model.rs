//! The mass model: every component of the galaxy that pulls, as the potential sees it (plan 02,
//! P02.T6.d and Design note 6).

use super::mge::{
    Gaussian, GridPoint, InPlane, bar_disc, boxy_bulge, double_exponential,
    holed_double_exponential,
};
use super::nfw::Nfw;
use super::spherical::{NuclearCluster, PointMass, SphericalMass};
use crate::galaxy::Population;
use crate::galaxy::params::{BulgeParams, GalaxyParams, NuclearDiscParams};
use crate::math;
use crate::units::{LightYears, SolarMasses};

/// Whether a model holds the galactic centre's black hole and nuclear cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Centre {
    /// Everything: the model of a built galaxy.
    Included,
    /// Neither: the model the bulge's dispersion is read from, before the black hole's mass is
    /// known (plan 02, Design note 8).
    Excluded,
}

/// The scales `(s, h)` of the bulge made axisymmetric in the plane, ly: `s = √((a² + b²) ÷ 2)` of
/// its in-plane scales `a` and `b`, and `h` its vertical scale (decision-p15-revalidation Q8).
///
/// The body `exp(−((R ÷ s)^c + (|z| ÷ h)^c)^(1 ÷ c))` has the bulge's unit body, so it keeps the
/// bulge's mass, its boxiness `c` and both its second moments, `⟨R²⟩ = s² ⟨ϖ²⟩ = (a² + b²) ⟨ϖ²⟩ ÷
/// 2` and `⟨z²⟩ = h² ⟨z²⟩` of the unit body's, exactly: plan 02's Design note 6, `⟨R²⟩` matched.
/// The potential expands it ([`boxy_bulge`]) and the bulge's tracer is it (plan 08, Design note 10;
/// [`bulge_projected_sigma`](crate::galaxy::kinematics::spheroid::bulge_projected_sigma)).
#[must_use]
pub(crate) fn axisymmetric_bulge_scales(bulge: &BulgeParams) -> (LightYears, LightYears) {
    let (a, b) = (bulge.scale_x().value(), bulge.scale_y().value());
    (
        LightYears::new(f64::midpoint(a * a, b * b).sqrt()),
        bulge.scale_z(),
    )
}

/// The galaxy's mass model: Gaussian expansions of the discs, the bar and the bulge, and the
/// spherical dark halo, nuclear cluster and black hole (plan 02, P02.T6.d).
///
/// The Gaussians come in a fixed order, which is part of the generator version because every
/// sum runs in it: the thin disc with the young disc, the thick disc, the gas disc, the nuclear
/// disc's main and inner parts ([`NuclearDiscParams`]), the bar and the bulge. Then the dark
/// halo, the nuclear cluster and the black hole. The thin and young discs are one double
/// exponential of their combined mass and the drawn mean height (Design note 6), with the thin
/// discs' central hole
/// ([`mge::holed_double_exponential`](super::mge::holed_double_exponential); plan 02, P02.T12.b),
/// whose Gaussians' masses are signed. The bar is an axisymmetric disc with its azimuthally averaged surface
/// density ([`mge::bar_disc`](super::mge::bar_disc)); the boxy bulge is its own body made
/// axisymmetric in the plane ([`mge::boxy_bulge`](super::mge::boxy_bulge), plan 15's P15.T3.c),
/// which keeps its mass, boxiness and second moments. The stellar halo's 1% is left out.
///
/// The discs' stars are cored in height ([`fields::vertical`](crate::galaxy::fields::vertical)),
/// but the potential keeps them exponential in height, at the drawn heights, which are the stars'
/// effective heights `Σ ÷ 2ρ₀`. The two have the same surface and mid-plane densities at every
/// radius, so `K_z` agrees in the plane, where its slope is `4πGρ₀`, and far above, where it is
/// `2πGΣ`; between, for the Milky Way fixture's thin disc, the cored stars hold up to 5% of the
/// disc's `2πGΣ` more within a given height, 4.6% of the whole `K_z` there, about 1.1 effective
/// heights up. The cored profiles are solved in this potential, so holding them in it as well
/// would make the solve an iteration, which Design note 6 rules out; plan 15's fitted tables may
/// replace the vertical expansion.
///
/// Radii are light-years in the galactic frame, potentials (km/s)² with zero at infinity, forces
/// (km/s)² per light-year.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
/// use hyperion_sim::units::LightYears;
///
/// let model = MassModel::new(&GalaxyParams::milky_way_like());
/// // The Sun's circular speed, about 230 km/s at 8 kpc.
/// let v = model.v_circ_sq(LightYears::new(26_100.0)).sqrt();
/// assert!((200.0..260.0).contains(&v), "{v} km/s");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MassModel {
    gaussians: Vec<Gaussian>,
    dark_halo: Nfw,
    nuclear_cluster: NuclearCluster,
    black_hole: PointMass,
    bar_corotation: LightYears,
}

impl MassModel {
    /// The mass model of `params`.
    #[must_use]
    pub fn new(params: &GalaxyParams) -> Self {
        Self::assemble(params, Centre::Included)
    }

    /// The model without the black hole and the nuclear cluster, from which the bulge's
    /// dispersion is read before the black hole's mass exists (plan 02, Design note 8; plan 08,
    /// P08.T4.d, whose
    /// [`bulge_projected_sigma`](crate::galaxy::kinematics::spheroid::bulge_projected_sigma)
    /// takes it).
    #[must_use]
    pub fn without_centre(params: &GalaxyParams) -> Self {
        Self::assemble(params, Centre::Excluded)
    }

    fn assemble(params: &GalaxyParams, centre: Centre) -> Self {
        let mass = |p| params.population_mass(p);
        let (thin, thick, gas) = (params.thin_disc(), params.thick_disc(), params.gas_disc());
        let (nuclear, bar) = (params.nuclear_disc(), params.bar());
        let bulge = params.bulge();
        let (s, h) = axisymmetric_bulge_scales(bulge);
        let parts = [
            holed_double_exponential(
                mass(Population::YoungThinDisc) + mass(Population::OldThinDisc),
                thin.length(),
                thin.height(),
            ),
            double_exponential(mass(Population::ThickDisc), thick.length(), thick.height()),
            double_exponential(gas.mass(), gas.length(), gas.height()),
            double_exponential(
                mass(Population::NuclearDisc) * (1.0 - NuclearDiscParams::INNER_SHARE),
                nuclear.length(),
                nuclear.height(),
            ),
            holed_double_exponential(
                mass(Population::NuclearDisc) * NuclearDiscParams::INNER_SHARE,
                nuclear.inner_length(),
                nuclear.inner_height(),
            ),
            bar_disc(mass(Population::LongBar), bar.half_length(), bar.height()),
            boxy_bulge(mass(Population::Bulge), s, h, bulge.boxiness()),
        ];
        let gaussians = parts
            .into_iter()
            .flat_map(|part| {
                part.expect(
                    "built parameters give positive masses and lengths, and a boxiness of 3–4",
                )
            })
            .collect();
        let (nuclear_cluster, black_hole) = match centre {
            Centre::Included => (
                NuclearCluster::of(params.nuclear_cluster()),
                params.black_hole().mass(),
            ),
            Centre::Excluded => (
                NuclearCluster::new(SolarMasses::ZERO).expect("zero is a valid mass"),
                SolarMasses::ZERO,
            ),
        };
        Self {
            gaussians,
            dark_halo: Nfw::from_params(params.dark_halo()),
            nuclear_cluster,
            black_hole: PointMass::new(black_hole).expect("the black hole's mass is positive"),
            bar_corotation: params.bar().corotation_radius(),
        }
    }

    /// The Gaussians, in the model's fixed order.
    #[must_use]
    pub fn gaussians(&self) -> &[Gaussian] {
        &self.gaussians
    }

    /// The dark halo.
    #[must_use]
    pub fn dark_halo(&self) -> &Nfw {
        &self.dark_halo
    }

    /// The nuclear star cluster.
    #[must_use]
    pub fn nuclear_cluster(&self) -> &NuclearCluster {
        &self.nuclear_cluster
    }

    /// The central black hole.
    #[must_use]
    pub fn black_hole(&self) -> &PointMass {
        &self.black_hole
    }

    /// The bar's corotation radius: its corotation ratio times its half-length.
    #[must_use]
    pub fn bar_corotation(&self) -> LightYears {
        self.bar_corotation
    }

    /// The bytes the model owns on the heap: the Gaussians and their node tables.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.gaussians.capacity() * size_of::<Gaussian>()
            + self.gaussians.len() * Gaussian::HEAP_BYTES
    }

    /// The spherical components in their fixed order: dark halo, nuclear cluster, black hole.
    pub(crate) fn spherical(&self) -> [&dyn SphericalMass; 3] {
        [&self.dark_halo, &self.nuclear_cluster, &self.black_hole]
    }

    /// The circular speed squared in the plane at radius `r > 0`, (km/s)², summed directly
    /// over every component.
    #[must_use]
    pub fn v_circ_sq(&self, r: LightYears) -> f64 {
        let extended = self
            .gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.v_circ_sq(r));
        self.spherical()
            .iter()
            .fold(extended, |sum, c| sum + c.v_circ_sq(r))
    }

    /// The mass of the spherical components inside radius `r`: the dark halo, the nuclear
    /// cluster and the black hole.
    ///
    /// The discs, the bar and the bulge are fields whose mass inside a sphere plan 02's P02.T11
    /// integrates from their true densities; [`expanded_enclosed_mass`](Self::expanded_enclosed_mass)
    /// gives that of their Gaussian stand-ins here.
    #[must_use]
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        self.spherical()
            .iter()
            .fold(SolarMasses::ZERO, |sum, c| sum + c.enclosed_mass(r))
    }

    /// The mass of the Gaussian expansions inside the sphere of radius `r`.
    #[must_use]
    pub fn expanded_enclosed_mass(&self, r: LightYears) -> SolarMasses {
        self.gaussians
            .iter()
            .fold(SolarMasses::ZERO, |sum, g| sum + g.enclosed_mass(r))
    }

    /// The potential at `(R, z)`, (km/s)², zero at infinity, summed directly over every
    /// component. Off the centre only: the black hole's is infinite there.
    #[must_use]
    pub fn potential(&self, r_cyl: LightYears, z: LightYears) -> f64 {
        let extended = self
            .gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.potential(r_cyl, z));
        let r = LightYears::new(math::hypot(r_cyl.value(), z.value()));
        self.spherical()
            .iter()
            .fold(extended, |sum, c| sum + c.potential(r))
    }

    /// The vertical force `K_z = ∂Φ ÷ ∂z` at `(R, z)`, (km/s)² per light-year: positive above
    /// the plane, odd in z. The spherical components give `G M(<r) z ÷ r³`.
    #[must_use]
    pub fn vertical_force(&self, r_cyl: LightYears, z: LightYears) -> f64 {
        let extended = self
            .gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.vertical_force(r_cyl, z));
        if z.value() == 0.0 {
            return extended;
        }
        let r = math::hypot(r_cyl.value(), z.value());
        let radius = LightYears::new(r);
        self.spherical().iter().fold(extended, |sum, c| {
            sum + c.v_circ_sq(radius) * z.value() / (r * r)
        })
    }

    /// The Gaussians' in-plane quantities at radius `r` (ly), summed in order.
    pub(crate) fn extended_in_plane(&self, r: f64) -> InPlane {
        let mut sum = InPlane::default();
        for g in &self.gaussians {
            sum += g.in_plane(r);
        }
        sum
    }

    /// The Gaussians' potential and its logarithmic derivatives at `(R, z)` (ly), summed in
    /// order.
    pub(crate) fn extended_grid_point(&self, r: f64, z: f64) -> GridPoint {
        let mut sum = [0.0; 4];
        for g in &self.gaussians {
            for (s, v) in sum.iter_mut().zip(g.grid_point(r, z)) {
                *s += v;
            }
        }
        sum
    }

    /// The Gaussians' total mass.
    #[cfg(test)]
    pub(crate) fn extended_mass(&self) -> f64 {
        self.gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.mass().value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::potential::mge::spheroidal_exponential;
    use crate::galaxy::quad::gl32;

    /// The second moments `⟨ϖ²⟩` and `⟨z²⟩` of the unit boxy body `exp(−m)`, `m = (ϖ^p +
    /// |z|^p)^(1÷p)` with `ϖ² = x² + y²` and `p` the boxiness, in closed form.
    ///
    /// `m` is homogeneous of degree 1, so for a function `h` homogeneous of degree 2, `∫ h e^(−m) dV
    /// = 5! ∫_{m≤1} h dV` and `∫ e^(−m) dV = 3! V(m ≤ 1)`. Over the unit body, with its boundary
    /// `z = (1 − ϖ^p)^(1÷p)`, `∫ ϖᵅ (1 − ϖ^p)^(β÷p) dϖ = B((α + 1) ÷ p, β ÷ p + 1) ÷ p`, so `⟨ϖ²⟩ =
    /// 20 B(4 ÷ p, 1 ÷ p + 1) ÷ B(2 ÷ p, 1 ÷ p + 1)` and `⟨z²⟩ = (20 ÷ 3) B(2 ÷ p, 3 ÷ p + 1) ÷
    /// B(2 ÷ p, 1 ÷ p + 1)`. At `p = 2`, the spherical exponential, they are 8 and 4.
    fn unit_bulge_moments(boxiness: f64) -> (f64, f64) {
        let p = boxiness;
        let ln_beta =
            |x: f64, y: f64| math::ln_gamma(x) + math::ln_gamma(y) - math::ln_gamma(x + y);
        let volume = ln_beta(2.0 / p, 1.0 / p + 1.0);
        let planar = 20.0 * math::exp(ln_beta(4.0 / p, 1.0 / p + 1.0) - volume);
        let vertical = 20.0 / 3.0 * math::exp(ln_beta(2.0 / p, 3.0 / p + 1.0) - volume);
        (planar, vertical)
    }

    /// The boxy bulge's second moments `⟨R²⟩` and `⟨z²⟩`, ly²: the unit body's scaled by the
    /// bulge's axes, `⟨R²⟩ = (a² + b²) ⟨ϖ²⟩ ÷ 2` and `⟨z²⟩ = c² ⟨z²⟩`.
    fn bulge_second_moments(bulge: &BulgeParams) -> (f64, f64) {
        let (planar, vertical) = unit_bulge_moments(bulge.boxiness());
        let (a, b, c) = (
            bulge.scale_x().value(),
            bulge.scale_y().value(),
            bulge.scale_z().value(),
        );
        (f64::midpoint(a * a, b * b) * planar, c * c * vertical)
    }

    /// The spheroidal exponential `exp(−√(R² ÷ a_r² + z² ÷ a_z²))` with the boxy bulge's second
    /// moments, which plan 02's potential held in the bulge's place until plan 15's P15.T3.c (its
    /// Design note 6): its own are `⟨R²⟩ = 8 a_r²` and `⟨z²⟩ = 4 a_z²`. Plan 15's T3.b measures
    /// the rotation curve's change from it.
    fn bulge_spheroid(bulge: &BulgeParams) -> (LightYears, LightYears) {
        let (r2, z2) = bulge_second_moments(bulge);
        (
            LightYears::new((r2 / 8.0).sqrt()),
            LightYears::new((z2 / 4.0).sqrt()),
        )
    }

    /// The axisymmetrised body keeps the bulge's second moments exactly, `s² ⟨ϖ²⟩` and `h²
    /// ⟨z²⟩`, and its Gaussians hold them to the expansion's error: the mass-weighted `⟨R²⟩ = Σ
    /// M 2σ²` and `⟨z²⟩ = Σ M q²σ²` of the Milky Way's bulge (c = 3.5) are 1.58% under and 1.62%
    /// over the body's, held to 2% (decision-p15-revalidation Q8). The fit weights the mass inside
    /// spheres and the midplane force, not the moments, and cannot follow the boxiness (plan 15,
    /// P15.T3.b), so its terms trade height for radius.
    #[test]
    fn the_axisymmetric_bulge_keeps_the_bulge_s_second_moments() {
        let params = GalaxyParams::milky_way_like();
        let bulge = params.bulge();
        let (s, h) = axisymmetric_bulge_scales(bulge);
        let (planar, vertical) = unit_bulge_moments(bulge.boxiness());
        let (r2, z2) = bulge_second_moments(bulge);
        assert!((s.value() * s.value() * planar / r2 - 1.0).abs() < 1e-14);
        assert!((h.value() * h.value() * vertical / z2 - 1.0).abs() < 1e-14);
        let gaussians = boxy_bulge(SolarMasses::new(1.0), s, h, bulge.boxiness()).unwrap();
        let (mut planar_sum, mut vertical_sum) = (0.0, 0.0);
        for g in &gaussians {
            let (m, sigma, q) = (g.mass().value(), g.sigma().value(), g.flattening());
            planar_sum += m * 2.0 * sigma * sigma;
            vertical_sum += m * q * q * sigma * sigma;
        }
        eprintln!(
            "MGE_BOXY at c {}: ⟨R²⟩ {:+.4}%, ⟨z²⟩ {:+.4}% from the body's",
            bulge.boxiness(),
            100.0 * (planar_sum / r2 - 1.0),
            100.0 * (vertical_sum / z2 - 1.0)
        );
        assert!(
            (planar_sum / r2 - 1.0).abs() < 0.02,
            "{planar_sum} against {r2}"
        );
        assert!(
            (vertical_sum / z2 - 1.0).abs() < 0.02,
            "{vertical_sum} against {z2}"
        );
    }

    /// Plan 15's P15.T3.b at Milky Way values, run at the swap (T3.c): the circular speed with the
    /// bulge's boxy body differs from the one with plan 02's moment-matched spheroidal exponential
    /// in its place by under 3% anywhere, at 200 radii from 0.3 ly to 300,000 ly, every other
    /// component as the model holds it.
    #[test]
    fn the_boxy_bulge_moves_the_milky_way_s_rotation_curve_under_three_per_cent() {
        let params = GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let bulge = params.bulge();
        let mass = params.population_mass(Population::Bulge);
        let (s, h) = axisymmetric_bulge_scales(bulge);
        let boxy = boxy_bulge(mass, s, h, bulge.boxiness()).unwrap();
        let (a_r, a_z) = bulge_spheroid(bulge);
        let spheroid = spheroidal_exponential(mass, a_r, a_z).unwrap();
        let others = model.gaussians.len() - boxy.len();
        assert_eq!(&model.gaussians[others..], boxy.as_slice());
        let sum = |gaussians: &[Gaussian], r: LightYears| {
            gaussians.iter().fold(0.0, |sum, g| sum + g.v_circ_sq(r))
        };
        let mut worst: (f64, f64) = (0.0, 0.0);
        for i in 0..200 {
            let r = LightYears::new(0.3 * math::exp(f64::from(i) / 199.0 * math::ln(1e6)));
            let rest = model
                .spherical()
                .iter()
                .fold(sum(&model.gaussians[..others], r), |sum, c| {
                    sum + c.v_circ_sq(r)
                });
            let change = ((rest + sum(&boxy, r)) / (rest + sum(&spheroid, r))).sqrt() - 1.0;
            if change.abs() > worst.1.abs() {
                worst = (r.value(), change);
            }
        }
        eprintln!(
            "the boxy bulge moves v_c by at most {:+.3}%, at {:.0} ly",
            100.0 * worst.1,
            worst.0
        );
        assert!(worst.1.abs() < 0.03, "{worst:?}");
    }

    /// The closed-form moments against a two-dimensional `gl32` quadrature of `exp(−m)` over the
    /// quarter plane in (ϖ, z), with the volume element `ϖ dϖ dz`, on panels in each variable.
    #[test]
    fn unit_bulge_moments_match_a_quadrature() {
        for p in [2.0, 3.0, 3.5, 4.0] {
            let edges = [0.0, 0.5, 1.5, 3.0, 6.0, 12.0, 24.0, 48.0];
            let integrate = |h: &dyn Fn(f64, f64) -> f64| {
                let mut total = 0.0;
                for zs in edges.windows(2) {
                    for ws in edges.windows(2) {
                        total += gl32(
                            |z| {
                                gl32(
                                    |w| {
                                        let m = math::powf(
                                            math::powf(w, p) + math::powf(z, p),
                                            1.0 / p,
                                        );
                                        h(w, z) * w * math::exp(-m)
                                    },
                                    ws[0],
                                    ws[1],
                                )
                            },
                            zs[0],
                            zs[1],
                        );
                    }
                }
                total
            };
            let norm = integrate(&|_, _| 1.0);
            let planar = integrate(&|w, _| w * w) / norm;
            let vertical = integrate(&|_, z| z * z) / norm;
            let (closed_planar, closed_vertical) = unit_bulge_moments(p);
            assert!(
                (planar / closed_planar - 1.0).abs() < 1e-6,
                "p {p}: {planar}"
            );
            assert!(
                (vertical / closed_vertical - 1.0).abs() < 1e-6,
                "p {p}: {vertical}"
            );
        }
        let (planar, vertical) = unit_bulge_moments(2.0);
        assert!((planar - 8.0).abs() < 1e-12 && (vertical - 4.0).abs() < 1e-12);
    }
}
