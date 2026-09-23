//! Stable zones in multiple systems, and the orbit hosts and discs they give planets (plan 14,
//! P14.T9; design note 10).
//!
//! In a binary, planets survive only close around one star (S-type) or wide around both
//! (P-type), as the brainstorm says after Holman and Wiegert (1999). This module applies their
//! two fits at every level of a hierarchy: around a single component out to the S-type limit set
//! by its companion at that level, and around a pair from the P-type limit outward, each bounded
//! in turn by every level above it. Each zone is an independent orbit host, with its own disc
//! share and class draw.
//!
//! # The fits (P14.T9.a)
//!
//! [`holman_wiegert_s_type`] and [`holman_wiegert_p_type`] are equations 1 and 3 of Holman and
//! Wiegert (1999, AJ 117, 621), with every coefficient re-checked against the paper
//! ([`HOLMAN_WIEGERT_S_TYPE`], [`HOLMAN_WIEGERT_P_TYPE`]). Both give the critical semi-major axis
//! of a planet on a circular, prograde orbit in the binary's plane, in units of the binary's
//! semi-major axis, from the binary's eccentricity e and a mass ratio μ = m₂ ÷ (m₁ + m₂), where
//! for the S-type fit m₁ is the star the planet orbits and m₂ the perturbing star (their §2). They
//! were fitted to integrations of 10⁴ binary periods over 0.1 ≤ μ ≤ 0.9 and 0 ≤ e ≤ 0.8 for the
//! S-type region (their Table 3; within 4% typically and 11% at worst) and 0.1 ≤ μ ≤ 0.5 and
//! 0 ≤ e ≤ 0.7 for the P-type region (their Table 7; within 3% and 6%), the outer region being
//! symmetric under μ ↔ 1 − μ (their §2).
//!
//! Outside those ranges plan 14 clamps the arguments, "which errs towards smaller zones". That
//! holds for two of the four edges and not for the other two, so this module clamps where
//! clamping errs smaller and continues each fit where it would not:
//!
//! - S-type, μ < 0.1 (a light perturber, such as a brown dwarf): clamped to 0.1. The limit falls
//!   with μ throughout the fitted range, so the clamp gives a smaller zone than the fit would.
//! - S-type, μ > 0.9 (a light host with a heavy companion): the value at 0.9 times
//!   ((1 − μ) ÷ 0.1)^⅓, the Hill-sphere scaling that Holman and Wiegert find the limit takes up
//!   there (their §3.1 and Fig. 1). They measured it on circular binaries only, so applying it at
//!   every eccentricity is this module's assumption. A clamp would hold the zone at its size for
//!   μ = 0.9 however light the host.
//! - S-type, e > 0.8: the value at 0.8 times ((1 − e) ÷ 0.2)^1.2, the eccentricity law of the
//!   invariant-loop limits of Pichardo et al. (2005), as Jaime, Aguilar and Pichardo (2014, MNRAS
//!   443, 260, eq. 11) give it, R = `R_Egg` 0.733 (1 − e)^1.2 q^0.07. For equal masses that law
//!   is within 1.4% and 2.7% of Holman and Wiegert's fit at e = 0 and 0.5, and gives their Table
//!   3's 0.04 at e = 0.8, where the fit gives 0.036. The zone closes
//!   as e → 1, a little faster than the companion's pericentre. A clamp would keep it at its size
//!   for e = 0.8 while the pericentre closed in. (The fit's own slope at e = 0.8 is (1 − e)^1.4; for
//!   γ Vir, at e = 0.881, Holman and Wiegert extrapolate the polynomial itself, to 0.61 au, where
//!   this gives 0.74.)
//! - P-type: μ is folded to min(μ, 1 − μ), as the paper's symmetry requires: the polynomial is
//!   not symmetric, and unfolded, μ = 0.9 on a circular orbit would give 1.19 binary separations
//!   against the 1.96 of its mirror image μ = 0.1. The folded μ is then held to at least 0.1. The
//!   inner limit rises with μ at 0.1, so the clamp gives a smaller zone.
//! - P-type, e > 0.7: the value at 0.7 times (1 + e) ÷ 1.7, the same multiple of the binary's
//!   apocentre separation, which Table 7 holds at 2.3–2.6 from e = 0.5 to 0.7. A clamp would let
//!   the zone reach in while the binary's apocentre moved out. The circumbinary law of Jaime et al.
//!   (2014, eq. 12) grows more slowly with e, so this errs towards the smaller zone.
//!
//! Every continuation is continuous with the fit at its edge, and inside the fitted ranges both
//! functions are the published polynomials exactly.
//!
//! # Zones from a hierarchy (P14.T9.b)
//!
//! [`stable_zones`] walks a [`ZoneHierarchy`] and returns one [`OrbitZone`] per component and per
//! pair, each with its host, the host's mass and its inner and outer limits:
//!
//! - A component's zone has no inner limit (its disc's inner edge bounds it). Its outer limit is
//!   the S-type limit a × `holman_wiegert_s_type(μ, e)` of the pair it is a member of, with μ its
//!   companion's share of the pair's mass, the companion being a star or a whole inner pair.
//! - A pair's zone starts at the P-type limit a × `holman_wiegert_p_type(μ, e)` of its own orbit.
//! - Every zone below the top of the hierarchy is also bounded by the zone of the pair it is a
//!   member of, taken about that pair's barycentre: its outer limit is at most that zone's outer
//!   limit less its own greatest distance from the barycentre, a (1 + e) μ. That is
//!   what makes the zones of a triple disjoint however the fits compare: a circumstellar zone of
//!   an inner pair lies inside the zone that pair has as a member of the outer one.
//! - The top of the hierarchy has no outer limit: a single star's one zone is bounded only by its
//!   disc and, when P14.T29 lands, by design note 14's strip radius.
//! - A pair's zone narrower than a factor of [`MIN_ZONE_WIDTH`] (1.5) in radius is dropped, and so
//!   is a component's zone with no room at all, which only a hierarchy that is not itself stable
//!   can give.
//!
//! Zones come in hierarchy order, inside out: depth first with a pair's inner member before its
//! outer one, each pair after its members. Under plan 11's numbering of the components (its design
//! note 5: depth first, inner before outer) the stars' zones therefore come in the order of their
//! body indices, and every pair's k below is 1–15. [`ZoneHierarchy::new`] does not check that
//! numbering, only that the indices run 0 to n − 1 and that no two pairs share a k. A star's zone is host [`Star(n)`](OrbitHost::Star), its
//! body index n, and host number n; a pair's is [`Pair(k)`](OrbitHost::Pair), k being the
//! lowest-indexed component of its outer member, the star plan 11 keys the pair's own streams by,
//! and host number 16 + k. So a single star is host 0, as plan 14's disc has it, and a star's disc
//! draws are the same whether it is single or has a companion.
//!
//! # Discs and hosts (P14.T9.c)
//!
//! [`ZoneDiscInputs::for_zone`] gathers what T3's [`disc::derive`] reads for one zone: a
//! circumstellar disc from its star's zero-age state, with the star's own disc lifetime (plan
//! 06's law on the star's `star.disc_lifetime` rank, ruling 33); a circumbinary disc from the
//! pair's total mass and summed zero-age luminosity, with a lifetime from the rank the zone's
//! host draws on `planet.disc`, under the same law at the total mass (design note 12). Both are
//! truncated to the zone's limits. The draws are [`DiscDraws::for_host`] of the zone's
//! [`host_number`](OrbitZone::host_number).
//!
//! [`OrbitZone::in_close_binary`] is design note 10's flag for the class draw (P14.T4.c): a zone
//! whose host is a member of a pair closer than [`CLOSE_BINARY_SEMI_MAJOR_AXIS`] (47 au) draws
//! `Barren` with added weight, so that its planets are about a third as common as a single
//! star's (Kraus et al. 2016). The class weights are `planetary::architecture`'s.
//!
//! # What this reads of plan 11, and the slice
//!
//! Plan 14 sketches `stable_zones(&SystemHierarchy)`. Plan 11's `SystemHierarchy` (P11.T2.a) is
//! not built yet, so the zones read a [`ZoneHierarchy`]: the least of it they need, the
//! components' body indices, initial masses and kinds, and each pair's members, semi-major axis
//! and eccentricity, built from [`ZoneNode`]s. When `SystemHierarchy` lands it converts into one
//! by a walk from its root (each `StarSlot` a [`ZoneNode::component`], each
//! `HierarchyNode::Pair` a [`ZoneNode::pair`] of its orbit's `semi_major_axis()` and
//! `eccentricity()`), and `stable_zones` takes it through that adapter. The zones are those at
//! birth: plan 11's binary evolution is deferred, so every pair is two single stars on a fixed
//! orbit, and design note 10's intersection with the evaluated state comes with `BinaryState`.

use std::error::Error;
use std::fmt;

use crate::id::SystemId;
use crate::math;
use crate::orbit::Eccentricity;
use crate::planetary::disc::{self, BuildDiscHostError, Disc, DiscDraws, DiscHost, Truncation};
use crate::planetary::index::{BodyIndex, STELLAR_SUB_END};
use crate::rng::Seed;
use crate::stellar::draws::UnitUniform;
use crate::stellar::premain;
use crate::units::{
    AstronomicalUnits, Dex, Megayears, Metres, SolarLuminosities, SolarMasses, SolarRadii,
};

/// The coefficients of Holman and Wiegert's S-type fit, in the order of their equation 1:
/// `a_c ÷ a_b` = 0.464 − 0.380μ − 0.631e + 0.586μe + 0.150e² − 0.198μe².
///
/// Holman and Wiegert (1999, AJ 117, 621, eq. 1), re-checked against the paper: (0.464 ± 0.006),
/// (−0.380 ± 0.010), (−0.631 ± 0.034), (0.586 ± 0.061), (0.150 ± 0.041) and (−0.198 ± 0.074), for
/// 0.1 ≤ μ ≤ 0.9 and 0 ≤ e ≤ 0.8, within 4% of their Table 3 typically and 11% at worst.
pub const HOLMAN_WIEGERT_S_TYPE: [f64; 6] = [0.464, -0.380, -0.631, 0.586, 0.150, -0.198];

