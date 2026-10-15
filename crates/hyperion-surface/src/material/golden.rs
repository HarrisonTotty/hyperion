//! The material goldens, `tests/golden/material/<world>.golden` (plan R10, T1.b): [`material_at`]
//! at a thousand points of each synthetic world, blessed natively and compared on `wasm32-wasip1`
//! and `wasm32-unknown-unknown` through the testkit's embedded arm, each named by a literal; and
//! `tests/golden/material/table.golden`, R10.T1.a's property table and the functions over it, which
//! T1.a's record left to T1.b.
//!
//! The server lands on the material [`material_at`] answers natively, and the client draws the
//! class its WebAssembly workers bake, so a split between the targets is ground drawn as one
//! material and landed on as another. They live in the library's tests, not in `tests/`, because
//! the synthetic worlds are the `testing` feature's, which the browser target's test build and
//! `ci`'s wasip1 selection do not enable for the crate's integration tests (R10.T3's ground and
//! R09.T9's heights live here for the same reason). They carry `GENERATOR_VERSION`, in a
//! subdirectory that `cube_golden.rs`'s check of the test planet's version does not read.
//!
//! # Format
//!
//! `<face> <u> <v> <code> <class> <entry>`: the point's face and its (u, v), each float `0x` and
//! its 16 hexadecimal digits of IEEE 754 bits; the material's class code and name; and its palette
//! entry, or `-` where the body's palette names no substance there. The points are the cube's eight
//! corners; up to 32 trunks' heads, on their channels' beds; up to 40 points in cells with ice and
//! 40 in the shallowest cells under water; and random points over the sphere to a thousand.
//!
//! The table's sections are the constants (the slopes, the classifier's thresholds and the
//! photometric anchors), each class's code and photometric form, each phase law's code, template
//! and ceiling with the family defaults, `limb_parameter` and each law's L(0) on a grid of normal
//! albedos and L(α) on a grid of phases, a few directional-hemispherical albedos, every (form,
//! family) row (its shear layers with their bearing factors, its friction at its knots and between
//! them, its strength, albedo factor and span, or `unmodelled`), and the two lunar capacities.

use std::fmt::Write as _;

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::units::{Kelvin, Metres, MetresPerSecondSquared, Radians};

use super::{
    BRIGHT_FROST_LIMB_PARAMETER, BRIGHT_FROST_NORMAL_ALBEDO, BearingFactors, CLIFF_SLOPE,
    FrictionModel, LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO, LUNAR_LIMB_ONSET_NORMAL_ALBEDO,
    MASSIVE_ICE_SHARE, Material, MaterialClass, PARTICULATE_NORMAL_ALBEDO_CEILING, PhaseLaw,
    SNOW_SHED_SLOPE, THIN_ATMOSPHERE_PRESSURE, TRANSITION_HALF_WIDTH, UNASSIGNED_PHASE_ROW,
    bearing_capacity, directional_hemispherical_albedo, form_row, interface_friction,
    limb_parameter, material_at, properties, ultimate_bearing_capacity,
};
use crate::cube::{PatchKey, unit_dir, xyz_to_face_uv};
use crate::field::{
    CoarseField, FieldView, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole,
    cell_at_index,
};
use crate::substance_key::SubstanceKey;
use crate::synth::test_support::{SEED, random_points_in, world};
use crate::synth::{SynthCache, Synthesiser};
use crate::testing::SyntheticWorld;

/// The points each world's golden pins.
const POINTS: usize = 1_000;

/// `value`'s IEEE 754 bits, as `0x` and 16 hexadecimal digits.
fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

/// Every cell of `field`'s level with its record, in cell-index order.
fn cells(field: &CoarseField) -> impl Iterator<Item = PatchKey> + '_ {
    let level = field.header().level().get();
    (0..field.synthesis().len()).map(move |n| {
        cell_at_index(level, u32::try_from(n).expect("a cell index fits a u32"))
            .expect("a cell of the field's level")
    })
}

/// The points the golden of `field` pins (the module documentation).
fn pinned_points(synth: &Synthesiser<'_, CoarseField>, field: &CoarseField) -> Vec<[f64; 3]> {
    let mut rng = Lcg::new(0x6d61_7465_7269_616c);
    let mut dirs: Vec<[f64; 3]> = [-1.0, 1.0]
        .into_iter()
        .flat_map(|x| [-1.0, 1.0].map(|y| [x, y]))
        .flat_map(|[x, y]| [-1.0, 1.0].map(|z| unit_dir([x, y, z])))
        .collect();
    let mut network = synth.channels().network(field);
    let mut heads = 0;
    for key in cells(field) {
        let cell = field.cell(key).expect("a whole field holds every cell");
        if cell.is_under_water() || cell.ice > 0 {
            continue;
        }
        if network
            .segment(key)
            .expect("a whole field holds every cell")
            .is_some()
        {
            dirs.push(network.key_point(key).dir());
            heads += 1;
            if heads == 32 {
                break;
            }
        }
    }
    let icy: Vec<PatchKey> = cells(field)
        .filter(|&k| field.cell(k).is_some_and(|c| c.ice > 0))
        .collect();
    for k in 0..icy.len().min(40) {
        let key = icy[k * icy.len() / icy.len().min(40)];
        dirs.extend(random_points_in(key, 1, &mut rng));
    }
    let mut sea: Vec<(i32, PatchKey)> = cells(field)
        .filter_map(|k| {
            let c = field.cell(k)?;
            c.is_under_water()
                .then_some((c.water_surface_mm - c.elevation_mm, k))
        })
        .collect();
    sea.sort_unstable();
    for &(_, key) in sea.iter().take(40) {
        dirs.extend(random_points_in(key, 1, &mut rng));
    }
    while dirs.len() < POINTS {
        dirs.push(unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)));
    }
    dirs
}

