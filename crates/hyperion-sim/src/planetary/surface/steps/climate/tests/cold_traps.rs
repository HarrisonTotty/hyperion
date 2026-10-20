//! R09.T13.f's tests: the cold traps (decision-r09-t13f-cold-traps §7.3), plan 09's three under the
//! ruled contract and the ruling's new ones. Fractions and areas are held to at least 2 × 10⁻³
//! relative or one cell, never to bits, since P14.T51.i's closed form moves them by up to
//! 1.1 × 10⁻³ at `trap_threshold`'s thresholds (decision-r09-t13f-cold-traps §3.4). One cell at
//! level 6 is 5.12 × 10⁻⁵ of the sphere, larger than most targets here, so on the Moon-like input
//! a trapped part's accounting is also held to [`ACCOUNTED_SHARE`] of its target.

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{
    MaterialPalette, PaletteEntry, PaletteRole, SynthesisCell, cell_index,
};
use hyperion_surface::substance_key::SubstanceKey;

use super::super::ice::{ICE_CODES, IceKey, PlacedTrap, place_ices, trap_fractions};
use super::super::{Diagnostics, run_diagnosed};
use super::{builder_of, grids};
use crate::math;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation;
use crate::planetary::hooks::cold_traps::{
    ColdTrapGround, ColdTrapGroundParts, over_latitude, trap_threshold,
};
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::{
    AreaShare, CoarseInputs, ColdTrapInputs, SeasonalOrbit, Spin,
};
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::{CellState, Pass, Working};
use crate::substance::by_key;
use crate::units::{Degrees, EarthFluxes, Kelvin, Radians, WattsPerSquareMetre};
use hyperion_testkit::float::assert_same_bits;

/// The flux at the Moon at Earth's perihelion, W m⁻²: 1,361 W m⁻² at 1 au taken to 0.98329 au,
/// plan 14's lunar sky as T51.f's as-built lunar case takes it (`cold_traps::lunar_sky`, whose
/// `lunar_parts` give it as the orbit mean of a circular orbit, so that `S_p` is it to the bit).
const LUNAR_PERIAPSIS_FLUX_W_M2: f64 = 1361.0 / (0.98329 * 0.98329);

/// The Moon's greatest solar declination, degrees: 1.54°, T51.f's lunar case (Hayne et al. 2021's
/// sky, as plan 14's `lunar_sky` takes it).
const LUNAR_DECLINATION_DEG: f64 = 1.54;

/// How far a trapped part's placed + undrawn + short may lie from its target on the Moon-like
/// input, a share of the target: 2 × 10⁻², tighter than one cell there, above main's tolerance
/// floor of 2 × 10⁻³, and above the quantisation's measured loss (plan 09's F2: about 0.9% of
/// water's 1.7 × 10⁻⁵ on the Moon-like input, a share carried past the last cell that can draw it).
const ACCOUNTED_SHARE: f64 = 2e-2;

/// The key `k`, which a test knows to be valid.
fn key(k: &str) -> SubstanceKey {
    SubstanceKey::new(k).unwrap()
}

/// The share of `k` over `area_fraction` of the surface.
fn share(k: &str, area_fraction: f64) -> AreaShare {
    AreaShare {
        substance: key(k),
        area_fraction,
    }
}

/// The cold-trap threshold of `k`, K.
fn threshold(k: &str) -> Kelvin {
    trap_threshold(by_key(k).unwrap()).unwrap()
}

/// The latitude of the body-frame unit vector `d`, degrees.
fn latitude_deg(d: [f64; 3]) -> f64 {
    math::asin(d[2].clamp(-1.0, 1.0)).to_degrees()
}

/// T51.f's lunar cold-trap inputs: the Sun's angular radius 0.2711° at Earth's perihelion, and the
/// bare regolith's Bond albedo 0.12 (Hayne et al. 2017, as plan 14's `lunar_parts` takes it).
fn lunar_group() -> ColdTrapInputs {
    ColdTrapInputs {
        star_angular_radius: Radians::from(Degrees::new(0.2711)),
        bond_albedo: 0.12,
    }
}