/// The coefficients of Holman and Wiegert's P-type fit, in the order of their equation 3:
/// `a_c ÷ a_b` = 1.60 + 5.10e − 2.22e² + 4.12μ − 4.27eμ − 5.09μ² + 4.61e²μ².
///
/// Holman and Wiegert (1999, eq. 3), re-checked against the paper: (1.60 ± 0.04), (5.10 ± 0.05),
/// (−2.22 ± 0.11), (4.12 ± 0.09), (−4.27 ± 0.17), (−5.09 ± 0.11) and (4.61 ± 0.36), fitted to
/// their Table 7, 0.1 ≤ μ ≤ 0.5 and 0 ≤ e ≤ 0.7, within 3% typically and 6% at worst.
pub const HOLMAN_WIEGERT_P_TYPE: [f64; 7] = [1.60, 5.10, -2.22, 4.12, -4.27, -5.09, 4.61];

/// The least mass ratio of the S-type fit's range, 0.1; below it μ is clamped.
pub const S_TYPE_MIN_MASS_RATIO: f64 = 0.1;

/// The greatest mass ratio of the S-type fit's range, 0.9; above it the limit takes the Hill
/// scaling ((1 − μ) ÷ 0.1)^⅓ (Holman and Wiegert 1999, §3.1, measured at e = 0).
pub const S_TYPE_MAX_MASS_RATIO: f64 = 0.9;

/// The greatest eccentricity of the S-type fit's range, 0.8; above it the limit scales as
/// ((1 − e) ÷ 0.2)^[`S_TYPE_ECCENTRICITY_EXPONENT`].
pub const S_TYPE_MAX_ECCENTRICITY: f64 = 0.8;

/// The power of (1 − e) that an S-type limit follows past the fitted range, 1.2 (Jaime, Aguilar
/// and Pichardo 2014, MNRAS 443, 260, eq. 11, after the invariant loops of Pichardo, Sparke and
/// Aguilar 2005).
pub const S_TYPE_ECCENTRICITY_EXPONENT: f64 = 1.2;

/// The least folded mass ratio of the P-type fit's range, 0.1; below it μ is clamped.
pub const P_TYPE_MIN_MASS_RATIO: f64 = 0.1;

/// The greatest eccentricity of the P-type fit's range, 0.7; above it the limit scales with the
/// binary's apocentre separation, (1 + e) ÷ 1.7.
pub const P_TYPE_MAX_ECCENTRICITY: f64 = 0.7;

/// The least ratio of a zone's outer limit to its inner one, 1.5 (P14.T9.b): a narrower zone is
/// dropped.
pub const MIN_ZONE_WIDTH: f64 = 1.5;

/// The semi-major axis inside which a companion suppresses planets: 47 au (design note 10).
///
/// Kraus et al. (2016, AJ 152, 8, abstract): inside `a_cut` = 47 (+59, −23) au, planets occur
/// in binaries at `S_bin` = 0.34 (+0.14, −0.15) of the rate of wider binaries and single stars.
/// Plan 14 rounds the cut to "about 50 au"; the measured value is used here.
pub const CLOSE_BINARY_SEMI_MAJOR_AXIS: AstronomicalUnits = AstronomicalUnits::new(47.0);

/// Holman and Wiegert's equation 1 as published.
#[must_use]
fn s_type_fit(mu: f64, e: f64) -> f64 {
    let [c0, c1, c2, c3, c4, c5] = HOLMAN_WIEGERT_S_TYPE;
    c0 + c1 * mu + c2 * e + c3 * mu * e + c4 * e * e + c5 * mu * e * e
}

/// Holman and Wiegert's equation 3 as published.
#[must_use]
fn p_type_fit(mu: f64, e: f64) -> f64 {
    let [c0, c1, c2, c3, c4, c5, c6] = HOLMAN_WIEGERT_P_TYPE;
    c0 + c1 * e + c2 * e * e + c3 * mu + c4 * e * mu + c5 * mu * mu + c6 * e * e * mu * mu
}

/// The outer limit of the stable zone around one star of a binary, in units of the binary's
/// semi-major axis: Holman and Wiegert's S-type fit (1999, eq. 1).
///
/// `mu` is the perturbing star's share of the pair's mass, m₂ ÷ (m₁ + m₂), with m₁ the star the
/// planet orbits, and `e` is the binary's eccentricity. Inside 0.1 ≤ μ ≤ 0.9 and e ≤ 0.8 this is
/// the published polynomial exactly; outside them it is clamped or continued towards a smaller
/// zone as the [module documentation](self) sets out, and it falls to 0 as μ or e reaches 1.
///
/// # Panics
///
/// In debug builds, if `mu` is outside 0–1 or `e` outside `[0, 1)`.
///
/// # Examples
///
/// α Centauri A (1.12 M☉) with B (0.95 M☉) 23.57 au away at e = 0.516 keeps planets inside
/// 2.79 au, as Holman and Wiegert's Table 4 gives:
///
/// ```
/// use hyperion_sim::planetary::placement::holman_wiegert_s_type;
///
/// let mu = 0.95 / (1.12 + 0.95);
/// let limit_au = 23.57 * holman_wiegert_s_type(mu, 0.516);
/// assert!((limit_au - 2.79).abs() < 0.005);
/// // Equal masses on a circular orbit: 0.274 binary separations.
/// assert!((holman_wiegert_s_type(0.5, 0.0) - 0.274).abs() < 1e-12);
/// ```
#[must_use]
pub fn holman_wiegert_s_type(mu: f64, e: f64) -> f64 {
    debug_assert!((0.0..=1.0).contains(&mu), "a mass ratio, got {mu}");
    debug_assert!((0.0..1.0).contains(&e), "an eccentricity, got {e}");
    let hill = if mu > S_TYPE_MAX_MASS_RATIO {
        math::cbrt((1.0 - mu) / (1.0 - S_TYPE_MAX_MASS_RATIO))
    } else {
        1.0
    };
    let pericentre = if e > S_TYPE_MAX_ECCENTRICITY {
        math::powf(
            (1.0 - e) / (1.0 - S_TYPE_MAX_ECCENTRICITY),
            S_TYPE_ECCENTRICITY_EXPONENT,
        )
    } else {
        1.0
    };
    let fit = s_type_fit(
        mu.clamp(S_TYPE_MIN_MASS_RATIO, S_TYPE_MAX_MASS_RATIO),
        e.min(S_TYPE_MAX_ECCENTRICITY),
    );
    fit * hill * pericentre
}

/// The inner limit of the stable zone around both stars of a binary, in units of the binary's
/// semi-major axis: Holman and Wiegert's P-type fit (1999, eq. 3).
///
/// `mu` is either star's share of the pair's mass, which the fit's symmetry folds to the lighter
/// star's, and `e` is the binary's eccentricity. Inside 0.1 ≤ min(μ, 1 − μ) and e ≤ 0.7 this is
/// the published polynomial exactly; outside them it is clamped or continued towards a smaller
/// zone as the [module documentation](self) sets out.
///
/// # Panics
///
/// In debug builds, if `mu` is outside 0–1 or `e` outside `[0, 1)`.
///
/// # Examples
///
/// Equal masses on a circular orbit keep circumbinary planets beyond 2.39 separations, and an
/// eccentric pair pushes them out:
///
/// ```
/// use hyperion_sim::planetary::placement::holman_wiegert_p_type;
///
/// assert!((holman_wiegert_p_type(0.5, 0.0) - 2.3875).abs() < 1e-12);
/// assert!(holman_wiegert_p_type(0.5, 0.5) > 3.5);
/// // Either star's share gives the same limit.
/// assert!((holman_wiegert_p_type(0.3, 0.2) - holman_wiegert_p_type(0.7, 0.2)).abs() < 1e-12);
/// ```
#[must_use]
pub fn holman_wiegert_p_type(mu: f64, e: f64) -> f64 {
    debug_assert!((0.0..=1.0).contains(&mu), "a mass ratio, got {mu}");
    debug_assert!((0.0..1.0).contains(&e), "an eccentricity, got {e}");
    let apocentre = if e > P_TYPE_MAX_ECCENTRICITY {
        (1.0 + e) / (1.0 + P_TYPE_MAX_ECCENTRICITY)
    } else {
        1.0
    };
    let folded = mu.min(1.0 - mu).max(P_TYPE_MIN_MASS_RATIO);
    p_type_fit(folded, e.min(P_TYPE_MAX_ECCENTRICITY)) * apocentre
}

/// What a body orbits: the host of a zone, of a belt, or of a moon (plan 14's `OrbitHost`).
///
/// Zones are hosted by [`Star`](Self::Star) and [`Pair`](Self::Pair) only. The other two
/// variants are for the bodies of later tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrbitHost {
    /// The component with body index n (0–15) of the stellar level: a circumstellar zone.
    Star(u8),
    /// The pair whose outer member's lowest-indexed component is n, the component plan 11 keys the
    /// pair's own streams by (its design note 5), and 1–15 under its numbering: a circumbinary
    /// zone.
    Pair(u8),
    /// The barycentre of the whole system, for what is bound to the system but to no zone, such
    /// as the cometary halo of a multiple system (P14.T21). No zone has this host.
    Barycentre,
    /// A body, as the host of its moons and rings (P14.T17–T20). No zone has this host.
    Body(BodyIndex),
}

/// What kind of body a component of a hierarchy is: plan 11's `StarSlot` kind.
///
/// A brown-dwarf companion bounds zones exactly as a star does (design note 10); its kind is
/// carried so that its own zone can draw from the `SubstellarCompact` row (design note 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentKind {
    /// A star.
    Star,
    /// A brown dwarf.
    BrownDwarf,
}

