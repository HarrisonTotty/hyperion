//! The hypervelocity survivors' weight and flights (plan 09, P09.T34.b; plan 08, Design note 25).
//!
//! The donors that double detonations fling out (ruling 128.1) are plan 08's hypervelocity class,
//! registered in layer D at zero weight. From `HYPERVELOCITY_VERSION` the class has its weight.
//! A survivor launched τ ago at `v` is inside the cube while τ < `path ÷ v`. It is the class's,
//! and not one of the Type Ia class's entries (P09.T34.a's member 1), while τ is at least S, the
//! latest start that class's interval can have: [`SHELL_WINDOW_CAP`] + L + H before the epoch,
//! 4.26 Myr. A galaxy's own start, its gas's cap + L + H (3.85 Myr in the Milky Way fixture), lies
//! within it, so no survivor is both an entry and the class's. Those launched between the two,
//! a few per cent, are neither, so that the class, as every other part of the fields, does not
//! depend on the seed through its gas's drawn pressure floor (lean, 2026-11-01). At the galaxy's own Type Ia rate, steady over the survivors' 10–20 Myr in the cube,
//! population i of [`SURVIVOR_POPULATIONS`] then holds `rate × shareᵢ × (path ÷ v_eff,ᵢ − S)`,
//! with `path` the form table's mean path from launch to the cube's face (P15.T6.f) and `v_eff,ᵢ`
//! the population's `1 ÷ ⟨1 ÷ v⟩`. That is ruling 128.1's weight less the interval. The rate is
//! the whole galaxy's ([`DelayTimeDistribution::from_galaxy`]), whose every Type Ia leaves its
//! share of survivors.
//!
//! A survivor's marks are drawn on plan 08's tags, so the class adds none, each from word 0 and
//! keyed by its ID:
//!
//! - its population, on `displaced.origin_bin`, by the populations' counts;
//! - its launch speed in that population's band, on `runaway.speed`, with density ∝ `path ÷ v − S`,
//!   by bisection;
//! - its time since launch, on `runaway.ejection`, uniform on `[S, path ÷ v]`.
//!
//! Placement draws its star from them (`placement::marks`), and the kinematics its velocity:
//! that speed, straight away from the galactic centre, the centre of `HYPERVELOCITY`'s cored power
//! law, whose γ ≈ 2 is a central source's `1 ÷ r²` (lean, 2026-11-01). The survivors outrun plan
//! 03's pad, so from the same version layer D pads at `UNBOUND_PAD_SPEED`
//! ([`pad_speed_in`](crate::galaxy::query::pad_speed_in)).

use super::class_table::{FormTable, SURVIVOR_POPULATIONS, SurvivorPopulation};
use crate::GeneratorVersion;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::supernova::SupernovaInterval;
use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::id::SystemId;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::{KilometresPerSecond, Years};

/// The generator version from which the hypervelocity survivors' class has its weight
/// ([`Survivors`], the hold of [`HypervelocityRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`HypervelocityRules::Unweighted`], so every generated value keeps its bits. The tests reach
/// the new arm through a galaxy built under `GalaxyRules::every_27_hold()`. The bump to 27 deletes
/// this hold, [`HypervelocityRules`] with its `Unweighted` arm, the galaxy's `hypervelocity` field
/// and its tripwire (`the_hypervelocity_survivors_are_held_only_until_the_27_bump`).
pub(crate) const HYPERVELOCITY_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the hypervelocity survivors' class has weight (the hold of [`HYPERVELOCITY_VERSION`]).
///
/// It is read where the class table is built, so `Galaxy::with_rules` refuses to change it, and
/// where a walk pads layer D (`query::pad_speed_in`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum HypervelocityRules {
    /// Version 26's and before: the class registered at zero weight (plan 08, Design note 25).
    Unweighted,
    /// Version 27's: the class weighted by [`Survivors`].
    Weighted,
}

/// Bisection steps of a survivor's speed quantile: 2⁻⁴⁸ of its band.
const SPEED_BISECTIONS: u32 = 48;

/// One of [`SURVIVOR_POPULATIONS`]: the slow donors or the fast (ruling 128.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurvivorBand {
    /// 1,000–1,500 km/s.
    Slow,
    /// 2,000–2,500 km/s.
    Fast,
}

impl SurvivorBand {
    /// Both, in [`SURVIVOR_POPULATIONS`]' order.
    pub const ALL: [Self; 2] = [Self::Slow, Self::Fast];

    /// Its index in [`SURVIVOR_POPULATIONS`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Slow => 0,
            Self::Fast => 1,
        }
    }

    /// Its share and speeds.
    #[must_use]
    pub const fn population(self) -> SurvivorPopulation {
        SURVIVOR_POPULATIONS[self.index()]
    }
}

