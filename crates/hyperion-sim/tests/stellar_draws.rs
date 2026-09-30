//! Golden values of the per-star draws (plan 06, P06.T2).
//!
//! Every field is written under the name of the domain tag it is read from, so a field added
//! later under a new tag adds lines and changes none; a changed line means a stream, its word
//! order or its sampler moved, which is a generator-version change.

use hyperion_sim::coords::{CellSize, GenCell, UnitVector};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::rng::Mark;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn normal(w: &mut GoldenWriter, tag: &str, value: StandardNormal) {
    w.f64(tag, value.value());
}

fn normals(w: &mut GoldenWriter, tag: &str, values: &[StandardNormal]) {
    for (i, v) in values.iter().enumerate() {
        w.f64(&format!("{tag}[{i}]"), v.value());
    }
}

fn mark(w: &mut GoldenWriter, tag: &str, value: Mark) {
    w.u64_hex(tag, value.get());
}

fn direction(w: &mut GoldenWriter, tag: &str, value: UnitVector) {
    for (axis, c) in ["x", "y", "z"].into_iter().zip(value.components()) {
        w.f64(&format!("{tag}.{axis}"), c);
    }
}

fn write_draws(w: &mut GoldenWriter, d: &StarDraws) {
    normal(w, "star.eta", d.eta());
    w.f64("star.rotation", d.rotation().value());
    mark(w, "star.magnetism", d.magnetism());
    direction(w, "star.spin_axis", d.spin_axis());
    w.f64("star.disc_lifetime", d.disc_lifetime().value());
    mark(w, "star.stripped", d.stripped());
    mark(w, "star.remnant.type", d.remnant_type());
    mark(w, "star.remnant.fallback", d.remnant_fallback());
    normal(w, "star.remnant.mass", d.remnant_mass());
    normals(w, "star.kick.score", d.kick_score());
    mark(w, "star.kick.mode", d.kick_mode());
    normals(w, "star.kick.low", d.kick_low());
    direction(w, "star.kick.direction", d.kick_direction());
    mark(w, "star.wd.atmosphere", d.wd_atmosphere());
    mark(w, "star.wd.carbon", d.wd_carbon());
    mark(w, "star.wd.metals", d.wd_metals());
    normals(w, "star.ns.spin", d.ns_spin());
    normal(w, "star.ns.field", d.ns_field());
    direction(w, "star.ns.geometry.spin_axis", d.ns_spin_axis());
    w.f64("star.ns.geometry.inclination", d.ns_inclination().value());
    w.f64("star.ns.phase", d.ns_phase().value());
    normal(w, "star.bh.spin", d.bh_spin());
    w.f64("star.nebula", d.nebula().value());
}

/// The three pinned stars: a layer-A primary, a layer-E primary and its companion (body 1), each
/// at attempt 0, and the first at attempt 1.
#[test]
fn star_draws_are_pinned() {
    let a = SystemId::from_parts(
        Layer::A,
        GenCell::new(CellSize::Ly8, [652, -4_584, 2_047]).unwrap(),
        7,
    )
    .unwrap();
    let e = SystemId::from_parts(
        Layer::E,
        GenCell::new(CellSize::Ly128, [-40, 17, 0]).unwrap(),
        2,
    )
    .unwrap();
    let cases = [
        (Seed::new(0x0123_4567_89ab_cdef), BodyId::new(a, 0), 0),
        (Seed::new(42), BodyId::new(e, 0), 0),
        (Seed::new(42), BodyId::new(e, 1), 0),
        (Seed::new(0x0123_4567_89ab_cdef), BodyId::new(a, 0), 1),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (seed, star, attempt) in cases {
        w.line("");
        w.line(&format!("seed {seed} body {star} attempt {attempt}"));
        write_draws(&mut w, &StarDraws::for_attempt(seed, star, attempt));
    }
    golden!("stellar/star_draws", w.as_str());
}
