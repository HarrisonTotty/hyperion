//! The density fields: every population as closed-form number densities of systems, with its age
//! and metallicity distributions (brainstorm, "Fields" and "Populations"; plan 02, P02.T7).
//!
//! The fields are what placement thins against (brainstorm, "Exact placement by thinning"), so
//! each density is exactly its closed form, in systems per cubic light-year of every mass band
//! together, at the epoch, with the feature share φ held at 0 (plan 02, Design note 12):
//!
//! - the young thin disc, the old thin disc's five sub-discs, the thick disc and the nuclear disc
//!   as double exponentials ([`disc`]), the young disc with the sharp arm and the sub-discs with
//!   the gentle one ([`arms`]);
//! - the boxy bulge `exp(−m)` ([`bulge`]) and the long bar ([`bar`]);
//! - the halo as a marked mixture of cored, flattened power laws, one component per smooth halo
//!   component ([`halo`]).
//!
//! Each [`Component`]'s density is normalised to its own share of the galaxy's systems: a
//! sub-disc's share of the old thin disc, a halo component's share of the halo. A still-forming
//! component, the young and the nuclear discs, is normalised so that its systems born at the
//! epoch are its share, and its unborn sliver is extra (plan 02, Design note 13). Discs, the bulge
//! and the bar are normalised over all space and lose their tails beyond the root cube; a halo
//! component is cut at 65,000 ly (50,000 ly in situ) and normalised inside the cut (Design note
//! 11).
//!
//! Every envelope never rises with |x|, |y| or |z|, so its maximum over a cell that straddles no
//! axis plane is at the corner nearest the origin; only the arm factors, which are unimodal in the
//! arm phase, need more ([`bounds`](super::bounds), plan 02, P02.T8). [`Component::envelope`] and
//! [`Component::arm`] expose the two parts, and a density is exactly their product.
//!
//! # Order
//!
//! The components come in a fixed order, which is part of the generator version because every sum
//! over them runs in it (Design note 18): the young thin disc, the old thin disc's sub-discs from
//! youngest to oldest, the thick disc, the bulge, the long bar, the nuclear disc, then the halo's
//! components in their own order (in situ, the dominant merger, the lesser progenitors by number,
//! the globular-born debris). That is 15 to 18 components, under [`MAX_COMPONENTS`].
//!
//! # Examples
//!
//! ```
//! use hyperion_sim::galaxy::PointLy;
//! use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS};
//! use hyperion_sim::galaxy::params::GalaxyParams;
//! use hyperion_sim::galaxy::potential::MassModel;
//!
//! let params = GalaxyParams::milky_way_like();
//! let fields = Fields::new(&params, &MassModel::new(&params));
//! // Every component's density at a point in the disc, into a buffer on the stack.
//! let mut densities = [0.0; MAX_COMPONENTS];
//! let total = fields.densities(&PointLy::new(22_000.0, 13_000.0, 60.0), &mut densities);
//! assert!((0.001..0.009).contains(&total), "{total} per ly³");
//! // The components' systems add up to the galaxy's.
//! let count: f64 = fields.components().iter().map(|c| c.count()).sum();
//! assert!((count / params.system_count() - 1.0).abs() < 1e-12);
//! ```

pub mod arms;
pub mod bar;
pub mod bulge;
pub mod disc;
pub mod halo;
pub mod metallicity;
mod sub_discs;

use std::error::Error;
use std::fmt;

pub use metallicity::FehDistribution;
pub use sub_discs::SubDiscHeights;

use self::arms::{Arm, ArmGeometry, ArmPoint, GentleArm, SharpArm};
use self::bar::LongBar;
use self::bulge::BoxyBulge;
use self::disc::DoubleExponential;
use self::halo::HaloProfile;
use self::metallicity::Metallicity;
use super::ages::{
    AgeDistribution, BULGE_AGES, FeatureShare, LONG_BAR_AGES, SubDisc, THICK_DISC_AGES,
};
use super::imf::MassBand;
use super::params::{GalaxyParams, HaloComponentKind};
use super::potential::MassModel;
use super::shares::ShareMatrix;
use super::{PointLy, Population};
use crate::units::Years;

