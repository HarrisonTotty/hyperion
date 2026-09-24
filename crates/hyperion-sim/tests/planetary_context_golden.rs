//! Golden values of plan 14's system context (P14.T1.d): what the planetary stage reads of real
//! systems of layers A, C and E at the solar circle, and of synthetic hosts.
//!
//! They pin how the context joins the stages above: the stars' masses and zero-age states, the
//! abundance, the age, the sphere of influence and the zones. A moved line is a change in one of
//! those stages or in the joining, which is a generator-version change. CI checks the same file on
//! 64-bit Arm and on wasm32.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::orbit::Eccentricity;
use hyperion_sim::planetary::SystemContext;
use hyperion_sim::planetary::context::SyntheticDraws;
use hyperion_sim::planetary::placement::OrbitHost;
use hyperion_sim::units::{AstronomicalUnits, Dex, Metres, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

const SEED: u64 = 0x0e14_0001_d000_901d;

/// The cell of `layer` at the solar circle, 26,000 ly out along +y in the plane, shifted by `step`
/// cells along x.
fn solar_cell(layer: Layer, step: i32) -> CellKey {
    let size = i32::try_from(layer.cell_size_ly()).unwrap();
    CellKey::new(layer, [step, 26_000 / size, 0]).unwrap()
}

/// The first `n` systems of `layer` along the row of cells from the solar circle.
fn records_of(galaxy: &Galaxy, layer: Layer, n: usize) -> Vec<SystemRecord> {
    let (mut found, mut cell) = (Vec::new(), Vec::new());
    for step in 0..1_000 {
        generate_cell(galaxy, solar_cell(layer, step), &mut cell);
        found.extend_from_slice(&cell);
        if found.len() >= n {
            found.truncate(n);
            return found;
        }
    }
    panic!("layer {layer:?} is empty near the Sun");
}

fn host_name(host: OrbitHost) -> String {
    match host {
        OrbitHost::Star(n) => format!("star {n}"),
        OrbitHost::Pair(k) => format!("pair {k}"),
        OrbitHost::Barycentre => "barycentre".to_owned(),
        OrbitHost::Body(index) => format!("body {:04x}", index.get()),
    }
}

fn write_context(w: &mut GoldenWriter, name: &str, context: &SystemContext) {
    w.line("");
    w.line(&format!(
        "{name}: {:?}, {} stars, {} zones",
        context.host_kind(),
        context.stars().len(),
        context.zones().len()
    ));
    w.u64_hex("id", context.id().raw());
    w.f64("fe_h", context.fe_h().value());
    w.f64("age_at_epoch_yr", context.age_at_epoch().value());
    w.f64("tidal_radius_m", context.tidal_radius().value());
    w.f64("strip_radius_m", context.strip_radius().value());
    for ((slot, star), zero_age) in context
        .hierarchy()
        .stars()
        .iter()
        .zip(context.stars())
        .zip(context.zone_stars())
    {
        let n = slot.body().body_index();
        w.line(&format!("star {n} {:?}", slot.kind()));
        w.f64(&format!("star {n} mass"), star.initial_mass().value());
        w.f64(
            &format!("star {n} zams_l"),
            zero_age.zams_luminosity.value(),
        );
        w.f64(&format!("star {n} zams_r"), zero_age.zams_radius.value());
        w.f64(
            &format!("star {n} disc_rank"),
            zero_age.disc_lifetime_rank.value(),
        );
    }
    for zone in context.zones() {
        let label = host_name(zone.host());
        match zone.inner() {
            Some(r) => w.f64(&format!("{label} inner_m"), r.value()),
            None => w.line(&format!("{label} inner none")),
        }
        match zone.outer() {
            Some(r) => w.f64(&format!("{label} outer_m"), r.value()),
            None => w.line(&format!("{label} outer none")),
        }
    }
}

#[test]
fn system_contexts_are_pinned() {
    let seed = Seed::new(SEED);
    let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for layer in [Layer::A, Layer::C, Layer::E] {
        for record in records_of(&galaxy, layer, 2) {
            let context = SystemContext::for_system(&galaxy, record.id()).unwrap();
            write_context(&mut w, &format!("layer {}", layer.letter()), &context);
        }
    }
    let key = solar_cell(Layer::C, 0);
    let id = |i| key.candidate_id(i).unwrap();
    let au = |x| Metres::from(AstronomicalUnits::new(x));
    let synthetic = [
        (
            "median sun",
            SystemContext::builder()
                .system(id(900))
                .star(SolarMasses::new(1.0))
                .age_at_epoch(Years::new(4.57e9)),
        ),
        (
            "drawn metal-poor dwarf",
            SystemContext::builder()
                .system(id(901))
                .star(SolarMasses::new(0.3))
                .fe_h(Dex::new(-0.8))
                .age_at_epoch(Years::new(1.1e10))
                .star_draws(SyntheticDraws::OfUniverse(seed)),
        ),
        (
            "binary with a brown dwarf",
            SystemContext::builder()
                .system(id(902))
                .binary(
                    SolarMasses::new(1.2),
                    SolarMasses::new(0.05),
                    au(8.0),
                    Eccentricity::new(0.35).unwrap(),
                )
                .fe_h(Dex::new(0.15))
                .age_at_epoch(Years::new(8.0e8))
                .star_draws(SyntheticDraws::OfUniverse(seed)),
        ),
    ];
    for (name, builder) in synthetic {
        write_context(&mut w, name, &builder.build().unwrap());
    }
    golden!("planetary/context", w.as_str());
}
