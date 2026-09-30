//! The sizes of a belt's largest members: their own shallow tail and a growth cap (ruling 100,
//! amending P14.T21.c).
//!
//! A belt's population follows N(> D) ∝ D^(−q) with q of 2.5–3.5, but the top of both Solar
//! belts is a separate, shallow tail: a rank–size fit `D_k` = D₁ k^(−1⁄`q_t`) gives `q_t` = 1.7
//! for the main belt (Ceres, Vesta, Pallas, Hygiea) and 1.8 for the largest trans-Neptunian
//! objects, and
//! streaming instability's birth law is N(> D) ∝ D^(−1.8) (Schäfer, Yang and Johansen 2017, A&A
//! 597, A69, α ≈ 0.6 in mass; Krivov and Wyatt 2021, MNRAS 500, 718, §2.4, `q_big` = 2.8 ± 0.1
//! differential). So:
//!
//! 1. **The largest member** holds a share F₁ of the belt's primordial mass, drawn log-uniform over
//!    [`LARGEST_MEMBER_SHARE`] (0.1–0.4, HYPERION's figure fitted to Ceres's 35% of the main belt,
//!    De Meo and Carry 2013, Icarus 226, 723, §8, and Eris's 14–27% of the Kuiper belt), which
//!    gives its diameter `D_F` at the belt's member density.
//! 2. **The k-th** has `D_k` = `D_F` k^(−1⁄1.8) ([`MEMBER_RANK_SLOPE`]).
//! 3. **Growth caps** hold each below what growth reaches. Icy members stop at 3,300 km ×
//!    `x_m`^0.2 × `u_k` ([`ICY_GROWTH_CAP`]; Kenyon and Bromley 2008, ApJS 179, 451, eq. 30, the
//!    median largest body's radius 1,650 `x_m`^0.2 km at 1–10 Gyr, as a diameter), `x_m` being the
//!    belt's starting solids over the minimum-mass nebula's in its ring ([`nebula_ratio`]), held
//!    to their 1⁄3–3 ([`NEBULA_RATIO_RANGE`]), and `u_k` a per-member spread drawn log-uniform over
//!    [`GROWTH_SPREAD`] (0.8–1.2: they find 10–20% in radius between identical discs). Their fit
//!    covers hosts of 1–3 M☉ at 30–150 au; about M dwarfs and closer in it is extrapolated. Rocky
//!    members stop at [`ROCKY_MEMBER_MASS_CAP`], 0.1 M⊕, the top of the main belt's primordial
//!    embryos (Morbidelli, Bottke, Nesvorný and Levison 2009, Icarus 204, 558, §2).
//! 4. **Kept:** the candidates are sorted by size, largest first, and those over
//!    [`MEMBER_MIN_DIAMETER`] (400 km), at most [`MAX_MEMBERS`] (8), are the members. As a
//!    backstop the smallest are dropped until the members hold at most [`MEMBER_MASS_SHARE_CAP`]
//!    (0.75) of the belt's primordial mass; F₁ = 0.4 gives 0.705 at most, so it removes none.
//!
//! Members are bodies above the collisional cascade's top (450 km), so they do not wear: they are
//! sized from the belt's primordial mass, depleted but unworn, and the belt reports its worn
//! cascade plus its members (ruling 100.2).
//!
//! Kenyon and Bromley's rare oligarch, a 0.02–0.1 M⊕ icy body in 5–10% of their runs, was
//! considered and is not built (ruling 100.4).

use core::f64::consts::PI;

use super::{BeltComposition, MAX_MEMBERS, MEMBER_MIN_DIAMETER};
use crate::math;
use crate::stellar::draws::UnitUniform;
use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU};
use crate::units::{EarthMasses, Metres, SolarMasses};

/// The range of the share of a belt's primordial mass its largest member holds, log-uniform:
/// 0.1–0.4 (ruling 100.3; HYPERION's figure, fitted to Ceres's 0.35 of the main belt and Eris's
/// 0.14–0.27 of the Kuiper belt).
pub const LARGEST_MEMBER_SHARE: (f64, f64) = (0.1, 0.4);