/// The golden text of `w`.
fn material_text(w: SyntheticWorld) -> String {
    let field = world(w);
    let synth = Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut line = String::new();
    for dir in pinned_points(&synth, field) {
        let at = xyz_to_face_uv(dir);
        let m = material_at(&synth, &mut cache, at).expect("a whole field holds every cell");
        line.clear();
        write!(
            line,
            "{} 0x{:016x} 0x{:016x} ",
            at.face.index(),
            bits(at.u),
            bits(at.v)
        )
        .expect("a string takes any write");
        write!(line, "{} {:?} ", u8::from(m.class()), m.class()).expect("a string takes any write");
        match m.entry() {
            Some(entry) => write!(line, "{entry}"),
            None => write!(line, "-"),
        }
        .expect("a string takes any write");
        out.line(&line);
    }
    out.finish()
}

#[test]
fn material_matches_its_goldens() {
    golden!(
        "material/earth_like",
        &material_text(SyntheticWorld::EarthLike)
    );
    golden!(
        "material/mars_like",
        &material_text(SyntheticWorld::MarsLike)
    );
    golden!(
        "material/moon_like",
        &material_text(SyntheticWorld::MoonLike)
    );
    golden!(
        "material/ceres_like",
        &material_text(SyntheticWorld::CeresLike)
    );
    golden!("material/flat", &material_text(SyntheticWorld::Flat));
    golden!(
        "material/one_crater",
        &material_text(SyntheticWorld::OneCrater)
    );
}

/// The property table's golden text (R10.T1.a's deferred finding, "A golden of the table, for
/// T1.b"): every figure the table and its functions give, bit for bit, by section.
fn table_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line("constants");
    for (label, value) in [
        ("cliff_slope_rad", CLIFF_SLOPE.value()),
        ("transition_half_width_rad", TRANSITION_HALF_WIDTH.value()),
        ("snow_shed_slope_rad", SNOW_SHED_SLOPE.value()),
        ("massive_ice_share", MASSIVE_ICE_SHARE),
        (
            "thin_atmosphere_pressure_pa",
            THIN_ATMOSPHERE_PRESSURE.value(),
        ),
        (
            "lommel_seeliger_normal_albedo_ratio",
            LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO,
        ),
        (
            "lunar_limb_onset_normal_albedo",
            LUNAR_LIMB_ONSET_NORMAL_ALBEDO,
        ),
        ("bright_frost_normal_albedo", BRIGHT_FROST_NORMAL_ALBEDO),
        ("bright_frost_limb_parameter", BRIGHT_FROST_LIMB_PARAMETER),
        (
            "particulate_normal_albedo_ceiling",
            PARTICULATE_NORMAL_ALBEDO_CEILING,
        ),
    ] {
        out.f64(label, value);
    }
    out.line("classes");
    for &class in MaterialClass::ALL {
        out.line(&format!(
            "class {} {class:?} = {:?}",
            u8::from(class),
            class.photometric_form()
        ));
    }
    out.line("phase laws");
    for &law in PhaseLaw::ALL {
        out.line(&format!(
            "law {} {law:?} = {}",
            u8::from(law),
            law.template()
        ));
        out.f64("ceiling", law.normal_albedo_ceiling());
    }
    for &family in MechanicsFamily::ALL {
        out.line(&format!(
            "for_family {family:?} = {:?}",
            PhaseLaw::for_family(family)
        ));
    }
    out.line("limb parameter");
    for k in 0..=40_u8 {
        let a = f64::from(k) / 20.0;
        out.f64(&format!("L(0) at {a}"), limb_parameter(a));
        for &law in PhaseLaw::ALL {
            out.f64(&format!("{law:?} L(0) at {a}"), law.limb_parameter(a));
        }
    }
    for degrees in (0..=180_u16).step_by(15) {
        let alpha = Radians::new(f64::from(degrees).to_radians());
        for &law in PhaseLaw::ALL {
            out.f64(
                &format!("{law:?} L({degrees}°) at L(0) 1"),
                law.lommel_seeliger_share(1.0, alpha),
            );
        }
    }
    for (a, l, mu0) in [
        (0.12, 1.0, 1.0),
        (0.5, 0.5, 0.5),
        (0.9, 0.0, 0.2),
        (1.2, 0.38, 0.05),
    ] {
        out.f64(
            &format!("A_h of A_N {a} L {l} at μ₀ {mu0}"),
            directional_hemispherical_albedo(a, l, mu0),
        );
    }
    table_rows(&mut out);
    table_lunar(&mut out);
    out.finish()
}