/// The reference Moon under T51.f's as-built lunar sky at `flux_w_m2` (its periapsis flux, on a
/// circular orbit) and greatest declination `declination_deg`: its spin at that obliquity, its
/// mean temperature plan 14's airless rule at that flux (the reference's own), its heat flow the
/// reference's 0.018 W m⁻², its craters the reference's, with the ices `ices`, the cold-trapped
/// shares `traps`, [`lunar_group`] and a carbon dioxide ice entry (the reference Mars's) beside
/// its water's.
fn moon_under(
    flux_w_m2: f64,
    declination_deg: f64,
    ices: &[(&str, f64)],
    traps: &[(&str, f64)],
) -> CoarseInputs {
    let moon = reference::moon_like();
    let mars = reference::mars_like();
    let co2 = mars
        .palette()
        .get(mars.palette().find(PaletteRole::Ice, key("CO2")).unwrap())
        .unwrap()
        .clone();
    let mut entries: Vec<PaletteEntry> = moon.palette().entries().to_vec();
    entries.push(co2);
    entries.sort_by_key(|e| (e.role, e.substance));
    let flux = WattsPerSquareMetre::new(flux_w_m2);
    let mean = irradiation::equilibrium_temperature(
        EarthFluxes::from(flux),
        SurfaceState::Airless.albedo(SurfaceMaterial::Rock),
    );
    builder_of(&moon)
        .spin(Spin {
            obliquity: Radians::from(Degrees::new(declination_deg)),
            ..*moon.spin()
        })
        .host_flux(flux)
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0,
            ..*moon.seasonal_orbit().unwrap()
        })
        .mean_surface_temperature(mean)
        .ices(ices.iter().map(|&(k, a)| share(k, a)))
        .cold_traps(traps.iter().map(|&(k, a)| share(k, a)))
        .cold_trap_inputs(lunar_group())
        .palette(MaterialPalette::new(entries).unwrap())
        .build()
        .unwrap()
}

/// The Moon-like input of the ruling's tests (T51.f's lunar case) with `ices` and `traps`.
fn moon_with(ices: &[(&str, f64)], traps: &[(&str, f64)]) -> CoarseInputs {
    moon_under(
        LUNAR_PERIAPSIS_FLUX_W_M2,
        LUNAR_DECLINATION_DEG,
        ices,
        traps,
    )
}

/// Plan 14's ground for the cold traps of `inputs`, from the same parts R09's placement reads.
fn plan_14_ground(inputs: &CoarseInputs) -> ColdTrapGround {
    let group = inputs.cold_trap_inputs().unwrap();
    ColdTrapGround::new(&ColdTrapGroundParts {
        obliquity: inputs.spin().obliquity,
        orbit_mean_flux: inputs.host_flux(),
        eccentricity: inputs.seasonal_orbit().map_or(0.0, |o| o.eccentricity),
        star_radius: group.star_angular_radius,
        bond_albedo: group.bond_albedo,
        heat_flow: inputs.heat_flow(),
        craters: inputs.craters(),
    })
    .unwrap()
}

/// Plan 14's cold-trap fraction `f_ct` of `threshold` over the whole surface for `ground`: the
/// latitude integral of its `fractions_at` at that threshold alone (`ColdTrapGround`'s own
/// `trap_fraction`, P14.T51.f's).
fn plan_14_fraction(ground: &ColdTrapGround, threshold: Kelvin) -> f64 {
    over_latitude(|latitude| {
        let mut fraction = [0.0];
        ground.fractions_at(latitude, &[threshold], &mut fraction);
        fraction[0]
    })
}

