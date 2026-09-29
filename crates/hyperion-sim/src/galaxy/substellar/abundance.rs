//! One galaxy's substellar abundances per system, and the density at which the rogue planets
//! saturate their cells' index (plan 13, P13.T2; ruling 125).
//!
//! The parameters count objects per star, and placement works in systems, so each abundance is
//! multiplied by the galaxy's own mean number of stars per system (Design note 2). The same number
//! goes into every population's column of the share matrix (Design note 3), always the parameter's:
//! nothing lowers it galaxy-wide.
//!
//! The rogue planets' 4 ly cells hold 64 ly³ and number their candidates in 16 bits. Plan 03's
//! headroom rule (its Design note 6) asks that the fullest cell's mean plus eight standard
//! deviations stay within 65,536, which allows a mean of about 63,520, or 992.5 per cubic
//! light-year ([`rogue_planet_saturation_density`]). The 64-bit ID layout fixes that limit at any
//! cell size, so it cannot be raised. By ruling 125 a rogue-planet cell's density is
//! min(a × ρ, C) and its bound min(a × B, C), with a the abundance per system, ρ and B the total
//! system density and its bound, and C that limit: placement saturates the few densest central
//! cells in place and leaves every other cell bit for bit as it was. The headroom check then passes
//! by construction for every galaxy and every abundance.
//!
//! The *saturation threshold* is the abundance at which the galaxy's densest cell reaches C: C over
//! the bound on the total system density over a root octant, the bound
//! [`check_index_headroom`](crate::galaxy::placement::check_index_headroom) takes. Below it nothing
//! saturates. For Milky Way values it is 37.5 per star (31.8 under Kroupa's function), and 45 for
//! the median seed; the default 21 saturates the centre in about one galaxy in 170, whose nuclear
//! disc is compact (rulings 125 and 140.6). The brown dwarfs need no saturation: their fullest 16 ly
//! cell expects at most some 55,000 candidates against 2¹⁹.

use super::params::SubstellarParams;
use crate::galaxy::Galaxy;
use crate::galaxy::fields::{Fields, MAX_COMPONENTS};
use crate::galaxy::placement::{rogue_planet_saturation_density, root_octant};
use crate::rng::PowerLaw;

/// One galaxy's free-floating brown dwarfs and rogue planets per system (plan 13, Design notes 2,
/// 3 and 8; ruling 125).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::substellar::{SubstellarAbundance, SubstellarParams};
///
/// let galaxy = Galaxy::from_params(Seed::new(1), GalaxyParams::milky_way_like())?;
/// let abundance = galaxy.substellar();
/// // About a quarter of a brown dwarf and thirty rogue planets for every system.
/// assert!((0.2..0.3).contains(&abundance.brown_dwarfs_per_system()));
/// assert!((25.0..35.0).contains(&abundance.rogue_planets_per_system()));
/// // The measured 21 per star is well below where the Milky Way's centre saturates.
/// assert!(!abundance.is_saturated());
/// // Far more saturates the densest cells, but the abundance itself is never lowered.
/// let crowded = SubstellarParams::generator_default().with_rogue_planets_per_star(500.0);
/// let saturated = SubstellarAbundance::for_galaxy(&galaxy, &crowded);
/// assert!(saturated.is_saturated());
/// assert!(saturated.rogue_planets_per_system() > saturated.rogue_planet_cap_per_system());
/// # Ok::<(), hyperion_sim::galaxy::BuildGalaxyError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubstellarAbundance {
    params: SubstellarParams,
    mean_stars_per_system: f64,
    brown_dwarfs_per_system: f64,
    rogue_planets_per_system: f64,
    rogue_planet_cap_per_system: f64,
    rogue_mass_law: PowerLaw,
}

impl SubstellarAbundance {
    /// The abundances of `galaxy` under `p`: its own stars per system and its own densest cell.
    #[must_use]
    pub fn for_galaxy(galaxy: &Galaxy, p: &SubstellarParams) -> Self {
        Self::from_fields(galaxy.fields(), galaxy.mean_stars_per_system(), *p)
    }

