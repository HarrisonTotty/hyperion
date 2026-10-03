//! The census's per-cell cache: each cell's systems at or above a mass floor (rendering plan R06,
//! Design note 12).
//!
//! The sim holds no cache; the server's is a byte-bounded LRU behind a lock (R06.T11.b). Whatever
//! holds one must keep it **monotone**: a cell's entry holds the records at or above the floor it
//! was built with, in candidate order, and serves a later query only if that query's floor is at or
//! above it, filtering the entry; a lower floor rebuilds the cell. So the cache never changes a
//! reply.

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell_where};
use crate::units::SolarMasses;

/// Where the census gets a cell's bright subset: every record of the cell whose primary's initial
/// mass is at least `floor`, in candidate order, as [`generate_cell_where`] makes it.
///
/// It takes `&self` so that the census's parallel jobs share one; an implementation that keeps
/// entries uses interior mutability behind a lock.
pub trait SkyCellCache: Sync {
    /// Writes the records of `key` at or above `floor` to `out` (cleared first).
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    );
}

/// No cache: every cell is generated afresh.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NoSkyCellCache;

impl SkyCellCache for NoSkyCellCache {
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    ) {
        generate_cell_where(galaxy, key, |m| m.value() >= floor.value(), out);
    }
}

/// What [`serve_from_entry`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use = "a refused entry leaves `out` untouched: the cell must be rebuilt"]
pub enum Served {
    /// `out` holds the entry's records at or above the floor.
    Served,
    /// The floor is below the entry's: `out` is untouched, and the cell must be rebuilt.
    Rebuild,
}

/// Serves a cell at `floor` from an entry built at `held` with `records`, if the rule allows:
/// [`Served::Served`] with `out` filled when `floor` is at or above `held`, [`Served::Rebuild`]
/// (and `out` untouched) when the cell must be rebuilt. The one place the monotone rule is written,
/// for every implementation.
pub fn serve_from_entry(
    held: SolarMasses,
    records: &[SystemRecord],
    floor: SolarMasses,
    out: &mut Vec<SystemRecord>,
) -> Served {
    if floor.value() < held.value() {
        return Served::Rebuild;
    }
    out.clear();
    out.extend(
        records
            .iter()
            .filter(|r| r.primary_initial_mass().value() >= floor.value())
            .copied(),
    );
    Served::Served
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::placement::generate_cell;
    use crate::id::Layer;

    #[test]
    fn the_monotone_rule_serves_at_or_above_the_floor_and_refuses_below() {
        let galaxy = milky_way_galaxy();
        let key = CellKey::new(Layer::C, [0, 26_000 / 32, 0]).expect("in the cube");
        let mut all = Vec::new();
        generate_cell(galaxy, key, &mut all);
        let held = SolarMasses::new(1.0);
        let mut entry = Vec::new();
        NoSkyCellCache.bright_subset(galaxy, key, held, &mut entry);
        let mut expected = all.clone();
        expected.retain(|r| r.primary_initial_mass().value() >= held.value());
        assert_eq!(
            entry, expected,
            "the bright subset is the cell filtered by mass"
        );
        for floor in [1.0, 1.4, 2.0] {
            let mut out = Vec::new();
            assert_eq!(
                serve_from_entry(held, &entry, SolarMasses::new(floor), &mut out),
                Served::Served
            );
            let mut fresh = Vec::new();
            NoSkyCellCache.bright_subset(galaxy, key, SolarMasses::new(floor), &mut fresh);
            assert_eq!(out, fresh, "floor {floor}");
        }
        let mut out = vec![all[0]];
        assert_eq!(
            serve_from_entry(held, &entry, SolarMasses::new(0.9), &mut out),
            Served::Rebuild
        );
        assert_eq!(out, vec![all[0]], "a refusal leaves out untouched");
    }
}
