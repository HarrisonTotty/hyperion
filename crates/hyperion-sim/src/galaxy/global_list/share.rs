//! The halo's discrete share: how much of its budget the streams and dwarf cores hold (plan 10,
//! P10.T3.d.a, Design note 1).
//!
//! Plan 02 draws a discrete share of 2–15% ([`HaloParams::discrete_share`]) and applies it
//! nowhere. That draw cannot be what the halo gives up: a halo of about 1% of the galaxy holds a
//! few 10⁸ M☉, while a recent progenitor runs to 10⁹·⁵ M☉, so a drawn share and drawn progenitors
//! agree only if one yields. The drawn share is therefore a ceiling, and the share applied is the
//! smaller of it and the physical share: the mass in tubes and cores inside the halo's 65,000 ly
//! ([`INSIDE_LY`]) over the halo's budget, `GalaxyParams::population_mass(Population::Halo)`.
//! Every count of the list is a mass over the halo's mean system mass, so the ratio of masses is
//! the ratio of systems. The physical share has two parts:
//!
//! - **Globular tubes**: [`ORPHAN_STREAMS_PER_GLOBULAR`] (1.5) tubes per expected globular,
//!   plan 02's count, each at the orphan law's mean mass, 1.63 × 10⁴ M☉
//!   ([`MassLaw::mean_solar_masses`](super::orphans::MassLaw::mean_solar_masses)). Nothing is
//!   drawn, and a living globular's own history is not read: the budget's 5% tolerance (P10.T12)
//!   covers the difference (plan 10, Risks, "Living globulars' tube masses").
//! - **Dwarfs**: each recent progenitor's tube mass times the share of its radial period spent
//!   inside 65,000 ly, by the spherical orbit's own quadrature of `dr ÷ v_r` in the mid-plane
//!   potential (no integration), plus its core's mass whole, since a core exists only where its
//!   progenitor lies inside the root cube at the epoch. The cores are those the list keeps: the
//!   first four in its order (Design note 12), as [`Debris`](super::Debris) numbers them.
//!
//! When the drawn share is the smaller, the dwarfs' cold masses (their tubes' and cores') are
//! scaled down together until the two parts meet it; when the physical share is the smaller, the
//! smooth components keep the difference. [`Debris::generate`](super::Debris::generate) applies
//! that scale from these same figures, so it is the same whether or not a galaxy stores the share
//! (P10.T3.d.b stores it through `FeatureShares::set_halo_discrete`). A scaled core left under
//! [`CORE_MIN_SYSTEMS`](super::dwarfs::CORE_MIN_SYSTEMS) and a tube scaled to nothing are then
//! dropped, as `Debris` documents. The share still counts them, so the list serves less than it
//! says by those cores alone, under 10³ systems each.
//!
//! The 2% floor of the drawn share is not enforced: globular tubes alone come to 0.84% of the
//! Milky Way fixture's halo, and a galaxy with no recent dwarf inside the cube has no more (plan
//! 10, Risks, "The discrete share against the dwarf mass range"). Over drawn galaxies they range
//! from 0.2% to 7% (P10.T3.d.a, as built): plan 02's globular count follows the dark halo's mass,
//! the halo's budget the stars'. Where they pass the drawn share, the dwarfs are scaled to nothing
//! and the applied share is the drawn one, below what the globular tubes hold.
//!
//! Everything comes from the galaxy's parameters, its seed (the dwarfs' stripping fractions on
//! `dwarf.stripping`, their arguments of pericentre on `dwarf.orbit`) and its in-plane potential
//! tables, so `Galaxy::from_params` can derive the share before the galaxy exists. Nothing is
//! drawn here.
//!
//! [`HaloParams::discrete_share`]: crate::galaxy::params::HaloParams::discrete_share

use crate::galaxy::Population;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::PotentialTables;
use crate::math;
use crate::rng::Seed;
use crate::tables::streams::ORPHAN_STREAMS_PER_GLOBULAR;
use crate::units::{LightYears, SolarMasses};

use super::DwarfCoreNumber;
use super::debris::in_list_order;
use super::dwarfs::{DwarfDebris, dwarfs};
use super::orphans::MASS_LAW;
use super::spec::SphericalOrbit;