/// Runs the climate step on `inputs` at level `level` from the initial state.
fn run_at(inputs: &CoarseInputs, level: u8) -> (CellGraph, Working, Diagnostics) {
    let (field, climate) = grids(level);
    let pass = Pass::new(reference::SEED, inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    let diagnostics = run_diagnosed(&pass, &mut working);
    (field, working, diagnostics)
}

/// The initial cells of `inputs` at level `level`, with keys colder toward the poles by latitude
/// (−|sin φ| K above 100 K, the warmest month 1 K above): a placement's own test, which needs no
/// climate.
fn polar_keys(inputs: &CoarseInputs, level: u8) -> (CellGraph, Vec<CellState>, Vec<IceKey>) {
    let (field, climate) = grids(level);
    let pass = Pass::new(reference::SEED, inputs, &field, &climate);
    let cells = Working::initial(&pass).cells;
    let keys = field
        .centres()
        .iter()
        .map(|c| IceKey {
            radiative_mean_k: 100.0 - c[2].abs(),
            warmest_k: 101.0 - c[2].abs(),
        })
        .collect();
    (field, cells, keys)
}

/// One cell's share of the surface at most, on `grid`.
fn one_cell(grid: &CellGraph) -> f64 {
    let total: f64 = grid.solid_angles_sr().iter().sum();
    grid.solid_angles_sr()
        .iter()
        .fold(0.0, |m: f64, &o| m.max(o))
        / total
}

/// The share of the surface the cells naming `entry` cover, Σ Ω f ÷ Σ Ω.
fn area_of(grid: &CellGraph, cells: &[CellState], entry: u8) -> f64 {
    let total: f64 = grid.solid_angles_sr().iter().sum();
    cells
        .iter()
        .zip(grid.solid_angles_sr())
        .filter(|(cell, _)| cell.ice_entry == Some(entry))
        .fold(0.0, |sum, (cell, &omega)| sum + omega * cell.ice_fraction)
        / total
}

/// The diagnostics of the cold-trapped part of `k` in `traps`.
fn trap_of<'a>(traps: &'a [PlacedTrap], k: &str) -> &'a PlacedTrap {
    traps.iter().find(|t| t.substance == key(k)).unwrap()
}

/// Asserts that `trap`'s placed + undrawn + short lies within [`ACCOUNTED_SHARE`] of its target.
fn assert_accounted(trap: &PlacedTrap) {
    let accounted = trap.placed + trap.undrawn + trap.short;
    let relative = (accounted - trap.target).abs() / trap.target;
    assert!(
        relative <= ACCOUNTED_SHARE,
        "placed + undrawn + short {accounted:e} against {:e}, {relative:e} of it: {trap:?}",
        trap.target
    );
}

/// Plan 09's first test, under the ruled contract: a Moon-like input whose 1.7 × 10⁻⁵ of water
/// ice is all cold-trapped (T51.f's lunar case: α 0.2711°, A 0.12, q 0.018 W m⁻², δ 1.54°) places
/// it only poleward of 70°, more than 90% of it poleward of 80°, no cell's share above its
/// fraction, none of it short.
#[test]
fn a_moon_like_input_places_its_trapped_water_poleward_of_70_degrees() {
    let inputs = moon_with(&[("H2O", 1.7e-5)], &[("H2O", 1.7e-5)]);
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let water = inputs.palette().find(PaletteRole::Ice, key("H2O")).unwrap();
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("H2O")]);
    let (mut all, mut past_70, mut past_80) = (0.0, 0.0, 0.0);
    for (((cell, centre), &omega), &f) in working
        .cells
        .iter()
        .zip(field.centres())
        .zip(field.solid_angles_sr())
        .zip(&fractions)
    {
        if cell.ice_fraction <= 0.0 {
            continue;
        }
        assert_eq!(cell.ice_entry, Some(water));
        let latitude = latitude_deg(*centre).abs();
        assert!(latitude > 70.0, "water ice at {latitude}°");
        assert!(
            cell.ice_fraction <= f + 1e-12,
            "a share {} above its fraction {f} at {latitude}°",
            cell.ice_fraction
        );
        all += omega * cell.ice_fraction;
        if latitude > 70.0 {
            past_70 += omega * cell.ice_fraction;
        }
        if latitude > 80.0 {
            past_80 += omega * cell.ice_fraction;
        }
    }
    assert!(diagnostics.ices.iter().all(|ice| ice.cells == 0));
    let trap = trap_of(&diagnostics.cold_traps, "H2O");
    eprintln!(
        "Moon: {:.4} of the drawn water poleward of 70°, {:.4} poleward of 80°; {trap:?}",
        past_70 / all,
        past_80 / all
    );
    assert!(past_80 / all > 0.9, "{}", past_80 / all);
    assert_same_bits(trap.short, 0.0);
    assert!(trap.scale < 1.0);
    assert!(
        (trap.placed + trap.undrawn - trap.target).abs() <= one_cell(&field),
        "{trap:?}"
    );
    assert_accounted(trap);
    assert!(
        (area_of(&field, &working.cells, water) - trap.target).abs() <= one_cell(&field),
        "{trap:?}"
    );
}

