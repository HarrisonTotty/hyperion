//! P09.T38's shells at the Milky Way's rates: the distinct shells at the epoch of every source the
//! catalogue holds, the field's core collapses (from 27 with their runaways and walkaways,
//! P09.T33.b) and Type Ia, the features' lists (P09.T36.a) and the centre's (P09.T36.b), over
//! seeds, by phase and size, with the longest window seen.
//!
//! The brainstorm's "The count is a result": 3,000 to 30,000 distinct shells across the ranges,
//! about 7,000 at the Milky Way's rates of about two core collapses and half a Type Ia a century
//! (Li et al. 2011, MNRAS 412, 1473), of which a thousand or two are still non-radiative, hot and
//! bright in radio and X-rays; the largest window 2–4 Myr, which the oldest known remnant bears out
//! at 4.3 Myr. A galaxy's own count is read at its own rates, each source's deaths in the source
//! horizon over its length, L + 2H, and scaled to the Milky Way's rates source by source. Radii
//! are plan 09's 10–800 ly at a window's end (P09.T16.a); a young shell is smaller.

use std::collections::BTreeMap;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::threads::in_shares;

use super::milky_way;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, ClassId, ClassProcess,
};
use crate::galaxy::displaced::ExplosionSite;
use crate::galaxy::features::catalogue::FeatureCatalogue;
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::recent::CentreRecent;
use crate::galaxy::features::ids::FeatureProcess;
use crate::galaxy::features::members::FeatureLevelList;
use crate::galaxy::features::members::recent::RecentClass;
use crate::galaxy::imf::MassFunctionKind;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::snr::{SHELL_WINDOW_CAP, ShellPhase, ShellWindow, shell_state_at};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon, UniverseTime};
use crate::units::Years;

/// The census walks in sixteen shares of its own (`in_shares`; `.config/nextest.toml`).
const SHARES: usize = 16;

/// A class cell's expected candidates below which a column of cells stops: none of its cells, nor
/// any farther from the plane, can hold an entry but by a chance of this.
const NEGLIGIBLE_MEAN: f64 = 1e-9;

/// The Milky Way's core collapses a century, the brainstorm's "about two" (Li et al. 2011).
const CORE_COLLAPSES_A_CENTURY: f64 = 2.0;

/// The Milky Way's Type Ia a century, the brainstorm's "half" (Li et al. 2011: 0.54 ± 0.12).
const TYPE_IA_A_CENTURY: f64 = 0.5;

/// The radii, light-years, a shell's size is binned against: plan 09's 10–800 ly at a window's end
/// (P09.T16.a).
const RADII_LY: (f64, f64) = (10.0, 800.0);

/// The phases in order, for the counts' rows.
const PHASES: [ShellPhase; 4] = [
    ShellPhase::FreeExpansion,
    ShellPhase::SedovTaylor,
    ShellPhase::PressureDrivenSnowplough,
    ShellPhase::MomentumConservingSnowplough,
];

/// Where an entry is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    /// The field's core-collapse class.
    FieldCoreCollapse,
    /// The field's Type Ia class.
    FieldTypeIa,
    /// A feature's list, its core collapses.
    FeatureCoreCollapse,
    /// A feature's list, its Type Ia.
    FeatureTypeIa,
    /// The centre's list, its core collapses.
    CentreCoreCollapse,
    /// The centre's list, its Type Ia.
    CentreTypeIa,
}

impl Source {
    /// Every source, in order.
    const ALL: [Self; 6] = [
        Self::FieldCoreCollapse,
        Self::FieldTypeIa,
        Self::FeatureCoreCollapse,
        Self::FeatureTypeIa,
        Self::CentreCoreCollapse,
        Self::CentreTypeIa,
    ];

    /// Whether its entries are core collapses.
    const fn is_core_collapse(self) -> bool {
        match self {
            Self::FieldCoreCollapse | Self::FeatureCoreCollapse | Self::CentreCoreCollapse => true,
            Self::FieldTypeIa | Self::FeatureTypeIa | Self::CentreTypeIa => false,
        }
    }
}

/// What a galaxy's census counts.
#[derive(Debug, Default, Clone)]
struct Census {
    /// Entries by source.
    entries: BTreeMap<Source, u64>,
    /// Deaths in the source horizon by source.
    horizon: BTreeMap<Source, u64>,
    /// Shells at the epoch by source and phase.
    shells: BTreeMap<(Source, ShellPhase), u64>,
    /// Shells at the epoch by source below, inside and above [`RADII_LY`].
    radii: BTreeMap<Source, [u64; 3]>,
    /// The largest shell's radius at the epoch by source, light-years.
    largest: BTreeMap<Source, f64>,
    /// The longest window of any entry, years.
    longest: f64,
}

