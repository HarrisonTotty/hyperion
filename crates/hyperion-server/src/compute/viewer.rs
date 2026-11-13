//! The planet viewer's systems (rendering plan R15): the planets `viewer_planet` generated, which
//! the system's kinds answer as they answer a galaxy system, and each galaxy's density grid, which
//! `viewer_randomize` draws from.
//!
//! A viewer system cannot be generated again from its ID, which is a hash of its inputs
//! (`hyperion_sim::planetary::viewer`), so the server keeps the last
//! [`VIEWER_SYSTEMS_PER_GALAXY`] it generated for each galaxy, by use: a system asked for again is
//! the last used. One evicted is refused `unknown_system` by every kind that reads it, and the
//! client generates it again from its inputs. Each is some hundreds of kilobytes, its star's track
//! the most of it, so the store holds a few megabytes a galaxy at most; it is not charged to the
//! body cache's budget, so that the `SYSTEM` display's systems never evict the one being viewed.
//!
//! A galaxy's [`DensityGrid`] is built once, on the first draw's pool job (about a second on one
//! core), and kept for the last [`DENSITY_GRIDS`] galaxies drawn from.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::viewer::DensityGrid;

use super::{GalaxyKey, GeneratedSystem};

/// How many viewer systems the server keeps for each galaxy: the planet on show, those a few
/// edits of its inputs back, and the planets of a few viewers at once.
pub const VIEWER_SYSTEMS_PER_GALAXY: usize = 16;

/// How many galaxies' density grids the server keeps, each 590 kB.
pub const DENSITY_GRIDS: usize = 4;

/// One viewer system kept.
#[derive(Debug)]
struct Kept {
    key: GalaxyKey,
    id: SystemId,
    system: Arc<GeneratedSystem>,
}

/// The planet viewer's systems and density grids, kept as `compute::viewer`'s documentation says.
#[derive(Debug, Default)]
pub struct ViewerSystems {
    /// The systems kept, the least recently used first.
    systems: Mutex<VecDeque<Kept>>,
    /// The grids kept, the least recently used first.
    grids: Mutex<VecDeque<(GalaxyKey, Arc<DensityGrid>)>>,
}

impl ViewerSystems {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps `system`, the viewer system `id` of the galaxy `key`, as the last used, evicting that
    /// galaxy's least recently used beyond [`VIEWER_SYSTEMS_PER_GALAXY`].
    pub fn insert(&self, key: GalaxyKey, id: SystemId, system: Arc<GeneratedSystem>) {
        let mut systems = self.systems.lock().unwrap_or_else(PoisonError::into_inner);
        systems.retain(|kept| (kept.key, kept.id) != (key, id));
        systems.push_back(Kept { key, id, system });
        let held = systems.iter().filter(|kept| kept.key == key).count();
        if held > VIEWER_SYSTEMS_PER_GALAXY
            && let Some(oldest) = systems.iter().position(|kept| kept.key == key)
        {
            systems.remove(oldest);
        }
    }

    /// The viewer system `id` of the galaxy `key`, now the last used; `None` if it is not kept.
    #[must_use]
    pub fn get(&self, key: GalaxyKey, id: SystemId) -> Option<Arc<GeneratedSystem>> {
        let mut systems = self.systems.lock().unwrap_or_else(PoisonError::into_inner);
        let at = systems
            .iter()
            .position(|kept| (kept.key, kept.id) == (key, id))?;
        let kept = systems.remove(at)?;
        let system = Arc::clone(&kept.system);
        systems.push_back(kept);
        Some(system)
    }

    /// The density grid of `galaxy`, the galaxy `key`: the kept one, or built here and kept. Call
    /// it on a pool job: a build takes about a second.
    #[must_use]
    pub fn grid(&self, key: GalaxyKey, galaxy: &Galaxy) -> Arc<DensityGrid> {
        let kept = {
            let mut grids = self.grids.lock().unwrap_or_else(PoisonError::into_inner);
            grids
                .iter()
                .position(|(held, _)| *held == key)
                .and_then(|at| grids.remove(at))
                .map(|entry| {
                    let grid = Arc::clone(&entry.1);
                    grids.push_back(entry);
                    grid
                })
        };
        if let Some(grid) = kept {
            return grid;
        }
        // Built outside the lock: two first draws at once build it twice, and keep one.
        let grid = Arc::new(DensityGrid::of(galaxy));
        let mut grids = self.grids.lock().unwrap_or_else(PoisonError::into_inner);
        if !grids.iter().any(|(held, _)| *held == key) {
            grids.push_back((key, Arc::clone(&grid)));
            if grids.len() > DENSITY_GRIDS {
                grids.pop_front();
            }
        }
        grid
    }

    /// How many viewer systems are kept, over every galaxy.
    #[must_use]
    pub fn len(&self) -> usize {
        self.systems
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether no viewer system is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::planetary::viewer::{PlanetInputs, RawPlanetInputs, ViewerSystem};

    use super::*;

    fn earth(seed: u64) -> (SystemId, Arc<GeneratedSystem>) {
        let raw = RawPlanetInputs {
            seed,
            star_mass_msun: 1.0,
            fe_h_dex: 0.0,
            age_gyr: 4.57,
            mass_mearth: 1.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0167,
            formed_at_au: None,
            day_hours: None,
            tilt_deg: None,
            water_rank: None,
            carbon_rank: None,
            nitrogen_rank: None,
        };
        let system = ViewerSystem::generate(PlanetInputs::try_from(raw).unwrap()).unwrap();
        (system.id(), Arc::new(system.into_parts()))
    }

    #[test]
    fn each_galaxy_keeps_its_last_used_systems_and_evicts_its_least_recently_used() {
        let store = ViewerSystems::new();
        let (a, b) = (
            GalaxyKey::new(1, GENERATOR_VERSION),
            GalaxyKey::new(2, GENERATOR_VERSION),
        );
        let systems: Vec<_> = (0..=u64::try_from(VIEWER_SYSTEMS_PER_GALAXY).unwrap())
            .map(earth)
            .collect();
        for (id, system) in &systems[..VIEWER_SYSTEMS_PER_GALAXY] {
            store.insert(a, *id, Arc::clone(system));
        }
        store.insert(b, systems[0].0, Arc::clone(&systems[0].1));
        // The first is used again, so the second is the least recently used of galaxy `a`.
        assert!(store.get(a, systems[0].0).is_some());
        let (last, system) = &systems[VIEWER_SYSTEMS_PER_GALAXY];
        store.insert(a, *last, Arc::clone(system));
        assert!(store.get(a, systems[1].0).is_none(), "evicted");
        assert!(store.get(a, systems[0].0).is_some());
        assert!(store.get(a, *last).is_some());
        assert!(
            store.get(b, systems[0].0).is_some(),
            "another galaxy's are its own"
        );
        assert_eq!(store.len(), VIEWER_SYSTEMS_PER_GALAXY + 1);
        // Kept again, a system is not held twice.
        store.insert(a, *last, Arc::clone(system));
        assert_eq!(store.len(), VIEWER_SYSTEMS_PER_GALAXY + 1);
    }
}