/// The trapped shares' quantisation (the ice module's "The quantisation", `ICE_CODES`), on the
/// Moon-like input whose water ice is all cold-trapped, so that every cell it draws is a free
/// cell: each drawn cell's share times 255 is a whole code to the bit, at most ⌊255 f⌋ of its
/// fraction f; the drawn total over the field is `placed` within one cell; and through the field's
/// own ice byte (`SynthesisCell::quantise_ice`, decoded in its steps of 1 ÷ 255) every code is kept
/// and the drawn area still matches the target within one cell, and within [`ACCOUNTED_SHARE`].
#[test]
fn the_trapped_shares_are_whole_codes_and_keep_their_area_through_the_field_s_ice_byte() {
    let inputs = moon_with(&[("H2O", 1.7e-5)], &[("H2O", 1.7e-5)]);
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let water = inputs.palette().find(PaletteRole::Ice, key("H2O")).unwrap();
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("H2O")]);
    let trap = trap_of(&diagnostics.cold_traps, "H2O");
    let total: f64 = field.solid_angles_sr().iter().sum();
    let (mut drawn, mut coded, mut cells) = (0.0, 0.0, 0_u32);
    for (((n, cell), &omega), &f) in working
        .cells
        .iter()
        .enumerate()
        .zip(field.solid_angles_sr())
        .zip(&fractions)
    {
        if cell.ice_entry != Some(water) || cell.ice_fraction <= 0.0 {
            continue;
        }
        let code = ICE_CODES * cell.ice_fraction;
        assert_same_bits(code, code.round());
        assert!(
            code <= (ICE_CODES * f).floor(),
            "cell {n}: code {code} above ⌊255 f⌋ of its fraction {f}"
        );
        let byte = SynthesisCell::quantise_ice(cell.ice_fraction).unwrap();
        assert_same_bits(f64::from(byte), code);
        drawn += omega * cell.ice_fraction;
        coded += omega * f64::from(byte) / ICE_CODES;
        cells += 1;
    }
    let one = one_cell(&field);
    eprintln!(
        "the Moon's codes: {cells} cells; drawn {:.6e}, through the ice byte {:.6e}, of {:.6e} \
         (placed {:.6e}); one cell {one:.3e}",
        drawn / total,
        coded / total,
        trap.target,
        trap.placed
    );
    assert!(cells > 0);
    assert_eq!(cells, trap.cells);
    assert!(diagnostics.ices.iter().all(|ice| ice.cells == 0));
    assert!((drawn / total - trap.placed).abs() <= one, "{trap:?}");
    assert!((coded / total - trap.target).abs() <= one, "{trap:?}");
    assert!(
        (coded / total - trap.target).abs() <= ACCOUNTED_SHARE * trap.target,
        "{} against {trap:?}",
        coded / total
    );
    assert_accounted(trap);
}