/// The radius inside which a dwarf's tube counts towards the share, ly: the halo's cut at 65,000
/// ly, which keeps it inside the root cube (brainstorm, "Populations"; plan 02's `HALO_CUT`).
pub const INSIDE_LY: f64 = 65_000.0;

/// The halo's discrete share of the galaxy of `seed` and `params` with in-plane tables `tables`:
/// the share of the halo's budget in streams and dwarf cores, at most the drawn
/// [`HaloParams::discrete_share`](crate::galaxy::params::HaloParams::discrete_share) (module
/// documentation).
///
/// The dwarfs' tubes beyond the list's 4,096 streams, which [`Debris`](super::Debris) would drop,
/// are counted: no galaxy comes near that. It costs the dwarfs' quadratures, nearly all of them
/// their epoch states' inversions of the time from pericentre: 7.5 ms for the Milky Way fixture in
/// the bench profile (pod, 4 vCPU; `global_list/discrete_share` in `benches/galaxy.rs`), against
/// P10.T3.d.a's "under a millisecond".
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::global_list::discrete_share;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
///
/// let params = GalaxyParams::milky_way_like();
/// let tables = PotentialTables::in_plane(&MassModel::new(&params));
/// let phi = discrete_share(Seed::new(7), &params, &tables);
/// // The drawn share is a ceiling (plan 10, Design note 1), and the globular tubes alone give
/// // some tenths of a per cent.
/// assert!(phi <= params.halo().discrete_share());
/// assert!(phi > 0.001);
/// ```
#[must_use]
pub fn discrete_share(seed: Seed, params: &GalaxyParams, tables: &PotentialTables) -> f64 {
    let mut debris = dwarfs(seed, params, tables);
    in_list_order(&mut debris);
    ShareFigures::of(params, tables, &debris).applied()
}

/// The figures the share is made of (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShareFigures {
    /// The globular tubes' mass.
    globular_solar_masses: f64,
    /// The dwarfs' cold mass inside [`INSIDE_LY`], before any scaling.
    dwarfs_solar_masses: f64,
    /// The halo's budget.
    budget_solar_masses: f64,
    /// The drawn share, the ceiling.
    drawn: f64,
}

impl ShareFigures {
    /// The figures of the galaxy of `params` and `tables` whose recent progenitors left `dwarfs`,
    /// in the list's order ([`in_list_order`]).
    #[must_use]
    pub(crate) fn of(
        params: &GalaxyParams,
        tables: &PotentialTables,
        dwarfs: &[DwarfDebris],
    ) -> Self {
        let globulars = f64::from(params.accretion().globular_count());
        let globular = ORPHAN_STREAMS_PER_GLOBULAR * globulars * MASS_LAW.mean_solar_masses();
        let phi = |r: f64| tables.potential_in_plane(LightYears::new(r));
        let mut cores = 0;
        let mut cold = 0.0;
        for d in dwarfs {
            if let Some(tube) = &d.tube {
                cold += tube.mass().value() * share_inside(tube.orbit(), &phi, INSIDE_LY);
            }
            if let Some(core) = &d.core
                && cores < DwarfCoreNumber::LIMIT
            {
                cores += 1;
                cold += core.mass().value();
            }
        }
        Self {
            globular_solar_masses: globular,
            dwarfs_solar_masses: cold,
            budget_solar_masses: params.population_mass(Population::Halo).value(),
            drawn: params.halo().discrete_share(),
        }
    }

    /// The globular tubes' share of the halo.
    #[cfg(test)]
    #[must_use]
    fn globular_share(&self) -> f64 {
        self.globular_solar_masses / self.budget_solar_masses
    }

    /// The physical share: the globular tubes and the dwarfs over the budget.
    ///
    /// The builder keeps the halo's share of the galaxy at 0.7–1.4%, so the budget is positive.
    #[must_use]
    pub(crate) fn physical(&self) -> f64 {
        (self.globular_solar_masses + self.dwarfs_solar_masses) / self.budget_solar_masses
    }

    /// The share applied: the smaller of the physical and the drawn.
    #[must_use]
    pub(crate) fn applied(&self) -> f64 {
        self.physical().min(self.drawn)
    }

