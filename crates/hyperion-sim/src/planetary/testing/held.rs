//! Every hold's rules as they are in force on one thread, carried to the threads of a test's own
//! pool.
//!
//! A hold's switch sets its rules on the calling thread alone, and a thread of a test's own pool
//! starts with none set, so a share run there reads the version's rules. [`in_shares`] reads every
//! hold's rules
//! on the calling thread ([`HeldRules::on_this_thread`]) and sets them on each share's thread
//! ([`HeldRules::install`]), so that a share reads the rules its test set on any number of threads,
//! as on the one thread where `hyperion_testkit::threads::in_shares` runs its shares on the caller's.
//! A test's oracles and counters (a pass trace, the exits taken) and its other switches of a
//! derivation's path, none a hold (the thermostat, the fields rebuilt and the like), stay the
//! calling thread's, as that pool asks of a share. A hold's bump deletes its line here with its
//! switch.

use crate::planetary::chem::envelope_abundances::{FormationDrawRules, with_formation_draws};
use crate::planetary::derive::atmosphere::{
    LineSwitchRules, MeltOxygenRules, with_line_switch, with_melt_oxygen,
};
use crate::planetary::derive::photometry::area_mix::{
    HazeLayerRules, IcyRegolithRules, MinorCloudsRules, OrganicSedimentRules, RayleighAddingRules,
    with_haze_layer, with_icy_regolith, with_minor_clouds, with_organic_sediment,
    with_rayleigh_adding,
};
use crate::planetary::fate::{MergedStepsRules, with_merged_steps};
use crate::planetary::hooks::envelope_atmosphere::{DaysideDecksRules, with_dayside_decks};
use crate::planetary::hooks::surface::deck::{LockedClimateRules, with_locked_climate};
use crate::planetary::hooks::surface::{CapBalanceRules, with_cap_balance};
use crate::planetary::hosts::strip::{StripRules, with_stripping};
use crate::substance::thermal::{RegolithInertiaRules, with_regolith_inertia};

/// Declares [`HeldRules`] over the holds, each its field, its rules' type, the reader of its rules
/// in force on this thread and its switch.
macro_rules! held_rules {
    ($($field:ident: $rules:ident = $reader:path => $switch:path,)+) => {
        /// Every hold's rules in force on one thread ([the module](self)).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct HeldRules {
            $($field: $rules,)+
        }

        impl HeldRules {
            /// The holds' rules types, by name, for the test that every hold is carried.
            #[cfg(not(target_family = "wasm"))]
            const TYPES: &[&str] = &[$(stringify!($rules),)+];

            /// Every hold's rules in force on this thread.
            #[must_use]
            pub(crate) fn on_this_thread() -> Self {
                Self { $($field: $reader(),)+ }
            }

            /// `run`'s result with these rules in force on this thread, each hold's switch set back
            /// as it returns, a panic in `run` included.
            pub(crate) fn install<R>(self, run: impl FnOnce() -> R) -> R {
                let held = self;
                held_rules!(@nest held, run; $($field => $switch,)+)
            }
        }
    };
    (@nest $held:ident, $run:ident;) => {
        $run()
    };
    (@nest $held:ident, $run:ident; $field:ident => $switch:path, $($rest:tt)*) => {
        $switch($held.$field, || held_rules!(@nest $held, $run; $($rest)*))
    };
}

held_rules! {
    melt_oxygen: MeltOxygenRules = MeltOxygenRules::in_force => with_melt_oxygen,
    cap_balance: CapBalanceRules = CapBalanceRules::in_force => with_cap_balance,
    stripping: StripRules = StripRules::in_force => with_stripping,
    line_switch: LineSwitchRules = LineSwitchRules::in_force => with_line_switch,
    dayside_decks: DaysideDecksRules = DaysideDecksRules::in_force => with_dayside_decks,
    formation_draws: FormationDrawRules = FormationDrawRules::in_force => with_formation_draws,
    regolith_inertia: RegolithInertiaRules =
        RegolithInertiaRules::in_force => with_regolith_inertia,
    icy_regolith: IcyRegolithRules = IcyRegolithRules::in_force => with_icy_regolith,
    minor_clouds: MinorCloudsRules = MinorCloudsRules::in_force => with_minor_clouds,
    organic_sediment: OrganicSedimentRules =
        OrganicSedimentRules::in_force => with_organic_sediment,
    rayleigh_adding: RayleighAddingRules = RayleighAddingRules::in_force => with_rayleigh_adding,
    haze_layer: HazeLayerRules = HazeLayerRules::in_force => with_haze_layer,
    locked_climate: LockedClimateRules = LockedClimateRules::in_force => with_locked_climate,
    merged_steps: MergedStepsRules = MergedStepsRules::in_force => with_merged_steps,
}