/// Plan 09's second: at 25° obliquity no cell has a water cold-trap fraction, so a trapped share
/// given by hand is short in full and placed nowhere.
#[test]
fn at_25_degrees_obliquity_no_cell_has_a_fraction_and_the_trap_is_short_in_full() {
    let inputs = moon_under(
        LUNAR_PERIAPSIS_FLUX_W_M2,
        25.0,
        &[("H2O", 1e-4)],
        &[("H2O", 1e-4)],
    );
    let (field, mut cells, keys) = polar_keys(&inputs, 6);
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("H2O")]);
    assert!(fractions.iter().all(|&f| f <= 0.0));
    let placed = place_ices(
        &inputs,
        field.solid_angles_sr(),
        field.centres(),
        &keys,
        &mut cells,
    );
    let trap = trap_of(&placed.traps, "H2O");
    eprintln!("25°: {trap:?}");
    assert_same_bits(trap.scale, 1.0);
    assert_same_bits(trap.short, 1e-4);
    assert_same_bits(trap.placed, 0.0);
    assert_same_bits(trap.undrawn, 0.0);
    assert_eq!(trap.cells, 0);
    assert!(
        cells
            .iter()
            .all(|cell| cell.ice_entry.is_none() && cell.ice_fraction <= 0.0)
    );
}

/// Plan 09's third: on a body whose zonal annual maximum lies below water's threshold everywhere
/// (the Moon at 40 au), water's fraction is zero in every cell, and a water ice with no
/// cold-trapped part, plan 14's zonal sheet, is placed by T13.d's own rule: whole cells coldest
/// first, to its area within one cell.
#[test]
fn a_water_ice_under_its_zonal_cap_is_placed_by_t13d_s_own_rule() {
    let inputs = moon_under(
        1361.0 / (40.0 * 40.0),
        LUNAR_DECLINATION_DEG,
        &[("H2O", 0.3)],
        &[],
    );
    assert_eq!(inputs.cold_traps(), []);
    let ground = plan_14_ground(&inputs);
    let (field, mut cells, keys) = polar_keys(&inputs, 6);
    let hottest = field
        .centres()
        .iter()
        .map(|c| {
            ground
                .annual_maximum(Radians::new(math::asin(c[2])))
                .value()
        })
        .fold(0.0, f64::max);
    eprintln!(
        "40 au: zonal annual maximum at most {hottest:.2} K, water's threshold {:?}",
        threshold("H2O")
    );
    assert!(hottest < threshold("H2O").value());
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("H2O")]);
    assert!(fractions.iter().all(|&f| f <= 0.0));
    let placed = place_ices(
        &inputs,
        field.solid_angles_sr(),
        field.centres(),
        &keys,
        &mut cells,
    );
    assert_eq!(placed.traps, []);
    let water = inputs.palette().find(PaletteRole::Ice, key("H2O")).unwrap();
    assert!((area_of(&field, &cells, water) - 0.3).abs() <= one_cell(&field));
    let iced: Vec<usize> = (0..cells.len())
        .filter(|&i| cells[i].ice_fraction > 0.0)
        .collect();
    let partial = iced
        .iter()
        .filter(|&&i| cells[i].ice_fraction < 1.0)
        .count();
    assert!(partial <= 1, "{partial} partial cells");
    let warmest_iced = iced
        .iter()
        .map(|&i| keys[i].radiative_mean_k)
        .fold(f64::NEG_INFINITY, f64::max);
    let coldest_free = (0..cells.len())
        .filter(|&i| cells[i].ice_fraction <= 0.0)
        .map(|i| keys[i].radiative_mean_k)
        .fold(f64::INFINITY, f64::min);
    assert!(
        warmest_iced <= coldest_free,
        "{warmest_iced} {coldest_free}"
    );
}