    /// What the dwarfs' cold masses are multiplied by: the factor that brings the globular tubes
    /// and the dwarfs to the drawn share.
    ///
    /// It is 1 when the physical share is at most the drawn one or there is no dwarf to scale,
    /// else `(drawn × budget − globular) ÷ dwarfs`, and 0 where the globular tubes alone reach
    /// the drawn share (module documentation).
    #[must_use]
    pub(crate) fn dwarf_scale(&self) -> f64 {
        if self.physical() <= self.drawn || self.dwarfs_solar_masses <= 0.0 {
            1.0
        } else {
            ((self.drawn * self.budget_solar_masses - self.globular_solar_masses)
                / self.dwarfs_solar_masses)
                .clamp(0.0, 1.0)
        }
    }
}

/// Scales every dwarf's cold mass by `scale` (module documentation).
///
/// That is its tube's mass and the bound mass the tube's progenitor keeps, and its core's mass and
/// systems. The orbits, the tidal radius and the mass at accretion are the progenitor's and stay.
pub(crate) fn scale_cold_masses(dwarfs: &mut [DwarfDebris], scale: f64) {
    for d in dwarfs {
        if let Some(tube) = &mut d.tube {
            tube.mass = SolarMasses::new(tube.mass.value() * scale);
            tube.progenitor_mass = SolarMasses::new(tube.progenitor_mass.value() * scale);
        }
        if let Some(core) = &mut d.core {
            core.mass = SolarMasses::new(core.mass.value() * scale);
            core.systems *= scale;
        }
    }
}