/// A node of a [`ZoneHierarchy`]: a component, or a pair of nodes on a Keplerian orbit.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneNode {
    shape: Shape,
    mass: SolarMasses,
    /// The components under this node, a bit per body index; bits past 31 are dropped, and
    /// [`ZoneHierarchy::new`] rejects any index past 15.
    members: u32,
    /// The lowest body index under this node.
    lowest: u8,
}

#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Component {
        index: u8,
        kind: ComponentKind,
    },
    Pair {
        inner: Box<ZoneNode>,
        outer: Box<ZoneNode>,
        semi_major_axis: Metres,
        eccentricity: Eccentricity,
    },
}

impl ZoneNode {
    /// The component with body index `index` (0–15), of initial mass `mass` and kind `kind`.
    ///
    /// Checked by [`ZoneHierarchy::new`].
    #[must_use]
    pub fn component(index: u8, mass: SolarMasses, kind: ComponentKind) -> Self {
        Self {
            shape: Shape::Component { index, kind },
            mass,
            members: 1_u32.checked_shl(u32::from(index)).unwrap_or(0),
            lowest: index,
        }
    }

    /// The pair of `inner` and `outer` on a relative orbit of semi-major axis `semi_major_axis`
    /// and eccentricity `eccentricity`, at birth.
    ///
    /// `inner` is plan 11's "node inside", the member the orbit brings `outer` to. Its mass is the
    /// sum of the two, inner first. Checked by [`ZoneHierarchy::new`].
    #[must_use]
    pub fn pair(
        inner: Self,
        outer: Self,
        semi_major_axis: Metres,
        eccentricity: Eccentricity,
    ) -> Self {
        let mass = inner.mass + outer.mass;
        let members = inner.members | outer.members;
        let lowest = inner.lowest.min(outer.lowest);
        Self {
            shape: Shape::Pair {
                inner: Box::new(inner),
                outer: Box::new(outer),
                semi_major_axis,
                eccentricity,
            },
            mass,
            members,
            lowest,
        }
    }

    /// The node's mass: a component's initial mass, or the sum of a pair's.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// A system's hierarchy as the stable zones read it: plan 11's components and pairs, reduced to
/// the components' body indices, initial masses and kinds, and each pair's members and orbit.
///
/// It stands in for plan 11's `SystemHierarchy` until P11.T2.a lands (see the
/// [module documentation](self)), and is checked once, when built.
///
/// # Examples
///
/// α Centauri A and B:
///
/// ```
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::placement::{ComponentKind, ZoneHierarchy, ZoneNode};
/// use hyperion_sim::units::{AstronomicalUnits, Metres, SolarMasses};
///
/// let star = |i: u8, m: f64| ZoneNode::component(i, SolarMasses::new(m), ComponentKind::Star);
/// let a = Metres::from(AstronomicalUnits::new(23.57));
/// let pair = ZoneNode::pair(star(0, 1.12), star(1, 0.95), a, Eccentricity::new(0.516)?);
/// let hierarchy = ZoneHierarchy::new(pair)?;
/// assert_eq!(hierarchy.component_count(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneHierarchy {
    root: ZoneNode,
    component_count: u8,
}

impl ZoneHierarchy {
    /// The hierarchy under `root`.
    ///
    /// # Errors
    ///
    /// - [`BuildZoneHierarchyError::ComponentOutOfRange`] for a body index past 15, plan 11's
    ///   `STAR_BODY_INDEX_END`.
    /// - [`BuildZoneHierarchyError::DuplicateComponent`] for a body index used twice.
    /// - [`BuildZoneHierarchyError::MissingComponent`] unless the indices are 0 to n − 1.
    /// - [`BuildZoneHierarchyError::MassNotPositive`] for a component whose mass is not positive
    ///   and finite.
    /// - [`BuildZoneHierarchyError::SemiMajorAxisNotPositive`] for a pair whose semi-major axis
    ///   is not positive and finite.
    /// - [`BuildZoneHierarchyError::SharedPairKey`] if two pairs have the same lowest-indexed
    ///   component in their outer members, which plan 11's numbering never gives.
    pub fn new(root: ZoneNode) -> Result<Self, BuildZoneHierarchyError> {
        let mut census = Census::default();
        check(&root, &mut census)?;
        // `check` admits indices 0–15 once each, so every index is below 16 and, when they are not
        // 0 to count − 1, one below the count is missing.
        if let Some(component) = (0..census.count).find(|&i| census.seen & (1 << i) == 0) {
            return Err(BuildZoneHierarchyError::MissingComponent { component });
        }
        Ok(Self {
            root,
            component_count: census.count,
        })
    }

    /// A single component of mass `mass` and kind `kind`, with body index 0.
    ///
    /// # Errors
    ///
    /// [`BuildZoneHierarchyError::MassNotPositive`] unless the mass is positive and finite.
    pub fn single(mass: SolarMasses, kind: ComponentKind) -> Result<Self, BuildZoneHierarchyError> {
        Self::new(ZoneNode::component(0, mass, kind))
    }

    /// The number of components, 1–16.
    #[must_use]
    pub const fn component_count(&self) -> u8 {
        self.component_count
    }

    /// The top of the hierarchy.
    #[must_use]
    pub const fn root(&self) -> &ZoneNode {
        &self.root
    }
}

/// What [`check`] has seen so far: the components and the pairs' keys, a bit per body index, and
/// the number of components.
#[derive(Debug, Default)]
struct Census {
    seen: u32,
    keys: u32,
    count: u8,
}

/// Checks `node` and everything under it, counting its components and marking them and its pairs'
/// keys in `census`.
fn check(node: &ZoneNode, census: &mut Census) -> Result<(), BuildZoneHierarchyError> {
    match &node.shape {
        Shape::Component { index, .. } => {
            let component = *index;
            if component >= STELLAR_SUB_END {
                return Err(BuildZoneHierarchyError::ComponentOutOfRange { component });
            }
            let mass = node.mass.value();
            if !(mass.is_finite() && mass > 0.0) {
                return Err(BuildZoneHierarchyError::MassNotPositive { component });
            }
            let bit = 1_u32 << component;
            if census.seen & bit != 0 {
                return Err(BuildZoneHierarchyError::DuplicateComponent { component });
            }
            census.seen |= bit;
            census.count += 1;
        }
        Shape::Pair {
            inner,
            outer,
            semi_major_axis,
            ..
        } => {
            check(inner, census)?;
            check(outer, census)?;
            let a = semi_major_axis.value();
            if !(a.is_finite() && a > 0.0) {
                return Err(BuildZoneHierarchyError::SemiMajorAxisNotPositive);
            }
            let key = outer.lowest;
            let bit = 1_u32 << key;
            if census.keys & bit != 0 {
                return Err(BuildZoneHierarchyError::SharedPairKey { component: key });
            }
            census.keys |= bit;
        }
    }
    Ok(())
}

/// A [`ZoneHierarchy`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildZoneHierarchyError {
    /// A component's body index was past 15.
    ComponentOutOfRange {
        /// The index.
        component: u8,
    },
    /// A body index was used twice.
    DuplicateComponent {
        /// The index.
        component: u8,
    },
    /// The body indices were not 0 to n − 1.
    MissingComponent {
        /// The lowest index missing.
        component: u8,
    },
    /// A component's mass was not positive and finite.
    MassNotPositive {
        /// The component's index.
        component: u8,
    },
    /// A pair's semi-major axis was not positive and finite.
    SemiMajorAxisNotPositive,
    /// Two pairs had the same lowest-indexed component in their outer members.
    SharedPairKey {
        /// That component's index.
        component: u8,
    },
}

impl fmt::Display for BuildZoneHierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComponentOutOfRange { component } => {
                write!(f, "component {component} is past the stellar level's 16")
            }
            Self::DuplicateComponent { component } => {
                write!(f, "component {component} appears twice in the hierarchy")
            }
            Self::MissingComponent { component } => {
                write!(f, "component {component} is missing from the hierarchy")
            }
            Self::MassNotPositive { component } => {
                write!(
                    f,
                    "component {component}'s mass must be positive and finite"
                )
            }
            Self::SemiMajorAxisNotPositive => {
                f.write_str("a pair's semi-major axis must be positive and finite")
            }
            Self::SharedPairKey { component } => write!(
                f,
                "two pairs have component {component} as their outer member's lowest"
            ),
        }
    }
}

impl Error for BuildZoneHierarchyError {}

/// The host of a zone: a star or a pair, and never the other two kinds of [`OrbitHost`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ZoneHost {
    Star(u8),
    Pair(u8),
}

/// One stable zone of a system: an orbit host, its mass and the limits within which its planets
/// stay (P14.T9.b).
///
/// A limit that is `None` is not set by the hierarchy: a component's zone has no inner limit, so
/// its disc's inner edge bounds it, and the top of the hierarchy has no outer limit, so its disc
/// and design note 14's strip radius (P14.T29) bound it. Limits are distances from the host, a
/// star's centre or a pair's barycentre, in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitZone {
    host: ZoneHost,
    host_mass: SolarMasses,
    kind: Option<ComponentKind>,
    members: u32,
    inner: Option<Metres>,
    outer: Option<Metres>,
    close_binary: bool,
}