/// The most components a galaxy's fields hold: the size of the buffer
/// [`Fields::densities`] fills. A galaxy has 15 to 18 today; the rest is room for later plans.
pub const MAX_COMPONENTS: usize = 24;

/// A density component could not be built: `quantity` is `value`, which is not finite or lies
/// outside its range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildFieldError {
    quantity: &'static str,
    value: f64,
}

impl BuildFieldError {
    pub(crate) fn new(quantity: &'static str, value: f64) -> Self {
        Self { quantity, value }
    }

    /// `Ok` if `value` is finite and above zero.
    pub(crate) fn check_positive(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() && value > 0.0 {
            Ok(())
        } else {
            Err(Self::new(quantity, value))
        }
    }

    /// `Ok` if `value` is finite and not negative.
    pub(crate) fn check_non_negative(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() && value >= 0.0 {
            Ok(())
        } else {
            Err(Self::new(quantity, value))
        }
    }

    /// `Ok` if `value` lies in `[0, 1]`.
    pub(crate) fn check_fraction(quantity: &'static str, value: f64) -> Result<(), Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(Self::new(quantity, value))
        }
    }

    /// What was wrong, such as `arm width` or `scale height`.
    #[must_use]
    pub fn quantity(&self) -> &'static str {
        self.quantity
    }

    /// The value given.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl fmt::Display for BuildFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a density field's {} is {}, outside its range",
            self.quantity, self.value
        )
    }
}

impl Error for BuildFieldError {}

/// The quantities of a point that several components read, computed once per point.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Site {
    pub x: f64,
    pub y: f64,
    pub abs_x: f64,
    pub abs_y: f64,
    pub abs_z: f64,
    /// `x² + y²`, ly².
    pub r_sq: f64,
    /// `√(x² + y²)`, ly.
    pub r: f64,
}

impl Site {
    pub(crate) fn new(p: &PointLy) -> Self {
        let r_sq = p.x * p.x + p.y * p.y;
        Self {
            x: p.x,
            y: p.y,
            abs_x: p.x.abs(),
            abs_y: p.y.abs(),
            abs_z: p.z.abs(),
            r_sq,
            r: r_sq.sqrt(),
        }
    }
}

/// A component's closed-form density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A double-exponential disc, with or without arms.
    Disc(DoubleExponential),
    /// The boxy bulge.
    Bulge(BoxyBulge),
    /// The long bar.
    Bar(LongBar),
    /// A smooth halo component.
    Halo(HaloProfile),
}

impl Shape {
    /// The density at `site`, with the arm quantities `arm` (read by discs with arms only).
    fn density_at(&self, site: &Site, arm: &ArmPoint) -> f64 {
        match self {
            Self::Disc(disc) => disc.density_at(site, arm),
            Self::Bulge(bulge) => bulge.density_at(site),
            Self::Bar(bar) => bar.density_at(site),
            Self::Halo(halo) => halo.density_at(site),
        }
    }

    /// The density without any arm factor at `site`.
    pub(crate) fn envelope_at(&self, site: &Site) -> f64 {
        match self {
            Self::Disc(disc) => disc.envelope(site.r, site.abs_z),
            Self::Bulge(bulge) => bulge.density_at(site),
            Self::Bar(bar) => bar.density_at(site),
            Self::Halo(halo) => halo.density_at(site),
        }
    }

    /// The disc's arm, if the shape is a disc with arms.
    fn arm(&self) -> Option<&Arm> {
        match self {
            Self::Disc(disc) => disc.arm(),
            Self::Bulge(_) | Self::Bar(_) | Self::Halo(_) => None,
        }
    }
}

/// A component's place in [`Fields::components`]: what placement's pick returns and what the ID's
/// origin records (plan 03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(u8);

impl ComponentId {
    /// The component's index in [`Fields::components`], below [`MAX_COMPONENTS`].
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// One density component: a population, or one sub-disc or halo component of it, with its density,
/// ages and metallicity.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    population: Population,
    shape: Shape,
    count: f64,
    ages: AgeDistribution,
    metallicity: Metallicity,
    sub_disc: Option<SubDisc>,
    halo: Option<HaloComponentKind>,
}

