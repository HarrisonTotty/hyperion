//! P14.T27's frame search: layers F and G walked with the stellar layers, against a brute-force
//! search over every nearby system of the layers.

use std::collections::BTreeMap;

use super::*;
use crate::Seed;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, generate_cell};

/// The seed of the galaxy these ships fly in.
const SEED: u64 = 0x0e14_0027_f000_0001;

fn galaxy() -> &'static Galaxy {
    static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// A cache that keeps every cell it is given, so a second walk is served from memory.
#[derive(Debug, Default)]
struct Keep {
    cells: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for Keep {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let cell = self.cells.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(cell)
    }
}

/// The layers a ship can be held by, in `frame_layers`' order, written out apart from it.
const LAYERS: [Layer; 7] = [
    Layer::E,
    Layer::D,
    Layer::C,
    Layer::B,
    Layer::A,
    Layer::BrownDwarf,
    Layer::RoguePlanet,
];

/// The first `n` objects of `layer`'s cells along the solar circle, 26,000 ly out on +y.
fn objects(layer: Layer, n: usize) -> Vec<SystemRecord> {
    let y = 26_000 / i32::try_from(layer.cell_size_ly()).expect("a cell edge of a few light-years");
    let mut found = Vec::new();
    let mut cell = Vec::new();
    for x in 0.. {
        let key = CellKey::new(layer, [x, y, 0]).expect("the solar circle lies inside the cube");
        generate_cell(galaxy(), key, &mut cell);
        found.extend(
            cell.iter()
                .filter(|r| r.age_at_epoch().value() > 0.0)
                .copied(),
        );
        if found.len() >= n {
            break;
        }
    }
    found.truncate(n);
    found
}

/// The frame by brute force: every system of each of `layers` within three times its own heaviest
/// object's sphere of influence at the ship, each with its tidal radius at its own position,
/// through `select_frame`. It shares with `frame_at` only the rule and the tidal radius.
fn brute_force_frame(
    cache: &mut Keep,
    layers: &[Layer],
    ship: &GalacticPosition,
    t: UniverseTime,
) -> Option<SystemId> {
    let galaxy = galaxy();
    let centre = ship.cell().to_array().map(i64::from);
    let mut candidates = Vec::new();
    for &layer in layers {
        let largest = SolarMasses::new(crate::galaxy::imf::MassBand::from(layer).hi());
        let reach = LightYears::from(
            galaxy
                .potential()
                .tidal_radius(largest, &PointLy::from(ship)),
        )
        .value();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a reach of a few tens of light-years, exact in i64"
        )]
        let reach = (3.0 * reach).ceil() as i64 + 1;
        let size = i64::from(layer.cell_size_ly());
        let span = |axis: usize| {
            (centre[axis] - reach).div_euclid(size)..=(centre[axis] + reach).div_euclid(size)
        };
        for x in span(0) {
            for y in span(1) {
                for z in span(2) {
                    let key = [x, y, z].map(|c| i32::try_from(c).expect("a cell near the ship"));
                    let key = CellKey::new(layer, key).expect("a cell inside the cube");
                    cache.with_cell(galaxy, key, |cell| {
                        candidates.extend(
                            cell.iter()
                                .filter_map(|record| candidate_at(galaxy, record, ship, t)),
                        );
                    });
                }
            }
        }
    }
    select_frame(&candidates, None)
}

/// The ships: beside each of the first brown dwarfs and rogue planets of the solar circle, at a
/// tenth of a light-year and at a quarter of the object's own sphere of influence.
fn ships_beside(records: &[SystemRecord]) -> Vec<(SystemRecord, GalacticPosition)> {
    let galaxy = galaxy();
    let mut ships = Vec::new();
    for record in records {
        let [x, y, z] = record.epoch_position().to_light_years_f64();
        let sphere = LightYears::from(galaxy.potential().tidal_radius(
            record.primary_initial_mass(),
            &PointLy::from(record.epoch_position()),
        ))
        .value();
        for offset in [
            [0.1, 0.0, 0.0],
            [0.0, 0.25 * sphere, 0.0],
            [0.0, 0.0, -0.25 * sphere],
        ] {
            let at =
                GalacticPosition::from_light_years([x + offset[0], y + offset[1], z + offset[2]])
                    .expect("a light-year stays in the cube");
            ships.push((*record, at));
        }
    }
    ships
}