/// `share(0)` to `share(shares − 1)` on at most `hyperion_testkit::threads::granted(reserved)`
/// threads, their results in share order, with this thread's holds in force on each share's thread
/// ([the module](self)): `hyperion_testkit::threads::in_shares`, the pool a lib test runs its shares
/// on.
///
/// # Panics
///
/// If a share panics: the panic of the lowest share that does is resumed here, as
/// `hyperion_testkit::threads::in_shares` resumes it.
pub(crate) fn in_shares<T: Send>(
    shares: usize,
    reserved: usize,
    share: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let held = HeldRules::on_this_thread();
    hyperion_testkit::threads::in_shares(shares, reserved, |k| held.install(|| share(k)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `run`'s result with one rule set apart from the version's on this thread: the locked
    /// climate's oracle, a switch every version's rules leave off.
    fn set_apart<R>(run: impl FnOnce() -> R) -> R {
        with_locked_climate(LockedClimateRules::Held, run)
    }

    /// A held rule is read on a pool's threads: under each helper every share reads the holds the
    /// calling thread has in force, and the version's outside them; and where the run grants the
    /// pool more than one thread, the shares ran on threads of their own.
    #[test]
    fn a_held_rule_is_read_on_a_pool_s_threads() {
        let caller = std::thread::current().id();
        let reads = || {
            let here = HeldRules::on_this_thread();
            let there = in_shares(4, 2, |_| {
                (std::thread::current().id(), HeldRules::on_this_thread())
            });
            (here, there)
        };
        let version = HeldRules::on_this_thread();
        let held = set_apart(HeldRules::on_this_thread);
        assert_ne!(held, version, "the switch moves a rule");
        let (here, there) = set_apart(reads);
        assert_eq!(here, held);
        for (_, rules) in &there {
            assert_eq!(rules, &held, "the switch's rule on a share's thread");
        }
        if hyperion_testkit::threads::granted(2) > 1 {
            assert!(
                there.iter().all(|(thread, ..)| *thread != caller),
                "the shares ran on the pool's own threads"
            );
        }
        let (here, there) = reads();
        assert_eq!(here, version);
        for (_, rules) in &there {
            assert_eq!(rules, &version, "outside every helper");
        }
    }

    /// Installing a thread's holds puts them in force inside and sets each back as it returns.
    #[test]
    fn installed_holds_are_in_force_inside_and_set_back() {
        let version = HeldRules::on_this_thread();
        let held = set_apart(HeldRules::on_this_thread);
        assert_eq!(held.install(HeldRules::on_this_thread), held);
        assert_eq!(HeldRules::on_this_thread(), version);
    }

    /// The tests that run threads, catch a panic or read the sources, none of which a wasm32-wasip1
    /// test binary can (it has no threads, and a panic aborts it).
    #[cfg(not(target_family = "wasm"))]
    mod native_only {
        use std::path::Path;

        use super::*;

        /// Installed holds are set back when their closure panics.
        #[test]
        fn installed_holds_are_set_back_after_a_panic() {
            let version = HeldRules::on_this_thread();
            let held = set_apart(HeldRules::on_this_thread);
            let caught = std::panic::catch_unwind(|| held.install(|| panic!("inside the holds")));
            let payload = caught.expect_err("the closure panics");
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"inside the holds"));
            assert_eq!(HeldRules::on_this_thread(), version);
        }

        /// A thread of its own reads the version's rules, none of its caller's holds, and every one
        /// once they are installed on it: the pool's fault and its fix, whatever the run grants.
        #[test]
        fn a_thread_of_its_own_reads_its_caller_s_holds_once_installed() {
            let version = HeldRules::on_this_thread();
            set_apart(|| {
                let here = HeldRules::on_this_thread();
                let (bare, installed) = std::thread::scope(|scope| {
                    scope
                        .spawn(|| {
                            (
                                HeldRules::on_this_thread(),
                                here.install(HeldRules::on_this_thread),
                            )
                        })
                        .join()
                        .expect("the thread returns")
                });
                assert_eq!(bare, version, "a bare thread reads the version's rules");
                assert_eq!(installed, here);
            });
        }

        /// The `.rs` files under `dir`, each with its text.
        fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
            for entry in std::fs::read_dir(dir).expect("a source directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    sources(&path, found);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).expect("a readable source");
                    found.push((path.display().to_string(), text));
                }
            }
        }

        /// The sim's sources and the surface crate's, but this file, which names the holds' cells
        /// and the testkit's pool in its tests' text.
        fn the_sources() -> Vec<(String, String)> {
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
            let mut found = Vec::new();
            sources(&manifest.join("src"), &mut found);
            sources(&manifest.join("../hyperion-surface/src"), &mut found);
            found.retain(|(path, _)| !Path::new(path).ends_with("planetary/testing/held.rs"));
            found
        }

        /// The cells of `Cell<Option<…>>` that are not holds: a test's capture, its counts, its
        /// mode or a folded hold's oracle (the passes' root, stop and step), each read by its own
        /// module alone.
        const NOT_HOLDS: [&str; 9] = [
            "HeldFill",
            "Kelvin",
            "PassColdTraps",
            "PassRoot",
            "PassStep",
            "PassStop",
            "Vec",
            "f64",
            "u32",
        ];

        /// Every `Cell<Option<…>>` in the sim's sources and the surface crate's (not a `RefCell` or
        /// a `OnceCell`) is a hold with its line in [`HeldRules`], once, or a cell known not to be
        /// one ([`NOT_HOLDS`], or of a tuple), so that a hold added later reaches a pool's threads
        /// too, or fails here until it is listed.
        #[test]
        fn every_cell_of_an_optional_rule_is_a_carried_hold_or_known_not_one() {
            let mut holds = Vec::new();
            for (_, text) in the_sources() {
                for (at, _) in text.match_indices("Cell<Option<") {
                    let before = &text[..at];
                    if before.ends_with("Ref") || before.ends_with("Once") {
                        continue;
                    }
                    let rest = &text[at + "Cell<Option<".len()..];
                    let path: String = rest
                        .chars()
                        .take_while(|&c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
                        .collect();
                    let name = path.rsplit("::").next().unwrap_or_default().to_owned();
                    if name.is_empty() || NOT_HOLDS.contains(&name.as_str()) {
                        continue;
                    }
                    holds.push(name);
                }
            }
            holds.sort_unstable();
            let mut carried: Vec<String> = HeldRules::TYPES.iter().map(|&t| t.to_owned()).collect();
            carried.sort_unstable();
            assert_eq!(holds, carried);
        }

        /// No lib test runs the testkit's pool of shares itself, which would run its shares under
        /// the version's rules whatever its test's holds: each runs [`in_shares`]. A pool of scoped
        /// threads of a test's own is past this check, and installs [`HeldRules`] on each thread
        /// where its caller may set a hold (the sky census's pools).
        #[test]
        fn no_lib_test_runs_the_testkit_pool_directly() {
            let mut offenders = Vec::new();
            for (path, text) in the_sources() {
                for line in text.lines().map(str::trim_start) {
                    if !line.starts_with("//") && line.contains("threads::in_shares") {
                        offenders.push(format!("{path}: {line}"));
                    }
                }
                for (at, _) in text.match_indices("hyperion_testkit::threads::{") {
                    let group = &text[at..];
                    let group = &group[..group.find('}').unwrap_or(group.len())];
                    if group.contains("in_shares") {
                        offenders.push(format!("{path}: {group}}}"));
                    }
                }
            }
            assert!(offenders.is_empty(), "{offenders:#?}");
        }
    }
}
