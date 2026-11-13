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
//!
//! `tests/golden/material/bearing_on_slope.golden` is the T1.a slope follow-up's capacities, which
//! the terrain readout states (plan R10, T12; decision-r10-t12-leans §8; the follow-up's deferred
//! finding 13): every layer of every shear table at its body's gravity, the lunar regolith's four
//! at the Moon's 1.62 m s⁻² and Mars's drift and Viking Lander 2's soil at Mars's 3.71 m s⁻², and
//! a constructed cohesionless φ 30° soil of 1,600 kg m⁻³ at a rounded Earth gravity, 9.81 m s⁻²
//! (decision-r10-slope-capacity §6), each through [`ultimate_bearing_capacity_on_slope`]
//! under the 1.0 m reference pad and a 0.1 m pad, on a grid of slopes from 0 to just under the
//! layer's φ and at φ, just above it and at 89°, where it is `none`; and each table's
//! [`bearing_capacity_on_slope`], which takes the layer at one pad's depth, on the same pads and
//! slopes. The ruled figures are among them (decision-r10-slope-capacity §6): 3,730.0 and
//! 2,331.0 kPa on the lunar 30–60 cm layer under 1 m at 10° and 20°, 133.3 kPa on its 0–15 cm pair
//! under 1 m at 10°, and 209.9 and 27.84 kPa on Lander 2's soil and the drift under 1 m at 10°.
//!
//! `tests/golden/material/weathering.golden` is R10.T1.d's space-weathering law: its constants
//! and the table's Λ, then τ and the weathered normal, spherical and directional-hemispherical
//! albedos (at the four incidences), each as its bits, on a grid of fresh albedo, `FeO_T`,
//! maturity, wavelength (inside iron's grid and beyond both its ends) and law at one grain density,
//! a line where every albedo is the fresh one's own bits saying so in a word; and
//! [`weathered_entry_bvr`] of every synthetic world's palette entries at maturities 0.5 and 1.
//!
//! `tests/golden/material/maturity.golden` is R10.T1.c's (decision-r10-t1c-maturity), counted
//! here and written by the `maturity` module's own tests, since the sim's tripwire
//! (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`, until P14.T54.b) reads every file
//! of the crate but `material/maturity.rs` for the time law's names: the time law on a grid of
//! exposures and rates, which P14.T51.d left unpinned across targets; the fresh share's parts (the
//! reworked depth, the excavated share and the halo) on grids of their inputs; the maturity at a
//! point ([`maturity_at`](super::maturity_at)) at the centres of an 8 × 8 grid of each face of the
//! Moon-like world weathered at maturity 1 and the Moon's rate and of the single crater's world
//! with its crater 100 Myr old and g 0.8, at its 4 Gyr surface's maturity, with a transect across
//! the crater from its centre to 3 rim radii; every form of every palette entry's exposed albedo
//! ([`exposed_properties`](super::exposed_properties)) at `m_body` and at maturities 0.3 and 0.7
//! (the single crater's palette is empty); and the organic cover's blend on the Pluto-like world
//! at covers 0, ½ and 1.
//!
//! `tests/golden/material/halo.golden` is R10.T1.c's halo follow-up (decision-r10-t1c-halo),
//! counted here and written by the `maturity` module's tests alike: the inverse halo
//! ([`halo_reach`](super::halo_reach)) on a grid of shares and ends; and on the Moon-like world
//! weathered at maturity 1 and the Moon's rate, its 3.5, 1 and 0.1 Gyr variants and the single
//! crater's world on an iron-bearing basalt, the body's maturity mix
//! ([`maturity_mix`](super::maturity_mix)), its nodes' maturities and weights as bits, its shares
//! above deficits of 0.05, 0.2 and 0.5, and the halo-free and haloed scales
//! ([`exposed_albedo_scale`](super::exposed_albedo_scale),
//! [`haloed_albedo_scale`](super::haloed_albedo_scale)) for p = 0.10, 0.12 and 0.14 with the
//! stated excess.

use std::fmt::Write as _;

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::units::{
    Kelvin, KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, Pascals, Radians,
};