/// The hypervelocity survivors of one galaxy (module documentation): their count inside the cube by
/// population, their birth components' odds, and what their flights are drawn from.
#[derive(Debug, Clone, PartialEq)]
pub struct Survivors {
    /// The galaxy's Type Ia rate, per year.
    rate_per_year: f64,
    /// By population of [`SURVIVOR_POPULATIONS`], the survivors inside the cube.
    count: [f64; SURVIVOR_POPULATIONS.len()],
    /// Per field component, its Type Ia rate per year: a survivor's odds of its birth component.
    births_per_year: Box<[f64]>,
    /// The least time since launch, years: S, the latest start the Type Ia class's interval can
    /// have, before the epoch.
    youngest: Years,
    /// The mean path from launch to the cube's face, ly (P15.T6.f).
    mean_exit_ly: f64,
}

/// A hypervelocity survivor's flight: its population, its launch speed and its time since launch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurvivorFlight {
    band: SurvivorBand,
    speed: KilometresPerSecond,
    since_launch: Years,
}

impl SurvivorFlight {
    /// Its population.
    #[must_use]
    pub const fn band(&self) -> SurvivorBand {
        self.band
    }

    /// Its speed from its launch site, in its population's band.
    #[must_use]
    pub const fn speed(&self) -> KilometresPerSecond {
        self.speed
    }

    /// The time since its launch, at least [`Survivors::youngest`].
    #[must_use]
    pub const fn since_launch(&self) -> Years {
        self.since_launch
    }
}

impl Survivors {
    /// The survivors of `galaxy` under the form table `forms`: its Type Ia rate, each component's
    /// part of it, and the latest start of the Type Ia class's interval.
    ///
    /// # Panics
    ///
    /// Never: the interval's start lies 4.26 Myr before the epoch, far inside the clock's range.
    #[must_use]
    pub(crate) fn of(galaxy: &Galaxy, forms: &FormTable) -> Self {
        let delays = DelayTimeDistribution::from_galaxy(galaxy);
        let fields = galaxy.fields();
        let births_per_year: Box<[f64]> = fields
            .component_ids()
            .map(|id| {
                fields.component(id).count_with_unborn() * delays.rate_per_system_per_year(id)
            })
            .collect();
        let rate_per_year = births_per_year.iter().sum();
        let start = SupernovaInterval::of_window(SHELL_WINDOW_CAP).start();
        let youngest = Years::new(-start.since_epoch().as_julian_years_f64());
        let mean_exit_ly = forms.rows().hypervelocity_mean_exit_ly;
        Self {
            rate_per_year,
            count: survivors_launched_before(rate_per_year, mean_exit_ly, youngest),
            births_per_year,
            youngest,
            mean_exit_ly,
        }
    }

    /// The survivors of `galaxy` where its rules give the class weight, from generator version 27;
    /// `None` under the held arm.
    #[must_use]
    pub(crate) fn where_weighted(galaxy: &Galaxy, forms: &FormTable) -> Option<Self> {
        match galaxy.rules().hypervelocity {
            HypervelocityRules::Unweighted => None,
            HypervelocityRules::Weighted => Some(Self::of(galaxy, forms)),
        }
    }

    /// The galaxy's Type Ia rate at the epoch, per year: its components' over their born and
    /// unborn systems.
    #[must_use]
    pub fn rate_per_year(&self) -> f64 {
        self.rate_per_year
    }

    /// The survivors inside the cube, by population of [`SURVIVOR_POPULATIONS`].
    #[must_use]
    pub fn count(&self) -> [f64; SURVIVOR_POPULATIONS.len()] {
        self.count
    }