/// The share of `orbit`'s radial period spent inside `radius` (ly), in the spherical potential
/// `phi`: twice the time from pericentre out to `radius` over the period, by the orbit's own
/// quadrature; 1 for an orbit wholly inside and 0 for one wholly outside.
#[must_use]
fn share_inside(orbit: &SphericalOrbit, phi: &impl Fn(f64) -> f64, radius: f64) -> f64 {
    let (peri, apo) = (orbit.pericentre().value(), orbit.apocentre().value());
    if apo <= radius {
        return 1.0;
    }
    if peri >= radius {
        return 0.0;
    }
    // `r = r̄ + ½ (r_a − r_p) sin θ` at `radius`.
    let sin = ((radius - f64::midpoint(apo, peri)) / (0.5 * (apo - peri))).clamp(-1.0, 1.0);
    let (time, _) = orbit.radial_integrals(phi, math::asin(sin));
    (2.0 * time / orbit.radial_period().value()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::potential::MassModel;

    /// G M in (km/s)² ly for 10¹¹ M☉, as `spec`'s tests take it.
    const GM: f64 = 4.300_9e-3 * 3.261_563_777 * 1e11;

    fn kepler(r: f64) -> f64 {
        -GM / r
    }

    /// A galaxy's figures from its seed, on in-plane tables: cheap enough for `just ci`.
    fn figures(seed: Seed) -> (GalaxyParams, Vec<DwarfDebris>, ShareFigures) {
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let tables = PotentialTables::in_plane(&MassModel::new(&params));
        let mut debris = dwarfs(seed, &params, &tables);
        in_list_order(&mut debris);
        let figures = ShareFigures::of(&params, &tables, &debris);
        let share = discrete_share(seed, &params, &tables);
        assert!(
            figures.applied().total_cmp(&share).is_eq(),
            "{share} {figures:?}"
        );
        (params, debris, figures)
    }

    #[test]
    fn the_time_inside_a_radius_is_kepler_s() {
        // For a Kepler orbit r = a (1 − e cos E) and the mean anomaly is E − e sin E, so the share
        // of the period inside R is (E_R − e sin E_R) ÷ π with cos E_R = (1 − R ÷ a) ÷ e.
        let orbit = SphericalOrbit::from_turning_points(kepler, 20_000.0, 120_000.0).unwrap();
        let (a, e) = (70_000.0, orbit.eccentricity());
        for radius in [25_000.0, 50_000.0, 65_000.0, 100_000.0, 119_000.0] {
            let anomaly = math::acos((1.0 - radius / a) / e);
            let want = (anomaly - e * math::sin(anomaly)) / core::f64::consts::PI;
            let got = share_inside(&orbit, &kepler, radius);
            assert!((got - want).abs() < 1e-9, "{radius}: {got} {want}");
        }
        assert!((share_inside(&orbit, &kepler, 120_000.0) - 1.0).abs() < 1e-15);
        assert!(share_inside(&orbit, &kepler, 20_000.0).abs() < 1e-15);
    }

    #[test]
    fn the_inside_radius_is_the_halo_s_cut() {
        let params = GalaxyParams::milky_way_like();
        let cut = params
            .halo()
            .components()
            .iter()
            .map(|c| c.cut_radius().value())
            .fold(0.0, f64::max);
        assert!((cut - INSIDE_LY).abs() < 1e-9, "{cut}");
    }

    /// The share is never above the drawn share, and a scaled galaxy's dwarfs, scaled, meet it,
    /// unless the globular tubes alone pass it: over the Milky Way fixture and sixteen drawn
    /// galaxies. The globular tubes' share is pinned provisionally to the 2,000 galaxies' range,
    /// 0.2–8% ([`GLOBULAR_SHARE_PIN`]), against P10.T3.d.a's 0.3–1% (a finding).
    #[test]
    fn the_share_is_capped_by_the_drawn_share_and_the_scale_meets_it() {
        let milky_way = GalaxyParams::milky_way_like();
        let tables = PotentialTables::in_plane(&MassModel::new(&milky_way));
        let mut debris = dwarfs(Seed::new(7), &milky_way, &tables);
        in_list_order(&mut debris);
        let fixture = ShareFigures::of(&milky_way, &tables, &debris);
        eprintln!(
            "Milky Way: globular tubes {:.5}, physical {:.5}, applied {:.5}, drawn {}",
            fixture.globular_share(),
            fixture.physical(),
            fixture.applied(),
            fixture.drawn
        );
        let mut scaled = 0;
        for n in 0..16_u64 {
            let seed = Seed::new(0x0a10_3d00 | n);
            let (params, mut debris, f) = figures(seed);
            assert!(f.applied() <= f.drawn, "seed {seed}: {f:?}");
            assert!(f.applied().is_finite() && f.applied() > 0.0, "{f:?}");
            let globular = f.globular_share();
            assert!(
                (GLOBULAR_SHARE_PIN.0..=GLOBULAR_SHARE_PIN.1).contains(&globular),
                "seed {seed}: globular tubes {globular}"
            );
            let scale = f.dwarf_scale();
            assert!((0.0..=1.0).contains(&scale));
            if globular >= f.drawn && f.dwarfs_solar_masses > 0.0 {
                assert!(scale.abs() < f64::MIN_POSITIVE, "seed {seed}: {f:?}");
            } else if scale < 1.0 {
                scaled += 1;
                let tables = PotentialTables::in_plane(&MassModel::new(&params));
                scale_cold_masses(&mut debris, scale);
                let after = ShareFigures::of(&params, &tables, &debris);
                assert!(
                    (after.physical() / f.drawn - 1.0).abs() < 1e-12,
                    "seed {seed}: {} against {}",
                    after.physical(),
                    f.drawn
                );
            } else {
                assert!(f.physical() <= f.drawn);
            }
        }
        eprintln!("{scaled} of 16 galaxies scale their dwarfs");
    }

    /// The globular tubes' share of the halo, pinned provisionally: 0.23–7.0% over the 2,000
    /// galaxies of the summary (median 1.0%) at `GENERATOR_VERSION` 23, against P10.T3.d.a's
    /// 0.3–1%, which holds for the Milky Way fixture (0.84%) and about half of them. Plan 02's
    /// globular count follows the dark halo's mass and the halo's budget the stars', so a galaxy
    /// of a low stellar-to-halo mass ratio has many globulars for a small halo (P10.T3.d.a, as
    /// built).
    const GLOBULAR_SHARE_PIN: (f64, f64) = (0.002, 0.08);

    /// The percentiles the golden summary records.
    const PERCENTILES: [u32; 11] = [0, 1, 5, 10, 25, 50, 75, 90, 95, 99, 100];

    /// `values`' percentiles by nearest rank, and their mean.
    fn summary(values: &[f64]) -> ([f64; 11], f64) {
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let last = u32::try_from(sorted.len() - 1).expect("2,000 galaxies");
        let at = |p: u32| {
            let rank = (p * last + 50) / 100;
            sorted[usize::try_from(rank).expect("a rank below 2,000")]
        };
        let mean = values.iter().fold(0.0, |s, v| s + v) / f64::from(last + 1);
        (PERCENTILES.map(at), mean)
    }

    /// P10.T3.d.a over 2,000 seeds: the applied share is never above the drawn share; the
    /// globular tubes alone give 0.3–1% of the halo, which holds for about half the galaxies and
    /// is pinned provisionally ([`GLOBULAR_SHARE_PIN`], the median to 0.8–1.2%); and the
    /// distribution of the applied share is recorded in a golden summary, with a digest of every
    /// galaxy's applied share in seed order, for the brainstorm's owner (plan 10, Risks, "The
    /// discrete share against the dwarf mass range"). It moves with the potential and the dwarfs,
    /// and is re-blessed then.
    #[test]
    #[ignore = "slow: builds the in-plane potential of 2,000 galaxies"]
    fn the_applied_share_over_2000_galaxies_is_summarised() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::{GoldenWriter, f64_digest};

        let rows: Vec<ShareFigures> = (0..2_000_u64)
            .map(|n| figures(Seed::new(0x0a10_3d00_0000_0000 | n)).2)
            .collect();
        for (n, f) in rows.iter().enumerate() {
            assert!(
                f.applied().is_finite() && f.applied() <= f.drawn,
                "galaxy {n}: {f:?}"
            );
        }
        let globular: Vec<f64> = rows.iter().map(ShareFigures::globular_share).collect();
        for (n, &g) in globular.iter().enumerate() {
            assert!(
                (GLOBULAR_SHARE_PIN.0..=GLOBULAR_SHARE_PIN.1).contains(&g),
                "galaxy {n}: globular tubes {g}"
            );
        }
        let median = summary(&globular).0[5];
        assert!((0.008..=0.012).contains(&median), "{median}");
        let outside = globular
            .iter()
            .filter(|g| !(0.003..=0.01).contains(*g))
            .count();
        let over = rows
            .iter()
            .filter(|f| f.globular_share() >= f.drawn)
            .count();
        let applied: Vec<f64> = rows.iter().map(ShareFigures::applied).collect();
        let columns: [(&str, Vec<f64>); 4] = [
            ("applied", applied.clone()),
            (
                "physical",
                rows.iter().map(ShareFigures::physical).collect(),
            ),
            ("globular", globular),
            ("drawn", rows.iter().map(|f| f.drawn).collect()),
        ];
        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        w.line(&format!("galaxies {}", rows.len()));
        for (name, values) in &columns {
            let (percentiles, mean) = summary(values);
            for (p, v) in PERCENTILES.iter().zip(percentiles) {
                w.f64(&format!("{name} share, percentile {p}"), v);
            }
            w.f64(&format!("{name} share, mean"), mean);
            eprintln!(
                "{name} share: percentiles {PERCENTILES:?} {percentiles:.5?}, mean {mean:.5}"
            );
        }
        let count = |lo: f64, hi: f64| applied.iter().filter(|&&a| lo <= a && a < hi).count();
        let bins = [
            ("under 1%", count(0.0, 0.01)),
            ("1–2%", count(0.01, 0.02)),
            ("2–5%", count(0.02, 0.05)),
            ("5–10%", count(0.05, 0.10)),
            ("10–15%", count(0.10, 0.15 + 1e-12)),
        ];
        for (bin, n) in bins {
            w.line(&format!("applied share {bin}: {n} galaxies"));
        }
        let capped = rows.iter().filter(|f| f.dwarf_scale() < 1.0).count();
        let no_dwarfs = rows.iter().filter(|f| f.dwarfs_solar_masses <= 0.0).count();
        w.line(&format!(
            "drawn share binds, dwarfs scaled: {capped} galaxies"
        ));
        w.line(&format!("no dwarf debris inside: {no_dwarfs} galaxies"));
        w.line(&format!(
            "globular tubes outside 0.3–1% of the halo: {outside} galaxies"
        ));
        w.line(&format!(
            "globular tubes at or above the drawn share: {over} galaxies"
        ));
        w.u64_hex("applied share digest, in seed order", f64_digest(&applied));
        eprintln!(
            "{bins:?}; capped {capped}; no dwarf debris inside {no_dwarfs}; globular tubes \
             outside 0.3–1% {outside}, at or above the drawn share {over}"
        );
        golden!("galaxy/global_list/discrete_share", w.as_str());
    }

    /// `Debris::generate` carries the dwarfs' tubes and cores scaled by the share's own figures,
    /// and drops a tube scaled to nothing and a core scaled under 10³ systems: in the first sweep
    /// galaxy whose drawn share binds, and in the first whose globular tubes alone reach it.
    #[test]
    #[ignore = "slow: builds two galaxies' (R, z) grids and kinematic tables"]
    fn debris_carries_the_scaled_dwarfs_and_drops_what_the_scale_empties() {
        use crate::galaxy::Galaxy;
        use crate::galaxy::global_list::dwarfs::CORE_MIN_SYSTEMS;
        use crate::galaxy::global_list::{Debris, StreamOrigin};

        let find = |want: &dyn Fn(&ShareFigures) -> bool| {
            (0..2_000_u64)
                .map(|n| Seed::new(0x0a10_3d00_0000_0000 | n))
                .find(|&seed| want(&figures(seed).2))
        };
        let scaled = find(&|f| f.dwarf_scale() > 0.0 && f.dwarf_scale() < 1.0)
            .expect("a sweep galaxy whose drawn share binds");
        let emptied = find(&|f| f.dwarfs_solar_masses > 0.0 && f.dwarf_scale() <= 0.0)
            .expect("a sweep galaxy whose globular tubes reach the drawn share");
        for seed in [scaled, emptied] {
            let (params, mut want, f) = figures(seed);
            let scale = f.dwarf_scale();
            scale_cold_masses(&mut want, scale);
            let galaxy = Galaxy::from_params(seed, params)
                .expect("a drawn galaxy builds")
                .with_full_potential();
            let debris = Debris::generate(&galaxy);
            // The full tables keep the in-plane ones, so the share `Galaxy::from_params` would
            // store and the scale `Debris` applies are the same bits.
            let full = discrete_share(seed, galaxy.params(), galaxy.potential());
            assert!(full.total_cmp(&f.applied()).is_eq(), "{full} {f:?}");
            let tubes: Vec<_> = want
                .iter()
                .filter_map(|d| d.tube.as_ref())
                .filter(|t| t.mass().value() > 0.0)
                .collect();
            let got: Vec<_> = debris
                .streams()
                .iter()
                .filter(|s| matches!(s.origin(), StreamOrigin::Dwarf(_)))
                .collect();
            assert_eq!(got.len(), tubes.len(), "seed {seed}: scale {scale}");
            for (g, w) in got.iter().zip(&tubes) {
                assert_eq!(g.origin(), w.origin());
                assert!(g.mass().value().total_cmp(&w.mass().value()).is_eq());
            }
            let cores: Vec<_> = want
                .iter()
                .filter_map(|d| d.core.as_ref())
                .take(usize::from(DwarfCoreNumber::LIMIT))
                .filter(|c| c.systems() >= CORE_MIN_SYSTEMS)
                .collect();
            assert_eq!(debris.cores().len(), cores.len(), "seed {seed}");
            for (g, w) in debris.cores().iter().zip(&cores) {
                assert_eq!(g.progenitor(), w.progenitor());
                assert!(g.mass().value().total_cmp(&w.mass().value()).is_eq());
                assert!(g.systems() >= CORE_MIN_SYSTEMS);
            }
            assert!(debris.streams().iter().all(|s| s.mass().value() > 0.0));
            if scale <= 0.0 {
                assert!(got.is_empty() && debris.cores().is_empty(), "seed {seed}");
            }
            eprintln!(
                "seed {seed}: scale {scale:.4}, {} dwarf tubes, {} cores",
                got.len(),
                debris.cores().len()
            );
        }
    }

    #[test]
    fn the_scale_brings_the_figures_to_the_drawn_share() {
        let f = ShareFigures {
            globular_solar_masses: 2e6,
            dwarfs_solar_masses: 8e7,
            budget_solar_masses: 4e8,
            drawn: 0.05,
        };
        // (2 × 10⁶ + 8 × 10⁷) ÷ 4 × 10⁸ = 0.205 against 0.05: the dwarfs keep 18 of their 80.
        assert!((f.physical() - 0.205).abs() < 1e-15);
        assert!((f.applied() - 0.05).abs() < 1e-15);
        assert!((f.dwarf_scale() - 18.0 / 80.0).abs() < 1e-15);
        let small = ShareFigures {
            dwarfs_solar_masses: 1e7,
            ..f
        };
        assert!((small.applied() - 0.03).abs() < 1e-15);
        assert!((small.dwarf_scale() - 1.0).abs() < f64::EPSILON);
    }
}
