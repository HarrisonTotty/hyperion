//! The classifier's tests (plan R10, T1.b): the order of its rules, and its rules on the synthetic
//! worlds and on fields built for each (R09's `FieldBuilder`).
//!
//! The fields' substances are illustrative test inputs, each near its real value as noted, not the
//! registry's (P14.T49): what the tests check is which rule decides, and which entry it names.

use std::collections::BTreeSet;

use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals, PerSquareKilometre};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::craters::{
    CraterParams, CraterParamsParts, LUNAR_IMPACT_VELOCITY, LUNAR_PROJECTILE_DENSITY, LUNAR_TARGET,
    SOLAR_SYSTEM_AGE, Screening,
};
use crate::cube::{Face, xyz_to_face_uv};
use crate::field::{CoarseField, MaterialPalette, PaletteEntry, cell_at_index};
use crate::material::PhaseLaw;
use crate::substance_key::SubstanceKey;
use crate::synth::test_support::{Held, SEED, king_distances, random_points_in, world};
use crate::synth::{
    BandSpectrum, STRUCTURAL_RMS_ICE_RICH, STRUCTURAL_RMS_STAGNANT_LID, SpectrumShape,
};
use crate::test_sizes::per_target;
use crate::testing::{CellSite, ClimateSample, FieldBuilder, Routing, SyntheticWorld};

/// A palette entry of `key` in `role`, illustrative values: a normal albedo of 0.5 in every band,
/// a grain density of `density`, a transition of `transition_k`, the family `mechanics` and no
/// liquid record.
fn entry(
    key: &str,
    role: PaletteRole,
    density: f64,
    transition_k: f64,
    mechanics: MechanicsFamily,
) -> PaletteEntry {
    PaletteEntry {
        substance: SubstanceKey::new(key).expect("a test key is in the grammar"),
        role,
        normal_albedo_bvr: [0.5; 3],
        phase_row: 0,
        grain_density_kg_m3: density,
        transition: Kelvin::new(transition_k),
        mechanics,
        liquid: None,
    }
}

/// A basalt crust's entry in `role`: a grain density of 3,010 kg m⁻³ and a solidus of 1,253 K, as
/// the synthetic worlds' basalt's.
fn basalt(role: PaletteRole) -> PaletteEntry {
    entry("basalt", role, 3_010.0, 1_253.15, MechanicsFamily::Silicate)
}

/// Water ice in `role`, ice Ih's 917 kg m⁻³, melting at 273.15 K; or, as a
/// [`PaletteRole::Liquid`], liquid water with the synthetic water's record and its density
/// ([`crate::testing::liquid_water`]).
fn water(role: PaletteRole) -> PaletteEntry {
    if role == PaletteRole::Liquid {
        let record = crate::testing::liquid_water();
        PaletteEntry {
            liquid: Some(record.clone()),
            ..entry(
                "H2O",
                role,
                record.density_kg_m3,
                273.15,
                MechanicsFamily::WaterIce,
            )
        }
    } else {
        entry("H2O", role, 917.0, 273.15, MechanicsFamily::WaterIce)
    }
}

fn palette(entries: Vec<PaletteEntry>) -> MaterialPalette {
    MaterialPalette::new(entries).expect("a test palette is sorted and valid")
}

/// The latitude of the unit direction `dir`, radians.
fn latitude(dir: [f64; 3]) -> f64 {
    math::asin(dir[2].clamp(-1.0, 1.0))
}

/// The unit direction at latitude `lat` and longitude `lon`, degrees.
fn lat_lon(lat: f64, lon: f64) -> [f64; 3] {
    let (sl, cl) = math::sin_cos(lat.to_radians());
    let (so, co) = math::sin_cos(lon.to_radians());
    [cl * co, cl * so, sl]
}

/// The angle between two unit directions, radians.
fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
    math::acos(dot(a, b).clamp(-1.0, 1.0))
}

/// The classifier's answer at the unit direction `dir`, from a fresh cache.
fn ground(synth: &Synthesiser<'_, CoarseField>, dir: [f64; 3]) -> GroundMaterial {
    material_at(synth, &mut SynthCache::new(), xyz_to_face_uv(dir)).expect("a whole field")
}

/// The material at the unit direction `dir`, from a fresh cache: `None` where no entry is named.
fn material(synth: &Synthesiser<'_, CoarseField>, dir: [f64; 3]) -> Option<Material> {
    ground(synth, dir).material()
}

/// The facts at the unit direction `dir`.
fn facts(synth: &Synthesiser<'_, CoarseField>, dir: [f64; 3]) -> Facts {
    Facts::at(synth, &mut SynthCache::new(), dir).expect("a whole field")
}

/// The material of form `class` and entry `entry`.
fn of(class: MaterialClass, entry: u8) -> Option<Material> {
    Material::new(class, entry)
}

/// The slope at `dir`, radians: the angle from the datum's normal of the plane through the surface
/// points at the finest heights 0.25 m of datum either side along two tangents, apart from the
/// classifier's own reading of the gradient. On a body as small as a cliff field's the surface at
/// height h stretches the datum's metre by (R + h) ÷ R, which the surface points carry.
fn slope_by_differences(synth: &Synthesiser<'_, CoarseField>, dir: [f64; 3]) -> f64 {
    let figure = synth.field().header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).expect("a band level");
    let mut cache = SynthCache::new();
    let nu = figure.normal(dir);
    let seed = if nu[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let along = dot(seed, nu);
    let e1 = unit_dir([0, 1, 2].map(|k| seed[k] - along * nu[k]));
    let e2 = [
        nu[1] * e1[2] - nu[2] * e1[1],
        nu[2] * e1[0] - nu[0] * e1[2],
        nu[0] * e1[1] - nu[1] * e1[0],
    ];
    let step_m = 0.25;
    let mut chords = [[0.0; 3]; 2];
    for (chord, e) in chords.iter_mut().zip([e1, e2]) {
        let mut ends = [[0.0; 3]; 2];
        for (end, sign) in ends.iter_mut().zip([1.0, -1.0]) {
            let p = figure.point(dir);
            let moved = unit_dir([0, 1, 2].map(|k| p[k] + sign * step_m * e[k]));
            let h = synth
                .height_at(&mut cache, moved, level)
                .expect("a whole field")
                .height_m;
            *end = figure.surface_point(moved, h);
        }
        *chord = [0, 1, 2].map(|k| ends[0][k] - ends[1][k]);
    }
    let [a, b] = chords;
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    math::acos((dot(n, nu) / dot(n, n).sqrt()).abs().min(1.0))
}

// ---------------------------------------------------------------------------------------------
// The order of the rules.
// ---------------------------------------------------------------------------------------------

/// Every combination of the facts' conditions, with every entry present or absent where a rule's
/// condition reads it.
fn every_combination() -> Vec<Facts> {
    let mut out = Vec::new();
    for bits in 0_u32..(1 << 15) {
        let bit = |k: u32| bits & (1 << k) != 0;
        let ice_kind = bits >> 12 & 0b11;
        if ice_kind == 3 {
            continue;
        }
        let facts = Facts {
            under_water: bit(0),
            ice: match ice_kind {
                0 => None,
                1 => Some(Ice {
                    entry: Some(4),
                    massive: false,
                }),
                _ => Some(Ice {
                    entry: Some(4),
                    massive: true,
                }),
            },
            airless: bit(1),
            thin_aired: bit(2) && !bit(1),
            molten: bit(3),
            snow: bit(4).then_some(5),
            above_cliff: bit(5) || bit(6),
            above_snow_shed: bit(6),
            closed_basin: bit(7),
            arid: bit(8),
            channel_floor: bit(9),
            province: bit(10),
            entries: Entries {
                crust: Some(0),
                floor: Some(1),
                salt: bit(11).then_some(2),
                regolith: Some(3),
                loose: Some(6),
                organic_cover: bit(14).then_some(7),
            },
        };
        out.push(facts);
    }
    out
}