    /// The survivors inside the cube: the class's count in layer D.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.count[0] + self.count[1]
    }

    /// Per field component, by index, its Type Ia rate per year: a survivor's odds of having been
    /// born in it.
    #[must_use]
    pub fn births_per_year(&self) -> &[f64] {
        &self.births_per_year
    }

    /// The survivors' mean path from launch to the cube's face, ly (P15.T6.f).
    #[must_use]
    pub fn mean_exit_ly(&self) -> f64 {
        self.mean_exit_ly
    }

    /// The least time since launch a member of the class has, S: the latest start the Type Ia
    /// class's interval can have, before the epoch, after which its entries hold the survivors.
    #[must_use]
    pub fn youngest(&self) -> Years {
        self.youngest
    }

    /// The speed at rank `u` in [0, 1] of a survivor of `band` inside the cube.
    ///
    /// Its density is ∝ `path ÷ v − S` over the band, the time a survivor at `v` is the class's,
    /// and its quantile a bisection of the cumulative `(path ÷ c) ln(v ÷ a) − S (v − a)`, which
    /// rises across both bands: `path ÷ v` is at least 9.4 Myr against S's 4.26 Myr.
    #[must_use]
    pub fn speed_at(&self, band: SurvivorBand, u: f64) -> KilometresPerSecond {
        let p = band.population();
        let reach = self.mean_exit_ly / LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let s = self.youngest.value();
        let cumulative = |v: f64| reach * math::ln(v / p.min_km_s) - s * (v - p.min_km_s);
        let target = u * cumulative(p.max_km_s);
        let (mut lo, mut hi) = (p.min_km_s, p.max_km_s);
        for _ in 0..SPEED_BISECTIONS {
            let mid = f64::midpoint(lo, hi);
            if cumulative(mid) < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        KilometresPerSecond::new(f64::midpoint(lo, hi))
    }

    /// The time since launch at rank `u` in [0, 1] of a survivor at `speed` inside the cube:
    /// uniform on `[S, path ÷ v]`.
    #[must_use]
    pub fn since_launch(&self, speed: KilometresPerSecond, u: f64) -> Years {
        let s = self.youngest.value();
        let inside = residence_years(self.mean_exit_ly, speed.value());
        Years::new(s + u * (inside - s).max(0.0))
    }

    /// The flight of the survivor `id` under `seed` (module documentation).
    ///
    /// # Panics
    ///
    /// Never: a weighted class has survivors in some population.
    #[must_use]
    pub fn flight(&self, seed: Seed, id: SystemId) -> SurvivorFlight {
        let open = |tag| Stream::open(seed, tag, ObjectKey::from(id));
        let index = open(tags::DISPLACED_ORIGIN_BIN)
            .mark()
            .pick_weighted(&self.count, self.total())
            // Rounding can leave the mark past every threshold: the last population with weight.
            .unwrap_or_else(|| {
                self.count
                    .iter()
                    .rposition(|&count| count > 0.0)
                    .expect("a weighted class has survivors")
            });
        let band = SurvivorBand::ALL[index];
        let launch = self.speed_at(band, open(tags::RUNAWAY_SPEED).uniform());
        SurvivorFlight {
            band,
            speed: launch,
            since_launch: self.since_launch(launch, open(tags::RUNAWAY_EJECTION).uniform()),
        }
    }
}

/// The years a survivor at `speed_km_s` takes over `path_ly`.
#[must_use]
fn residence_years(path_ly: f64, speed_km_s: f64) -> f64 {
    path_ly / (speed_km_s * LIGHT_YEARS_PER_YEAR_PER_KM_S)
}