impl OrbitZone {
    /// The zone's host: [`OrbitHost::Star`] or [`OrbitHost::Pair`].
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        match self.host {
            ZoneHost::Star(n) => OrbitHost::Star(n),
            ZoneHost::Pair(n) => OrbitHost::Pair(n),
        }
    }

    /// The host's number in the draw numbers of the system-level streams (design note 4): a
    /// star's body index n (0–15), or 16 + k for [`Pair(k)`](OrbitHost::Pair) (17–31). A single
    /// star is host 0, as [`DiscDraws::for_host`] has it.
    #[must_use]
    pub const fn host_number(&self) -> u8 {
        match self.host {
            ZoneHost::Star(n) => n,
            ZoneHost::Pair(k) => STELLAR_SUB_END + k,
        }
    }

    /// The mass the zone's planets orbit: the star's initial mass, or the pair's total.
    #[must_use]
    pub const fn host_mass(&self) -> SolarMasses {
        self.host_mass
    }

    /// The kind of a component's zone's host; `None` for a pair's.
    #[must_use]
    pub const fn component_kind(&self) -> Option<ComponentKind> {
        self.kind
    }

    /// The body indices of the components the zone's planets orbit, in ascending order: the
    /// star's alone, or every component under the pair.
    pub fn members(&self) -> impl Iterator<Item = u8> + use<> {
        let members = self.members;
        (0..STELLAR_SUB_END).filter(move |i| members & (1 << i) != 0)
    }

    /// The zone's inner limit, from the host: a pair's P-type limit, and `None` for a component.
    #[must_use]
    pub const fn inner(&self) -> Option<Metres> {
        self.inner
    }

    /// The zone's outer limit, from the host; `None` at the top of the hierarchy.
    #[must_use]
    pub const fn outer(&self) -> Option<Metres> {
        self.outer
    }

    /// The zone's limits as the radii T3's disc is cut to.
    #[must_use]
    pub fn truncation(&self) -> Truncation {
        let truncation = self
            .inner
            .map_or(Truncation::NONE, |r| Truncation::NONE.with_inner(r));
        self.outer.map_or(truncation, |r| truncation.with_outer(r))
    }

    /// Whether the host is a member of a pair closer than [`CLOSE_BINARY_SEMI_MAJOR_AXIS`]
    /// (47 au), at any level above it: design note 10's flag for the class draw.
    ///
    /// A pair's own orbit does not count for its circumbinary zone, whose disc it clears from
    /// inside rather than truncates from outside; the companions that suppress planets in Kraus et
    /// al.'s (2016) sample orbit the planet host. A flagged zone draws `Barren` with added weight,
    /// so that its planets are `S_bin` = 0.34 as common as a single star's (P14.T4.c).
    #[must_use]
    pub const fn in_close_binary(&self) -> bool {
        self.close_binary
    }
}

/// The stable zones of the hierarchy `h`, in hierarchy order, inside out (P14.T9.b; design
/// note 10).
///
/// One zone per component and per pair, less the pairs' zones narrower than a factor of
/// [`MIN_ZONE_WIDTH`]; see the [module documentation](self) for the limits.
///
/// # Examples
///
/// α Centauri A and B: a zone around each star, and one around both from 87 au:
///
/// ```
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::placement::{
///     ComponentKind, OrbitHost, ZoneHierarchy, ZoneNode, stable_zones,
/// };
/// use hyperion_sim::units::{AstronomicalUnits, Metres, SolarMasses};
///
/// let star = |i: u8, m: f64| ZoneNode::component(i, SolarMasses::new(m), ComponentKind::Star);
/// let a = Metres::from(AstronomicalUnits::new(23.57));
/// let pair = ZoneNode::pair(star(0, 1.12), star(1, 0.95), a, Eccentricity::new(0.516)?);
/// let zones = stable_zones(&ZoneHierarchy::new(pair)?);
///
/// let hosts: Vec<_> = zones.iter().map(|z| z.host()).collect();
/// assert_eq!(hosts, [OrbitHost::Star(0), OrbitHost::Star(1), OrbitHost::Pair(1)]);
/// let au = |m: Option<Metres>| AstronomicalUnits::from(m.expect("a limit")).value();
/// assert!((au(zones[0].outer()) - 2.79).abs() < 0.01);
/// assert!((au(zones[2].inner()) - 87.4).abs() < 0.1);
/// assert_eq!(zones[2].outer(), None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn stable_zones(h: &ZoneHierarchy) -> Vec<OrbitZone> {
    let mut zones = Vec::with_capacity(2 * usize::from(h.component_count) - 1);
    visit(&h.root, Bound::TOP, &mut zones);
    zones
}

/// What the levels above a node impose on its zone.
#[derive(Debug, Clone, Copy)]
struct Bound {
    /// The zone's outer limit from the node's centre; `None` at the top.
    outer: Option<Metres>,
    /// Whether a pair above the node is closer than the close-binary cut.
    close: bool,
}

impl Bound {
    const TOP: Self = Self {
        outer: None,
        close: false,
    };
}

/// Pushes the zones of `node` and everything under it, inside out.
fn visit(node: &ZoneNode, bound: Bound, zones: &mut Vec<OrbitZone>) {
    match &node.shape {
        Shape::Component { index, kind } => {
            if bound.outer.is_some_and(|r| r.value() <= 0.0) {
                return;
            }
            zones.push(OrbitZone {
                host: ZoneHost::Star(*index),
                host_mass: node.mass,
                kind: Some(*kind),
                members: node.members,
                inner: None,
                outer: bound.outer,
                close_binary: bound.close,
            });
        }
        Shape::Pair {
            inner,
            outer,
            semi_major_axis,
            eccentricity,
        } => {
            let a = semi_major_axis.value();
            let e = eccentricity.value();
            let total = node.mass.value();
            let close =
                bound.close || *semi_major_axis < Metres::from(CLOSE_BINARY_SEMI_MAJOR_AXIS);
            // A member's zone about its own centre: the S-type limit its companion sets, and
            // within the pair's own zone less the member's greatest distance from the barycentre.
            let member = |companion: SolarMasses| {
                let mu = companion.value() / total;
                let limit = a * holman_wiegert_s_type(mu, e);
                let reach = a * (1.0 + e) * mu;
                let outer = bound.outer.map_or(limit, |r| limit.min(r.value() - reach));
                Bound {
                    outer: Some(Metres::new(outer)),
                    close,
                }
            };
            visit(inner, member(outer.mass), zones);
            visit(outer, member(inner.mass), zones);
            let lighter = inner.mass.value().min(outer.mass.value());
            let inner_limit = a * holman_wiegert_p_type(lighter / total, e);
            if bound
                .outer
                .is_some_and(|r| r.value() < MIN_ZONE_WIDTH * inner_limit)
            {
                return;
            }
            zones.push(OrbitZone {
                host: ZoneHost::Pair(outer.lowest),
                host_mass: node.mass,
                kind: None,
                members: node.members,
                inner: Some(Metres::new(inner_limit)),
                outer: bound.outer,
                close_binary: bound.close,
            });
        }
    }
}

/// What a zone's disc reads of one component (P14.T9.c): its zero-age luminosity and radius and
/// its disc-lifetime rank.
///
/// A star's are plan 06's [`zams::luminosity`](crate::stellar::sse::zams::luminosity) and
/// [`zams::radius`](crate::stellar::sse::zams::radius) of its initial mass and the system's
/// composition, and its
/// [`StarDraws::disc_lifetime`](crate::stellar::draws::StarDraws::disc_lifetime) rank; a brown
/// dwarf's luminosity will be its cooling fit's at 10 Myr (design note 6, P14.T27).
/// The fields are plain inputs with no invariant between them, so they are public; they are
/// checked when [`ZoneDiscInputs::for_zone`] builds the disc's host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneStar {
    /// The zero-age main-sequence luminosity.
    pub zams_luminosity: SolarLuminosities,
    /// The zero-age main-sequence radius.
    pub zams_radius: SolarRadii,
    /// The star's own `star.disc_lifetime` rank (ruling 33).
    pub disc_lifetime_rank: UnitUniform,
}

/// Everything T3's [`disc::derive`] reads for one zone: its host, lifetime, draws and truncation
/// (P14.T9.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneDiscInputs {
    host: DiscHost,
    lifetime: Megayears,
    draws: DiscDraws,
    truncation: Truncation,
}