/// The ruling's agreement test: on the Moon-like input, the field's mean cold-trap fraction,
/// Σ Ω f ÷ Σ Ω over its cells at the reference Moon's level 6, lies within 5% of plan 14's `f_ct`
/// for the same parts, for water and for carbon dioxide (decision-r09-t13f-cold-traps §7.3).
#[test]
fn the_field_s_cold_traps_agree_with_plan_14_s_within_5_percent() {
    let inputs = moon_with(&[], &[]);
    let ground = plan_14_ground(&inputs);
    let (field, _) = grids(6);
    let thresholds = [threshold("H2O"), threshold("CO2")];
    let fractions = trap_fractions(&inputs, field.centres(), &thresholds);
    let total: f64 = field.solid_angles_sr().iter().sum();
    for (s, (k, t)) in ["H2O", "CO2"].into_iter().zip(thresholds).enumerate() {
        let field_mean = fractions
            .as_chunks::<2>()
            .0
            .iter()
            .zip(field.solid_angles_sr())
            .fold(0.0, |sum, (row, &omega)| sum + omega * row[s])
            / total;
        let plan_14 = plan_14_fraction(&ground, t);
        let ratio = field_mean / plan_14;
        eprintln!(
            "{k} at {:.3} K: the field's Σ Ω f ÷ Σ Ω {field_mean:.6e}, plan 14's f_ct \
             {plan_14:.6e}, ratio {ratio:.5}",
            t.value()
        );
        assert!((ratio - 1.0).abs() < 0.05, "{k}: {ratio}");
    }
}

/// The split: a body given 0.02 of water ice, 10⁻⁵ of it cold-trapped, lays its zonal part
/// 0.02 − 10⁻⁵ on T13.d's coldest cells, and its cold traps in proportion to the fraction outside
/// them, and its whole drawn ice matches 0.02 to one cell.
#[test]
fn a_split_share_lays_its_zonal_part_coldest_first_and_its_traps_by_their_fraction() {
    let inputs = moon_with(&[("H2O", 0.02)], &[("H2O", 1e-5)]);
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let water = inputs.palette().find(PaletteRole::Ice, key("H2O")).unwrap();
    // The zonal part alone, on the same keys: the sheet.
    let zonal_only = builder_of(&inputs)
        .ices([share("H2O", 0.02 - 1e-5)])
        .cold_traps([])
        .build()
        .unwrap();
    let pass_grid = grids(6);
    let pass = Pass::new(reference::SEED, &zonal_only, &pass_grid.0, &pass_grid.1);
    let mut sheet = Working::initial(&pass).cells;
    let alone = place_ices(
        &zonal_only,
        field.solid_angles_sr(),
        field.centres(),
        &diagnostics.keys,
        &mut sheet,
    );
    let zonal = &diagnostics.ices[0];
    assert_same_bits(zonal.target, 0.02 - 1e-5);
    assert_eq!(
        alone.zonal[0], *zonal,
        "the zonal part is T13.d's on its own"
    );
    assert!((zonal.placed - zonal.target).abs() <= one_cell(&field));
    let trap = trap_of(&diagnostics.cold_traps, "H2O");
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("H2O")]);
    let (mut outside, mut outside_fraction) = (0.0, 0.0);
    let total: f64 = field.solid_angles_sr().iter().sum();
    for (i, cell) in working.cells.iter().enumerate() {
        let omega = field.solid_angles_sr()[i];
        let f = fractions[i];
        if sheet[i].ice_entry == Some(water) {
            // The sheet's cells keep their zonal part, with the traps' share added to at most 1.
            assert_eq!(cell.ice_entry, Some(water));
            assert!(cell.ice_fraction >= sheet[i].ice_fraction);
            continue;
        }
        outside_fraction += omega * f;
        if cell.ice_entry == Some(water) && cell.ice_fraction > 0.0 {
            assert!(f > 0.0, "a trap where the fraction is 0");
            assert!(cell.ice_fraction <= f + 1e-12);
            outside += omega * cell.ice_fraction;
        }
    }
    let expected = trap.scale * outside_fraction / total;
    let drawn = area_of(&field, &working.cells, water);
    eprintln!(
        "split: zonal {zonal:?}; {trap:?}; traps outside the sheet {:.4e} against k Σ Ω f \
         outside {expected:.4e}; drawn total {drawn:.6}",
        outside / total
    );
    assert!((outside / total - expected).abs() <= one_cell(&field));
    assert!((drawn - 0.02).abs() <= one_cell(&field), "{drawn}");
    assert!(
        (trap.placed + trap.undrawn + trap.short - trap.target).abs() <= one_cell(&field),
        "{trap:?}"
    );
    assert_accounted(trap);
}