/// The cumulative slope `q_t` of a belt's largest members, `D_k` = D₁ k^(−1⁄`q_t`): 1.8 (ruling
/// 100.3; Schäfer et al. 2017's birth law and the trans-Neptunian objects' own rank fit; the main
/// belt's is 1.7).
pub const MEMBER_RANK_SLOPE: f64 = 1.8;

/// The growth cap of an icy member for a minimum-mass nebula, m: 3,300 km, twice Kenyon and
/// Bromley's (2008, eq. 30) median largest radius r₀ ≈ 1,650 `x_m`^0.2 km at 1–10 Gyr (ruling
/// 100.4).
pub const ICY_GROWTH_CAP: Metres = Metres::new(3.3e6);

/// The exponent of the nebula ratio `x_m` in [`ICY_GROWTH_CAP`]'s scaling: 0.2 (Kenyon and Bromley
/// 2008, eq. 30).
pub const NEBULA_RATIO_EXPONENT: f64 = 0.2;

/// The range the nebula ratio `x_m` is held to: 1⁄3–3, Kenyon and Bromley's (2008) grid (ruling
/// 100.4).
pub const NEBULA_RATIO_RANGE: (f64, f64) = (1.0 / 3.0, 3.0);

/// The range of an icy member's growth-cap spread `u_k`, log-uniform: 0.8–1.2 (ruling 100.4; Kenyon
/// and Bromley 2008 find 10–20% in radius between identical discs).
pub const GROWTH_SPREAD: (f64, f64) = (0.8, 1.2);

/// The heaviest a rocky member grows: 0.1 M⊕, the top of the main belt's primordial embryos
/// (ruling 100.4; Morbidelli et al. 2009, §2).
pub const ROCKY_MEMBER_MASS_CAP: EarthMasses = EarthMasses::new(0.1);

/// The largest share of a belt's primordial mass its members hold together: 0.75 (ruling 100.2),
/// a backstop that the law's own 0.705 at F₁ = 0.4 stays under.
pub const MEMBER_MASS_SHARE_CAP: f64 = 0.75;

/// The minimum-mass solar nebula's surface density of solids at 1 au about a star of 1 M☉, kg m⁻²:
/// 296, that is 29.6 g cm⁻², with Σ = Σ₀ (M★ ÷ M☉) (r ÷ au)^(−3⁄2) (Krivov and Wyatt 2021, eq. 7,
/// after Kenyon and Bromley 2008, eq. 27: 0.18 g cm⁻² at 30 au).
pub const NEBULA_SOLID_SURFACE_DENSITY: f64 = 296.0;

/// The solids the minimum-mass solar nebula holds between `inner` and `outer` about a host of mass
/// `host_mass`: 4π Σ₀ (M★ ÷ M☉) au² (√(r₂ ÷ au) − √(r₁ ÷ au)), with Σ₀ =
/// [`NEBULA_SOLID_SURFACE_DENSITY`]. None for an empty or inverted ring.
///
/// # Examples
///
/// The classical Kuiper belt's ring, 39.4–47.7 au, holds about 8.8 M⊕ of the Sun's nebula:
///
/// ```
/// use hyperion_sim::planetary::belts::minimum_mass_nebula_solids;
/// use hyperion_sim::units::{AstronomicalUnits, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let m = minimum_mass_nebula_solids(au(39.4), au(47.7), SolarMasses::new(1.0));
/// assert!((8.7..8.9).contains(&m.value()));
/// ```
#[must_use]
pub fn minimum_mass_nebula_solids(
    inner: Metres,
    outer: Metres,
    host_mass: SolarMasses,
) -> EarthMasses {
    let (r1, r2) = (
        inner.value().max(0.0) / METRES_PER_AU,
        outer.value().max(0.0) / METRES_PER_AU,
    );
    if r2 <= r1 {
        return EarthMasses::ZERO;
    }
    let kg = 4.0
        * PI
        * NEBULA_SOLID_SURFACE_DENSITY
        * host_mass.value()
        * METRES_PER_AU
        * METRES_PER_AU
        * (r2.sqrt() - r1.sqrt());
    EarthMasses::new(kg / EARTH_MASS_KG)
}