/// The order of rules is total: on every combination of conditions exactly one rule decides, the
/// first whose condition holds, the last holding everywhere; every rule decides somewhere, so that
/// no rule's place leaves it unreachable; and a rule's material is its own form of the entry it
/// names (Design note 1).
#[test]
fn material_the_order_of_rules_is_total() {
    let mut decided = BTreeSet::new();
    for facts in every_combination() {
        let (rule, ground) = facts.decide();
        let first = Rule::ALL
            .iter()
            .position(|r| r.holds(&facts))
            .expect("some rule holds");
        assert_eq!(rule, Rule::ALL[first], "{facts:?}");
        assert!(Rule::Soil.holds(&facts), "the last rule holds everywhere");
        assert!(ground.entry().is_some(), "every entry is present");
        let want = match rule {
            Rule::SeaIce => MaterialClass::SeaIce,
            Rule::Seabed => MaterialClass::Seabed,
            Rule::LandIce => match facts.ice.map(|i| (i.massive, facts.airless)) {
                Some((false, _)) => MaterialClass::Frost,
                Some((true, false)) => MaterialClass::Glacier,
                Some((true, true)) => MaterialClass::Regolith,
                None => unreachable!("land ice holds only on ice"),
            },
            Rule::Melt => MaterialClass::Melt,
            Rule::Snow => MaterialClass::Snow,
            Rule::Cliff => MaterialClass::Bedrock,
            Rule::Evaporite => MaterialClass::Evaporite,
            Rule::Sediment => MaterialClass::Sediment,
            Rule::Regolith => MaterialClass::Regolith,
            Rule::OrganicSediment => MaterialClass::OrganicSediment,
            Rule::Dust => MaterialClass::Dust,
            Rule::VolcanicRock => MaterialClass::VolcanicRock,
            Rule::Sand => MaterialClass::Sand,
            Rule::Soil => MaterialClass::Soil,
        };
        assert_eq!(ground.class(), want, "{rule:?} on {facts:?}");
        decided.insert(rule);
    }
    assert_eq!(
        decided.into_iter().collect::<Vec<_>>(),
        Rule::ALL.to_vec(),
        "every rule decides somewhere"
    );
}

/// A rule whose substance the palette does not name still gives its form, with no entry, never a
/// stand-in (signoff-9 §17.5); and gardened ice on an airless body is never `Glacier`
/// (decision-r10-limb.md §2).
#[test]
fn material_a_missing_entry_is_no_material_and_airless_ice_is_never_a_glacier() {
    let bare = Facts {
        entries: Entries::default(),
        ..Facts::default()
    };
    let (rule, bare_ground) = bare.decide();
    assert_eq!(rule, Rule::Soil);
    assert_eq!(
        (bare_ground.class(), bare_ground.entry()),
        (MaterialClass::Soil, None)
    );
    assert_eq!(bare_ground.material(), None);
    for facts in every_combination() {
        let (rule, ground) = facts.decide();
        if facts.airless && rule == Rule::LandIce {
            assert_ne!(ground.class(), MaterialClass::Glacier, "{facts:?}");
        }
    }
}

/// The cut's quantile interpolates between order statistics (Hyndman and Fan's definition 7): the
/// ends are the extremes, a half of an even count the mean of the middle two, and the smallest
/// share a cell can hold, 1 ÷ 255, still falls below the grid's largest value.
#[test]
fn material_the_cut_is_the_share_s_quantile() {
    let sorted: Vec<f64> = (0..256).map(f64::from).collect();
    assert_eq!(bits(quantile(&sorted, 0.0)), bits(0.0));
    assert_eq!(bits(quantile(&sorted, 1.0)), bits(255.0));
    assert!((quantile(&sorted, 0.5) - 127.5).abs() < 1e-12);
    assert!(quantile(&sorted, 1.0 - 1.0 / 255.0) < 255.0);
    let cut = IceCut {
        tilt_scale: 0.0,
        depth_scale_per_m: 0.0,
        cut: None,
    };
    let flat = ShadowTerms {
        tilt: 0.0,
        depth_m: 0.0,
    };
    assert!(
        cut.holds(flat, 0.6),
        "a cell with nothing to rank is ice where most is"
    );
    assert!(!cut.holds(flat, 0.4));
}

// ---------------------------------------------------------------------------------------------
// The synthetic worlds.
// ---------------------------------------------------------------------------------------------

/// On every synthetic world each material's entry is in its palette, a body with an empty palette
/// has none, and every answer is the same bits from a fresh cache and a shared one, in any order;
/// and no palette source writes the Lambert code, 3, which only a form chooses (main's ruling on
/// R10.T1.a's follow-up: a source writes 0 to 2).
#[test]
fn material_on_the_synthetic_worlds_names_its_palette_and_is_order_independent() {
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let palette = field.header().palette();
        for e in palette.entries() {
            assert!(
                e.phase_row != u8::from(PhaseLaw::Lambert),
                "{w:?}: {e:?} names the Lambert law"
            );
        }
        let mut rng = Lcg::new(0x6d61_7465_7269_616c);
        let dirs: Vec<[f64; 3]> = (0..40)
            .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
            .collect();
        let mut shared = SynthCache::new();
        let forward: Vec<GroundMaterial> = dirs
            .iter()
            .map(|&d| material_at(&synth, &mut shared, xyz_to_face_uv(d)).expect("whole"))
            .collect();
        let mut backward_cache = SynthCache::new();
        for (k, &d) in dirs.iter().enumerate().rev() {
            let again = material_at(&synth, &mut backward_cache, xyz_to_face_uv(d)).expect("whole");
            assert_eq!(again, forward[k], "{w:?} at {d:?}, backwards");
            assert_eq!(ground(&synth, d), forward[k], "{w:?} at {d:?}, fresh");
        }
        for g in &forward {
            match g.entry() {
                Some(e) => assert!(palette.get(e).is_some(), "{w:?}: {g:?} past the palette"),
                None => assert!(palette.is_empty(), "{w:?}: no entry on a modelled body"),
            }
        }
    }
}

/// A view that lacks a cell the classifier reads refuses it as not surveyed.
#[test]
fn material_a_cell_not_held_is_not_surveyed() {
    let field = world(SyntheticWorld::MoonLike);
    let level = field.header().level().get();
    let cell = cell_at_index(level, 1_000).expect("a cell of the level");
    let view = Held {
        inner: field,
        held: BTreeSet::from([cell]),
    };
    let synth = Synthesiser::new(&view, SEED);
    let dir = cell.vertex_dir(32, 32);
    let result = material_at(&synth, &mut SynthCache::new(), xyz_to_face_uv(dir));
    assert!(
        matches!(result, Err(QueryHeightError::NotSurveyed(_))),
        "{result:?}"
    );
}