/// The nesting: the Moon-like input with water's traps and carbon dioxide's, at one exposed
/// fraction (each its plan 14 `f_ct` times water's scale): each cell names one ice, the trapped
/// ice of larger share; water's drawn area matches its target to one cell, and carbon dioxide's
/// drawn plus undrawn matches its own.
#[test]
fn nested_traps_draw_the_ice_of_larger_share_and_count_the_other_as_undrawn() {
    let probe = moon_with(&[], &[]);
    let ground = plan_14_ground(&probe);
    let (water_ct, co2_ct) = (
        plan_14_fraction(&ground, threshold("H2O")),
        plan_14_fraction(&ground, threshold("CO2")),
    );
    let water_area = 1.7e-5;
    let co2_area = water_area * co2_ct / water_ct;
    let inputs = moon_with(
        &[("H2O", water_area), ("CO2", co2_area)],
        &[("H2O", water_area), ("CO2", co2_area)],
    );
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let palette = inputs.palette();
    let (water, co2) = (
        palette.find(PaletteRole::Ice, key("H2O")).unwrap(),
        palette.find(PaletteRole::Ice, key("CO2")).unwrap(),
    );
    let order: Vec<&str> = diagnostics
        .cold_traps
        .iter()
        .map(|t| t.substance.as_str())
        .collect();
    assert_eq!(order, ["CO2", "H2O"], "the most volatile first");
    let (water_trap, co2_trap) = (
        trap_of(&diagnostics.cold_traps, "H2O"),
        trap_of(&diagnostics.cold_traps, "CO2"),
    );
    let fractions = trap_fractions(
        &inputs,
        field.centres(),
        &[threshold("CO2"), threshold("H2O")],
    );
    for (i, cell) in working.cells.iter().enumerate() {
        let (f_co2, f_water) = (fractions[2 * i], fractions[2 * i + 1]);
        assert!(
            f_co2 <= f_water + 1e-12,
            "the bowls nest: {f_co2} > {f_water}"
        );
        match cell.ice_entry {
            // Ties go to the more volatile, the first in the ices' order.
            Some(entry) if entry == co2 => {
                assert!(co2_trap.scale * f_co2 >= water_trap.scale * f_water);
            }
            Some(entry) => assert_eq!(entry, water),
            None => {}
        }
    }
    let one = one_cell(&field);
    eprintln!(
        "nesting: water {water_trap:?}; CO₂ {co2_trap:?}; plan 14's f_ct water {water_ct:.6e}, \
         CO₂ {co2_ct:.6e}; one cell {one:.3e}"
    );
    assert!((area_of(&field, &working.cells, water) - water_area).abs() <= one);
    assert!((water_trap.placed - water_area).abs() <= one);
    assert!((co2_trap.placed + co2_trap.undrawn - co2_area).abs() <= one);
    assert_accounted(water_trap);
    assert_accounted(co2_trap);
    assert!(
        (area_of(&field, &working.cells, co2) - co2_trap.placed).abs() <= 1e-15,
        "{co2_trap:?}"
    );
}