    /// The abundances over `fields` with `mean_stars_per_system` stars to a system, for the galaxy
    /// being built.
    #[must_use]
    pub(crate) fn from_fields(
        fields: &Fields,
        mean_stars_per_system: f64,
        params: SubstellarParams,
    ) -> Self {
        Self {
            params,
            mean_stars_per_system,
            brown_dwarfs_per_system: params.brown_dwarfs_per_star() * mean_stars_per_system,
            rogue_planets_per_system: params.rogue_planets_per_star() * mean_stars_per_system,
            rogue_planet_cap_per_system: saturation_threshold_per_system(fields),
            rogue_mass_law: params.rogue_mass_law(),
        }
    }

    /// The parameters the abundances were derived from.
    #[must_use]
    pub const fn params(&self) -> &SubstellarParams {
        &self.params
    }

    /// Free-floating brown dwarfs per system: 1 ÷ 5.5 per star times the stars per system, 0.24–0.26.
    #[must_use]
    pub const fn brown_dwarfs_per_system(&self) -> f64 {
        self.brown_dwarfs_per_system
    }

    /// Rogue planets per system: 21 per star times the stars per system, 28–30. It is always the
    /// parameter's; the densest cells saturate in place instead (ruling 125).
    #[must_use]
    pub const fn rogue_planets_per_system(&self) -> f64 {
        self.rogue_planets_per_system
    }

    /// The saturation threshold per system: the abundance at which the galaxy's densest
    /// rogue-planet cell reaches [`rogue_planet_saturation_density`], about 54 for Milky Way
    /// values (rulings 125 and 140.6; the name is the former cap's, kept for the parameters panel).
    #[must_use]
    pub const fn rogue_planet_cap_per_system(&self) -> f64 {
        self.rogue_planet_cap_per_system
    }

    /// [`rogue_planet_cap_per_system`](Self::rogue_planet_cap_per_system) per star: 37.5 for Milky
    /// Way values (31.8 under Kroupa's function), 45 for the median seed and 13.5 for the most
    /// compact nuclear disc in 2,000 seeds (rulings 125 and 140.6).
    #[must_use]
    pub fn rogue_planet_cap_per_star(&self) -> f64 {
        self.rogue_planet_cap_per_system / self.mean_stars_per_system
    }

    /// Whether the abundance exceeds the saturation threshold, so that the galaxy's densest
    /// rogue-planet cells hold fewer than a × ρ: in about one galaxy in 170 at the default, and
    /// never for Milky Way values. The parameters panel's `rogue_planets_capped` reads this.
    #[must_use]
    pub fn is_saturated(&self) -> bool {
        self.rogue_planets_per_system > self.rogue_planet_cap_per_system
    }

    /// The rogue planets' mass law, built once with the galaxy
    /// ([`SubstellarParams::rogue_mass_law`]).
    #[must_use]
    pub(crate) const fn rogue_mass_law(&self) -> &PowerLaw {
        &self.rogue_mass_law
    }
}

/// The bound on the total system density over a root octant, systems per cubic light-year: every
/// component's bound, summed in component order.
fn total_system_bound(fields: &Fields) -> f64 {
    let mut bounds = [0.0; MAX_COMPONENTS];
    fields.component_bounds(&root_octant(), &mut bounds);
    bounds.iter().fold(0.0, |sum, &bound| sum + bound)
}