/// `Seabed` lies exactly below the water surface: a point of a cell under water whose ground is
/// below the cell's water surface is seabed (or sea ice), of the floor's entry and never the
/// liquid's, and one at or above it is not (signoff-9).
#[test]
fn material_seabed_lies_exactly_below_the_water_surface() {
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let header = field.header();
    let figure = header.figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).expect("a band level");
    let liquid = header
        .main_liquid()
        .expect("the Earth-like world has a sea");
    let coarse = header.level().get();
    let mut rng = Lcg::new(0x7365_6162_6564_0001);
    let (mut below, mut above) = (0, 0);
    // Shallow sea cells, whose ground's fine relief reaches the surface in places.
    let mut sea: Vec<(i32, PatchKey)> = (0..field.synthesis().len())
        .filter_map(|n| {
            let key = cell_at_index(coarse, u32::try_from(n).expect("a cell index"))?;
            let cell = field.cell(key)?;
            cell.is_under_water()
                .then_some((cell.water_surface_mm - cell.elevation_mm, key))
        })
        .collect();
    sea.sort_unstable();
    let shallow: Vec<PatchKey> = sea.into_iter().take(24).map(|(_, key)| key).collect();
    assert!(!shallow.is_empty(), "the Earth-like world has shallow seas");
    for key in shallow {
        let cell = field.cell(key).expect("held");
        for dir in random_points_in(key, 12, &mut rng) {
            let h = synth
                .height_at(&mut SynthCache::new(), dir, level)
                .expect("whole")
                .height_m;
            let m = material(&synth, dir).expect("the Earth-like palette names its floor");
            let surface = cell.water_surface().value();
            if h < surface {
                below += 1;
                assert!(
                    matches!(m.class(), MaterialClass::Seabed | MaterialClass::SeaIce),
                    "{m:?} {} m below the surface",
                    surface - h
                );
                if m.class() == MaterialClass::Seabed {
                    assert_eq!(
                        Some(m.entry()),
                        header.crust_palette()[usize::from(u8::from(cell.crust))]
                    );
                    assert_ne!(m.entry(), liquid, "the seabed is never its liquid");
                }
            } else {
                above += 1;
                assert_ne!(
                    m.class(),
                    MaterialClass::Seabed,
                    "{h} m at or above {surface} m"
                );
            }
        }
    }
    assert!(below > 0, "no point below the surface");
    assert!(above > 0, "no point above the surface in a sea cell");
}

/// A channel's bed is `Sediment` where no earlier rule holds: at the head of trunks in dry,
/// ice-free cells of the routed worlds the point is on a channel's floor.
#[test]
fn material_a_channel_s_bed_is_sediment() {
    for w in [SyntheticWorld::EarthLike, SyntheticWorld::MarsLike] {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut network = synth.channels().network(field);
        let mut seen = 0;
        for n in 0..field.synthesis().len() {
            let key = cell_at_index(coarse, u32::try_from(n).expect("a cell index"))
                .expect("a cell of the level");
            let cell = field.cell(key).expect("held");
            if cell.is_under_water() || cell.ice > 0 {
                continue;
            }
            if network.segment(key).expect("whole").is_none() {
                continue;
            }
            // The trunk's head, its key point's direction, where its chord meets the spheroid:
            // along the chord the sag below the surface may exceed a narrow floor (R09.T6.a's
            // note). The spheroid point's own direction is not the key point's on an oblate body.
            let dir = network.key_point(key).dir();
            let f = facts(&synth, dir);
            if f.under_water {
                continue;
            }
            assert!(
                f.channel_floor,
                "{w:?}: a trunk's axis off its floor at {dir:?}"
            );
            let (rule, m) = f.decide();
            if !(f.molten || f.snow.is_some() || f.above_cliff || f.closed_basin) {
                assert_eq!(rule, Rule::Sediment, "{w:?} at {dir:?}");
                assert_eq!(m.class(), MaterialClass::Sediment);
            }
            seen += 1;
            if seen == 12 {
                break;
            }
        }
        assert!(seen > 0, "{w:?} has trunks on dry land");
    }
}

// ---------------------------------------------------------------------------------------------
// Built fields.
// ---------------------------------------------------------------------------------------------

/// A cliff field: an airless body of radius `radius_m` metres, its ground `height_m` metres high on
/// the side of the great circle normal to `strike` that `strike` points to and at the datum on the
/// other, of a basalt lid, with `climate` everywhere and an atmosphere of `pressure`. The coarse
/// cells' spline blends the step over about two cells (R09's "Deviations in T4, as built"), so a
/// small body's cliff is steep.
fn cliff_field(
    radius_m: f64,
    height_m: f64,
    strike: [f64; 3],
    pressure: Pascals,
    climate: &ClimateSample,
    with_water: bool,
) -> CoarseField {
    let climate = *climate;
    let mut entries = vec![basalt(PaletteRole::SecondaryCrust)];
    if with_water {
        entries.push(water(PaletteRole::Ice));
        entries.push(water(PaletteRole::Liquid));
    }
    let mut builder = FieldBuilder::new(Metres::new(radius_m))
        .palette(palette(entries))
        .crust_palette([None, None, Some(0), None])
        .surface_pressure(pressure)
        .climate(move |_| climate)
        .elevation(move |site: &CellSite| {
            Metres::new(if dot(site.dir, strike) > 0.0 {
                height_m
            } else {
                0.0
            })
        });
    if with_water {
        builder = builder.main_liquid(2);
    }
    builder.build()
}

/// Points along the great circle through `strike` and `across`, both unit and perpendicular, from
/// −`half_width` to +`half_width` radians about the cliff, `count` of them.
fn across_the_cliff(
    strike: [f64; 3],
    across: [f64; 3],
    half_width: f64,
    count: u32,
) -> Vec<[f64; 3]> {
    (0..count)
        .map(|k| {
            let t = -half_width + 2.0 * half_width * f64::from(k) / f64::from(count - 1);
            let (s, c) = math::sin_cos(t);
            unit_dir([0, 1, 2].map(|i| c * across[i] + s * strike[i]))
        })
        .collect()
}

/// A `cliff_field` is `Bedrock` above 40° and not below, by a slope measured apart from the
/// classifier's own (Design note 1's `CLIFF_SLOPE`).
#[test]
fn material_a_cliff_field_is_bedrock_above_40_degrees_and_not_below() {
    let strike = [0.0, 1.0, 0.0];
    let across = [1.0, 0.0, 0.0];
    let field = cliff_field(
        2_000.0,
        1_000.0,
        strike,
        Pascals::ZERO,
        &ClimateSample::still(Kelvin::new(250.0)),
        false,
    );
    let synth = Synthesiser::new(&field, DetailSeed::new(7));
    let (mut steep, mut gentle) = (0, 0);
    for dir in across_the_cliff(strike, across, 0.2, 400) {
        let slope = slope_by_differences(&synth, dir).to_degrees();
        let m = material(&synth, dir).expect("the cliff names its crust");
        if slope > 40.5 {
            steep += 1;
            assert_eq!(
                m,
                of(MaterialClass::Bedrock, 0).expect("an entry"),
                "{slope}°"
            );
        } else if slope < 39.5 {
            gentle += 1;
            assert_ne!(m.class(), MaterialClass::Bedrock, "{slope}°");
            assert_eq!(
                m.class(),
                MaterialClass::Regolith,
                "an airless body's loose ground"
            );
        }
    }
    assert!(
        steep > 10 && gentle > 10,
        "{steep} steep and {gentle} gentle points"
    );
}

