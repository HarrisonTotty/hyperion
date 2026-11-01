//! P09.T38's slow tests: the catalogue classes against the grid, the features and the budget,
//! across every class at once, under every 27 hold and once under the 26 arm for comparison (plan
//! 09, "P09.T38 Complementarity, budgets and the Milky Way's shells").
//!
//! - [`complementarity`]: 10⁵ mark sets about the shared test's interval and across the clock
//!   window, each held by exactly one of the grid and the catalogue, the predicate's reject path
//!   forced; and the grid's own cells, claimed or kept, through the production hook.
//! - [`budgets`]: field, members and catalogue systems against each population's budget in twelve
//!   test volumes, the centre's left out.
//! - [`shells`]: the distinct shells of every source at the epoch, over seeds, by phase and size.
//! - [`recent_in_range`]: every recent entry of the features' and the centre's lists (P09.T36)
//!   inside a range query's sphere returned once by the sources the server registers (P09.T37).
//!
//! The per-class tests stay where they are (`core_collapse`, `lbv`, `type_ia`, plan 11's binary
//! classes, the features' and the centre's lists); these are the shared, sampled ones.

mod budgets;
mod complementarity;
mod recent_in_range;
mod shells;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::global_list::{GlobalList, StreamNumber, TubeLookup, TubeTable};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellCache, CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::math;

/// The seed of the Milky Way fixture: P09.T35's carve-out's and P09.T36's lists', so that the
/// figures compare with theirs.
const SEED: u64 = 0x0935_0000_0000_0001;

/// The fixture under `rules`, with its kinematic tables, as every production galaxy has them: each
/// grid site is the epoch position moved along the record's velocity to its death.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
        .expect("the Milky Way fixture builds")
        .with_full_potential()
}

/// A lookup of `galaxy`'s global list that builds each tube table the first time a query reaches
/// its stream and keeps it, for the global list's source as the server registers it from 27.
struct Tubes<'a> {
    galaxy: &'a Galaxy,
    list: GlobalList,
    built: Mutex<BTreeMap<StreamNumber, Arc<TubeTable>>>,
}

impl<'a> Tubes<'a> {
    /// The lookup of `galaxy`'s list, built now ([`GlobalList::build`]).
    fn new(galaxy: &'a Galaxy) -> Self {
        Self {
            galaxy,
            list: GlobalList::build(galaxy),
            built: Mutex::default(),
        }
    }
}

impl TubeLookup for Tubes<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let spec = self.list.stream(stream).expect("a stream of the list");
        let mut tubes = self
            .built
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(tubes.entry(stream).or_insert_with(|| {
            Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"))
        }))
    }
}

/// A grid cache that keeps every cell it is asked for, for one galaxy: the layer-E cell about the
/// centre takes over a minute to generate under every 27 hold (pod, 16 vCPU), and every query
/// there reads it.
#[derive(Debug, Default)]
struct KeepCells(BTreeMap<CellKey, Vec<SystemRecord>>);

impl CellCache for KeepCells {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        f(self.0.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        }))
    }
}

/// `count` layer-E cells along each of `galaxy`'s arm ridges, in the plane, at each radius of
/// `radii` ly: the young field's richest cells, where the recently dead are.
fn ridge_cells(galaxy: &Galaxy, radii: &[f64], count: i32) -> Vec<CellKey> {
    let arms = galaxy.fields().arms();
    let mut keys = Vec::new();
    for &r in radii {
        for arm in 0..4 {
            let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, arm));
            for step in -count / 2..count / 2 {
                let along = f64::from(step) * f64::from(Layer::E.cell_size_ly());
                let at = GalacticPosition::from_light_years([
                    r * cos - along * sin,
                    r * sin + along * cos,
                    0.0,
                ])
                .expect("in the cube");
                let key = CellKey::containing(Layer::E, &at).expect("a cell of the cube");
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    keys
}
