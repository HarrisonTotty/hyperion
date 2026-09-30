//! The stellar halo as a marked mixture of smooth components.

use crate::galaxy::ages::AgeDistribution;
use crate::units::{Dex, LightYears};

/// Which smooth component of the halo a system belongs to: a derived mark, as the population is
/// (brainstorm, "Streams and accreted structure").
///
/// The components come in a fixed order, which is also the item number `n` of their
/// `galaxy.params.halo.component.*` streams ([`item`](Self::item)): in situ 0, the dominant merger
/// 1, the lesser progenitors 2–6, the globular-born debris 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HaloComponentKind {
    /// The heated early disc: flattened, prograde, \[Fe/H\] near −0.6.
    InSitu,
    /// The dominant ancient merger: radial orbits, no net rotation, \[Fe/H\] near −1.2, a break
    /// where its stars pile up at apocentre.
    DominantMerger,
    /// Lesser old progenitor `n`, 1–5, each with its own rotation, metallicity and age.
    Lesser(u8),
    /// Debris of dissolved globular clusters, steeper inward.
    GlobularDebris,
}

impl HaloComponentKind {
    /// The component's item number in its streams' keys: in situ 0, dominant 1, lesser `n` at
    /// `1 + n`, debris 7.
    #[must_use]
    pub fn item(self) -> u64 {
        match self {
            Self::InSitu => 0,
            Self::DominantMerger => 1,
            Self::Lesser(n) => 1 + u64::from(n),
            Self::GlobularDebris => 7,
        }
    }
}

/// Where the dominant merger's slope steepens: its stars pile up at their apocentres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HaloBreak {
    pub(super) radius: LightYears,
    pub(super) steepening: f64,
}

impl HaloBreak {
    /// The ellipsoidal radius beyond which the slope steepens, 52,000–91,000 ly (16–28 kpc).
    #[must_use]
    pub fn radius(&self) -> LightYears {
        self.radius
    }

    /// How much the power-law slope steepens beyond the break, 1.5–2.5.
    #[must_use]
    pub fn steepening(&self) -> f64 {
        self.steepening
    }
}

/// One smooth component of the halo: a cored power law `(1 + m² ÷ a²)^(−γ ÷ 2)` in the
/// ellipsoidal radius `m² = x² + y² + z² ÷ q²`, broken beyond a radius for the dominant merger, and
/// cut at a radius.
#[derive(Debug, Clone, PartialEq)]
pub struct HaloComponentParams {
    pub(super) kind: HaloComponentKind,
    pub(super) share: f64,
    pub(super) slope: f64,
    pub(super) core: LightYears,
    pub(super) flattening: f64,
    pub(super) outer_break: Option<HaloBreak>,
    pub(super) cut_radius: LightYears,
    pub(super) ages: AgeDistribution,
    pub(super) feh_mean: Dex,
}

impl HaloComponentParams {
    /// Which component this is.
    #[must_use]
    pub fn kind(&self) -> HaloComponentKind {
        self.kind
    }

    /// The component's share of the halo's smooth systems, after renormalising: the shares of all
    /// components sum to 1.
    #[must_use]
    pub fn share(&self) -> f64 {
        self.share
    }

    /// The power-law slope γ inside any break: 2.2–2.8, or 4.0–4.5 for the globular-born debris.
    #[must_use]
    pub fn slope(&self) -> f64 {
        self.slope
    }

    /// The core radius `a`.
    #[must_use]
    pub fn core(&self) -> LightYears {
        self.core
    }

    /// The vertical axis ratio `q`, at most 1: 0.45–0.55 in situ, 0.6–0.8 for the dominant
    /// merger, 0.6–1.0 for a lesser progenitor, 1 for the debris.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        self.flattening
    }

    /// The dominant merger's break; `None` for every other component.
    #[must_use]
    pub fn outer_break(&self) -> Option<HaloBreak> {
        self.outer_break
    }

    /// The radius of the sphere outside which the component has no systems: 50,000 ly in situ,
    /// 65,000 ly otherwise, so that the halo fits inside the root cube (brainstorm,
    /// "Populations"; plan 02, Design note 11, with the sphere of
    /// [`HaloProfile`](crate::galaxy::fields::halo::HaloProfile) in place of its ellipsoid).
    #[must_use]
    pub fn cut_radius(&self) -> LightYears {
        self.cut_radius
    }

    /// The component's ages: uniform over one gigayear inside 10–13 Gyr. The in-situ and dominant
    /// components' youngest stars are no younger than the last major merger, and a lesser
    /// progenitor's no younger than its accretion.
    #[must_use]
    pub fn ages(&self) -> &AgeDistribution {
        &self.ages
    }

    /// The component's mean \[Fe/H\]: −0.6 in situ, −1.2 for the dominant merger, −2.0 to −1.0 for
    /// a lesser progenitor, −1.5 for the debris (brainstorm, "Streams and accreted structure";
    /// plan 02, P02.T7.e, marked there for re-checking).
    #[must_use]
    pub fn feh_mean(&self) -> Dex {
        self.feh_mean
    }
}

/// The halo's smooth components and its discrete share.
#[derive(Debug, Clone, PartialEq)]
pub struct HaloParams {
    pub(super) components: Vec<HaloComponentParams>,
    pub(super) discrete_share: f64,
}

impl HaloParams {
    /// The smooth components in their fixed order: in situ, dominant merger, the lesser
    /// progenitors by number, globular-born debris.
    #[must_use]
    pub fn components(&self) -> &[HaloComponentParams] {
        &self.components
    }

    /// The share of the halo in discrete structures, streams and dwarf cores, 2–15%.
    ///
    /// Drawn now, but held at zero until plan 10: the smooth components carry the whole halo
    /// budget (plan 02, Design note 12).
    #[must_use]
    pub fn discrete_share(&self) -> f64 {
        self.discrete_share
    }
}