/// Snow lies all year where the warmest month is below the ice's transition and it snows, to
/// `SNOW_SHED_SLOPE`, above which the ground is rock: snow on a 45° slope is snow, and on 65° it is
/// bedrock.
#[test]
fn material_snow_lies_to_its_shed_slope() {
    let strike = [0.0, 1.0, 0.0];
    let across = [1.0, 0.0, 0.0];
    let mut climate = ClimateSample::still(Kelvin::new(250.0));
    climate.month_precipitation_kg_m2_s = [500.0 / SECONDS_PER_JULIAN_YEAR; 12];
    let field = cliff_field(
        2_000.0,
        1_000.0,
        strike,
        Pascals::new(101_325.0),
        &climate,
        true,
    );
    let synth = Synthesiser::new(&field, DetailSeed::new(7));
    let ice = field
        .header()
        .palette()
        .find(PaletteRole::Ice, SubstanceKey::new_const("H2O"));
    let (mut snowy_steep, mut shed) = (0, 0);
    for dir in across_the_cliff(strike, across, 0.2, 400) {
        let slope = slope_by_differences(&synth, dir).to_degrees();
        let m = material(&synth, dir).expect("named");
        if slope < 59.5 {
            assert_eq!(
                Some(m),
                ice.and_then(|e| of(MaterialClass::Snow, e)),
                "{slope}°"
            );
            if slope > 40.5 {
                snowy_steep += 1;
            }
        } else if slope > 60.5 {
            shed += 1;
            assert_eq!(
                m,
                of(MaterialClass::Bedrock, 0).expect("an entry"),
                "{slope}°"
            );
        }
    }
    assert!(
        snowy_steep > 0 && shed > 0,
        "{snowy_steep} snowy above 40°, {shed} shed"
    );
    // At 280 K at the datum, with the standard lapse rate, snow lies only above about 1 km.
    let mut warm = ClimateSample::still(Kelvin::new(280.0));
    warm.month_precipitation_kg_m2_s = [500.0 / SECONDS_PER_JULIAN_YEAR; 12];
    let warm_field = FieldBuilder::new(Metres::new(2_000.0))
        .palette(palette(vec![
            basalt(PaletteRole::SecondaryCrust),
            water(PaletteRole::Ice),
            water(PaletteRole::Liquid),
        ]))
        .crust_palette([None, None, Some(0), None])
        .main_liquid(2)
        .surface_pressure(Pascals::new(101_325.0))
        .lapse_rate(0.0065)
        .climate(move |_| warm)
        .elevation(move |site: &CellSite| {
            Metres::new(if dot(site.dir, strike) > 0.0 {
                2_000.0
            } else {
                0.0
            })
        })
        .build();
    let warm_synth = Synthesiser::new(&warm_field, DetailSeed::new(7));
    let figure = warm_field.header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).expect("a band level");
    let (mut high, mut low) = (0, 0);
    for dir in across_the_cliff(strike, across, 0.4, 200) {
        let h = warm_synth
            .height_at(&mut SynthCache::new(), dir, level)
            .expect("whole")
            .height_m;
        let snowy = material(&warm_synth, dir).map(Material::class) == Some(MaterialClass::Snow);
        let line_m = (280.0 - 273.15) / 0.0065;
        if h > line_m + 50.0 && slope_by_differences(&warm_synth, dir) < 0.6 {
            high += 1;
            assert!(snowy, "{h} m above the snow line");
        } else if h < line_m - 50.0 {
            low += 1;
            assert!(!snowy, "{h} m below the snow line");
        }
    }
    assert!(high > 0 && low > 0, "{high} high, {low} low");
}

/// A field with a closed basin about `centre`, on a body of radius 1,000 km whose ground is basalt
/// and, where `salt` says, whose palette has a salt deposit, under an atmosphere of 50 kPa at 300 K
/// with `precipitation_mm` a year, its water routed.
fn basin_field(salt: bool, precipitation_mm: f64, centre: [f64; 3]) -> CoarseField {
    let mut entries = vec![basalt(PaletteRole::SecondaryCrust)];
    if salt {
        // Halite: 2,165 kg m⁻³ and a melting point of 1,074 K, near the real ones.
        entries.push(entry(
            "NaCl",
            PaletteRole::Deposit,
            2_165.0,
            1_074.0,
            MechanicsFamily::Salt,
        ));
    }
    let mut climate = ClimateSample::still(Kelvin::new(300.0));
    climate.month_precipitation_kg_m2_s = [precipitation_mm / SECONDS_PER_JULIAN_YEAR; 12];
    FieldBuilder::new(Metres::new(1.0e6))
        .palette(palette(entries))
        .crust_palette([None, None, Some(0), None])
        .surface_pressure(Pascals::new(50_000.0))
        .climate(move |_| climate)
        .routing(Routing::SteepestDescent)
        .runoff(move |_| precipitation_mm * 1e-3 * 0.3)
        .elevation(move |site: &CellSite| {
            let x = arc(site.dir, centre) / 0.25;
            Metres::new(1_000.0 * (1.0 - math::exp(-x * x)))
        })
        .build()
}

/// The basin's floor: the dry terminal cell with the largest drainage area.
fn basin_floor(field: &CoarseField) -> PatchKey {
    let coarse = field.header().level().get();
    (0..field.synthesis().len())
        .filter_map(|n| {
            let key = cell_at_index(coarse, u32::try_from(n).expect("a cell index"))?;
            let cell = field.cell(key)?;
            (!cell.is_under_water()
                && cell.flow == FlowDirection::Terminal
                && cell.drainage > LogArea::ZERO)
                .then_some((cell.drainage, key))
        })
        .max()
        .map(|(_, key)| key)
        .expect("the bowl's floor is a closed basin")
}

/// An arid filled basin is `Evaporite`, of the palette's salt; a wet one, or an arid one on a body
/// with no salt, is `Sediment` of its crust (Design note 1).
#[test]
fn material_an_arid_filled_basin_is_evaporite() {
    let centre = lat_lon(20.0, 30.0);
    let cases = [
        (true, 0.0, MaterialClass::Evaporite, 1),
        (true, 2_000.0, MaterialClass::Sediment, 0),
        (false, 0.0, MaterialClass::Sediment, 0),
    ];
    for (salt, precipitation_mm, class, entry) in cases {
        let field = basin_field(salt, precipitation_mm, centre);
        let synth = Synthesiser::new(&field, DetailSeed::new(3));
        let floor = basin_floor(&field);
        let mut rng = Lcg::new(0x6261_7369_6e00_0001);
        for dir in random_points_in(floor, 16, &mut rng) {
            let f = facts(&synth, dir);
            assert!(f.closed_basin, "the floor's cell");
            assert_eq!(
                f.arid,
                precipitation_mm <= 0.0,
                "{precipitation_mm} mm a year"
            );
            assert_eq!(
                material(&synth, dir),
                of(class, entry),
                "salt {salt}, {precipitation_mm} mm"
            );
        }
    }
}

/// Köppen–Geiger's arid threshold (Peel, Finlayson and McMahon 2007, Table 1) at a 20 °C annual
/// mean with even seasons is 540 mm a year, ten times 2 × 20 + 14: 500 mm is arid and 600 mm is
/// not; and ground colder than −7 °C is arid only where nothing falls.
#[test]
fn material_aridity_is_koppen_geiger_s_threshold() {
    let centre = lat_lon(20.0, 30.0);
    for (mm, arid) in [(500.0, true), (600.0, false), (0.0, true)] {
        let field = basin_field(false, mm, centre);
        let header = field.header();
        let key = basin_floor(&field);
        let climate = field.climate(key).expect("held");
        let annual = annual_precipitation_mm(header, climate);
        assert!(
            (annual / mm.max(1.0) - 1.0).abs() < 0.03 || mm <= 0.0,
            "{annual} mm"
        );
        assert_eq!(is_arid(header, climate, 293.15, annual), arid, "{mm} mm");
    }
    let frozen = basin_field(false, 10.0, centre);
    let key = basin_floor(&frozen);
    let climate = frozen.climate(key).expect("held");
    let annual = annual_precipitation_mm(frozen.header(), climate);
    assert!(
        !is_arid(frozen.header(), climate, 200.0, annual),
        "frozen, 10 mm"
    );
    assert!(
        is_arid(frozen.header(), climate, 200.0, 0.0),
        "frozen, nothing falls"
    );
}