impl Component {
    /// The population the component belongs to: the population of every system it places.
    #[must_use]
    pub fn population(&self) -> Population {
        self.population
    }

    /// The closed form of the density.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// The number density at `p`, systems per cubic light-year, all mass bands together, at the
    /// epoch, φ = 0. It is exactly what [`Fields::densities`] returns for the component.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let site = Site::new(p);
        let arm = match self.shape.arm() {
            Some(arm) => arm.geometry().point_with(site.x, site.y, site.r_sq, site.r),
            None => ArmPoint::CENTRE,
        };
        self.shape.density_at(&site, &arm)
    }

    /// The density without its arm factor at `p`, systems per cubic light-year: the envelope,
    /// which never rises with |x|, |y| or |z|. For a component without arms it is the density.
    #[must_use]
    pub fn envelope(&self, p: &PointLy) -> f64 {
        self.shape.envelope_at(&Site::new(p))
    }

    /// The arm factor that multiplies the envelope: sharp for the young disc, gentle for the old
    /// thin disc's sub-discs, none elsewhere.
    #[must_use]
    pub fn arm(&self) -> Option<&Arm> {
        self.shape.arm()
    }

    /// The distribution of the ages of the component's systems at the epoch.
    #[must_use]
    pub fn ages(&self) -> &AgeDistribution {
        &self.ages
    }

    /// The distribution of \[Fe/H\] among the component's systems of `age` at `p`.
    #[must_use]
    pub fn metallicity(&self, p: &PointLy, age: Years) -> FehDistribution {
        self.metallicity.at((p.x * p.x + p.y * p.y).sqrt(), age)
    }

    /// The halo component a system of this component belongs to: the mark of the halo's mixture;
    /// `None` outside the halo.
    #[must_use]
    pub fn halo_component(&self) -> Option<HaloComponentKind> {
        self.halo
    }

    /// The old thin disc's sub-disc this component is; `None` for every other component.
    #[must_use]
    pub fn sub_disc(&self) -> Option<SubDisc> {
        self.sub_disc
    }

    /// The expected number of the component's systems born at the epoch: its share of the
    /// galaxy's system count.
    #[must_use]
    pub fn count(&self) -> f64 {
        self.count
    }

    /// The integral of the density: [`count`](Self::count) and, for a still-forming component,
    /// the systems born during the clock window after the epoch.
    #[must_use]
    pub fn count_with_unborn(&self) -> f64 {
        self.count / self.ages.born_fraction()
    }
}

/// The galaxy's density fields: its components in their fixed order, and the arms they share.
///
/// Built once per galaxy, immutable, and a pure function of the parameters and the mass model.
/// Solving the sub-discs' heights costs some 50 `K_z` evaluations of the mass model, most of the
/// build (plan 02, Design note 9).
#[derive(Debug, Clone, PartialEq)]
pub struct Fields {
    components: Vec<Component>,
    arms: ArmGeometry,
    sub_discs: SubDiscHeights,
}

impl Fields {
    /// The fields of the galaxy `params`, whose mass model is `model`.
    ///
    /// # Panics
    ///
    /// Never for built parameters: every size, share and amplitude they hold is inside its range.
    #[must_use]
    pub fn new(params: &GalaxyParams, model: &MassModel) -> Self {
        let sub_discs = SubDiscHeights::solve(params, model);
        let mut components = Vec::with_capacity(10 + params.halo().components().len());
        push_thin_discs(params, &sub_discs, &mut components);
        push_inner_populations(params, &mut components);
        push_halo(params, &mut components);
        assert!(
            components.len() <= MAX_COMPONENTS,
            "{} components exceed MAX_COMPONENTS",
            components.len()
        );
        let arms = ArmGeometry::of(params);
        debug_assert!(
            components
                .iter()
                .filter_map(Component::arm)
                .all(|arm| *arm.geometry() == arms),
            "every arm follows the galaxy's geometry"
        );
        Self {
            components,
            arms,
            sub_discs,
        }
    }