/// The nebula ratio `x_m` of a ring between `inner` and `outer` about a host of mass `host_mass`
/// that started with `solids`: those solids over the minimum-mass nebula's there
/// ([`minimum_mass_nebula_solids`]), not yet held to [`NEBULA_RATIO_RANGE`] (which
/// [`member_sizes`] does). Infinite for an empty ring.
#[must_use]
pub fn nebula_ratio(
    solids: EarthMasses,
    inner: Metres,
    outer: Metres,
    host_mass: SolarMasses,
) -> f64 {
    solids.value() / minimum_mass_nebula_solids(inner, outer, host_mass).value()
}

/// The draws that size a belt's members: the largest member's share, word 8n + 5 of the belt's
/// `belt.population` stream, and each candidate's growth-cap spread, word 8 of the k-th member's
/// `belt.member` stream in rank order, before the candidates are sorted by size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeDraws {
    /// The rank of the largest member's share in [`LARGEST_MEMBER_SHARE`].
    pub largest_share: UnitUniform,
    /// The rank of the k-th candidate's growth-cap spread in [`GROWTH_SPREAD`], k = 1–8.
    pub spreads: [UnitUniform; MEMBER_CANDIDATES],
}

/// The candidates a belt's members are chosen from: [`MAX_MEMBERS`], 8, the rank law's k = 1–8.
pub const MEMBER_CANDIDATES: usize = 8;

/// One member's size: its diameter and its mass, a sphere of that diameter at its belt's member
/// density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberSize {
    diameter: Metres,
    mass: EarthMasses,
}

impl MemberSize {
    /// Its diameter.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.diameter
    }

    /// Its mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }
}

/// A belt's largest bodies: the diameter of the largest, whether or not a member, and the members,
/// largest first.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberSizes {
    largest_diameter: Metres,
    members: Vec<MemberSize>,
}

impl MemberSizes {
    /// The diameter of the belt's largest body, its largest candidate's.
    #[must_use]
    pub const fn largest_diameter(&self) -> Metres {
        self.largest_diameter
    }

    /// The members, those of the candidates over [`MEMBER_MIN_DIAMETER`], largest first.
    #[must_use]
    pub fn members(&self) -> &[MemberSize] {
        &self.members
    }

    /// The members' mass, summed largest first.
    #[must_use]
    pub fn total_mass(&self) -> EarthMasses {
        self.members
            .iter()
            .fold(EarthMasses::ZERO, |sum, m| sum + m.mass)
    }
}

/// The diameter of a sphere of mass `kg` at density `density`, kg m⁻³.
#[must_use]
fn sphere_diameter(kg: f64, density: f64) -> f64 {
    math::cbrt(6.0 * kg / (PI * density))
}

/// The mass of a sphere of diameter `diameter`, m, at density `density`, kg m⁻³, in kg.
#[must_use]
fn sphere_mass(diameter: f64, density: f64) -> f64 {
    PI / 6.0 * diameter * diameter * diameter * density
}

/// `lo` × (`hi` ÷ `lo`)^`rank`.
#[must_use]
fn log_uniform((lo, hi): (f64, f64), rank: UnitUniform) -> f64 {
    lo * math::powf(hi / lo, rank.value())
}