/// A Pluto-like world (decision-composition's): Pluto's radius, 1,188.3 km (Nimmo et al. 2017,
/// Icarus 287, 12), a water-ice crust, nitrogen and methane ices and a tholin cover, under 1 Pa of
/// air (New Horizons' 10.2–12.8 µbar at exit and entry; Hinson et al. 2017, Icarus 290, 96, as
/// Meza et al. 2019, A&A 625, A42, §4.3, quote it) at 40 K, with a nitrogen
/// glacier filling a basin about 25° N, 175° E (Sputnik Planitia's place) and methane frost over a
/// part of each mid-northern cell; `pressure` overrides the air.
fn pluto_like(pressure: Pascals) -> CoarseField {
    let radius = 1.1883e6;
    let basin = lat_lon(25.0, 175.0);
    // Illustrative values near the real ones: water ice at 40 K, 934 kg m⁻³ (IAPWS R10-06);
    // methane ice, 522 kg m⁻³, melting at 90.7 K; β-nitrogen, the phase above 35.6 K, about 990 kg
    // m⁻³, melting at 63.15 K (science-composition-crusts §3.1); a tholin of 1,500 kg m⁻³ that
    // decomposes near 600 K, unsourced.
    let pluto = palette(vec![
        entry(
            "H2O",
            PaletteRole::PrimaryCrust,
            934.0,
            273.15,
            MechanicsFamily::WaterIce,
        ),
        entry(
            "CH4",
            PaletteRole::Ice,
            522.0,
            90.7,
            MechanicsFamily::VolatileIce,
        ),
        entry(
            "N2",
            PaletteRole::Ice,
            990.0,
            63.15,
            MechanicsFamily::VolatileIce,
        ),
        entry(
            "tholin",
            PaletteRole::Cover,
            1_500.0,
            600.0,
            MechanicsFamily::Organic,
        ),
    ]);
    let in_basin = move |dir: [f64; 3]| arc(dir, basin) < 0.3;
    let in_band = |dir: [f64; 3]| {
        let lat = latitude(dir).to_degrees();
        (35.0..60.0).contains(&lat)
    };
    FieldBuilder::new(Metres::new(radius))
        .palette(pluto)
        .crust_palette([None, None, Some(0), None])
        .surface_pressure(pressure)
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::ICE_RICH,
            Metres::new(radius),
            STRUCTURAL_RMS_ICE_RICH,
        ))
        .climate(|_| ClimateSample::still(Kelvin::new(40.0)))
        .elevation(move |site: &CellSite| {
            Metres::new(if in_basin(site.dir) { -3_000.0 } else { 0.0 })
        })
        .ice(move |site, _| {
            if in_basin(site.dir) {
                1.0
            } else if in_band(site.dir) {
                0.3
            } else {
                0.0
            }
        })
        .ice_entry(move |site| if in_basin(site.dir) { 2 } else { 1 })
        .build()
}

/// The Pluto-like world takes its frost and its glacier: the basin's nitrogen is `Glacier · N2`
/// (signoff-9's `ICE · N2`), the mid-latitudes' methane is `Frost · CH4` on a part of each cell,
/// and the bare ground between is the tholin cover's dust. With no air, the same massive nitrogen
/// is a regolith of its ice, never a glacier (decision-r10-limb.md §2).
#[test]
fn material_the_pluto_like_world_takes_its_frost_and_its_glacier() {
    let field = pluto_like(Pascals::new(1.0));
    let synth = Synthesiser::new(&field, DetailSeed::new(11));
    let basin = lat_lon(25.0, 175.0);
    let mut rng = Lcg::new(0x706c_7574_6f00_0001);
    for _ in 0..24 {
        let a = rng.next_f64() * 0.2;
        let b = rng.next_f64() * core::f64::consts::TAU;
        let (sa, ca) = math::sin_cos(a);
        let (sb, cb) = math::sin_cos(b);
        let e1 = unit_dir([-basin[1], basin[0], 0.0]);
        let e2 = [
            basin[1] * e1[2] - basin[2] * e1[1],
            basin[2] * e1[0] - basin[0] * e1[2],
            basin[0] * e1[1] - basin[1] * e1[0],
        ];
        let dir = unit_dir([0, 1, 2].map(|k| ca * basin[k] + sa * (cb * e1[k] + sb * e2[k])));
        assert_eq!(
            material(&synth, dir),
            of(MaterialClass::Glacier, 2),
            "in the basin"
        );
    }
    // Sixteen points in each of 25 cells of the band, through one cache, which keeps each cell's
    // cut.
    let coarse = field.header().level().get();
    let mut band = BTreeSet::new();
    while band.len() < 25 {
        let dir = lat_lon(40.0 + 15.0 * rng.next_f64(), 360.0 * rng.next_f64() - 180.0);
        if arc(dir, basin) > 0.35 {
            band.insert(PatchKey::containing(coarse, dir).expect("a cell"));
        }
    }
    let mut cache = SynthCache::new();
    let (mut frost, mut bare) = (0, 0);
    for &cell in &band {
        for dir in random_points_in(cell, 16, &mut rng) {
            match material_at(&synth, &mut cache, xyz_to_face_uv(dir))
                .expect("whole")
                .material()
            {
                Some(m) if m == of(MaterialClass::Frost, 1).expect("an entry") => frost += 1,
                Some(m)
                    if m == of(MaterialClass::Dust, 3).expect("an entry")
                        || m == of(MaterialClass::Bedrock, 0).expect("an entry") =>
                {
                    bare += 1;
                }
                other => panic!("{other:?} at {dir:?}"),
            }
        }
    }
    let share = f64::from(frost) / f64::from(frost + bare);
    assert!(
        (0.15..0.45).contains(&share),
        "frost on {share} of the band, its share 0.3"
    );
    let airless = pluto_like(Pascals::ZERO);
    let synth = Synthesiser::new(&airless, DetailSeed::new(11));
    assert_eq!(
        material(&synth, basin),
        of(MaterialClass::Regolith, 2),
        "gardened"
    );
    assert_eq!(
        material(&synth, lat_lon(0.0, 0.0)),
        of(MaterialClass::Regolith, 0),
        "airless water-ice ground is its crust's regolith"
    );
}

/// A locked lava world (decision-composition's): Earth's radius, a basalt crust melting at 1,253 K
/// and no air, one month a year, the ground at `T_ss` cos^¼ θ of 2,474 K on the day side about the
/// substellar point at +x, and 60 K on the night side, as CoRoT-7 b's (2,474 ± 71 K at the
/// substellar point and 50–75 K on the night side; Léger et al. 2011, Icarus 213, 1, abstract).
/// Its crust entry carries its melt's record, the synthetic basaltic melt's stand-in
/// (decision-r11-palette-liquids §1.3 and §4), which the classifier does not read.
fn lava_world() -> CoarseField {
    let crust = PaletteEntry {
        liquid: Some(crate::testing::basaltic_melt()),
        ..basalt(PaletteRole::SecondaryCrust)
    };
    FieldBuilder::new(Metres::new(6.371e6))
        .palette(palette(vec![crust]))
        .crust_palette([None, None, Some(0), None])
        .climate(|dir| {
            let cos = dir[0];
            let t = if cos > 0.0 {
                num::max(2_474.0 * cos.sqrt().sqrt(), 60.0)
            } else {
                60.0
            };
            ClimateSample::still(Kelvin::new(t))
        })
        .build()
}