use super::{
    BRIGHT_FROST_LIMB_PARAMETER, BRIGHT_FROST_NORMAL_ALBEDO, BearingFactors, CLIFF_SLOPE,
    FrictionModel, LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO, LUNAR_LIMB_ONSET_NORMAL_ALBEDO,
    MASSIVE_ICE_SHARE, Material, MaterialClass, MaterialProperties, MohrCoulomb,
    PARTICULATE_NORMAL_ALBEDO_CEILING, PhaseLaw, SNOW_SHED_SLOPE, THIN_ATMOSPHERE_PRESSURE,
    TRANSITION_HALF_WIDTH, UNASSIGNED_PHASE_ROW, bearing_capacity, bearing_capacity_on_slope,
    directional_hemispherical_albedo, form_row, interface_friction, limb_parameter, material_at,
    properties, ultimate_bearing_capacity, ultimate_bearing_capacity_on_slope,
};
use super::{
    FEO_MOLAR_MASS_G_PER_MOL, ICE_HOST_INDEX, IRON_DENSITY_KG_M3, IRON_MOLAR_MASS_G_PER_MOL,
    REGOLITH_HOST_INDEX, npfe_optical_depth, weathered_entry_bvr, weathered_hemispherical_albedos,
    weathered_normal_albedo, weathered_spherical_albedo,
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

/// The Moon's surface gravity, 1.62 m s⁻², as the Lunar Sourcebook takes it (§9.1.9, p. 517): the
/// figure of `bearing.rs`'s tests' `MOON_G`, which is private to them.
const MOON_G: MetresPerSecondSquared = MetresPerSecondSquared::new(1.62);

/// Mars's surface gravity, 3.71 m s⁻², as plan R10's T12 drafts take it for the Viking soils: the
/// figure of `bearing.rs`'s tests' `MARS_G`, which is private to them.
const MARS_G: MetresPerSecondSquared = MetresPerSecondSquared::new(3.71);

/// The gravity the constructed cohesionless soil is taken at, 9.81 m s⁻²: a rounded Earth gravity,
/// standard gravity's 9.80665 m s⁻² (3rd CGPM, 1901) to three figures. The soil is no body's
/// (decision-r10-slope-capacity §6 constructs it to pin the slope's ratio q(β) ÷ q(0)), so the
/// figure is a plausible setting, not a measurement.
const CONSTRUCTED_SOIL_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(9.81);

/// The constructed cohesionless soil's bulk density, 1,600 kg m⁻³: a medium-dense dry sand's,
/// chosen for decision-r10-slope-capacity §6's φ 30° soil, not a measured soil's (the ruled ratio
/// does not depend on it).
const CONSTRUCTED_SOIL_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_600.0);

/// The constructed cohesionless soil's friction angle, 30° (decision-r10-slope-capacity §6).
const CONSTRUCTED_SOIL_FRICTION_DEG: f64 = 30.0;

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
                        feo_mass_fraction: None,
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
        feo_mass_fraction: None,
    }])
    .expect("a one-entry palette is valid");
    let soil = properties(
        Material::new(MaterialClass::Regolith, 0).expect("entry 0 is a palette index"),
        &lunar,
    )
    .expect("entry 0 is in the palette");
    let deep = bearing_capacity(&soil, MOON_G, Metres::new(1.0)).expect("lunar regolith has data");
    out.f64("1 m footing, Pa", deep.value());
    let shallow = soil
        .shear_layer_at(Metres::new(0.1))
        .expect("the 0–15 cm layer");
    out.f64(
        "0–15 cm pair under 1 m, Pa",
        ultimate_bearing_capacity(
            shallow.strength(),
            soil.bulk_density(shallow),
            MOON_G,
            Metres::new(1.0),
        )
        .value(),
    );
}

#[test]
fn material_the_property_table_matches_its_golden() {
    golden!("material/table", &table_text());
}

/// A silicate entry's form `class` at the grain density `grain_density`, kg m⁻³: the Lunar
/// Sourcebook's 3,100 for the lunar soil, so that its layers' bulk densities are the Sourcebook's,
/// and Moore et al.'s assumed 2,600 for the Viking soils (`properties.rs`), so that theirs are
/// 1,200 and 1,400 kg m⁻³.
fn silicate(class: MaterialClass, grain_density: f64) -> MaterialProperties {
    let palette = MaterialPalette::new(vec![PaletteEntry {
        substance: SubstanceKey::new_const("feldspathic"),
        role: PaletteRole::PrimaryCrust,
        normal_albedo_bvr: [0.12; 3],
        phase_row: UNASSIGNED_PHASE_ROW,
        grain_density_kg_m3: grain_density,
        transition: Kelvin::new(1_500.0),
        mechanics: MechanicsFamily::Silicate,
        liquid: None,
        feo_mass_fraction: None,
    }])
    .expect("a one-entry palette is valid");
    properties(
        Material::new(class, 0).expect("entry 0 is a palette index"),
        &palette,
    )
    .expect("entry 0 is in the palette")
}

/// The pads the slope golden takes: the 1.0 m reference pad and a narrow 0.1 m one, metres.
const SLOPE_GOLDEN_PADS: [f64; 2] = [1.0, 0.1];