    /// The components, in their fixed order (module documentation, "Order").
    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// The component `id`.
    ///
    /// # Panics
    ///
    /// If `id` does not index these fields' components, as an ID from a galaxy with more
    /// components would not.
    #[must_use]
    pub fn component(&self, id: ComponentId) -> &Component {
        &self.components[id.index()]
    }

    /// The ID of the component at `index`, if there is one.
    #[must_use]
    pub fn component_id(&self, index: usize) -> Option<ComponentId> {
        if index < self.components.len() {
            u8::try_from(index).ok().map(ComponentId)
        } else {
            None
        }
    }

    /// Every component's ID, in order.
    pub fn component_ids(&self) -> impl Iterator<Item = ComponentId> + use<> {
        (0..self.component_count()).map(ComponentId)
    }

    /// The number of components, at most [`MAX_COMPONENTS`], which [`Fields::new`] asserts.
    fn component_count(&self) -> u8 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Fields::new asserts at most MAX_COMPONENTS = 24 components"
        )]
        let count = self.components.len() as u8;
        count
    }

    /// The arms' geometry, which the gas field shares (plan 07).
    #[must_use]
    pub fn arms(&self) -> &ArmGeometry {
        &self.arms
    }

    /// The old thin disc's sub-discs as the Jeans equation shaped them.
    #[must_use]
    pub fn sub_disc_heights(&self) -> &SubDiscHeights {
        &self.sub_discs
    }

    /// Every component's density at `p` into `out`, in component order, and their sum; entries
    /// past the last component are set to 0. Systems per cubic light-year, before layer shares.
    ///
    /// This is placement's innermost loop (plan 03): it evaluates the arm phase once for every
    /// disc, allocates nothing, and sums in component order, so the sum is the same bits as
    /// adding the entries in order. Each entry is the same bits as [`Component::density`].
    pub fn densities(&self, p: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> f64 {
        let site = Site::new(p);
        let arm = self.arms.point_with(site.x, site.y, site.r_sq, site.r);
        let (used, rest) = out.split_at_mut(self.components.len());
        let mut sum = 0.0;
        for (slot, component) in used.iter_mut().zip(&self.components) {
            let density = component.shape.density_at(&site, &arm);
            *slot = density;
            sum += density;
        }
        rest.fill(0.0);
        sum
    }

    /// The density of `population` at `p`, systems per cubic light-year: the sum of its
    /// components' in component order.
    #[must_use]
    pub fn population_density(&self, population: Population, p: &PointLy) -> f64 {
        let site = Site::new(p);
        let arm = self.arms.point_with(site.x, site.y, site.r_sq, site.r);
        self.components
            .iter()
            .filter(|c| c.population == population)
            .fold(0.0, |sum, c| sum + c.shape.density_at(&site, &arm))
    }

    /// The density of the layer of `band` at `p`, systems per cubic light-year: `Σ share ×
    /// density` over the components in order, each weighted by its population's share of the band
    /// (brainstorm, "Sizing the layers").
    #[must_use]
    pub fn layer_density(&self, shares: &ShareMatrix, band: MassBand, p: &PointLy) -> f64 {
        let site = Site::new(p);
        let arm = self.arms.point_with(site.x, site.y, site.r_sq, site.r);
        self.components.iter().fold(0.0, |sum, c| {
            sum + shares.component_share(band, c) * c.shape.density_at(&site, &arm)
        })
    }
}

/// Why a component of built parameters cannot fail to build.
const VALID: &str = "built parameters give valid components";