/// The locked lava world takes its `Melt`: its day side above the basalt's solidus is melt, of the
/// crust's entry, and its night side its regolith (decision-composition §1.8).
#[test]
fn material_the_lava_world_takes_its_melt() {
    let field = lava_world();
    let melt = field
        .header()
        .palette()
        .get(0)
        .and_then(|e| e.liquid.as_ref());
    assert!(
        melt.is_some_and(
            |m| m.temperature.value() > field.header().palette().entries()[0].transition.value()
        ),
        "the crust entry carries a melt's record above its solidus"
    );
    let synth = Synthesiser::new(&field, DetailSeed::new(13));
    let substellar = [1.0, 0.0, 0.0];
    let mut rng = Lcg::new(0x6c61_7661_0000_0001);
    let (mut day, mut night) = (0, 0);
    for _ in 0..200 {
        let dir = unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0));
        let theta = arc(dir, substellar).to_degrees();
        let m = material(&synth, dir);
        if theta < 80.0 {
            day += 1;
            assert_eq!(
                m,
                of(MaterialClass::Melt, 0),
                "{theta}° from the substellar point"
            );
        } else if theta > 95.0 {
            night += 1;
            assert_eq!(
                m,
                of(MaterialClass::Regolith, 0),
                "{theta}° from the substellar point"
            );
        }
    }
    assert!(day > 0 && night > 0, "{day} day, {night} night");
}

/// An airless, cratered body like the Moon whose cells poleward of 85° hold `share` of water ice.
fn cold_trapped(share: f64) -> CoarseField {
    let radius = 1.7374e6;
    FieldBuilder::new(Metres::new(radius))
        .palette(palette(vec![
            entry(
                "feldspathic",
                PaletteRole::PrimaryCrust,
                2_900.0,
                1_373.0,
                MechanicsFamily::Silicate,
            ),
            water(PaletteRole::Ice),
        ]))
        .crust_palette([None, None, Some(0), None])
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::SILICATE,
            Metres::new(radius),
            STRUCTURAL_RMS_STAGNANT_LID,
        ))
        // The Moon-like world's crater contract, so that small craters' floors are among the
        // cell's hollows.
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.05),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(1.62),
                k_target: 1.0,
                impact_velocity: Some(LUNAR_IMPACT_VELOCITY),
                projectile_density: LUNAR_PROJECTILE_DENSITY,
                target: LUNAR_TARGET,
                system_age: SOLAR_SYSTEM_AGE,
            })
            .expect("the Moon's crater contract is valid"),
        )
        .climate(|dir| ClimateSample::still(Kelvin::new(100.0 + 150.0 * math::cos(latitude(dir)))))
        .ice(move |site, _| {
            if latitude(site.dir).abs() > 85_f64.to_radians() {
                share
            } else {
                0.0
            }
        })
        .build()
}

/// A cell's cold-trapped share lies on its shadowed ground, not evenly over it (R09's "Cold traps"
/// ruling): over a polar cell the ice covers about its share, and lies lower and on slopes more
/// pole-facing than the bare ground; it is frost, thin and particulate, and a massive share on the
/// airless body is a regolith of its ice.
#[test]
fn material_the_cold_trapped_share_lies_on_the_shadowed_ground() {
    let field = cold_trapped(0.2);
    let synth = Synthesiser::new(&field, DetailSeed::new(17));
    let coarse = field.header().level().get();
    let cell = PatchKey::containing(coarse, lat_lon(87.0, 40.0)).expect("a cell");
    let held = field.cell(cell).expect("held");
    assert!(held.ice > 0 && held.ice < u8::MAX, "a partial share");
    let mut rng = Lcg::new(0x636f_6c64_0000_0001);
    let mut cache = SynthCache::new();
    let figure = field.header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).expect("a band level");
    let cut = ice_cut(&synth, &mut cache, cell, held, level).expect("whole");
    let (mut ice, mut bare) = (Vec::new(), Vec::new());
    let points = random_points_in(cell, 400, &mut rng);
    // A fresh cache, which computes the cut afresh, answers as the shared one.
    for &dir in &points[..4] {
        assert_eq!(
            material(&synth, dir),
            material_at(&synth, &mut cache, xyz_to_face_uv(dir))
                .expect("whole")
                .material(),
            "{dir:?}"
        );
    }
    for dir in points {
        let sample = synth.height_at(&mut cache, dir, level).expect("whole");
        let terms = shadow_terms(&field, dir, &sample).expect("whole");
        match material_at(&synth, &mut cache, xyz_to_face_uv(dir))
            .expect("whole")
            .material()
        {
            Some(m) if m == of(MaterialClass::Frost, 1).expect("an entry") => ice.push(terms),
            Some(m)
                if m == of(MaterialClass::Regolith, 0).expect("an entry")
                    || m == of(MaterialClass::Bedrock, 0).expect("an entry") =>
            {
                bare.push(terms);
            }
            other => panic!("{other:?} at {dir:?}"),
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "a few hundred points")]
    let share = ice.len() as f64 / 400.0;
    assert!(
        (0.13..0.27).contains(&share),
        "ice on {share} of the cell, its share 0.2"
    );
    let lowest_ice = ice
        .iter()
        .map(|&t| cut.index(t))
        .fold(f64::INFINITY, num::min);
    assert!(
        bare.iter().all(|&t| cut.index(t) <= lowest_ice),
        "the ice holds the cell's most shadowed ground, above {lowest_ice}"
    );
    #[expect(clippy::cast_precision_loss, reason = "a few hundred points")]
    let mean = |terms: &[ShadowTerms], f: fn(&ShadowTerms) -> f64| {
        terms.iter().map(f).sum::<f64>() / terms.len() as f64
    };
    assert!(
        mean(&ice, |t| t.depth_m) > mean(&bare, |t| t.depth_m),
        "the ice lies lower"
    );
    assert!(
        mean(&ice, |t| t.tilt) > mean(&bare, |t| t.tilt),
        "the ice lies on slopes facing the pole"
    );
    let massive = cold_trapped(0.8);
    let synth = Synthesiser::new(&massive, DetailSeed::new(17));
    let mut cache = SynthCache::new();
    let mut ices = 0;
    for dir in random_points_in(cell, 100, &mut rng) {
        let m = material_at(&synth, &mut cache, xyz_to_face_uv(dir))
            .expect("whole")
            .material()
            .expect("named");
        assert_ne!(
            m.class(),
            MaterialClass::Glacier,
            "airless ice is never a glacier"
        );
        if m == of(MaterialClass::Regolith, 1).expect("an entry") {
            ices += 1;
        }
    }
    assert!(ices > 60, "{ices} of 100 points on the massive share's ice");
}

/// Sea ice lies over the sea where a cell's share is above a half, and is of the cell's ice entry;
/// the ground under a cell's open water is its seabed.
#[test]
fn material_sea_ice_lies_on_the_sea() {
    let build = |share: f64| {
        FieldBuilder::new(Metres::new(1.0e6))
            .palette(palette(vec![
                basalt(PaletteRole::SecondaryCrust),
                water(PaletteRole::Ice),
                water(PaletteRole::Liquid),
            ]))
            .crust_palette([None, None, Some(0), None])
            .main_liquid(2)
            .surface_pressure(Pascals::new(101_325.0))
            .sea(Metres::new(100.0))
            .ice(move |_, _| share)
            .build()
    };
    for (share, want) in [
        (1.0, MaterialClass::SeaIce),
        (0.6, MaterialClass::SeaIce),
        (0.4, MaterialClass::Seabed),
    ] {
        let field = build(share);
        let synth = Synthesiser::new(&field, DetailSeed::new(19));
        let entry = u8::from(want == MaterialClass::SeaIce);
        for dir in [lat_lon(10.0, 20.0), lat_lon(-50.0, 100.0)] {
            assert_eq!(material(&synth, dir), of(want, entry), "share {share}");
        }
    }
}