/// The slopes the slope golden pins for a layer of friction angle `phi`, radians, each with its
/// label: whole degrees from 0 to 50, shares of φ to 0.99, and φ less 10⁻⁹ rad, all below φ for
/// the layers' angles above 50°, and those at or past it `none`; then φ, the next float above it and
/// 89°, which are `none`.
fn slope_grid(phi: Radians) -> Vec<(String, Radians)> {
    let phi = phi.value();
    let mut slopes: Vec<(String, Radians)> = [0_u8, 2, 5, 10, 15, 20, 25, 30, 40, 45, 50]
        .into_iter()
        .map(|deg| (format!("{deg}°"), Radians::new(f64::from(deg).to_radians())))
        .collect();
    slopes.extend(
        [0.25, 0.5, 0.75, 0.9, 0.99].map(|share| (format!("{share} φ"), Radians::new(share * phi))),
    );
    slopes.extend([
        ("φ − 1e-9 rad".to_owned(), Radians::new(phi - 1e-9)),
        ("φ".to_owned(), Radians::new(phi)),
        ("next above φ".to_owned(), Radians::new(phi.next_up())),
        ("89°".to_owned(), Radians::new(89.0_f64.to_radians())),
    ]);
    slopes
}

/// Writes `q` under `label`, or `none` where it is not computed.
fn capacity_line(out: &mut GoldenWriter, label: &str, q: Option<Pascals>) {
    match q {
        Some(q) => out.f64(label, q.value()),
        None => out.line(&format!("{label} none")),
    }
}

/// One layer's capacities, `strength` and bulk density `rho` at gravity `g`, under each pad on its
/// slope grid ([`slope_grid`]).
fn layer_lines(
    out: &mut GoldenWriter,
    strength: MohrCoulomb,
    rho: KilogramsPerCubicMetre,
    g: MetresPerSecondSquared,
) {
    out.f64("c_pa", strength.cohesion().value());
    out.f64("phi_rad", strength.friction_angle().value());
    out.f64("rho_kg_m3", rho.value());
    for pad in SLOPE_GOLDEN_PADS {
        for (name, slope) in slope_grid(strength.friction_angle()) {
            let q = ultimate_bearing_capacity_on_slope(strength, rho, g, Metres::new(pad), slope);
            capacity_line(out, &format!("pad {pad} m at {name}, Pa"), q);
        }
    }
}

/// The slope golden's text (the module documentation).
fn bearing_on_slope_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for (name, class, grain_density, g) in [
        ("lunar regolith", MaterialClass::Regolith, 3_100.0, MOON_G),
        ("Mars's drift", MaterialClass::Dust, 2_600.0, MARS_G),
        (
            "Viking Lander 2's soil",
            MaterialClass::Soil,
            2_600.0,
            MARS_G,
        ),
    ] {
        let soil = silicate(class, grain_density);
        out.line(&format!("table {name}, {class:?} of silicate"));
        out.f64("gravity_m_s2", g.value());
        out.f64("grain_density_kg_m3", grain_density);
        for layer in soil.shear() {
            out.line(&format!(
                "layer {} {}",
                hex(layer.top().value()),
                hex(layer.bottom().value())
            ));
            layer_lines(&mut out, layer.strength(), soil.bulk_density(*layer), g);
        }
        for pad in SLOPE_GOLDEN_PADS {
            let deep = soil
                .shear_layer_at(Metres::new(pad))
                .expect("a shear table has a layer at any depth");
            out.line(&format!(
                "the table under {pad} m, its layer at {} {}",
                hex(deep.top().value()),
                hex(deep.bottom().value())
            ));
            for (name, slope) in slope_grid(deep.strength().friction_angle()) {
                let q = bearing_capacity_on_slope(&soil, g, Metres::new(pad), slope);
                capacity_line(&mut out, &format!("pad {pad} m at {name}, Pa"), q);
            }
        }
    }
    out.line("a cohesionless soil of φ 30°, 1,600 kg m⁻³");
    out.f64("gravity_m_s2", CONSTRUCTED_SOIL_GRAVITY.value());
    layer_lines(
        &mut out,
        MohrCoulomb::new(
            Pascals::ZERO,
            Radians::new(CONSTRUCTED_SOIL_FRICTION_DEG.to_radians()),
        ),
        CONSTRUCTED_SOIL_DENSITY,
        CONSTRUCTED_SOIL_GRAVITY,
    );
    out.finish()
}

#[test]
fn material_the_bearing_capacity_on_slope_matches_its_golden() {
    golden!("material/bearing_on_slope", &bearing_on_slope_text());
}