impl Census {
    /// One entry of `source`, dying at `site`, its shell's window `window`.
    fn count(&mut self, source: Source, site: &ExplosionSite, window: Option<&ShellWindow>) {
        *self.entries.entry(source).or_default() += 1;
        if SourceHorizon::contains(site.death()) {
            *self.horizon.entry(source).or_default() += 1;
        }
        let Some(window) = window else {
            return;
        };
        self.longest = self.longest.max(window.duration().value());
        if site.death() > UniverseTime::EPOCH {
            return;
        }
        let age = UniverseTime::EPOCH
            .checked_since(site.death())
            .expect("a death before the epoch")
            .as_julian_years_f64();
        let Some(shell) = shell_state_at(window, Years::new(age)) else {
            return;
        };
        *self.shells.entry((source, shell.phase())).or_default() += 1;
        let radius = shell.radius().value();
        let largest = self.largest.entry(source).or_insert(0.0);
        *largest = largest.max(radius);
        let bin = if radius < RADII_LY.0 {
            0
        } else if radius <= RADII_LY.1 {
            1
        } else {
            2
        };
        self.radii.entry(source).or_default()[bin] += 1;
    }

    fn add(mut self, other: Self) -> Self {
        for (key, n) in other.entries {
            *self.entries.entry(key).or_default() += n;
        }
        for (key, n) in other.horizon {
            *self.horizon.entry(key).or_default() += n;
        }
        for (key, n) in other.shells {
            *self.shells.entry(key).or_default() += n;
        }
        for (key, bins) in other.radii {
            let mine = self.radii.entry(key).or_default();
            for (a, b) in mine.iter_mut().zip(bins) {
                *a += b;
            }
        }
        for (key, radius) in other.largest {
            let mine = self.largest.entry(key).or_insert(0.0);
            *mine = mine.max(radius);
        }
        self.longest = self.longest.max(other.longest);
        self
    }

    /// Shells at the epoch of the sources `keep` keeps.
    fn shells_where(&self, keep: impl Fn(Source, ShellPhase) -> bool) -> u64 {
        self.shells
            .iter()
            .filter(|((source, phase), _)| keep(*source, *phase))
            .map(|(_, n)| n)
            .sum()
    }

    /// The largest shell's radius at the epoch of any source, light-years.
    fn largest_radius(&self) -> f64 {
        self.largest.values().fold(0.0, |a, &b| a.max(b))
    }

    /// Shells at the epoch of every source below, inside and above [`RADII_LY`].
    fn radii_bins(&self) -> [u64; 3] {
        self.radii
            .values()
            .fold([0; 3], |[a, b, c], [d, e, f]| [a + d, b + e, c + f])
    }

    /// Every distinct shell at the epoch.
    fn distinct(&self) -> u64 {
        self.shells_where(|_, _| true)
    }

    /// The shells still non-radiative.
    fn non_radiative(&self) -> u64 {
        self.shells_where(|_, phase| phase.is_non_radiative())
    }

    /// Deaths a century of the core collapses (`core`) or the Type Ia, at the galaxy's own rate:
    /// deaths in the source horizon over its length.
    fn rate(&self, core: bool) -> f64 {
        let deaths: u64 = self
            .horizon
            .iter()
            .filter(|(source, _)| source.is_core_collapse() == core)
            .map(|(_, n)| n)
            .sum();
        let years =
            LIGHT_CROSSING_L.as_julian_years_f64() + 2.0 * CLOCK_WINDOW_H.as_julian_years_f64();
        100.0 * f64::from(u32::try_from(deaths).unwrap()) / years
    }

    /// `shells` of the core collapses (`core`) or the Type Ia, scaled from the galaxy's rate to
    /// the Milky Way's.
    fn at_milky_way_rates(&self, keep: impl Fn(ShellPhase) -> bool) -> f64 {
        let part = |core: bool, rate: f64| {
            let n = self.shells_where(|s, p| s.is_core_collapse() == core && keep(p));
            f64::from(u32::try_from(n).unwrap()) * rate / self.rate(core)
        };
        part(true, CORE_COLLAPSES_A_CENTURY) + part(false, TYPE_IA_A_CENTURY)
    }
}