/// The table golden's rows: every (form, family) row, or `unmodelled`.
fn table_rows(out: &mut GoldenWriter) {
    out.line("rows");
    for &class in MaterialClass::ALL {
        for &family in MechanicsFamily::ALL {
            let row = form_row(class, family);
            if !row.is_modelled() {
                out.line(&format!("row {class:?} {family:?} unmodelled"));
                continue;
            }
            out.line(&format!("row {class:?} {family:?}"));
            for layer in row.shear() {
                let s = layer.strength();
                let factors = BearingFactors::of(s);
                out.line(&format!(
                    "shear {} {} c {} phi {} porosity {} n_q {} n_c {} n_gamma {}",
                    hex(layer.top().value()),
                    hex(layer.bottom().value()),
                    hex(s.cohesion().value()),
                    hex(s.friction_angle().value()),
                    hex(layer.porosity()),
                    hex(factors.n_q()),
                    hex(factors.n_c()),
                    hex(factors.n_gamma()),
                ));
            }
            match row.interface_friction() {
                Some(FrictionModel::Constant(mu)) => out.f64("friction", mu),
                Some(FrictionModel::ByTemperature(knots)) => {
                    let palette = MaterialPalette::new(vec![PaletteEntry {
                        substance: SubstanceKey::new_const("H2O"),
                        role: PaletteRole::Ice,
                        normal_albedo_bvr: [0.9; 3],
                        phase_row: UNASSIGNED_PHASE_ROW,
                        grain_density_kg_m3: 917.0,
                        transition: Kelvin::new(273.15),
                        mechanics: family,
                        liquid: None,
                    }])
                    .expect("a one-entry palette is valid");
                    let p = properties(
                        Material::new(class, 0).expect("entry 0 is a palette index"),
                        &palette,
                    )
                    .expect("entry 0 is in the palette");
                    for pair in knots.windows(2) {
                        let (t0, mu0) = pair[0];
                        let mid = Kelvin::new(f64::midpoint(t0.value(), pair[1].0.value()));
                        out.line(&format!(
                            "friction at {} = {} and at {} = {:?}",
                            hex(t0.value()),
                            hex(mu0),
                            hex(mid.value()),
                            interface_friction(&p, mid).map(hex)
                        ));
                    }
                    let last = knots[knots.len() - 1];
                    out.line(&format!(
                        "friction at {} = {}",
                        hex(last.0.value()),
                        hex(last.1)
                    ));
                }
                None => out.line("friction none"),
            }
            out.line(&format!(
                "strength {:?} factor {:?} span {:?}",
                row.compressive_strength().map(|s| hex(s.value())),
                row.albedo_factor().map(hex),
                row.normal_albedo_v().map(|s| (hex(s.low()), hex(s.high())))
            ));
        }
    }
}

/// The table golden's two lunar capacities, as T1.a's bearing test pins them.
fn table_lunar(out: &mut GoldenWriter) {
    out.line("lunar capacities");
    let lunar = MaterialPalette::new(vec![PaletteEntry {
        substance: SubstanceKey::new_const("feldspathic"),
        role: PaletteRole::PrimaryCrust,
        normal_albedo_bvr: [0.12; 3],
        phase_row: UNASSIGNED_PHASE_ROW,
        grain_density_kg_m3: 3_100.0,
        transition: Kelvin::new(1_500.0),
        mechanics: MechanicsFamily::Silicate,
        liquid: None,
    }])
    .expect("a one-entry palette is valid");
    let soil = properties(
        Material::new(MaterialClass::Regolith, 0).expect("entry 0 is a palette index"),
        &lunar,
    )
    .expect("entry 0 is in the palette");
    let moon_g = MetresPerSecondSquared::new(1.62);
    let deep = bearing_capacity(&soil, moon_g, Metres::new(1.0)).expect("lunar regolith has data");
    out.f64("1 m footing, Pa", deep.value());
    let shallow = soil
        .shear_layer_at(Metres::new(0.1))
        .expect("the 0–15 cm layer");
    out.f64(
        "0–15 cm pair under 1 m, Pa",
        ultimate_bearing_capacity(
            shallow.strength(),
            soil.bulk_density(shallow),
            moon_g,
            Metres::new(1.0),
        )
        .value(),
    );
}

#[test]
fn material_the_property_table_matches_its_golden() {
    golden!("material/table", &table_text());
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// The material goldens: one for each synthetic world, and the property table's.
    const MATERIAL_GOLDENS: usize = 7;

    /// Every golden file of `tests/golden/material/` carries `GENERATOR_VERSION`, not R05's
    /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`.
    #[test]
    fn material_every_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("material");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the material golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert_eq!(seen, MATERIAL_GOLDENS, "the goldens in {}", dir.display());
    }
}