impl ZoneDiscInputs {
    /// The disc inputs of `zone`, a zone of `system` in the universe of `seed`, whose components
    /// are `stars` (indexed by body index) and whose \[Fe/H\] is `fe_h`.
    ///
    /// A star's zone takes the star's zero-age state and its own disc lifetime, plan 06's
    /// [`disc_lifetime`](premain::disc_lifetime) of its rank. A pair's takes the pair's total
    /// mass, its members' summed luminosity (in body-index order) and their largest radius, and
    /// the lifetime [`DiscDraws::circumbinary_lifetime`] of its own draws at the total mass
    /// (ruling 33; design note 12). The draws are [`DiscDraws::for_host`] of the zone's
    /// [`host_number`](OrbitZone::host_number), and the truncation is the zone's limits.
    ///
    /// # Errors
    ///
    /// - [`ResolveZoneDiscError::MissingStar`] if `stars` has no entry for one of the zone's
    ///   components.
    /// - [`ResolveZoneDiscError::InvalidStar`] if a component's luminosity or radius is not
    ///   positive and finite.
    /// - [`ResolveZoneDiscError::Host`] if `fe_h` is not finite, or a pair's summed luminosity is
    ///   not.
    ///
    /// # Examples
    ///
    /// The circumbinary disc of a pair of Suns 0.2 au apart starts beyond their P-type limit:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::SystemId;
    /// use hyperion_sim::orbit::Eccentricity;
    /// use hyperion_sim::planetary::placement::{
    ///     ComponentKind, OrbitHost, ZoneDiscInputs, ZoneHierarchy, ZoneNode, ZoneStar,
    ///     stable_zones,
    /// };
    /// use hyperion_sim::stellar::draws::UnitUniform;
    /// use hyperion_sim::units::{
    ///     AstronomicalUnits, Dex, Metres, SolarLuminosities, SolarMasses, SolarRadii,
    /// };
    ///
    /// let sun = |i: u8| ZoneNode::component(i, SolarMasses::new(1.0), ComponentKind::Star);
    /// let a = Metres::from(AstronomicalUnits::new(0.2));
    /// let pair = ZoneNode::pair(sun(0), sun(1), a, Eccentricity::CIRCULAR);
    /// let zones = stable_zones(&ZoneHierarchy::new(pair)?);
    /// let star = ZoneStar {
    ///     zams_luminosity: SolarLuminosities::new(0.7),
    ///     zams_radius: SolarRadii::new(0.89),
    ///     disc_lifetime_rank: UnitUniform::HALF,
    /// };
    /// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
    /// let circumbinary = &zones[2];
    /// assert_eq!(circumbinary.host(), OrbitHost::Pair(1));
    ///
    /// let stars = [star, star];
    /// let inputs = ZoneDiscInputs::for_zone(seed, system, circumbinary, &stars, Dex::ZERO)?;
    /// assert!((inputs.host().zams_luminosity().value() - 1.4).abs() < 1e-12);
    /// let disc = inputs.derive();
    /// let profile = disc.profile().expect("room for a disc");
    /// assert!(profile.inner_edge() >= circumbinary.inner().expect("a P-type limit"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn for_zone(
        seed: Seed,
        system: SystemId,
        zone: &OrbitZone,
        stars: &[ZoneStar],
        fe_h: Dex,
    ) -> Result<Self, ResolveZoneDiscError> {
        let draws = DiscDraws::for_host(seed, system, zone.host_number());
        let star = |component: u8| -> Result<&ZoneStar, ResolveZoneDiscError> {
            let star = stars
                .get(usize::from(component))
                .ok_or(ResolveZoneDiscError::MissingStar { component })?;
            let positive = |x: f64| x.is_finite() && x > 0.0;
            let invalid = |cause| ResolveZoneDiscError::InvalidStar { component, cause };
            if !positive(star.zams_luminosity.value()) {
                return Err(invalid(BuildDiscHostError::LuminosityNotPositive));
            }
            if !positive(star.zams_radius.value()) {
                return Err(invalid(BuildDiscHostError::RadiusNotPositive));
            }
            Ok(star)
        };
        let (host, lifetime) = match zone.host {
            ZoneHost::Star(n) => {
                let star = star(n)?;
                let host =
                    DiscHost::new(zone.host_mass, fe_h, star.zams_luminosity, star.zams_radius)?;
                let lifetime = premain::disc_lifetime(zone.host_mass, star.disc_lifetime_rank);
                (host, lifetime)
            }
            ZoneHost::Pair(_) => {
                let mut luminosity = 0.0;
                let mut radius = 0.0_f64;
                for component in zone.members() {
                    let star = star(component)?;
                    luminosity += star.zams_luminosity.value();
                    radius = radius.max(star.zams_radius.value());
                }
                let host = DiscHost::new(
                    zone.host_mass,
                    fe_h,
                    SolarLuminosities::new(luminosity),
                    SolarRadii::new(radius),
                )?;
                (host, draws.circumbinary_lifetime(zone.host_mass))
            }
        };
        Ok(Self {
            host,
            lifetime,
            draws,
            truncation: zone.truncation(),
        })
    }

    /// The disc's host: a star's zero-age state, or a pair's total mass, summed luminosity and
    /// largest radius.
    #[must_use]
    pub const fn host(&self) -> &DiscHost {
        &self.host
    }

    /// The disc's lifetime.
    #[must_use]
    pub const fn lifetime(&self) -> Megayears {
        self.lifetime
    }

    /// The disc's draws, [`DiscDraws::for_host`] of the zone's host number.
    #[must_use]
    pub const fn draws(&self) -> &DiscDraws {
        &self.draws
    }

    /// The zone's limits, which the disc is cut to.
    #[must_use]
    pub const fn truncation(&self) -> Truncation {
        self.truncation
    }

    /// The zone's disc: [`disc::derive`] of these inputs.
    #[must_use]
    pub fn derive(&self) -> Disc {
        disc::derive(&self.host, self.lifetime, &self.draws, self.truncation)
    }
}

/// A zone's disc inputs could not be gathered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveZoneDiscError {
    /// The components given had no entry for one of the zone's.
    MissingStar {
        /// The component's body index.
        component: u8,
    },
    /// A component's zero-age luminosity or radius was not positive and finite.
    InvalidStar {
        /// The component's body index.
        component: u8,
        /// Which of the two, as the disc's host would report it.
        cause: BuildDiscHostError,
    },
    /// The disc's host could not be built from the components given.
    Host(BuildDiscHostError),
}

impl From<BuildDiscHostError> for ResolveZoneDiscError {
    fn from(error: BuildDiscHostError) -> Self {
        Self::Host(error)
    }
}

impl fmt::Display for ResolveZoneDiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStar { component } => {
                write!(f, "no zero-age state was given for component {component}")
            }
            Self::InvalidStar { component, .. } => {
                write!(f, "component {component}'s zero-age state is invalid")
            }
            Self::Host(_) => f.write_str("a zone's disc host could not be built"),
        }
    }
}