/// The face of a direction does not change its material: a point on a face edge, given as either
/// face's (u, v), is classified alike.
#[test]
fn material_a_face_edge_point_is_classified_alike_from_either_face() {
    let field = world(SyntheticWorld::MarsLike);
    let synth = Synthesiser::new(field, SEED);
    let dir = unit_dir([1.0, 0.3, 1.0]);
    let on_x = FaceUv {
        face: Face::PosX,
        u: 0.3,
        v: 1.0,
    };
    let on_z = FaceUv {
        face: Face::PosZ,
        u: -1.0,
        v: -0.3,
    };
    let from_x = material_at(&synth, &mut SynthCache::new(), on_x).expect("whole");
    let from_z = material_at(&synth, &mut SynthCache::new(), on_z).expect("whole");
    assert_eq!(from_x, ground(&synth, dir));
    assert_eq!(from_z, from_x);
}

/// A Titan-like world (Risks, "Classes plan 14 cannot yet reach"): Titan's radius, 2,574.7 km,
/// under 1.5 bar of air at 94 K (Huygens' 1,467 hPa and 93.65 K at the surface; Fulchignoni et al. 2005,
/// Nature 438, 785), with a water-ice crust, an organic cover and methane rain of `rain_mm` a year.
fn titan_like(rain_mm: f64) -> CoarseField {
    let radius = 2.5747e6;
    let mut climate = ClimateSample::still(Kelvin::new(94.0));
    climate.month_precipitation_kg_m2_s = [rain_mm / SECONDS_PER_JULIAN_YEAR; 12];
    FieldBuilder::new(Metres::new(radius))
        .palette(palette(vec![
            entry(
                "H2O",
                PaletteRole::PrimaryCrust,
                935.0,
                273.15,
                MechanicsFamily::WaterIce,
            ),
            entry(
                "tholin",
                PaletteRole::Cover,
                1_500.0,
                600.0,
                MechanicsFamily::Organic,
            ),
        ]))
        .crust_palette([None, None, Some(0), None])
        .surface_pressure(Pascals::new(146_700.0))
        .climate(move |_| climate)
        .build()
}

/// A Titan-like world's organic cover is `OrganicSediment` where it rains, and its sand where
/// nothing falls (Design note 1's Titan-like haze regime), each of the cover's entry.
#[test]
fn material_a_titan_like_world_takes_its_organic_sediment() {
    let mut rng = Lcg::new(0x7469_7461_6e00_0001);
    for (rain_mm, class) in [
        (5.0, MaterialClass::OrganicSediment),
        (0.0, MaterialClass::Sand),
    ] {
        let field = titan_like(rain_mm);
        let synth = Synthesiser::new(&field, DetailSeed::new(23));
        let mut cache = SynthCache::new();
        for _ in 0..12 {
            let dir = unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0));
            let got = material_at(&synth, &mut cache, xyz_to_face_uv(dir)).expect("whole");
            assert_eq!(
                got.material(),
                of(class, 1),
                "{rain_mm} mm of rain at {dir:?}"
            );
        }
    }
}

/// A field of one climate everywhere: still at `temperature_k` (K) with twelve months, each month
/// m's anomaly `anomaly_k[m]` (K) and its precipitation `rain_mm[m]` (mm a year as a rate).
fn seasonal_field(temperature_k: f64, anomaly_k: [f64; 12], rain_mm: [f64; 12]) -> CoarseField {
    let mut climate = ClimateSample::still(Kelvin::new(temperature_k));
    climate.month_anomaly_k = anomaly_k;
    climate.month_precipitation_kg_m2_s = rain_mm.map(|mm| mm / SECONDS_PER_JULIAN_YEAR);
    FieldBuilder::new(Metres::new(1.0e6))
        .months(12)
        .climate(move |_| climate)
        .build()
}

/// Köppen–Geiger's threshold takes its seasonal offsets (Peel, Finlayson and McMahon 2007,
/// Table 1): at a 20 °C annual mean, 400 mm a year is arid where it falls in winter (a threshold of 400 mm,
/// ten times 2 × 20) only if less than that, and arid where it falls in summer (680 mm, ten times
/// 2 × 20 + 28); and the summer is the warmer half-year whichever months it is.
#[test]
fn material_aridity_takes_koppen_geiger_s_seasons() {
    // Months 0–5 warm in the first field, 6–11 in the second.
    let warm_first: [f64; 12] = core::array::from_fn(|m| if m < 6 { 5.0 } else { -5.0 });
    let warm_last = warm_first.map(|a| -a);
    let rain_early: [f64; 12] = core::array::from_fn(|m| if m < 6 { 900.0 } else { 0.0 });
    for (anomaly, summer_rain) in [(warm_first, true), (warm_last, false)] {
        let field = seasonal_field(293.15, anomaly, rain_early);
        let header = field.header();
        let key = cell_at_index(header.level().get(), 7).expect("a cell");
        let climate = field.climate(key).expect("held");
        let annual = annual_precipitation_mm(header, climate);
        let summer = summer_share(header, climate, annual).expect("twelve months");
        assert!(
            (summer - f64::from(u8::from(summer_rain))).abs() < 1e-12,
            "{summer}"
        );
        // About 450 mm a year: above the winter threshold of 400 mm and below the summer's 680.
        assert!((430.0..470.0).contains(&annual), "{annual} mm");
        assert_eq!(
            is_arid(header, climate, 293.15, annual),
            summer_rain,
            "summer rain {summer_rain}"
        );
    }
    // Even seasons take the middle offset, 540 mm at 20 °C.
    let even = seasonal_field(293.15, warm_first, [500.0; 12]);
    let key = cell_at_index(even.header().level().get(), 7).expect("a cell");
    let climate = even.climate(key).expect("held");
    let annual = annual_precipitation_mm(even.header(), climate);
    assert!(
        is_arid(even.header(), climate, 293.15, annual),
        "{annual} mm below 540"
    );
}

/// Each rule's entry chain: the seabed falls back from the crust to the bulk and then to a deposit;
/// the regolith from the crust to a deposit that is not a salt, then to a cover; sand and dust take
/// such a deposit first, then a cover, then the crust; a salt deposit is the last resort of both;
/// and the salt is a salt-family deposit first.
#[test]
fn material_each_rule_names_its_entry_chain() {
    let field_of = |entries: Vec<PaletteEntry>, crust: Option<u8>| {
        FieldBuilder::new(Metres::new(1.0e6))
            .palette(palette(entries))
            .crust_palette([None, None, crust, None])
            .build()
    };
    let deposit = entry(
        "mars_dust",
        PaletteRole::Deposit,
        2_600.0,
        1_400.0,
        MechanicsFamily::Silicate,
    );
    let cover = entry(
        "tholin",
        PaletteRole::Cover,
        1_500.0,
        600.0,
        MechanicsFamily::Organic,
    );
    let bulk = entry(
        "chondrite",
        PaletteRole::Bulk,
        3_500.0,
        1_400.0,
        MechanicsFamily::Silicate,
    );
    let salt = entry(
        "NaCl",
        PaletteRole::Deposit,
        2_165.0,
        1_074.0,
        MechanicsFamily::Salt,
    );
    // A crust, a cover and a dust deposit: the crust for the floor and the regolith, the deposit
    // for sand and dust.
    let f = field_of(
        vec![
            basalt(PaletteRole::SecondaryCrust),
            cover.clone(),
            deposit.clone(),
        ],
        Some(0),
    );
    let e = Entries::of(f.header(), Crust::Lid);
    assert_eq!(
        (e.crust, e.floor, e.regolith, e.loose),
        (Some(0), Some(0), Some(0), Some(2))
    );
    assert_eq!((e.salt, e.organic_cover), (None, Some(1)));
    // No crust's entry: the cover, then the bulk.
    let f = field_of(vec![cover, bulk], None);
    let e = Entries::of(f.header(), Crust::Lid);
    assert_eq!(
        (e.crust, e.floor, e.regolith, e.loose),
        (None, Some(1), Some(0), Some(0))
    );
    // A salt deposit and a dust deposit, the salt first by key: the salt is the salt, and sand
    // and dust the deposit that is not a salt.
    let f = field_of(
        vec![
            basalt(PaletteRole::SecondaryCrust),
            salt.clone(),
            deposit.clone(),
        ],
        Some(0),
    );
    let e = Entries::of(f.header(), Crust::Lid);
    assert_eq!((e.salt, e.loose, e.floor), (Some(1), Some(2), Some(0)));
    // A salt deposit alone: no crust's entry, so the regolith and the loose ground fall to it.
    let f = field_of(vec![salt], None);
    let e = Entries::of(f.header(), Crust::Lid);
    assert_eq!((e.salt, e.regolith, e.loose), (Some(0), Some(0), Some(0)));
    // A palette with only a deposit: the floor falls to it.
    let f = field_of(vec![deposit], None);
    let e = Entries::of(f.header(), Crust::Lid);
    assert_eq!((e.floor, e.regolith, e.loose), (Some(0), Some(0), Some(0)));
}