/// The search walks the stellar layers and then layers F and G.
#[test]
fn the_free_floating_layers_are_walked() {
    let walked: Vec<Layer> = frame_layers().map(|spec| spec.layer()).collect();
    assert_eq!(walked, LAYERS);
}

/// The plan's test: beside brown dwarfs and rogue planets, at the epoch and at the clock window's
/// end, `frame_at` finds exactly the frame a brute-force search over every layer finds, and a ship
/// beside an object is in its frame at least 18 times of the 36 for each kind.
#[test]
fn beside_free_floating_objects_the_search_finds_the_brute_force_frame() {
    let galaxy = galaxy();
    let dwarfs = objects(Layer::BrownDwarf, 6);
    let rogues = objects(Layer::RoguePlanet, 6);
    let ships: Vec<_> = ships_beside(&dwarfs)
        .into_iter()
        .chain(ships_beside(&rogues))
        .collect();
    let mut cache = Keep::default();
    let (mut by_dwarfs, mut by_rogues) = (0_usize, 0_usize);
    for (object, ship) in &ships {
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            let found = frame_at(galaxy, &mut cache, &[], ship, t, None)
                .expect("a time inside the clock window");
            assert_eq!(
                found,
                brute_force_frame(&mut cache, &LAYERS, ship, t),
                "a ship beside {:?} at {t}",
                object.id()
            );
            if found == Some(object.id()) {
                match object.layer() {
                    Layer::BrownDwarf => by_dwarfs += 1,
                    _ => by_rogues += 1,
                }
            }
        }
    }
    println!("ships held by their object: {by_dwarfs} brown dwarfs, {by_rogues} rogue planets");
    assert!(
        by_dwarfs >= 18 && by_rogues >= 18,
        "{by_dwarfs} {by_rogues}"
    );
}

/// A brown dwarf a light-year away takes a ship from a star three light-years away exactly when
/// its ratio is smaller by [`FRAME_HYSTERESIS`], with the tidal radii of the solar circle: at
/// 0.05 M☉ the star must be lighter than (3 × 0.9)³ × 0.05 M☉, 0.984 M☉.
#[test]
fn a_brown_dwarf_a_light_year_away_takes_the_ship_from_a_star_three_light_years_away_only_past_the_hysteresis()
 {
    let galaxy = galaxy();
    let dwarf = objects(Layer::BrownDwarf, 1)[0];
    let star = objects(Layer::C, 1)[0];
    let here = PointLy::from(dwarf.epoch_position());
    let radius = |mass: f64| {
        galaxy
            .potential()
            .tidal_radius(SolarMasses::new(mass), &here)
    };
    let ly = |value: f64| Metres::from(LightYears::new(value));
    let near = FrameCandidate::new(dwarf.id(), ly(1.0), radius(0.05)).unwrap();
    let mut took = 0;
    for step in 0..=200_u32 {
        let mass = 0.5 + f64::from(step) * 0.005;
        let far = FrameCandidate::new(star.id(), ly(3.0), radius(mass)).unwrap();
        assert!(near.contains_ship() && far.contains_ship(), "{mass} M☉");
        let takes = near.ratio() <= (1.0 - FRAME_HYSTERESIS) * far.ratio();
        let expected = if takes { dwarf.id() } else { star.id() };
        assert_eq!(
            select_frame(&[far, near], Some(star.id())),
            Some(expected),
            "{mass}"
        );
        // Arriving from interstellar space, the smaller ratio wins outright.
        let first = if near.ratio() < far.ratio() {
            dwarf.id()
        } else {
            star.id()
        };
        assert_eq!(select_frame(&[far, near], None), Some(first));
        took += usize::from(takes);
        assert_eq!(takes, mass < 0.05 * 2.7 * 2.7 * 2.7, "{mass} M☉");
    }
    assert!(took > 50 && took < 150, "{took}");
}