/// The survivors of each population inside the cube launched at least `since` before the epoch.
///
/// At the Type Ia rate `rate_per_year`, the mean path to the cube's face being `mean_exit_ly`,
/// they are `rate × shareᵢ × (residenceᵢ − since)`, residence the path over `v_eff,ᵢ` (module
/// documentation); at `since` zero, [`survivors_inside`](super::class_table::survivors_inside).
#[must_use]
fn survivors_launched_before(
    rate_per_year: f64,
    mean_exit_ly: f64,
    since: Years,
) -> [f64; SURVIVOR_POPULATIONS.len()] {
    SURVIVOR_POPULATIONS.map(|p: SurvivorPopulation| {
        let inside = residence_years(mean_exit_ly, p.effective_speed_km_s());
        rate_per_year * p.share * (inside - since.value()).max(0.0)
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::displaced::class_table::survivors_inside;
    use crate::galaxy::rules::GalaxyRules;
    use crate::tables::displaced_forms::HYPERVELOCITY_MEAN_EXIT_LY;

    /// Survivors at the Milky Way's rate, 0.54 per century, and the latest interval, 4.26 Myr.
    fn milky_way_like() -> Survivors {
        let rate_per_year = 0.0054;
        let youngest = Years::new(4.263_144e6);
        Survivors {
            rate_per_year,
            count: survivors_launched_before(rate_per_year, HYPERVELOCITY_MEAN_EXIT_LY, youngest),
            births_per_year: Box::new([rate_per_year]),
            youngest,
            mean_exit_ly: HYPERVELOCITY_MEAN_EXIT_LY,
        }
    }

    /// The tripwire of the hold: below 27 the class has no weight.
    #[test]
    fn the_hypervelocity_survivors_are_held_only_until_the_27_bump() {
        assert!(
            GENERATOR_VERSION < HYPERVELOCITY_VERSION,
            "the 26 → 27 bump gives the hypervelocity survivors their weight: layer D's thinning \
             and every record of its cells move. Delete `HYPERVELOCITY_VERSION`, \
             `HypervelocityRules` with its `Unweighted` arm, `GalaxyRules::hypervelocity`, \
             `ClassTable`'s `Option` on its survivors and this test in the bump's own commit; \
             raise `query::pad_speed(Layer::D)` to `UNBOUND_PAD_SPEED`, which the sky census and \
             the server's sky read, and fold `pad_speed_in` and `escape_cut_holds_in` into it; and \
             re-bless the goldens plan 09's \"P09.T34.b, as built\" lists"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).hypervelocity,
            HypervelocityRules::Unweighted
        );
    }

    /// With no interval the count is ruling 128.1's, and the interval takes its share of each
    /// population's residence: 4.26 Myr of the slow survivors' 19 Myr and of the fast ones' 10.5.
    #[test]
    fn the_interval_takes_its_share_of_each_population() {
        let all = survivors_inside(0.0054, HYPERVELOCITY_MEAN_EXIT_LY);
        let none = survivors_launched_before(0.0054, HYPERVELOCITY_MEAN_EXIT_LY, Years::ZERO);
        for (a, b) in all.iter().zip(&none) {
            assert!((a - b).abs() <= 1e-12 * a, "{all:?} {none:?}");
        }
        let held = milky_way_like().count();
        for (i, p) in SURVIVOR_POPULATIONS.iter().enumerate() {
            let residence = residence_years(HYPERVELOCITY_MEAN_EXIT_LY, p.effective_speed_km_s());
            let kept = held[i] / all[i];
            assert!(
                (kept - (1.0 - 4.263_144e6 / residence)).abs() < 1e-12,
                "{i}: {kept}"
            );
        }
        eprintln!("all {all:?}, launched before the interval {held:?}");
    }

    /// The speed quantile inverts the density `path ÷ v − S`: its ends are the band's, it rises
    /// with the rank, and at each rank the density's integral to it, by a midpoint sum, is that
    /// share of the band's.
    #[test]
    fn the_speed_quantile_inverts_the_survivors_density() {
        let survivors = milky_way_like();
        let s = survivors.youngest().value();
        for band in SurvivorBand::ALL {
            let p = band.population();
            assert!((survivors.speed_at(band, 0.0).value() - p.min_km_s).abs() < 1e-9);
            assert!((survivors.speed_at(band, 1.0).value() - p.max_km_s).abs() < 1e-9);
            let density = |v: f64| residence_years(HYPERVELOCITY_MEAN_EXIT_LY, v) - s;
            let integral = |to: f64| {
                let n = 20_000;
                let step = (to - p.min_km_s) / f64::from(n);
                (0..n)
                    .map(|k| density(p.min_km_s + (f64::from(k) + 0.5) * step) * step)
                    .sum::<f64>()
            };
            let whole = integral(p.max_km_s);
            let mut last = p.min_km_s;
            for u in [0.1, 0.25, 0.5, 0.75, 0.9] {
                let v = survivors.speed_at(band, u).value();
                assert!(v > last, "{band:?}: {v} at {u}");
                last = v;
                let share = integral(v) / whole;
                assert!((share - u).abs() < 1e-6, "{band:?}: {share} at {u}");
            }
        }
    }

    /// A flight's time since launch is never under S, nor over the path at its speed, and its
    /// speed is in its population's band; the populations come in the counts' odds.
    #[test]
    fn a_flight_is_launched_before_the_interval_at_its_populations_speed() {
        let survivors = milky_way_like();
        let seed = Seed::new(0x0934_b000_0000_0001);
        let key = crate::galaxy::placement::CellKey::new(crate::id::Layer::D, [0, 400, 0])
            .expect("a cell of the cube");
        let n = 20_000;
        let mut fast = 0_u32;
        for i in 0..n {
            let id = key.candidate_id(i).expect("under the capacity");
            let flight = survivors.flight(seed, id);
            let band = flight.band().population();
            let v = flight.speed().value();
            assert!((band.min_km_s..=band.max_km_s).contains(&v), "{flight:?}");
            let tau = flight.since_launch().value();
            assert!(tau >= survivors.youngest().value(), "{flight:?}");
            assert!(
                tau <= residence_years(HYPERVELOCITY_MEAN_EXIT_LY, v),
                "{flight:?}"
            );
            fast += u32::from(flight.band() == SurvivorBand::Fast);
        }
        let p = survivors.count()[1] / survivors.total();
        let expected = p * f64::from(n);
        let z = (f64::from(fast) - expected) / (expected * (1.0 - p)).sqrt();
        assert_p_value(
            "the fast survivors' share",
            2.0 * normal_cdf(-z.abs()),
            ALPHA,
        );
    }
}