/// A class's entries over the galaxy, each counted as `count` reads it: every class cell whose
/// mean is not negligible, a column at a time from the plane outwards (the bound never rises away
/// from the plane), in [`SHARES`] shares.
fn class_census<P: ClassProcess + Sync>(
    galaxy: &Galaxy,
    process: &P,
    class: ClassId,
    count: impl Fn(&P::Entry, &mut Census) + Sync,
) -> Census {
    let edge = 1_i32 << class.cell_log2_ly().expect("a gridded class");
    let reach = 65_536 / edge;
    let column = |x: i32, y: i32, census: &mut Census| {
        for side in [1, -1] {
            for k in 0.. {
                let z = if side > 0 { k } else { -1 - k };
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    break;
                };
                if process.bound(galaxy, &key).value() * key.volume_ly3() < NEGLIGIBLE_MEAN {
                    break;
                }
                for entry in CatalogueClassCell::generate(galaxy, process, key).entries() {
                    count(entry.value(), census);
                }
            }
        }
    };
    in_shares(SHARES, SHARES, |share| {
        let mut census = Census::default();
        for x in (-reach..reach).skip(share).step_by(SHARES) {
            for y in -reach..reach {
                column(x, y, &mut census);
            }
        }
        census
    })
    .into_iter()
    .fold(Census::default(), Census::add)
}

/// Every catalogue feature's listed core collapses and Type Ia, in [`SHARES`] shares of the
/// catalogue's walk.
fn feature_census(galaxy: &Galaxy) -> Census {
    in_shares(SHARES, SHARES, |share| {
        let mut census = Census::default();
        for process in FeatureProcess::ALL {
            if process == FeatureProcess::Cloud {
                continue;
            }
            let features = FeatureCatalogue::walk_process(galaxy, process)
                .skip(share)
                .step_by(SHARES);
            for feature in features {
                for entry in FeatureLevelList::of(galaxy, &feature).recent_entries(galaxy) {
                    let source = match entry.class() {
                        RecentClass::CoreCollapse => Source::FeatureCoreCollapse,
                        RecentClass::TypeIa => Source::FeatureTypeIa,
                        RecentClass::LuminousBlueVariable => continue,
                    };
                    let site = entry.site().expect("a death has a site");
                    census.count(source, site, entry.window());
                }
            }
        }
        census
    })
    .into_iter()
    .fold(Census::default(), Census::add)
}

/// The centre's listed core collapses and Type Ia.
fn centre_census(galaxy: &Galaxy) -> Census {
    let model = galaxy.centre().expect("the fixture's centre builds");
    let recent = CentreRecent::new(
        galaxy,
        model,
        victim_count(galaxy).expect("the centre's disruptions"),
    );
    let mut census = Census::default();
    for listed in recent.list(galaxy).entries() {
        // The list holds plan 11's binary classes too, which leave no shell.
        let Some(class) = RecentClass::of(listed.class()) else {
            continue;
        };
        let source = match class {
            RecentClass::CoreCollapse => Source::CentreCoreCollapse,
            RecentClass::TypeIa => Source::CentreTypeIa,
            RecentClass::LuminousBlueVariable => continue,
        };
        if let Some(entry) = recent.entry(galaxy, model, class, listed.index()) {
            let site = entry.site().expect("a death has a site");
            census.count(source, site, entry.window());
        }
    }
    census
}

/// Every source's census of `galaxy`.
fn census(galaxy: &Galaxy) -> Census {
    let core = class_census(
        galaxy,
        &CoreCollapseProcess::new(galaxy),
        ClassId::CORE_COLLAPSE,
        |entry, census| {
            census.count(
                Source::FieldCoreCollapse,
                entry.site(),
                Some(entry.supernova().window()),
            );
        },
    );
    let ia = class_census(
        galaxy,
        &TypeIaProcess::new(galaxy),
        ClassId::TYPE_IA,
        |entry, census| {
            census.count(
                Source::FieldTypeIa,
                entry.site(),
                Some(entry.supernova().window()),
            );
        },
    );
    core.add(ia)
        .add(feature_census(galaxy))
        .add(centre_census(galaxy))
}