/// Covered: a cold body (the Moon at 5.2 au) whose water lies zonally, plan 14's sheet with no
/// cold-trapped part, and whose carbon dioxide is all cold-trapped: its shares on the sheet's
/// cells are undrawn, and every cell that draws it has a fraction of it at least its share.
#[test]
fn traps_on_another_ice_s_sheet_are_undrawn() {
    let au = 5.2;
    let probe = moon_under(1361.0 / (au * au), LUNAR_DECLINATION_DEG, &[], &[]);
    let co2_ct = plan_14_fraction(&plan_14_ground(&probe), threshold("CO2"));
    let co2_area = 0.05 * co2_ct;
    let inputs = moon_under(
        1361.0 / (au * au),
        LUNAR_DECLINATION_DEG,
        &[("H2O", 0.01), ("CO2", co2_area)],
        &[("CO2", co2_area)],
    );
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let palette = inputs.palette();
    let (water, co2) = (
        palette.find(PaletteRole::Ice, key("H2O")).unwrap(),
        palette.find(PaletteRole::Ice, key("CO2")).unwrap(),
    );
    let fractions = trap_fractions(&inputs, field.centres(), &[threshold("CO2")]);
    let trap = trap_of(&diagnostics.cold_traps, "CO2");
    let total: f64 = field.solid_angles_sr().iter().sum();
    let mut on_the_sheet = 0.0;
    for (i, cell) in working.cells.iter().enumerate() {
        let w = trap.scale * fractions[i];
        if cell.ice_entry == Some(water) {
            on_the_sheet += field.solid_angles_sr()[i] * w;
        }
        if cell.ice_entry == Some(co2) {
            assert!(fractions[i] > 0.0, "carbon dioxide where its fraction is 0");
            assert!(cell.ice_fraction <= fractions[i] + 1e-12);
        }
    }
    let one = one_cell(&field);
    eprintln!(
        "covered at {au} au: water {:?}; CO₂ {trap:?}; its shares on the sheet {:.4e}",
        diagnostics.ices,
        on_the_sheet / total
    );
    assert!(on_the_sheet > 0.0);
    assert!((trap.undrawn - on_the_sheet / total).abs() <= 1e-12 * trap.target);
    assert!(
        (trap.placed + trap.undrawn - co2_area).abs() <= one,
        "{trap:?}"
    );
}

/// The re-application: `place_ices` run again on the step's cells with its own keys, as R09.T14.d
/// will, leaves every cell's ice to the bit and places the same parts.
#[test]
fn a_second_placement_leaves_every_cell_s_ice_to_the_bit() {
    let inputs = moon_with(
        &[("H2O", 0.02), ("CO2", 3.5e-7)],
        &[("H2O", 1e-5), ("CO2", 3.5e-7)],
    );
    let (field, working, diagnostics) = run_at(&inputs, 6);
    let mut cells = working.cells.clone();
    let again = place_ices(
        &inputs,
        field.solid_angles_sr(),
        field.centres(),
        &diagnostics.keys,
        &mut cells,
    );
    assert_eq!(again.zonal, diagnostics.ices);
    assert_eq!(again.traps, diagnostics.cold_traps);
    for (n, (a, b)) in working.cells.iter().zip(&cells).enumerate() {
        assert_eq!(a.ice_entry, b.ice_entry, "cell {n}");
        assert_same_bits(a.ice_fraction, b.ice_fraction);
    }
}

/// The symmetry: a cell and its mirror across the equator take equal fractions, water's and
/// carbon dioxide's, since the ground reads a southern latitude's northern mirror.
#[test]
fn a_cell_and_its_mirror_across_the_equator_take_equal_fractions() {
    let inputs = moon_with(&[], &[]);
    let (field, _) = grids(6);
    let thresholds = [threshold("CO2"), threshold("H2O")];
    let fractions = trap_fractions(&inputs, field.centres(), &thresholds);
    let (mut pairs, mut worst) = (0_u32, 0.0_f64);
    for (i, centre) in field.centres().iter().enumerate() {
        if centre[2].abs() < 1e-9 {
            continue;
        }
        let mirror =
            PatchKey::containing(field.level(), [centre[0], centre[1], -centre[2]]).unwrap();
        let j = usize::try_from(cell_index(mirror)).unwrap();
        assert_ne!(i, j);
        let mirrored = field.centres()[j];
        assert!(
            (mirrored[2] + centre[2]).abs() < 1e-12,
            "{centre:?} {mirrored:?}"
        );
        for s in 0..thresholds.len() {
            let (a, b) = (fractions[2 * i + s], fractions[2 * j + s]);
            worst = worst.max((a - b).abs());
            assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()), "{a} {b}");
        }
        pairs += 1;
    }
    eprintln!("{pairs} cells against their mirrors, the largest difference {worst:e}");
    assert!(pairs > 0);
}