/// The saturation threshold per system: the rogue-planet saturation density over the octant's bound
/// on the total system density (module documentation).
fn saturation_threshold_per_system(fields: &Fields) -> f64 {
    rogue_planet_saturation_density() / total_system_bound(fields)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParamsBuilder;
    use crate::galaxy::placement::{check_index_headroom, largest_headroom_mean};

    const SEED: u64 = 0x1302_0000_0000_0000;

    fn milky_way(kind: MassFunctionKind) -> Galaxy {
        let params = GalaxyParamsBuilder::new()
            .mass_function(kind)
            .build()
            .expect("the fixture's values lie inside their ranges");
        Galaxy::from_params(Seed::new(SEED), params)
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// P13.T2's Milky Way figures (ruling 125): 0.23–0.27 brown dwarfs and 27–31 rogue planets per
    /// system, a saturation threshold of 37.5 per star ±3% (ruling 140.6: 992.5 ÷ (18.42 × 1.437),
    /// the octant bound per ly³ times the stars per system at Chabrier's fitted scale 0.92; 31.8
    /// ±3% under Kroupa's function), not saturated, and room in the index.
    #[test]
    fn the_milky_way_fixture_has_the_plan_s_abundances() {
        for (kind, threshold_per_star) in [
            (MassFunctionKind::Chabrier, 37.5),
            (MassFunctionKind::Kroupa, 31.8),
        ] {
            let galaxy = milky_way(kind);
            let a = galaxy.substellar();
            let bd = a.brown_dwarfs_per_system();
            let rp = a.rogue_planets_per_system();
            assert!((0.23..=0.27).contains(&bd), "{kind:?}: {bd}");
            assert!((27.0..=31.0).contains(&rp), "{kind:?}: {rp}");
            let threshold = a.rogue_planet_cap_per_star();
            println!(
                "{kind:?}: threshold {threshold:.3} per star, octant bound {:.3} per ly³, {:.4} \
                 stars per system",
                total_system_bound(galaxy.fields()),
                galaxy.mean_stars_per_system()
            );
            assert!(
                (threshold / threshold_per_star - 1.0).abs() <= 0.03,
                "{kind:?}: a threshold of {threshold} per star"
            );
            assert!(!a.is_saturated(), "{kind:?}");
            assert_eq!(check_index_headroom(&galaxy), Ok(()), "{kind:?}");
        }
    }

    /// Asked for 60 per star, the Milky Way fixture saturates, the abundance stays the parameter's
    /// and the headroom check passes (ruling 125; the cells are checked in `placement::cell`).
    #[test]
    fn an_abundance_over_the_threshold_saturates_and_keeps_the_headroom() {
        let crowded = SubstellarParams::generator_default().with_rogue_planets_per_star(60.0);
        let galaxy = milky_way(MassFunctionKind::Chabrier).with_substellar_params(crowded);
        let a = galaxy.substellar();
        assert!(a.is_saturated());
        assert_same_bits(
            a.rogue_planets_per_system(),
            60.0 * galaxy.mean_stars_per_system(),
        );
        assert_eq!(check_index_headroom(&galaxy), Ok(()));
        // The threshold is where the fullest cell's unsaturated mean reaches the headroom's.
        let fullest = a.rogue_planet_cap_per_system() * total_system_bound(galaxy.fields()) * 64.0;
        assert!(fullest <= largest_headroom_mean(65_536), "{fullest}");
        assert!(
            fullest > 0.999_999 * largest_headroom_mean(65_536),
            "{fullest}"
        );
    }

    #[test]
    fn the_share_matrix_holds_the_abundances_in_every_column() {
        use crate::galaxy::POPULATIONS;
        use crate::galaxy::imf::MassBand;
        let galaxy = milky_way(MassFunctionKind::Chabrier);
        let a = galaxy.substellar();
        for population in POPULATIONS {
            assert_same_bits(
                galaxy.shares().share(MassBand::BrownDwarf, population),
                a.brown_dwarfs_per_system(),
            );
            assert_same_bits(
                galaxy.shares().share(MassBand::RoguePlanet, population),
                a.rogue_planets_per_system(),
            );
        }
    }

    /// The share of `galaxy`'s rogue planets that saturation removes: the integral of
    /// max(a × ρ − C, 0) over a 256 ly box about the centre, in 2 ly steps, over a × the system
    /// count. The box's faces must be unsaturated, or the box is too small.
    fn saturation_loss(galaxy: &Galaxy) -> f64 {
        use crate::galaxy::PointLy;
        const HALF_LY: f64 = 128.0;
        const STEP_LY: f64 = 2.0;
        let a = galaxy.substellar().rogue_planets_per_system();
        let limit = rogue_planet_saturation_density();
        let mut densities = [0.0; MAX_COMPONENTS];
        let mut at = |p: PointLy| a * galaxy.fields().densities(&p, &mut densities);
        for p in [
            PointLy::new(HALF_LY, 0.0, 0.0),
            PointLy::new(0.0, HALF_LY, 0.0),
            PointLy::new(0.0, 0.0, HALF_LY),
        ] {
            assert!(at(p) < limit, "the box's faces saturate: widen it");
        }
        let steps = 128_u32;
        let mid = |i: u32| -HALF_LY + (f64::from(i) + 0.5) * STEP_LY;
        let mut lost = 0.0;
        for ix in 0..steps {
            for iy in 0..steps {
                for iz in 0..steps {
                    let excess = at(PointLy::new(mid(ix), mid(iy), mid(iz))) - limit;
                    if excess > 0.0 {
                        lost += excess * STEP_LY * STEP_LY * STEP_LY;
                    }
                }
            }
        }
        lost / (a * galaxy.system_count())
    }

    /// Over 2,000 seeds (P13.T2, ruling 125): the default saturates the centre of 10–20 seeds, the
    /// smallest threshold is 12–14.5 per star and the median 40–48, the worst seed loses under 10⁻³
    /// of its rogue planets, and the brown dwarfs' fullest 16 ly cell expects under 65,536
    /// candidates, an eighth of 2¹⁹. Re-run at the fitted Chabrier scale (ruling 140.6, whose
    /// thresholds scale × 1.044): 12 seeds saturated, the smallest threshold 13.52 per star, the
    /// median 45.26, the worst seed losing 7.3 × 10⁻⁵ and the fullest brown-dwarf cell 54,688.
    #[test]
    #[ignore = "slow: builds the fields of 2,000 galaxies"]
    fn the_default_saturates_few_galaxies_and_loses_almost_nothing() {
        let mut thresholds = Vec::with_capacity(2_000);
        let mut saturated = 0_u32;
        let mut fullest_brown_dwarf_cell = 0.0_f64;
        for n in 0..2_000_u64 {
            let galaxy = Galaxy::new(Seed::new(SEED | n));
            let a = galaxy.substellar();
            if a.is_saturated() {
                saturated += 1;
            }
            thresholds.push((a.rogue_planet_cap_per_star(), n));
            let bd = a.brown_dwarfs_per_system() * total_system_bound(galaxy.fields()) * 4_096.0;
            fullest_brown_dwarf_cell = fullest_brown_dwarf_cell.max(bd);
        }
        thresholds.sort_by(|x, y| x.0.total_cmp(&y.0));
        let (smallest, worst) = thresholds[0];
        let median = f64::midpoint(thresholds[999].0, thresholds[1_000].0);
        let loss = saturation_loss(&Galaxy::new(Seed::new(SEED | worst)));
        println!(
            "{saturated} of 2000 seeds saturated; threshold smallest {smallest:.3} per star (seed \
             {worst}), median {median:.2}; worst seed loses {loss:.3e}; fullest brown-dwarf cell \
             {fullest_brown_dwarf_cell:.0}"
        );
        assert!(
            (10..=20).contains(&saturated),
            "{saturated} seeds saturated"
        );
        assert!((12.0..=14.5).contains(&smallest), "{smallest}");
        assert!((40.0..=48.0).contains(&median), "{median}");
        assert!(loss < 1e-3, "{loss}");
        assert!(
            fullest_brown_dwarf_cell < 65_536.0,
            "{fullest_brown_dwarf_cell}"
        );
    }
}