/// The young thin disc and the old thin disc's five sub-discs, youngest first.
fn push_thin_discs(
    params: &GalaxyParams,
    sub_discs: &SubDiscHeights,
    components: &mut Vec<Component>,
) {
    let n = params.system_count();
    let tau = params.sfh_timescale();
    let thin = params.thin_disc();
    let metallicity = Metallicity::thin_disc(params.metallicity_gradient(), thin.length());

    let young_ages = AgeDistribution::young_disc(tau, FeatureShare::None).expect(VALID);
    let young = n * params.population_share(Population::YoungThinDisc);
    components.push(Component {
        population: Population::YoungThinDisc,
        shape: Shape::Disc(
            DoubleExponential::new(
                young / young_ages.born_fraction(),
                params.young_disc().length(),
                params.young_disc().height(),
                Some(Arm::Sharp(SharpArm::young_disc(params))),
            )
            .expect(VALID),
        ),
        count: young,
        ages: young_ages,
        metallicity,
        sub_disc: None,
        halo: None,
    });

    let old = n * params.population_share(Population::OldThinDisc);
    let gentle = Arm::Gentle(GentleArm::old_disc(params));
    let (shares, heights) = (sub_discs.shares(), sub_discs.heights());
    for ((bin, share), height) in SubDisc::ALL.into_iter().zip(shares).zip(heights) {
        let count = old * share;
        components.push(Component {
            population: Population::OldThinDisc,
            shape: Shape::Disc(
                DoubleExponential::new(count, thin.length(), height, Some(gentle)).expect(VALID),
            ),
            count,
            ages: AgeDistribution::old_thin_disc(tau, bin).expect(VALID),
            metallicity,
            sub_disc: Some(bin),
            halo: None,
        });
    }
}

/// The thick disc, the bulge, the long bar and the nuclear disc, in that order.
fn push_inner_populations(params: &GalaxyParams, components: &mut Vec<Component>) {
    let n = params.system_count();
    let uniform = |[lo, hi]: [Years; 2]| AgeDistribution::uniform(lo, hi).expect(VALID);
    let mut push = |population, shape: Shape, ages: AgeDistribution, feh| {
        components.push(Component {
            population,
            shape,
            count: n * params.population_share(population),
            ages,
            metallicity: Metallicity::Fixed(feh),
            sub_disc: None,
            halo: None,
        });
    };
    let count = |population| n * params.population_share(population);

    let thick = params.thick_disc();
    push(
        Population::ThickDisc,
        Shape::Disc(
            DoubleExponential::new(
                count(Population::ThickDisc),
                thick.length(),
                thick.height(),
                None,
            )
            .expect(VALID),
        ),
        uniform(THICK_DISC_AGES),
        metallicity::THICK_DISC,
    );
    push(
        Population::Bulge,
        Shape::Bulge(BoxyBulge::of(count(Population::Bulge), params.bulge()).expect(VALID)),
        uniform(BULGE_AGES),
        metallicity::BULGE,
    );
    push(
        Population::LongBar,
        Shape::Bar(LongBar::of(count(Population::LongBar), params.bar()).expect(VALID)),
        uniform(LONG_BAR_AGES),
        metallicity::LONG_BAR,
    );
    // Still forming: the unborn sliver is extra (plan 02, Design note 13).
    let nuclear_ages = AgeDistribution::nuclear_disc();
    let nuclear = params.nuclear_disc();
    push(
        Population::NuclearDisc,
        Shape::Disc(
            DoubleExponential::new(
                count(Population::NuclearDisc) / nuclear_ages.born_fraction(),
                nuclear.length(),
                nuclear.height(),
                None,
            )
            .expect(VALID),
        ),
        nuclear_ages,
        metallicity::NUCLEAR_DISC,
    );
}

/// One component per smooth halo component, in the halo's order.
///
/// The discrete share is drawn but not yet applied: the smooth components carry the whole halo
/// (plan 02, Design note 12).
fn push_halo(params: &GalaxyParams, components: &mut Vec<Component>) {
    let halo = params.system_count() * params.population_share(Population::Halo);
    for c in params.halo().components() {
        let count = halo * c.share();
        components.push(Component {
            population: Population::Halo,
            shape: Shape::Halo(HaloProfile::of(count, c).expect(VALID)),
            count,
            ages: c.ages().clone(),
            metallicity: Metallicity::Fixed(FehDistribution::new(
                c.feh_mean(),
                metallicity::HALO_SIGMA,
            )),
            sub_disc: None,
            halo: Some(c.kind()),
        });
    }
}