/// The space-weathering law's golden text (the module documentation).
fn weathering_text() -> String {
    use crate::tables::space_weathering as table;
    const DENSITY: f64 = 3_000.0;
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line("constants");
    for (label, value) in [
        ("regolith_host_index", REGOLITH_HOST_INDEX),
        ("ice_host_index", ICE_HOST_INDEX),
        ("iron_density_kg_m3", IRON_DENSITY_KG_M3),
        ("iron_molar_mass_g_per_mol", IRON_MOLAR_MASS_G_PER_MOL),
        ("feo_molar_mass_g_per_mol", FEO_MOLAR_MASS_G_PER_MOL),
        ("regolith_lambda_m", table::REGOLITH_LAMBDA_M),
        ("frost_lambda_m", table::FROST_LAMBDA_M),
    ] {
        out.f64(label, value);
    }
    out.line(
        "law: <law> a0 <fresh> f <FeO_T> m <maturity> nm <wavelength>: tau, then A_N, A_S and A_h \
         at the four incidences from a fresh a0 each, or `same` where all seven are a0's bits",
    );
    let mut line = String::new();
    for law in PhaseLaw::ALL.iter().copied() {
        for fresh in [0.05, 0.2, 0.667, 1.2] {
            for feo in [0.0, 0.05, 0.2] {
                for maturity in [0.0, 0.5, 1.0] {
                    for wavelength in [250.0, 440.0, 550.0, 640.0, 2_200.0, 5_200.0] {
                        let tau = npfe_optical_depth(law, wavelength, DENSITY, feo, maturity);
                        let normal =
                            weathered_normal_albedo(fresh, wavelength, law, DENSITY, feo, maturity);
                        let sphere = weathered_spherical_albedo(
                            fresh, wavelength, law, DENSITY, feo, maturity,
                        );
                        let plane = weathered_hemispherical_albedos(
                            [fresh; 4], wavelength, law, DENSITY, feo, maturity,
                        );
                        line.clear();
                        write!(
                            line,
                            "{law:?} a0 {fresh} f {feo} m {maturity} nm {wavelength}: {}",
                            hex(tau)
                        )
                        .expect("a string takes any write");
                        let albedos = [normal, sphere, plane[0], plane[1], plane[2], plane[3]];
                        // Where the law is the identity every albedo is the fresh one's own bits,
                        // which the line states in a word, and the bits it compares pin it.
                        if albedos.iter().all(|&a| bits(a) == bits(fresh)) {
                            line.push_str(" same");
                        } else {
                            for a in albedos {
                                write!(line, " {}", hex(a)).expect("a string takes any write");
                            }
                        }
                        out.line(&line);
                    }
                }
            }
        }
    }
    out.line("entries: <world> <entry> <key> m <maturity>: A_N in B, V and R");
    for &w in SyntheticWorld::ALL {
        let palette = world(w).header().palette();
        for (index, entry) in palette.entries().iter().enumerate() {
            for maturity in [0.5, 1.0] {
                let [b, v, r] = weathered_entry_bvr(entry, maturity);
                line.clear();
                write!(
                    line,
                    "{w:?} {index} {} m {maturity}: {} {} {}",
                    entry.substance,
                    hex(b),
                    hex(v),
                    hex(r)
                )
                .expect("a string takes any write");
                out.line(&line);
            }
        }
    }
    out.finish()
}

#[test]
fn material_the_weathering_law_matches_its_golden() {
    golden!("material/weathering", &weathering_text());
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// The material goldens, by name: one for each synthetic world, the property table's, the
    /// space-weathering law's, R10.T1.c's maturity's and halo's, and the slope capacities'
    /// (R10.T12).
    const MATERIAL_GOLDENS: [&str; 11] = [
        "bearing_on_slope",
        "ceres_like",
        "earth_like",
        "flat",
        "halo",
        "mars_like",
        "maturity",
        "moon_like",
        "one_crater",
        "table",
        "weathering",
    ];

    /// Every golden file of `tests/golden/material/` carries `GENERATOR_VERSION`, not R05's
    /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`, and the files
    /// are the named ones, so that a stray file fails as a missing one does
    /// (decision-r10-t16-contracts §4's rule, decision-r10-t12-leans §8).
    #[test]
    fn material_every_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("material");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = Vec::new();
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
                let stem = path.file_stem().and_then(|s| s.to_str());
                seen.push(stem.expect("a golden file's name is UTF-8").to_owned());
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, MATERIAL_GOLDENS, "the goldens in {}", dir.display());
    }
}