/// A point beside a trunk, beyond half its bankfull width from its axis, is off its bed.
#[test]
fn material_a_point_beside_a_channel_is_off_its_bed() {
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let figure = field.header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).expect("a band level");
    let through = synth
        .level_set(level)
        .channels_through()
        .expect("the finest level evaluates channels");
    let coarse = field.header().level().get();
    let mut network = synth.channels().network(field);
    let mut seen = 0;
    for n in 0..field.synthesis().len() {
        let key = cell_at_index(coarse, u32::try_from(n).expect("a cell index"))
            .expect("a cell of the level");
        let cell = field.cell(key).expect("held");
        if cell.is_under_water() || cell.ice > 0 {
            continue;
        }
        let Some(segment) = network.segment(key).expect("whole") else {
            continue;
        };
        let head = network.key_point(key).dir();
        let from = segment.from_m();
        let along = segment.point_m(0.01);
        let chord = unit_dir([0, 1, 2].map(|k| along[k] - from[k]));
        let nu = figure.normal(head);
        let across = unit_dir([
            nu[1] * chord[2] - nu[2] * chord[1],
            nu[2] * chord[0] - nu[0] * chord[2],
            nu[0] * chord[1] - nu[1] * chord[0],
        ]);
        let offset_m = 1.5 * segment.bankfull_width().value() + 10.0;
        let beside = unit_dir(
            [0, 1, 2].map(|k| head[k] + across[k] * offset_m / figure.equatorial_radius_m),
        );
        let mut cache = SynthCache::new();
        assert!(
            synth
                .channels()
                .on_channel_floor(field, head, through, &mut cache)
                .expect("whole"),
            "on the head"
        );
        assert!(
            !synth
                .channels()
                .on_channel_floor(field, beside, through, &mut cache)
                .expect("whole"),
            "{offset_m} m beside the trunk of {key:?}"
        );
        seen += 1;
        if seen == 8 {
            break;
        }
    }
    assert!(seen > 0, "the Earth-like world has trunks on dry land");
}

/// The answers on partial ice and on channels' beds do not depend on the cache or on the order of
/// queries (the sim-determinism skill's order independence): through one cache warm from pass to
/// pass, and through a fresh cache for each.
#[test]
fn material_is_order_and_cache_independent_on_partial_ice_and_channel_beds() {
    let icy = cold_trapped(0.2);
    let synth = Synthesiser::new(&icy, DetailSeed::new(17));
    let coarse = icy.header().level().get();
    let cell = PatchKey::containing(coarse, lat_lon(87.0, 40.0)).expect("a cell");
    let mut rng = Lcg::new(0x6f72_6465_7200_0001);
    // A fresh cache's query of partial ice costs its cell's cut, 256 heights: 12 points in one
    // polar cell and 6 in the other, 4 and 2 on the browser target, where both cells and both
    // caches are still asked (`crate::test_sizes`).
    let mut dirs = random_points_in(cell, per_target(12, 4), &mut rng);
    let other = PatchKey::containing(coarse, lat_lon(-88.0, -100.0)).expect("a cell");
    dirs.extend(random_points_in(other, per_target(6, 2), &mut rng));
    let shared = std::cell::RefCell::new(SynthCache::new());
    assert_order_independent(&dirs, |&d| {
        material_at(&synth, &mut shared.borrow_mut(), xyz_to_face_uv(d)).expect("whole")
    });
    assert_order_independent(&dirs, |&d| ground(&synth, d));
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let mut network = synth.channels().network(field);
    let heads: Vec<[f64; 3]> = (0..field.synthesis().len())
        .filter_map(|n| {
            let key = cell_at_index(field.header().level().get(), u32::try_from(n).ok()?)?;
            let cell = field.cell(key)?;
            (!cell.is_under_water() && network.segment(key).ok()??.is_trunk())
                .then(|| network.key_point(key).dir())
        })
        .take(8)
        .collect();
    let shared = std::cell::RefCell::new(SynthCache::new());
    assert_order_independent(&heads, |&d| {
        material_at(&synth, &mut shared.borrow_mut(), xyz_to_face_uv(d)).expect("whole")
    });
    assert_order_independent(&heads, |&d| ground(&synth, d));
}

/// One cache kept across two fields of one body and seed whose heights agree and whose ice shares
/// differ, and across another detail seed, answers as a fresh cache: a cut is kept by its cell,
/// level and ice code, for the field and seed the channels' memo is kept for.
#[test]
fn material_a_cache_kept_across_fields_answers_as_a_fresh_one() {
    let thin = cold_trapped(0.2);
    let thick = cold_trapped(0.6);
    let coarse = thin.header().level().get();
    let cell = PatchKey::containing(coarse, lat_lon(87.0, 40.0)).expect("a cell");
    let mut rng = Lcg::new(0x6669_656c_6400_0001);
    // Eight points, three on the browser target, each asked of the four pairs of field and seed
    // in turn (`crate::test_sizes`).
    let dirs = random_points_in(cell, per_target(8, 3), &mut rng);
    let mut cache = SynthCache::new();
    for (field, seed) in [(&thin, 17), (&thick, 17), (&thin, 18), (&thin, 17)] {
        let synth = Synthesiser::new(field, DetailSeed::new(seed));
        for &d in &dirs {
            assert_eq!(
                material_at(&synth, &mut cache, xyz_to_face_uv(d)).expect("whole"),
                ground(&synth, d),
                "{d:?}"
            );
        }
    }
}

/// A view that holds exactly the cells within the synthesis's margin of a partial-ice cell answers
/// its points from a fresh cache: the cut's grid reads no cell its points do not.
#[test]
fn material_a_partial_ice_cell_answers_within_its_margin() {
    let field = cold_trapped(0.2);
    let coarse = field.header().level().get();
    let cell = PatchKey::containing(coarse, lat_lon(87.0, 40.0)).expect("a cell");
    let view = Held {
        inner: &field,
        held: king_distances(cell, crate::field::SYNTHESIS_MARGIN_CELLS)
            .into_keys()
            .collect(),
    };
    let synth = Synthesiser::new(&view, DetailSeed::new(17));
    let whole = Synthesiser::new(&field, DetailSeed::new(17));
    let mut rng = Lcg::new(0x6d61_7267_696e_0001);
    // Six points, two on the browser target, each from a fresh cache: its own cut, 256 heights
    // (`crate::test_sizes`).
    for d in random_points_in(cell, per_target(6, 2), &mut rng) {
        let held = material_at(&synth, &mut SynthCache::new(), xyz_to_face_uv(d)).expect("held");
        assert_eq!(held, ground(&whole, d), "{d:?}");
    }
}