/// One galaxy's census, printed.
fn report(name: &str, census: &Census) {
    eprintln!("{name}: entries {:?}", census.entries);
    eprintln!("{name}: deaths in the source horizon {:?}", census.horizon);
    eprintln!("{name}: shells by source and phase {:?}", census.shells);
    eprintln!(
        "{name}: {} distinct shells, {} non-radiative; {:.2} core collapses and {:.3} Type Ia a \
         century; at the Milky Way's rates {:.0} shells, {:.0} non-radiative; radii below, in and \
         above 10–800 ly {:?}, the largest {:.1} ly; the longest window {:.0} yr; by source, \
         the radii's bins {:?} and the largest {:?}",
        census.distinct(),
        census.non_radiative(),
        census.rate(true),
        census.rate(false),
        census.at_milky_way_rates(|_| true),
        census.at_milky_way_rates(ShellPhase::is_non_radiative),
        census.radii_bins(),
        census.largest_radius(),
        census.longest,
        census.radii,
        census.largest
    );
}

/// The golden summary of the fixture's census: entries, deaths in the horizon and shells by
/// source and phase, the radii's bins, the largest radius and the longest window.
fn golden_summary(census: &Census) -> String {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("rules every 27 hold");
    for source in Source::ALL {
        let get = |map: &BTreeMap<Source, u64>| map.get(&source).copied().unwrap_or(0);
        w.line(&format!(
            "{source:?} entries {} deaths in the horizon {}",
            get(&census.entries),
            get(&census.horizon)
        ));
        for phase in PHASES {
            let n = census.shells.get(&(source, phase)).copied().unwrap_or(0);
            w.line(&format!("{source:?} {phase:?} shells {n}"));
        }
        let bins = census.radii.get(&source).copied().unwrap_or_default();
        w.line(&format!(
            "{source:?} radii below, in and above 10–800 ly {bins:?}"
        ));
        w.f64(
            &format!("{source:?} largest radius ly"),
            census.largest.get(&source).copied().unwrap_or(0.0),
        );
    }
    w.line(&format!(
        "distinct shells {} non-radiative {}",
        census.distinct(),
        census.non_radiative()
    ));
    w.f64("longest window yr", census.longest);
    w.finish()
}

/// P09.T38's shells at the Milky Way's rates under every 27 hold, the fixture and three galaxies
/// drawn from their seeds (P09.T33.a's), natively; on wasm32-wasip1, which has no threads, the
/// fixture alone. Each galaxy holds 3,000–30,000 distinct shells, and every window lies under
/// `SHELL_WINDOW_CAP`. The rest is printed against the brainstorm and pinned, the fixture's counts
/// by source and phase, its radii's bins and largest radius and its longest window being the golden
/// summary `catalogue_classes/shells_27`: as measured (2026-11-01), 7,910–13,725 shells over the
/// four, 9,870–10,544 at the Milky Way's rates against its "about 7,000", 3,856–5,674
/// non-radiative against "a thousand or two" (most of them the features' Sedov–Taylor shells in
/// their bubbles' hot interiors), the longest windows 2.35–2.85 Myr against 2–4 Myr and the 4.3
/// Myr of the oldest known remnant, and some 4% of shells larger than 800 ly in radius, the field's
/// in thin gas, the largest 1,832 ly (findings, logged).
#[test]
#[ignore = "slow: every source's entries of four galaxies under every 27 hold"]
fn the_shells_at_milky_way_rates_under_every_27_hold() {
    let galaxies = [
        ("the fixture", None),
        ("seed 1", Some(0x0933_0010)),
        ("seed 2", Some(0x0933_0011)),
        ("seed 3", Some(0x0933_0012)),
    ];
    let galaxies = if cfg!(target_family = "wasm") {
        &galaxies[..1]
    } else {
        &galaxies[..]
    };
    for &(name, drawn) in galaxies {
        let galaxy = drawn.map_or_else(
            || milky_way(GalaxyRules::every_27_hold()),
            |seed| {
                Galaxy::from_params_with_rules(
                    Seed::new(seed),
                    GalaxyParams::from_seed(Seed::new(seed), MassFunctionKind::default()),
                    GalaxyRules::every_27_hold(),
                )
                .expect("a drawn galaxy builds")
                .with_full_potential()
            },
        );
        let census = census(&galaxy);
        report(name, &census);
        assert!(census.longest < SHELL_WINDOW_CAP.value(), "{name}");
        assert!(
            (3_000..=30_000).contains(&census.distinct()),
            "{name}: {}",
            census.distinct()
        );
        if drawn.is_none() {
            golden!("catalogue_classes/shells_27", &golden_summary(&census));
        }
    }
}