impl Error for ResolveZoneDiscError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingStar { .. } => None,
            Self::InvalidStar { cause, .. } => Some(cause),
            Self::Host(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::stellar::Composition;
    use crate::stellar::sse::{ZCoeffs, zams};

    const SEED: Seed = Seed::new(0x5eed_0014_0009_0000);

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn in_au(m: Metres) -> f64 {
        AstronomicalUnits::from(m).value()
    }

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [-7, 31, 4]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn star(index: u8, mass: f64) -> ZoneNode {
        ZoneNode::component(index, SolarMasses::new(mass), ComponentKind::Star)
    }

    fn pair(inner: ZoneNode, outer: ZoneNode, a_au: f64, e: f64) -> ZoneNode {
        ZoneNode::pair(inner, outer, au(a_au), Eccentricity::new(e).unwrap())
    }

    fn zones_of(root: ZoneNode) -> Vec<OrbitZone> {
        stable_zones(&ZoneHierarchy::new(root).unwrap())
    }

    fn hosts(zones: &[OrbitZone]) -> Vec<OrbitHost> {
        zones.iter().map(OrbitZone::host).collect()
    }

    // P14.T9.a, the fits.

    #[test]
    fn equal_masses_on_a_circular_orbit_give_0_274_and_2_39_separations() {
        assert!((holman_wiegert_s_type(0.5, 0.0) - 0.274).abs() < 1e-15);
        assert!((holman_wiegert_p_type(0.5, 0.0) - 2.3875).abs() < 1e-15);
        let zones = zones_of(pair(star(0, 1.0), star(1, 1.0), 1.0, 0.0));
        assert_eq!(
            hosts(&zones),
            [OrbitHost::Star(0), OrbitHost::Star(1), OrbitHost::Pair(1)]
        );
        for zone in &zones[..2] {
            assert!((in_au(zone.outer().unwrap()) - 0.274).abs() < 1e-12);
            assert_eq!(zone.inner(), None);
        }
        assert!((in_au(zones[2].inner().unwrap()) - 2.3875).abs() < 1e-12);
        assert_eq!(zones[2].outer(), None);
    }

    #[test]
    fn alpha_centauri_a_keeps_planets_inside_2_8_au() {
        // Holman and Wiegert's Table 4: a = 23.57 au, e = 0.516, 1.12 and 0.95 M☉, giving 2.79 au
        // around A, 2.54 au around B and 87 au around both.
        let zones = zones_of(pair(star(0, 1.12), star(1, 0.95), 23.57, 0.516));
        assert!((in_au(zones[0].outer().unwrap()) - 2.79).abs() < 0.005);
        assert!((in_au(zones[1].outer().unwrap()) - 2.54).abs() < 0.005);
        assert!((in_au(zones[2].inner().unwrap()) - 87.0).abs() < 0.5);
        // Plan 14's round figures, 23.5 au and e = 0.52: about 2.8 au around A.
        let rounded = zones_of(pair(star(0, 1.12), star(1, 0.95), 23.5, 0.52));
        let a = in_au(rounded[0].outer().unwrap());
        assert!((2.7..2.85).contains(&a), "{a}");
    }

    #[test]
    fn the_fits_reproduce_holman_and_wiegert_s_tables_within_their_stated_errors() {
        // Table 3 (S-type), μ from 0.1 to 0.9 across and e from 0 to 0.8 down.
        let s_table: [[f64; 9]; 9] = [
            [0.45, 0.38, 0.37, 0.30, 0.26, 0.23, 0.20, 0.16, 0.13],
            [0.37, 0.32, 0.30, 0.27, 0.24, 0.20, 0.18, 0.15, 0.11],
            [0.34, 0.27, 0.25, 0.23, 0.20, 0.18, 0.16, 0.13, 0.10],
            [0.28, 0.24, 0.21, 0.19, 0.18, 0.16, 0.14, 0.12, 0.09],
            [0.23, 0.20, 0.18, 0.16, 0.15, 0.13, 0.11, 0.10, 0.07],
            [0.18, 0.16, 0.14, 0.13, 0.12, 0.10, 0.09, 0.08, 0.06],
            [0.13, 0.12, 0.11, 0.10, 0.09, 0.08, 0.07, 0.06, 0.045],
            [0.09, 0.08, 0.07, 0.07, 0.06, 0.05, 0.05, 0.045, 0.035],
            [0.05, 0.05, 0.04, 0.04, 0.04, 0.035, 0.03, 0.025, 0.0225],
        ];
        // Table 7 (P-type), μ from 0.1 to 0.5 across and e from 0 to 0.7 down.
        let p_table: [[f64; 5]; 8] = [
            [2.0, 2.2, 2.3, 2.3, 2.3],
            [2.4, 2.7, 2.7, 2.8, 2.8],
            [2.7, 3.1, 3.1, 3.1, 3.1],
            [3.1, 3.5, 3.5, 3.3, 3.2],
            [3.5, 3.5, 3.6, 3.5, 3.6],
            [3.8, 3.9, 3.9, 3.6, 3.7],
            [3.9, 3.9, 3.9, 3.8, 3.7],
            [4.2, 4.3, 4.3, 4.1, 4.1],
        ];
        let grid = |i: usize| f64::from(u8::try_from(i).unwrap()) / 10.0;
        let mut worst: f64 = 0.0;
        for (row, values) in s_table.iter().enumerate() {
            for (column, &value) in values.iter().enumerate() {
                let fit = holman_wiegert_s_type(grid(column + 1), grid(row));
                worst = worst.max((fit / value - 1.0).abs());
            }
        }
        // Their "11% worst case": 11.4% at μ = 0.6 and e = 0.7, where the table's 0.05 has one
        // significant figure.
        assert!(worst <= 0.115, "S-type worst case {worst}");
        worst = 0.0;
        for (row, values) in p_table.iter().enumerate() {
            for (column, &value) in values.iter().enumerate() {
                let fit = holman_wiegert_p_type(grid(column + 1), grid(row));
                worst = worst.max((fit / value - 1.0).abs());
            }
        }
        assert!(worst <= 0.06, "P-type worst case {worst}");
    }

    #[test]
    fn inside_their_ranges_the_fits_are_the_published_polynomials() {
        for (mu, e) in [(0.1, 0.0), (0.37, 0.41), (0.9, 0.8), (0.5, 0.7)] {
            assert_same_bits(holman_wiegert_s_type(mu, e), s_type_fit(mu, e));
        }
        for (mu, e) in [(0.1, 0.0), (0.25, 0.33), (0.5, 0.7)] {
            assert_same_bits(holman_wiegert_p_type(mu, e), p_type_fit(mu, e));
        }
    }

    #[test]
    fn outside_their_ranges_the_fits_err_towards_smaller_zones() {
        let s = holman_wiegert_s_type;
        let p = holman_wiegert_p_type;
        // A light perturber is clamped to μ = 0.1, below what the polynomial would give.
        for e in [0.0, 0.3, 0.8] {
            assert_same_bits(s(0.02, e), s(0.1, e));
            assert!(s(0.02, e) < s_type_fit(0.02, e));
        }
        // A light host follows the Hill sphere, (1 − μ)^⅓, below the clamp.
        for e in [0.0, 0.5] {
            assert!(s(0.95, e) < s(0.9, e));
            assert!((s(0.99, e) / s(0.999, e) - math::cbrt(10.0)).abs() < 1e-12);
        }
        assert_same_bits(s(1.0, 0.0), 0.0);
        // Past e = 0.8 the limit keeps its share of the companion's pericentre distance.
        for mu in [0.1, 0.5, 0.9] {
            for e in [0.85, 0.9, 0.99] {
                assert!(s(mu, e) < s(mu, 0.8));
                // It falls as (1 − e)^1.2 (Jaime et al. 2014), faster than the pericentre.
                let law = math::powf((1.0 - e) / 0.2, 1.2);
                assert!((s(mu, e) / law / s(mu, 0.8) - 1.0).abs() < 1e-12);
                assert!(s(mu, e) / (1.0 - e) < s(mu, 0.8) / 0.2);
            }
        }
        // The P-type fit is folded, so either star's share gives the same limit.
        for (mu, e) in [(0.2, 0.0), (0.35, 0.6), (0.05, 0.3)] {
            assert!((p(mu, e) - p(1.0 - mu, e)).abs() < 1e-12);
        }
        // Unfolded, μ = 0.9 would give 1.19 separations, against its mirror image's 1.96.
        assert!(p_type_fit(0.9, 0.0) < 1.2);
        // A light companion is clamped to μ = 0.1, whose inner limit lies farther out.
        for e in [0.0, 0.4, 0.7] {
            assert_same_bits(p(0.03, e), p(0.1, e));
            assert!(p(0.03, e) > p_type_fit(0.03, e));
        }
        // Past e = 0.7 the inner limit keeps its multiple of the binary's apocentre.
        for mu in [0.1, 0.3, 0.5] {
            let at_edge = p(mu, 0.7) / 1.7;
            for e in [0.75, 0.9, 0.99] {
                assert!(p(mu, e) > p(mu, 0.7));
                assert!((p(mu, e) / (1.0 + e) - at_edge).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn the_fits_are_continuous_at_the_edges_of_their_ranges() {
        let step = 1e-9;
        let close = |a: f64, b: f64| (a - b).abs() < 1e-8;
        for e in [0.0, 0.4, 0.8] {
            assert!(close(
                holman_wiegert_s_type(0.1 - step, e),
                holman_wiegert_s_type(0.1 + step, e)
            ));
            assert!(close(
                holman_wiegert_s_type(0.9 - step, e),
                holman_wiegert_s_type(0.9 + step, e)
            ));
        }
        for mu in [0.1, 0.5, 0.9] {
            assert!(close(
                holman_wiegert_s_type(mu, 0.8 - step),
                holman_wiegert_s_type(mu, 0.8 + step)
            ));
        }
        for mu in [0.1, 0.3, 0.5] {
            assert!(close(
                holman_wiegert_p_type(mu, 0.7 - step),
                holman_wiegert_p_type(mu, 0.7 + step)
            ));
        }
        for e in [0.0, 0.7] {
            assert!(close(
                holman_wiegert_p_type(0.1 - step, e),
                holman_wiegert_p_type(0.1 + step, e)
            ));
            assert!(close(
                holman_wiegert_p_type(0.5 - step, e),
                holman_wiegert_p_type(0.5 + step, e)
            ));
        }
    }

    #[test]
    fn the_s_type_limit_is_positive_and_the_p_type_limit_outside_the_binary_everywhere() {
        let mut rng = Lcg::new(0x5eed_0014_0009_000a);
        for _ in 0..100_000 {
            let mu = rng.next_f64() * 0.999_999;
            let e = rng.next_f64() * 0.999_999;
            let s = holman_wiegert_s_type(mu, e);
            assert!(s > 0.0 && s < 0.5, "S-type {s} at μ = {mu}, e = {e}");
            // The two stars' zones never touch at pericentre.
            let both = s + holman_wiegert_s_type(1.0 - mu, e);
            assert!(both < 1.0 - e, "{both} at μ = {mu}, e = {e}");
            let p = holman_wiegert_p_type(mu, e);
            assert!(p > 1.0 + e, "P-type {p} at μ = {mu}, e = {e}");
        }
    }

    // P14.T9.b, zones from the hierarchy.

    #[test]
    fn a_single_star_has_exactly_one_zone_bounded_only_by_its_disc() {
        let single = ZoneHierarchy::single(SolarMasses::new(1.0), ComponentKind::Star).unwrap();
        let zones = stable_zones(&single);
        assert_eq!(zones.len(), 1);
        let zone = zones[0];
        assert_eq!(zone.host(), OrbitHost::Star(0));
        assert_eq!(zone.host_number(), 0);
        assert_same_bits(zone.host_mass().value(), 1.0);
        assert_eq!(zone.component_kind(), Some(ComponentKind::Star));
        assert_eq!(zone.members().collect::<Vec<_>>(), [0]);
        assert_eq!((zone.inner(), zone.outer()), (None, None));
        assert_eq!(zone.truncation(), Truncation::NONE);
        assert!(!zone.in_close_binary());
    }

    /// The pairs above `host` in `root`, outermost first, each with the side `host` is on.
    fn path_to(root: &ZoneNode, host: OrbitHost) -> Option<Vec<(&ZoneNode, bool)>> {
        let here = match &root.shape {
            Shape::Component { index, .. } => OrbitHost::Star(*index),
            Shape::Pair { outer, .. } => OrbitHost::Pair(outer.lowest),
        };
        if here == host {
            return Some(Vec::new());
        }
        let Shape::Pair { inner, outer, .. } = &root.shape else {
            return None;
        };
        for (child, is_outer) in [(inner, false), (outer, true)] {
            if let Some(mut path) = path_to(child, host) {
                path.insert(0, (root, is_outer));
                return Some(path);
            }
        }
        None
    }

    /// A pair's semi-major axis in metres, eccentricity and members' masses.
    fn orbit_of(node: &ZoneNode) -> (f64, f64, f64, f64) {
        let Shape::Pair {
            inner,
            outer,
            semi_major_axis,
            eccentricity,
        } = &node.shape
        else {
            panic!("not a pair");
        };
        (
            semi_major_axis.value(),
            eccentricity.value(),
            inner.mass.value(),
            outer.mass.value(),
        )
    }

    /// The greatest distance from the barycentre of the pair at `path[0]` that the node at the
    /// end of `path` reaches.
    fn reach(path: &[(&ZoneNode, bool)]) -> f64 {
        path.iter()
            .map(|&(node, is_outer)| {
                let (a, e, m_in, m_out) = orbit_of(node);
                let companion = if is_outer { m_in } else { m_out };
                a * (1.0 + e) * companion / (m_in + m_out)
            })
            .sum()
    }

    /// Asserts that no two zones of `root` can ever overlap, at any phase of any orbit.
    fn assert_disjoint(root: &ZoneNode, zones: &[OrbitZone]) {
        let slack = 1.0 + 1e-12;
        for (i, z1) in zones.iter().enumerate() {
            for z2 in &zones[i + 1..] {
                let p1 = path_to(root, z1.host()).unwrap();
                let p2 = path_to(root, z2.host()).unwrap();
                let common = p1
                    .iter()
                    .zip(&p2)
                    .take_while(|(a, b)| std::ptr::eq(a.0, b.0) && a.1 == b.1)
                    .count();
                let outer = |z: &OrbitZone| z.outer().unwrap().value();
                let (d1, d2) = (&p1[common..], &p2[common..]);
                if d1.is_empty() || d2.is_empty() {
                    // One zone's host is under the other's: the inner one lies inside the other's
                    // inner limit.
                    let (outside, inside, path) = if d1.is_empty() {
                        (z1, z2, d2)
                    } else {
                        (z2, z1, d1)
                    };
                    let hole = outside.inner().expect("a pair's zone").value();
                    assert!(
                        reach(path) + outer(inside) <= hole * slack,
                        "{:?} reaches into {:?}",
                        inside.host(),
                        outside.host()
                    );
                } else {
                    // The two lie on either side of their lowest common pair, whose members are
                    // never closer than its pericentre.
                    let (a, e, _, _) = orbit_of(d1[0].0);
                    let r1 = reach(&d1[1..]) + outer(z1);
                    let r2 = reach(&d2[1..]) + outer(z2);
                    assert!(
                        r1 + r2 <= a * (1.0 - e) * slack,
                        "{:?} and {:?} meet",
                        z1.host(),
                        z2.host()
                    );
                }
            }
        }
    }

    /// A random hierarchy of `count` components numbered as plan 11 numbers them, depth first,
    /// inner before outer, each pair wider than the pairs inside it.
    fn random_node(rng: &mut Lcg, next: &mut u8, count: u8) -> (ZoneNode, f64) {
        if count == 1 {
            let mass = 0.08 * math::powf(40.0, rng.next_f64());
            let node = star(*next, mass);
            *next += 1;
            return (node, 0.0);
        }
        let inner_count = 1 + u8::try_from(rng.next_below(u64::from(count - 1))).unwrap();
        let (inner, inner_a) = random_node(rng, next, inner_count);
        let (outer, outer_a) = random_node(rng, next, count - inner_count);
        let widest = inner_a.max(outer_a);
        let a = if widest > 0.0 {
            widest * (2.0 + 60.0 * rng.next_f64())
        } else {
            0.01 * math::powf(1e4, rng.next_f64())
        };
        let e = 0.97 * rng.next_f64();
        (pair(inner, outer, a, e), a)
    }

    #[test]
    fn zones_of_hierarchical_triples_and_quadruples_never_overlap() {
        // A triple like α Centauri's, and a quadruple of two close pairs.
        let triple = pair(
            pair(star(0, 1.1), star(1, 0.9), 23.5, 0.52),
            star(2, 0.12),
            8_700.0,
            0.5,
        );
        let zones = zones_of(triple.clone());
        assert_eq!(
            hosts(&zones),
            [
                OrbitHost::Star(0),
                OrbitHost::Star(1),
                OrbitHost::Pair(1),
                OrbitHost::Star(2),
                OrbitHost::Pair(2)
            ]
        );
        assert_disjoint(&triple, &zones);
        let quadruple = pair(
            pair(star(0, 1.0), star(1, 0.5), 0.3, 0.1),
            pair(star(2, 0.8), star(3, 0.7), 1.0, 0.3),
            200.0,
            0.4,
        );
        let zones = zones_of(quadruple.clone());
        assert_eq!(zones.len(), 7);
        assert_disjoint(&quadruple, &zones);

        let mut rng = Lcg::new(0x5eed_0014_0009_000b);
        let mut kept = 0;
        for _ in 0..3_000 {
            let count = 3 + u8::try_from(rng.next_below(4)).unwrap();
            let (root, _) = random_node(&mut rng, &mut 0, count);
            let zones = zones_of(root.clone());
            assert_disjoint(&root, &zones);
            // Hierarchy order: the stars in index order, every pair after its members.
            let stars: Vec<u8> = zones
                .iter()
                .filter_map(|z| match z.host() {
                    OrbitHost::Star(n) => Some(n),
                    OrbitHost::Pair(_) | OrbitHost::Barycentre | OrbitHost::Body(_) => None,
                })
                .collect();
            assert!(stars.is_sorted(), "stars out of order: {stars:?}");
            for (i, zone) in zones.iter().enumerate() {
                for member in zone.members() {
                    let position = zones
                        .iter()
                        .position(|z| z.host() == OrbitHost::Star(member));
                    assert!(
                        position.is_none_or(|p| p <= i),
                        "{:?} comes before its member {member}",
                        zone.host()
                    );
                }
            }
            kept += zones.len();
        }
        // Most zones survive the width cut and the eccentric pairs.
        assert!(kept > 3_000 * 5, "{kept}");
    }

    #[test]
    fn pairs_are_hosts_keyed_by_the_lowest_component_of_their_outer_member() {
        // (A, (B, C)): the outer pair's outer member is (B, C), whose lowest component is B.
        let root = pair(
            star(0, 2.0),
            pair(star(1, 0.6), star(2, 0.4), 0.5, 0.0),
            60.0,
            0.2,
        );
        let zones = zones_of(root);
        assert_eq!(
            hosts(&zones),
            [
                OrbitHost::Star(0),
                OrbitHost::Star(1),
                OrbitHost::Star(2),
                OrbitHost::Pair(2),
                OrbitHost::Pair(1)
            ]
        );
        let numbers: Vec<u8> = zones.iter().map(OrbitZone::host_number).collect();
        assert_eq!(numbers, [0, 1, 2, 18, 17]);
        assert_eq!(zones[3].members().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(zones[4].members().collect::<Vec<_>>(), [0, 1, 2]);
        assert_same_bits(zones[4].host_mass().value(), 3.0);
        assert_eq!(zones[3].component_kind(), None);
    }

    #[test]
    fn a_brown_dwarf_companion_bounds_zones_like_any_other_component() {
        let dwarf = ZoneNode::component(1, SolarMasses::new(0.05), ComponentKind::BrownDwarf);
        let with_dwarf = zones_of(pair(star(0, 1.0), dwarf, 10.0, 0.2));
        let with_star = zones_of(pair(star(0, 1.0), star(1, 0.05), 10.0, 0.2));
        assert_eq!(with_dwarf.len(), 3);
        for (a, b) in with_dwarf.iter().zip(&with_star) {
            assert_eq!(
                (a.host(), a.inner(), a.outer()),
                (b.host(), b.inner(), b.outer())
            );
        }
        assert_eq!(
            with_dwarf[1].component_kind(),
            Some(ComponentKind::BrownDwarf)
        );
        assert_eq!(with_star[1].component_kind(), Some(ComponentKind::Star));
        // The star's zone is the fit at μ clamped to 0.1 (design note 10).
        let star_zone = in_au(with_dwarf[0].outer().unwrap());
        assert!((star_zone - 10.0 * holman_wiegert_s_type(0.1, 0.2)).abs() < 1e-12);
        // The dwarf keeps a zone of its own, the Hill-scaled continuation of the fit: 0.78 au,
        // against a Hill radius of 2.0 au at the star's pericentre distance of 8 au.
        let dwarf_zone = in_au(with_dwarf[1].outer().unwrap());
        let mu = 1.0 / 1.05;
        let hill = math::cbrt((1.0 - mu) / 0.1);
        assert!((dwarf_zone - 10.0 * holman_wiegert_s_type(0.9, 0.2) * hill).abs() < 1e-9);
        let hill_radius = 8.0 * math::cbrt(0.05 / 3.0);
        assert!(
            (0.3..0.5).contains(&(dwarf_zone / hill_radius)),
            "{dwarf_zone}"
        );
    }

    #[test]
    fn a_zone_narrower_than_a_factor_of_one_and_a_half_is_dropped() {
        // An inner pair at 1 au whose circumbinary zone starts at 2.39 au, and a third star that
        // cuts it off at 0.339 of its orbit's semi-major axis.
        let triple = |a_out: f64| {
            let inner = pair(star(0, 1.0), star(1, 1.0), 1.0, 0.0);
            zones_of(pair(inner, star(2, 1.0), a_out, 0.0))
        };
        let wide = triple(20.0);
        assert!(wide.iter().any(|z| z.host() == OrbitHost::Pair(1)));
        let tight = triple(8.0);
        assert!(!tight.iter().any(|z| z.host() == OrbitHost::Pair(1)));
        // The threshold is 1.5: the zone survives at an outer limit of 1.5 × 2.3875 au.
        let mu = 1.0 / 3.0;
        let at = 1.5 * 2.3875 / holman_wiegert_s_type(mu, 0.0);
        let kept = |a: f64| triple(a).iter().any(|z| z.host() == OrbitHost::Pair(1));
        assert!(kept(at * (1.0 + 1e-9)));
        assert!(!kept(at * (1.0 - 1e-9)));
        // The stars keep their zones either way.
        assert_eq!(tight.len(), 4);
    }

    #[test]
    fn a_member_s_zone_stays_inside_its_pair_s_zone() {
        // A wide inner pair in a triple too tight to be stable, which only a hand-built hierarchy
        // gives: A's own S-type limit in (A, B) is 13.7 au, but A swings 25 au from AB's
        // barycentre, and AB's zone in the triple ends 26.1 au out, so A keeps 1.1 au.
        let inner = pair(star(0, 1.0), star(1, 1.0), 50.0, 0.0);
        let root = pair(inner, star(2, 0.1), 120.0, 0.4);
        let zones = zones_of(root.clone());
        let a_zone = zones[0].outer().unwrap().value();
        let ab_bound = au(120.0).value() * holman_wiegert_s_type(0.1 / 2.1, 0.4);
        assert!((a_zone / (ab_bound - au(25.0).value()) - 1.0).abs() < 1e-12);
        assert!((in_au(Metres::new(a_zone)) - 1.14).abs() < 0.01);
        // AB's own zone, from 119 au, is gone; C keeps its zone.
        assert_eq!(
            hosts(&zones),
            [
                OrbitHost::Star(0),
                OrbitHost::Star(1),
                OrbitHost::Star(2),
                OrbitHost::Pair(2)
            ]
        );
        assert_disjoint(&root, &zones);
    }

    #[test]
    fn hierarchies_are_checked_when_built() {
        let err = |root: ZoneNode| ZoneHierarchy::new(root).unwrap_err();
        assert_eq!(
            err(pair(star(0, 1.0), star(0, 1.0), 1.0, 0.0)),
            BuildZoneHierarchyError::DuplicateComponent { component: 0 }
        );
        assert_eq!(
            err(pair(star(0, 1.0), star(2, 1.0), 1.0, 0.0)),
            BuildZoneHierarchyError::MissingComponent { component: 1 }
        );
        assert_eq!(
            err(star(16, 1.0)),
            BuildZoneHierarchyError::ComponentOutOfRange { component: 16 }
        );
        assert_eq!(
            err(star(200, 1.0)),
            BuildZoneHierarchyError::ComponentOutOfRange { component: 200 }
        );
        assert_eq!(
            err(star(0, 0.0)),
            BuildZoneHierarchyError::MassNotPositive { component: 0 }
        );
        assert_eq!(
            err(star(0, f64::NAN)),
            BuildZoneHierarchyError::MassNotPositive { component: 0 }
        );
        let zero_orbit = ZoneNode::pair(
            star(0, 1.0),
            star(1, 1.0),
            Metres::ZERO,
            Eccentricity::CIRCULAR,
        );
        assert_eq!(
            err(zero_orbit),
            BuildZoneHierarchyError::SemiMajorAxisNotPositive
        );
        // (C, (B, A)) numbered so that both pairs' outer members start at the same component.
        let shared = pair(
            star(2, 1.0),
            pair(star(1, 1.0), star(0, 1.0), 1.0, 0.0),
            30.0,
            0.0,
        );
        assert_eq!(
            err(shared),
            BuildZoneHierarchyError::SharedPairKey { component: 0 }
        );
        // Sixteen components are the most, and are accepted.
        let mut node = star(0, 1.0);
        for i in 1..16 {
            node = pair(
                node,
                star(i, 0.5),
                10.0 * math::powi(4.0, i32::from(i)),
                0.0,
            );
        }
        assert_eq!(ZoneHierarchy::new(node).unwrap().component_count(), 16);
    }

    // P14.T9.c, host assignment.

    fn zams_star(mass: f64, rank: f64) -> ZoneStar {
        let m = SolarMasses::new(mass);
        let coeffs = ZCoeffs::new(Composition::SOLAR.z_fit());
        ZoneStar {
            zams_luminosity: zams::luminosity(m, &coeffs),
            zams_radius: zams::radius(m, &coeffs),
            disc_lifetime_rank: UnitUniform::new(rank).unwrap(),
        }
    }

    #[test]
    fn a_circumstellar_disc_takes_its_star_s_own_lifetime_and_host_number() {
        let zones = zones_of(pair(star(0, 1.0), star(1, 0.6), 40.0, 0.3));
        let stars = [zams_star(1.0, 0.3), zams_star(0.6, 0.8)];
        for (zone, star) in zones[..2].iter().zip(&stars) {
            let inputs =
                ZoneDiscInputs::for_zone(SEED, system(3), zone, &stars, Dex::new(0.1)).unwrap();
            let expected = premain::disc_lifetime(zone.host_mass(), star.disc_lifetime_rank);
            assert_same_bits(inputs.lifetime().value(), expected.value());
            assert_eq!(
                *inputs.draws(),
                DiscDraws::for_host(SEED, system(3), zone.host_number())
            );
            assert_eq!(inputs.host().zams_luminosity(), star.zams_luminosity);
            assert_eq!(inputs.host().mass(), zone.host_mass());
            assert_eq!(
                inputs.truncation(),
                Truncation::NONE.with_outer(zone.outer().unwrap())
            );
        }
    }

    #[test]
    fn a_circumbinary_disc_draws_its_lifetime_at_the_pair_s_total_mass() {
        let zones = zones_of(pair(star(0, 1.0), star(1, 0.6), 0.1, 0.1));
        let stars = [zams_star(1.0, 0.3), zams_star(0.6, 0.8)];
        let zone = &zones[2];
        assert_eq!(zone.host(), OrbitHost::Pair(1));
        let inputs = ZoneDiscInputs::for_zone(SEED, system(4), zone, &stars, Dex::ZERO).unwrap();
        let draws = DiscDraws::for_host(SEED, system(4), 17);
        assert_eq!(*inputs.draws(), draws);
        assert_same_bits(
            inputs.lifetime().value(),
            draws.circumbinary_lifetime(SolarMasses::new(1.6)).value(),
        );
        let host = inputs.host();
        assert_same_bits(host.mass().value(), 1.6);
        assert_same_bits(
            host.zams_luminosity().value(),
            stars[0].zams_luminosity.value() + stars[1].zams_luminosity.value(),
        );
        assert_eq!(host.zams_radius(), stars[0].zams_radius);
        assert_eq!(
            inputs.truncation(),
            Truncation::NONE.with_inner(zone.inner().unwrap())
        );
    }

    #[test]
    fn a_single_star_s_zone_disc_is_its_disc_alone() {
        let single = ZoneHierarchy::single(SolarMasses::new(0.8), ComponentKind::Star).unwrap();
        let zone = stable_zones(&single)[0];
        let star = zams_star(0.8, 0.55);
        let inputs =
            ZoneDiscInputs::for_zone(SEED, system(5), &zone, &[star], Dex::new(-0.2)).unwrap();
        let host = DiscHost::new(
            SolarMasses::new(0.8),
            Dex::new(-0.2),
            star.zams_luminosity,
            star.zams_radius,
        )
        .unwrap();
        let lifetime = premain::disc_lifetime(SolarMasses::new(0.8), star.disc_lifetime_rank);
        let alone = disc::derive(
            &host,
            lifetime,
            &DiscDraws::for_host(SEED, system(5), 0),
            Truncation::NONE,
        );
        assert_eq!(inputs.derive(), alone);
    }

    #[test]
    fn every_zone_s_disc_lies_inside_its_zone() {
        let mut rng = Lcg::new(0x5eed_0014_0009_000c);
        let mut discs = 0;
        for index in 0..10_000 {
            let count = 2 + u8::try_from(rng.next_below(2)).unwrap();
            let (root, _) = random_node(&mut rng, &mut 0, count);
            let masses = {
                let mut out = [0.0; 3];
                collect_masses(&root, &mut out);
                out
            };
            let stars: Vec<ZoneStar> = masses[..usize::from(count)]
                .iter()
                .map(|&m| zams_star(m, rng.next_f64().clamp(1e-9, 1.0 - 1e-9)))
                .collect();
            for zone in zones_of(root) {
                let inputs =
                    ZoneDiscInputs::for_zone(SEED, system(index), &zone, &stars, Dex::ZERO)
                        .unwrap();
                let Some(profile) = inputs.derive().profile().copied() else {
                    continue;
                };
                discs += 1;
                if let Some(inner) = zone.inner() {
                    assert!(
                        profile.inner_edge() >= inner,
                        "system {index}, {:?}: disc from {:?} inside {inner:?}",
                        zone.host(),
                        profile.inner_edge()
                    );
                }
                if let Some(outer) = zone.outer() {
                    assert!(
                        profile.outer_edge() <= outer,
                        "system {index}, {:?}: disc to {:?} outside {outer:?}",
                        zone.host(),
                        profile.outer_edge()
                    );
                }
            }
        }
        assert!(discs > 10_000, "{discs}");
    }

    fn collect_masses(node: &ZoneNode, out: &mut [f64; 3]) {
        match &node.shape {
            Shape::Component { index, .. } => out[usize::from(*index)] = node.mass.value(),
            Shape::Pair { inner, outer, .. } => {
                collect_masses(inner, out);
                collect_masses(outer, out);
            }
        }
    }

    #[test]
    fn zone_discs_name_what_is_missing_or_invalid() {
        let zones = zones_of(pair(star(0, 1.0), star(1, 0.6), 5.0, 0.0));
        let one = [zams_star(1.0, 0.5)];
        assert_eq!(
            ZoneDiscInputs::for_zone(SEED, system(0), &zones[1], &one, Dex::ZERO),
            Err(ResolveZoneDiscError::MissingStar { component: 1 })
        );
        assert_eq!(
            ZoneDiscInputs::for_zone(SEED, system(0), &zones[2], &one, Dex::ZERO),
            Err(ResolveZoneDiscError::MissingStar { component: 1 })
        );
        let mut dark = zams_star(0.6, 0.5);
        dark.zams_luminosity = SolarLuminosities::new(-1.0);
        let stars = [one[0], dark];
        assert_eq!(
            ZoneDiscInputs::for_zone(SEED, system(0), &zones[2], &stars, Dex::ZERO),
            Err(ResolveZoneDiscError::InvalidStar {
                component: 1,
                cause: BuildDiscHostError::LuminosityNotPositive
            })
        );
        let mut point = zams_star(0.6, 0.5);
        point.zams_radius = SolarRadii::ZERO;
        let error =
            ZoneDiscInputs::for_zone(SEED, system(0), &zones[1], &[one[0], point], Dex::ZERO)
                .unwrap_err();
        assert_eq!(
            error,
            ResolveZoneDiscError::InvalidStar {
                component: 1,
                cause: BuildDiscHostError::RadiusNotPositive
            }
        );
        let source = error
            .source()
            .and_then(|e| e.downcast_ref::<BuildDiscHostError>());
        assert_eq!(source, Some(&BuildDiscHostError::RadiusNotPositive));
        assert_eq!(
            ZoneDiscInputs::for_zone(SEED, system(0), &zones[0], &one, Dex::new(f64::NAN)),
            Err(ResolveZoneDiscError::Host(
                BuildDiscHostError::MetallicityNotFinite
            ))
        );
    }

    #[test]
    fn components_of_pairs_inside_47_au_are_in_a_close_binary() {
        let close = zones_of(pair(star(0, 1.0), star(1, 0.5), 20.0, 0.3));
        assert!(close[0].in_close_binary() && close[1].in_close_binary());
        // The pair's own orbit does not flag its circumbinary zone.
        assert!(!close[2].in_close_binary());
        let wide = zones_of(pair(star(0, 1.0), star(1, 0.5), 100.0, 0.3));
        assert!(wide.iter().all(|z| !z.in_close_binary()));
        // Just either side of the cut.
        let cut = CLOSE_BINARY_SEMI_MAJOR_AXIS.value();
        let at = |a: f64| zones_of(pair(star(0, 1.0), star(1, 0.5), a, 0.0))[0].in_close_binary();
        assert!(at(cut * (1.0 - 1e-12)));
        assert!(!at(cut));
        // A close pair inside a triple whose third star is 30 au out: everything below the
        // triple is flagged, the triple's own zone is not.
        let triple = zones_of(pair(
            pair(star(0, 1.0), star(1, 0.5), 0.1, 0.0),
            star(2, 0.3),
            30.0,
            0.1,
        ));
        let flags: Vec<(OrbitHost, bool)> = triple
            .iter()
            .map(|z| (z.host(), z.in_close_binary()))
            .collect();
        assert_eq!(
            flags,
            [
                (OrbitHost::Star(0), true),
                (OrbitHost::Star(1), true),
                (OrbitHost::Pair(1), true),
                (OrbitHost::Star(2), true),
                (OrbitHost::Pair(2), false)
            ]
        );
    }
}