/// The sizes of the members of a belt of primordial mass `mass` and composition `composition`,
/// whose ring started with `nebula_ratio` times the minimum-mass nebula's solids (read for an icy
/// belt only, and held to [`NEBULA_RATIO_RANGE`]), with the draws `draws` (ruling 100; the module
/// documentation).
///
/// # Examples
///
/// The main belt's 4.5 × 10⁻⁴ M⊕ of rock, with Ceres's share of 0.35, gives four members close to
/// Ceres, Vesta, Pallas and Hygiea (939, 525, 512 and 432 km):
///
/// ```
/// use hyperion_sim::planetary::belts::{BeltComposition, LARGEST_MEMBER_SHARE};
/// use hyperion_sim::planetary::belts::{SizeDraws, member_sizes};
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::EarthMasses;
///
/// let (lo, hi) = LARGEST_MEMBER_SHARE;
/// let rank = (0.35_f64 / lo).ln() / (hi / lo).ln();
/// let draws = SizeDraws {
///     largest_share: UnitUniform::new(rank).expect("inside (0, 1)"),
///     spreads: [UnitUniform::new(0.5).expect("inside (0, 1)"); 8],
/// };
/// let sizes = member_sizes(EarthMasses::new(4.5e-4), BeltComposition::Rocky, 1.0, &draws);
/// let km: Vec<f64> = sizes.members().iter().map(|m| m.diameter().value() / 1e3).collect();
/// assert_eq!(km.len(), 4);
/// assert!((850.0..900.0).contains(&km[0]) && (400.0..410.0).contains(&km[3]));
/// ```
#[must_use]
pub fn member_sizes(
    mass: EarthMasses,
    composition: BeltComposition,
    nebula_ratio: f64,
    draws: &SizeDraws,
) -> MemberSizes {
    let density = composition.member_density().value();
    let primordial = mass.value().max(0.0) * EARTH_MASS_KG;
    let share = log_uniform(LARGEST_MEMBER_SHARE, draws.largest_share);
    let first = sphere_diameter(share * primordial, density);
    let rocky_cap = sphere_diameter(ROCKY_MEMBER_MASS_CAP.value() * EARTH_MASS_KG, density);
    let icy_cap = ICY_GROWTH_CAP.value()
        * math::powf(
            nebula_ratio.clamp(NEBULA_RATIO_RANGE.0, NEBULA_RATIO_RANGE.1),
            NEBULA_RATIO_EXPONENT,
        );
    let mut candidates = [0.0; MEMBER_CANDIDATES];
    for ((k, candidate), spread) in (1..=MAX_MEMBERS)
        .zip(candidates.iter_mut())
        .zip(draws.spreads)
    {
        let rank = first * math::powf(f64::from(k), -1.0 / MEMBER_RANK_SLOPE);
        let cap = match composition {
            BeltComposition::Rocky => rocky_cap,
            BeltComposition::Icy => icy_cap * log_uniform(GROWTH_SPREAD, spread),
        };
        *candidate = rank.min(cap);
    }
    candidates.sort_unstable_by(|a, b| b.total_cmp(a));
    let mut members: Vec<MemberSize> = candidates
        .iter()
        .take_while(|&&d| d > MEMBER_MIN_DIAMETER.value())
        .map(|&d| {
            let kg = sphere_mass(d, density) / EARTH_MASS_KG;
            let mass = match composition {
                BeltComposition::Rocky => kg.min(ROCKY_MEMBER_MASS_CAP.value()),
                BeltComposition::Icy => kg,
            };
            MemberSize {
                diameter: Metres::new(d),
                mass: EarthMasses::new(mass),
            }
        })
        .collect();
    let bound = MEMBER_MASS_SHARE_CAP * mass.value();
    while members.iter().map(|m| m.mass.value()).sum::<f64>() > bound {
        members.pop();
    }
    MemberSizes {
        largest_diameter: Metres::new(candidates[0]),
        members,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::AstronomicalUnits;

    fn rank(x: f64) -> UnitUniform {
        UnitUniform::new(x).unwrap()
    }

    /// The share ranks and spread patterns each window is checked over: the ends and middle of
    /// the share's range, with every spread low, high, alternating, or spread across the range.
    fn draw_grid() -> Vec<SizeDraws> {
        let shares = [1e-9, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0 - 1e-9];
        let patterns: [[f64; 8]; 5] = [
            [1e-9; 8],
            [1.0 - 1e-9; 8],
            [
                1e-9,
                1.0 - 1e-9,
                1e-9,
                1.0 - 1e-9,
                1e-9,
                1.0 - 1e-9,
                1e-9,
                1.0 - 1e-9,
            ],
            [0.05, 0.95, 0.35, 0.65, 0.15, 0.85, 0.45, 0.55],
            [0.5; 8],
        ];
        shares
            .iter()
            .flat_map(|&s| {
                patterns.iter().map(move |p| SizeDraws {
                    largest_share: rank(s),
                    spreads: p.map(rank),
                })
            })
            .collect()
    }

    fn km(m: &MemberSize) -> f64 {
        m.diameter().value() / 1e3
    }

    /// Ruling 100.6: the main belt's 4.5 × 10⁻⁴ M⊕ of rock has a largest member of 550–1,000 km
    /// (Ceres 939) holding 0.1–0.4 of the belt, and 1–4 members over 400 km (four in the Solar
    /// System); at Ceres's share, 0.35, they are 873, 594, 474 and 404 km (939, 525, 512 and 432).
    #[test]
    fn a_main_belt_has_a_ceres_and_up_to_three_more() {
        let mass = EarthMasses::new(4.5e-4);
        for draws in draw_grid() {
            let sizes = member_sizes(mass, BeltComposition::Rocky, 1.0, &draws);
            let members = sizes.members();
            assert!((1..=4).contains(&members.len()), "{sizes:?}");
            assert!((550.0..=1_000.0).contains(&km(&members[0])), "{sizes:?}");
            let share = members[0].mass().value() / mass.value();
            assert!((0.1 - 1e-9..=0.4 + 1e-9).contains(&share), "{share}");
        }
        let (lo, hi) = LARGEST_MEMBER_SHARE;
        let ceres = SizeDraws {
            largest_share: rank(math::ln(0.35 / lo) / math::ln(hi / lo)),
            spreads: [rank(0.5); 8],
        };
        let sizes = member_sizes(mass, BeltComposition::Rocky, 1.0, &ceres);
        let found: Vec<f64> = sizes.members().iter().map(km).collect();
        for (d, expected) in found.iter().zip([873.0, 594.0, 474.0, 404.0]) {
            assert!((d - expected).abs() < 1.0, "{found:?}");
        }
        assert_eq!(found.len(), 4);
    }

    /// `n` quasi-random draws: each rank the fractional part of a multiple of its own irrational
    /// step, so the sample covers the ranges evenly and is the same on every run.
    fn draw_sample(n: u32) -> Vec<SizeDraws> {
        let steps = [
            0.618_033_988_749_894_9,
            0.414_213_562_373_095,
            0.732_050_807_568_877_2,
            0.236_067_977_499_789_8,
            0.645_751_311_064_590_6,
            0.162_277_660_168_379_5,
            0.316_624_790_355_4,
            0.605_551_275_463_989,
            0.123_105_625_617_660_5,
        ];
        let at = |i: u32, step: f64| {
            let x = (0.5 + f64::from(i) * step).fract();
            rank(x.clamp(1e-9, 1.0 - 1e-9))
        };
        (0..n)
            .map(|i| SizeDraws {
                largest_share: at(i, steps[0]),
                spreads: core::array::from_fn(|k| at(i, steps[k + 1])),
            })
            .collect()
    }

    /// Ruling 100.6: an icy belt of the Kuiper belt's 0.02 M⊕, at 0.27 of the nebula's solids
    /// (2.4 of 8.8 M⊕, held to 1⁄3), has a largest member of 1,800–3,200 km (Pluto 2,377, Eris
    /// 2,326) and at least four of eight over 1,000 km (six) for every draw; its top eight hold
    /// 0.1–0.5 of it (the real top seven, 0.33) for all but the heaviest few per cent of draws,
    /// with a median near 0.30, and never more than 0.6.
    #[test]
    fn a_kuiper_belt_has_a_pluto_and_several_more() {
        let mass = EarthMasses::new(0.02);
        let share = |draws: &SizeDraws| {
            let sizes = member_sizes(mass, BeltComposition::Icy, 0.27, draws);
            let members = sizes.members();
            assert_eq!(members.len(), 8, "{sizes:?}");
            assert!((1_800.0..=3_200.0).contains(&km(&members[0])), "{sizes:?}");
            assert!(members.iter().filter(|m| km(m) > 1_000.0).count() >= 4);
            sizes.total_mass().value() / mass.value()
        };
        for draws in draw_grid() {
            let s = share(&draws);
            assert!((0.1..=0.6).contains(&s), "{s}");
        }
        let mut shares: Vec<f64> = draw_sample(4_000).iter().map(share).collect();
        shares.sort_by(f64::total_cmp);
        let (low, median, high) = (shares[200], shares[2_000], shares[3_800]);
        eprintln!("top-eight share of 0.02 M⊕: 5% {low:.3}, median {median:.3}, 95% {high:.3}");
        assert!(low >= 0.1 && high <= 0.5, "{low} {high}");
        assert!((0.25..=0.35).contains(&median), "{median}");
    }

    /// Ruling 100.6: the Solar-like golden's 1.29 M⊕ icy belt at 66.8–81.0 au, at 0.1–0.3 of the
    /// nebula's solids (all held to 1⁄3), has all eight members at 1,600–3,200 km, each of at most
    /// 0.006 M⊕, holding at most 5% of the belt (Kenyon and Bromley 2008 put 1.4% at 74 au in
    /// bodies over 1,000 km).
    #[test]
    fn a_massive_cold_belt_has_eight_plutos() {
        let mass = EarthMasses::new(1.29);
        for ratio in [0.1, 0.2, 0.3] {
            for draws in draw_grid() {
                let sizes = member_sizes(mass, BeltComposition::Icy, ratio, &draws);
                let members = sizes.members();
                assert_eq!(members.len(), 8);
                for m in members {
                    assert!((1_600.0..=3_200.0).contains(&km(m)), "{sizes:?}");
                    assert!(m.mass().value() <= 0.006, "{m:?}");
                }
                assert!(sizes.total_mass().value() <= 0.05 * mass.value());
            }
        }
    }

    /// Ruling 100.4 and 100.6: no member of any belt outweighs 0.1 M⊕, rocky members stop there,
    /// the largest never holds over 0.4 of its belt, and the members together at most 0.75.
    #[test]
    fn no_member_outweighs_a_tenth_of_an_earth() {
        assert_eq!(MEMBER_CANDIDATES, usize::from(MAX_MEMBERS));
        for composition in [BeltComposition::Rocky, BeltComposition::Icy] {
            for mass in [1e-5, 1e-3, 0.1, 1.0, 10.0, 100.0] {
                for ratio in [1e-3, 1.0, 1e3] {
                    for draws in draw_grid() {
                        let mass = EarthMasses::new(mass);
                        let sizes = member_sizes(mass, composition, ratio, &draws);
                        let members = sizes.members();
                        for (k, m) in members.iter().enumerate() {
                            assert!(m.mass() <= ROCKY_MEMBER_MASS_CAP, "{m:?}");
                            assert!(m.diameter() > MEMBER_MIN_DIAMETER);
                            if k > 0 {
                                assert!(m.diameter() <= members[k - 1].diameter());
                            }
                        }
                        if let Some(first) = members.first() {
                            assert!(first.mass().value() <= 0.4 * mass.value() * (1.0 + 1e-12));
                            assert_eq!(first.diameter(), sizes.largest_diameter());
                        }
                        let total = sizes.total_mass().value();
                        assert!(total <= MEMBER_MASS_SHARE_CAP * mass.value());
                    }
                }
            }
        }
        // A massive rocky belt's members all reach the embryos' cap.
        let draws = draw_grid()[0];
        let sizes = member_sizes(EarthMasses::new(100.0), BeltComposition::Rocky, 1.0, &draws);
        assert_eq!(sizes.members().len(), 8);
        for m in sizes.members() {
            assert!((m.mass().value() / ROCKY_MEMBER_MASS_CAP.value() - 1.0).abs() < 1e-12);
        }
    }

    /// The minimum-mass nebula (Krivov and Wyatt 2021, eq. 7): 11.5 M⊕ at 66.8–81.0 au about the
    /// Sun, 8.8 at 39.4–47.7 au, twice that about a star of 2 M☉, and none in an empty ring.
    #[test]
    fn the_minimum_mass_nebula_holds_its_solids() {
        let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
        let sun = SolarMasses::new(1.0);
        let golden = minimum_mass_nebula_solids(au(66.84), au(80.97), sun).value();
        assert!((golden - 11.47).abs() < 0.05, "{golden}");
        let kuiper = minimum_mass_nebula_solids(au(39.4), au(47.7), sun).value();
        assert!((kuiper - 8.78).abs() < 0.05, "{kuiper}");
        let heavy = minimum_mass_nebula_solids(au(39.4), au(47.7), SolarMasses::new(2.0)).value();
        assert!((heavy / kuiper - 2.0).abs() < 1e-12);
        assert_eq!(
            minimum_mass_nebula_solids(au(5.0), au(5.0), sun),
            EarthMasses::ZERO
        );
        let x = nebula_ratio(EarthMasses::new(2.4), au(39.4), au(47.7), sun);
        assert!((x - 0.273).abs() < 0.005, "{x}");
    }
}
