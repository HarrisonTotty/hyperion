//! Plan 12, P12.T4.b: the optical depth to microlensing towards the bulge.
//!
//! The optical depth τ is the chance that a source lies within an Einstein radius of some lens at
//! one instant: towards the bulge it is of order 10⁻⁶, about 1 × 10⁻⁶ near Baade's window (Mróz et
//! al. 2019, ApJS 244, 29, §7: τ = 1.36 × 10⁻⁶ exp(0.39 (3° − |b|))) and up to 1.5 × 10⁻⁶ from
//! all-source samples (Sumi et al. 2013, ApJ 778, 150, §6: 2.35 × 10⁻⁶ exp(0.51 (3 − |b|))); over
//! this test's field, 3–5° south, 0.6–2.4 × 10⁻⁶. "Of order" is taken as within a factor of three
//! of 10⁻⁶ (P12.T4.b as built). It is estimated here from 10⁴ random
//! sightlines from the Sun to bulge systems: the lenses within U Einstein radii of a source number
//! τ U² on average, since the lenses' surface density is uniform on that scale, so τ is their count
//! over all sightlines divided by 10⁴ U².
//!
//! The optical depth does not depend on the lenses' velocities, so the galaxy is built without its
//! kinematic tables: its systems keep their epoch positions, the walk needs no pad for their motion,
//! and its tube is the thin one of Design note 13 (P12.T4.b as built).

use std::num::NonZeroU32;

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, NoCache, SystemRecord, generate_cell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::lensing::{LensQuery, LensSightline, lenses_along};
use hyperion_sim::time::UniverseTime;
use hyperion_testkit::lcg::Lcg;

/// The reach, in Einstein radii: 100, so that 10⁴ sightlines at τ ≈ 10⁻⁶ find about 100 lenses.
const REACH: f64 = 100.0;

/// The sightlines.
const SIGHTLINES: usize = 10_000;

/// P12.T4.b: the optical depth towards the bulge from 10⁴ random sightlines is of order 10⁻⁶,
/// taken as within a factor of three of it.
#[test]
#[ignore = "slow: 10^4 lens walks of 26,000 ly towards the bulge"]
fn lensing_optical_depth_towards_the_bulge() {
    let galaxy = Galaxy::from_params(Seed::new(0x1204_b200), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    // The sources: layer-B systems of the bulge in a field 3–5° south of the centre seen from the
    // Sun, 1,400–2,300 ly below the plane, as the surveys' fields near Baade's window are.
    let mut sources: Vec<SystemId> = Vec::new();
    let mut cell = Vec::new();
    'fill: for z in -143..-88 {
        for x in -30..30 {
            for y in -30..30 {
                generate_cell(
                    &galaxy,
                    CellKey::new(Layer::B, [x, y, z]).unwrap(),
                    &mut cell,
                );
                sources.extend(cell.iter().map(SystemRecord::id));
                if sources.len() >= 2 * SIGHTLINES {
                    break 'fill;
                }
            }
        }
    }
    assert!(
        sources.len() >= SIGHTLINES,
        "{} bulge sources",
        sources.len()
    );
    let mut lcg = Lcg::new(0x1204_b201);
    let picked: Vec<SystemId> = (0..SIGHTLINES)
        .map(|_| {
            let n = u64::try_from(sources.len()).unwrap();
            sources[usize::try_from(lcg.next_below(n)).unwrap()]
        })
        .collect();

    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let day = UniverseTime::new(86_400, 0).unwrap();
    let window = TimeWindow::new(UniverseTime::EPOCH, day).unwrap();
    let count = |ids: &[SystemId]| -> (u64, u64) {
        let mut cache = NoCache::new();
        let mut lenses = 0;
        let mut cells = 0;
        for &source in ids {
            let query = LensQuery::builder(LensSightline::new(sun, source), window)
                .max_impact(REACH)
                .cell_budget(NonZeroU32::new(1 << 20).unwrap())
                .build()
                .unwrap();
            let result = lenses_along(&galaxy, &mut cache, &[], &query).unwrap();
            lenses += u64::try_from(result.events().len()).unwrap();
            cells += result.walked().cells();
        }
        (lenses, cells)
    };
    // Four threads of a quarter each; the sums are integers, so their order does not matter.
    #[cfg(not(target_family = "wasm"))]
    let (lenses, cells) = std::thread::scope(|scope| {
        let handles: Vec<_> = picked
            .chunks(SIGHTLINES / 4)
            .map(|chunk| scope.spawn(move || count(chunk)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .fold((0, 0), |(l, c), (dl, dc)| (l + dl, c + dc))
    });
    // wasm32-wasip1 has no threads, so all of them on this one.
    #[cfg(target_family = "wasm")]
    let (lenses, cells) = count(&picked);
    let tau = f64::from(u32::try_from(lenses).unwrap())
        / (f64::from(u32::try_from(SIGHTLINES).unwrap()) * REACH * REACH);
    // The figure the plan records (P12.T4.b as built).
    eprintln!("optical depth {tau:e}: {lenses} lenses over {SIGHTLINES} sightlines, {cells} cells");
    assert!(
        (1.0 / 3.0e6..=3.0e-6).contains(&tau),
        "τ = {tau:e} from {lenses} lenses within {REACH} Einstein radii over {SIGHTLINES} \
         sightlines ({cells} cells)"
    );
}
