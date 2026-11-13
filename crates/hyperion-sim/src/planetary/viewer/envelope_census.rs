//! The envelope column read whole, on `RANDOMIZE`'s draws (P14.T24.h.b, the lanes
//! `fix-dark-sub-neptune` and `fix-dark-sub-neptune-2`; `envelope_bond`'s "The column read
//! whole", held at 32): the warm soot-hazed sub-Neptune the planet viewer drew teal at 29 and near
//! black once its air came, its photometry against its disc twin; the census of what the hold
//! moves; and the census of the warm sub-Neptunes' hazes, soot and the bright haze, by kind.

use super::tests::{galaxy, grid};
use super::*;
use crate::planetary::derive::photometry::{Bands, PhaseTemplate};
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::Section;
use crate::planetary::testing::{in_shares, with_every_32_hold};

/// `RANDOMIZE`'s draw 2 of the universe of seed 0x4d2 (the server's test universe, which the
/// planet-variety lane's captures drew, 2026-11-06), as `draw_inputs` gives it at 31: a 10.0 M⊕
/// sub-Neptune 0.0311 au from a 0.436 M☉ host of about 3,700 K, its `T_eq` 667 K, under Gao et
/// al.'s soot (τ(550) 2.2 between 0.1 and 0.27 Pa).
const DARK_SUB_NEPTUNE: RawPlanetInputs = RawPlanetInputs {
    seed: 15_988_379_025_882_168_482,
    star_mass_msun: 0.436_028_048_684_587_55,
    fe_h_dex: -0.042_617_489_251_125_85,
    age_gyr: 5.119_501_800_035_8,
    mass_mearth: 10.009_003_489_366_89,
    semi_major_axis_au: 0.031_061_166_153_550_04,
    eccentricity: 0.283_932_084_122_545_5,
    formed_at_au: Some(0.031_061_166_153_550_04),
    day_hours: None,
    tilt_deg: None,
    water_rank: None,
    carbon_rank: None,
    nitrogen_rank: None,
};

/// [`DARK_SUB_NEPTUNE`]'s disc as R08's disc twin draws it under 32's rules (the lane
/// `fix-dark-sub-neptune-3`'s measurement, 2026-11-13): the client's `test/discAirCases.ts`
/// (`bakedDiscAir`, `integratedRadianceAt` at phase 0 from 10⁹ m, `geometricAlbedoOf`, its table's
/// disc) on the record's air as `body_detail` sends it, assembled by `assembleMedium` under the
/// reference Sun, its upper medium drawn on its floor's datum. Its soot shares its column with the
/// bright haze (its zero-albedo `T_eq` 672 K, s 0.72): soot τ(550) 2.35, the bright haze's slab
/// 2.80 and its coagulated mode 12.41 beneath. Without that mode (the lane `fix-dark-sub-neptune-2`)
/// the table gave 0.0342, 0.0483 and 0.0553; with it, though it lies under the slab, the table's
/// floor rises from 6.8 bar to 390 Pa and it reads 0.38, 0.30 and 0.23 stops darker, where the
/// full passes' march ([`DARK_SUB_NEPTUNE_MARCH`]) moves by under 0.1 stop and the closed form not
/// at all. The same mode moved to 10–100 kPa, or of the slab's 0.1 µm particles, darkens the table
/// alike: the table reads layers the slab hides (a finding). With the soot alone (the lane
/// `fix-dark-sub-neptune`'s rules) it was 0.0224, 0.0166 and 0.0122, which the same harness gives
/// to within 0.0001; at 31 0.0226, 0.0169 and 0.0124.
const DARK_SUB_NEPTUNE_TWIN: Bands = Bands {
    b: 0.0263,
    v: 0.0393,
    r: 0.0471,
};

/// [`DARK_SUB_NEPTUNE`]'s disc as the client's full passes draw it under 32's rules, routed through
/// its thick table: [`DARK_SUB_NEPTUNE_TWIN`]'s harness's `routed` (the same measurement). Without
/// the coagulated mode it gave 0.0424, 0.0569 and 0.0647.
const DARK_SUB_NEPTUNE_MARCH: Bands = Bands {
    b: 0.0434,
    v: 0.0549,
    r: 0.0604,
};

/// How far above [`DARK_SUB_NEPTUNE_TWIN`] the draw's p lies in B and V under every 32 hold, in
/// stops, pinned as it stands (a finding: the table reads the layers its slab hides): +0.67 and
/// +0.39 measured, past the brief's third of a stop; R lies within a third.
const TWIN_EXCESS_AS_IT_STANDS: [f64; 2] = [0.70, 0.42];

/// The Bond albedo under which the owner's draw is still dark (research-leans-50: "stays dark
/// (`A_B` under about 0.1)"): a GJ 436 b or GJ 3470 b analogue, not a GJ 1214 b one.
const DARK: f64 = 0.1;

/// A third of a stop, the brief's agreement: a factor of 2^(1/3).
const THIRD_OF_A_STOP: f64 = 1.0 / 3.0;

/// The geometric albedo the record of `raw`'s planet publishes under the rules in force.
fn photometry_of(raw: RawPlanetInputs) -> Bands {
    let system = ViewerSystem::generate(PlanetInputs::try_from(raw).expect("in range"))
        .expect("a drawn planet's system");
    let index = BodyIndex::try_from(system.planet().body_index()).expect("a planet's index");
    let record = system
        .planets()
        .body_at(system.context(), index, UniverseTime::EPOCH)
        .expect("the drawn planet's record");
    let Section::Ok(photometry) = record.photometry() else {
        panic!("the drawn planet has its photometry");
    };
    photometry.geometric_albedo()
}

/// Each band's p over `twin`'s, in stops: log₂ of the ratio, B, V and R.
fn stops(p: Bands, twin: Bands) -> [f64; 3] {
    [p.b / twin.b, p.v / twin.v, p.r / twin.r].map(crate::math::log2)
}

/// The warm soot-hazed sub-Neptune's photometry meets its disc twin under every 32 hold: each of B,
/// V and R within a third of a stop of the client's full passes ([`DARK_SUB_NEPTUNE_MARCH`]), and
/// its table's disc ([`DARK_SUB_NEPTUNE_TWIN`]) in R, in B and V as it stands
/// ([`TWIN_EXCESS_AS_IT_STANDS`]); at 31 its R lies a stop under the twin: its column started below
/// its soot, which lies above 1 Pa, and the δ-Eddington held its dark layers at R = 0, so that its
/// p was Neptune's row scaled by a Bond albedo of 0.027 that the gas alone returned. From 32 its
/// soot keeps 0.72 of its column and the bright haze the rest (its zero-albedo `T_eq` 672 K), its q
/// weighed toward the bright haze's by that haze's share of the hazes' light, and it stays dark:
/// its Bond albedo under [`DARK`], 0.048 with the coagulated mode (research-leans-52).
#[test]
fn under_every_32_hold_the_dark_sub_neptune_meets_its_full_passes_and_stays_dark() {
    let held = photometry_of(DARK_SUB_NEPTUNE);
    let (whole, bond, zero_albedo) = with_every_32_hold(|| {
        let system =
            ViewerSystem::generate(PlanetInputs::try_from(DARK_SUB_NEPTUNE).expect("in range"))
                .expect("a drawn planet's system");
        let index = BodyIndex::try_from(system.planet().body_index()).expect("a planet's index");
        let record = system
            .planets()
            .body_at(system.context(), index, UniverseTime::EPOCH)
            .expect("the drawn planet's record");
        let (Section::Ok(photometry), Section::Ok(bulk)) = (record.photometry(), record.bulk())
        else {
            panic!("the drawn planet has its photometry and bulk");
        };
        let bond = photometry.bond_albedo().value();
        let t_eq = bulk.equilibrium_temperature().value();
        (
            photometry.geometric_albedo(),
            bond,
            t_eq / crate::math::powf(1.0 - bond, 0.25),
        )
    });
    let twin = DARK_SUB_NEPTUNE_TWIN;
    let (at_31, at_32) = (stops(held, twin), stops(whole, twin));
    let march = stops(whole, DARK_SUB_NEPTUNE_MARCH);
    eprintln!(
        "at 31: p {held:?}, {at_31:+.2?} stops; under every 32 hold: p {whole:?}, {at_32:+.2?} \
         stops (B, V, R) over the table's disc and {march:+.2?} over the full passes, its Bond \
         albedo {bond:.4}, its zero-albedo T_eq {zero_albedo:.1} K"
    );
    for (band, stop) in ["B", "V", "R"].into_iter().zip(march) {
        assert!(stop.abs() <= THIRD_OF_A_STOP, "{band}: {stop:+.3} stops");
    }
    for (stop, limit) in at_32.into_iter().zip(
        TWIN_EXCESS_AS_IT_STANDS
            .into_iter()
            .chain([THIRD_OF_A_STOP]),
    ) {
        assert!((-THIRD_OF_A_STOP..=limit).contains(&stop), "{at_32:?}");
    }
    assert!(at_31[2] < -2.0 * THIRD_OF_A_STOP, "{at_31:?}");
    assert!(bond < DARK, "{bond}");
    assert!((zero_albedo - 672.0).abs() < 1.0, "{zero_albedo}");
}

/// The draws the census takes: `RANDOMIZE`'s draw seeds 1 to 200 of the viewer tests' fixture.
const CENSUS_DRAWS: usize = 200;

/// The threads the census generates its draws on (its `threads-required` in
/// `.config/nextest.toml`).
const CENSUS_THREADS: usize = 16;

/// One body of a draw: its index, whether it is a gas envelope or one's satellite, the template of
/// that envelope (its own or its parent's), and its record's text under the rules in force.
struct Censused {
    index: BodyIndex,
    enveloped: Option<PhaseTemplate>,
    text: String,
}

/// One draw's bodies ([`Censused`]); `None` where the draw finds no planet.
fn bodies_of(seed: u64) -> Option<Vec<Censused>> {
    let drawn = draw_inputs(galaxy(), grid(), seed).ok()?;
    let system = ViewerSystem::generate(drawn.inputs).ok()?;
    let (ctx, planets) = (system.context(), system.planets());
    let mut out = Vec::new();
    for index in planets.indices() {
        let Ok(record) = planets.body_at(ctx, index, UniverseTime::EPOCH) else {
            continue;
        };
        // A gas envelope: a body drawn with a giant's template, which only a gas envelope takes
        // (`photometry::choose`), whether or not its record derives its envelope section.
        let enveloped = |record: &crate::planetary::record::BodyRecord| match record.photometry() {
            Section::Ok(photometry) => match photometry.template() {
                template @ (PhaseTemplate::Jupiter
                | PhaseTemplate::Saturn
                | PhaseTemplate::Uranus
                | PhaseTemplate::Neptune) => Some(template),
                PhaseTemplate::AirlessIce
                | PhaseTemplate::Moon
                | PhaseTemplate::Mercury
                | PhaseTemplate::Mars
                | PhaseTemplate::Earth
                | PhaseTemplate::Venus
                | PhaseTemplate::Magma
                | PhaseTemplate::Snowball => None,
            },
            Section::NotModelled | Section::NotApplicable | Section::NotResolved => None,
        };
        let parent = record.identity().parent().and_then(|host| match host {
            OrbitHost::Body(parent) => planets.body_at(ctx, parent, UniverseTime::EPOCH).ok(),
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => None,
        });
        out.push(Censused {
            index,
            enveloped: enveloped(&record).or_else(|| parent.as_ref().and_then(enveloped)),
            text: format!("{record:?}"),
        });
    }
    Some(out)
}

/// What the envelope column moves (its hold's census): over `RANDOMIZE`'s first 200 draws, every
/// body's record under every 32 hold against 31's, the movers counted. Only gas envelopes and the
/// satellites of one move, every other body bit for bit; the movers are stated by their
/// envelope's template (Jupiter's and Saturn's for the gas giants, Neptune's for the ice giants and
/// the sub-Neptunes).
#[test]
#[ignore = "slow: 200 viewer draws generated twice, some minutes"]
fn under_every_32_hold_the_envelope_column_moves_only_gas_envelopes_and_their_satellites() {
    let draws = |rules_32: bool| {
        let run = || {
            in_shares(CENSUS_DRAWS, CENSUS_THREADS, |k| {
                bodies_of(u64::try_from(k).expect("a share's index fits") + 1)
            })
        };
        if rules_32 {
            with_every_32_hold(run)
        } else {
            run()
        }
    };
    let (held, whole) = (draws(false), draws(true));
    let (mut bodies, mut envelopes) = (0_usize, 0_usize);
    let mut moved: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (seed, (a, b)) in held.iter().zip(&whole).enumerate() {
        let (Some(a), Some(b)) = (a, b) else {
            assert_eq!(a.is_none(), b.is_none(), "draw {}", seed + 1);
            continue;
        };
        assert_eq!(a.len(), b.len(), "draw {}: the same bodies", seed + 1);
        for (before, after) in a.iter().zip(b) {
            bodies += 1;
            envelopes += usize::from(before.enveloped.is_some());
            if before.text != after.text {
                let template = before.enveloped.unwrap_or_else(|| {
                    panic!(
                        "draw {} body {:?}: moved, but neither a gas envelope nor one's satellite",
                        seed + 1,
                        before.index
                    )
                });
                *moved.entry(format!("{template:?}")).or_default() += 1;
            }
        }
    }
    let total: usize = moved.values().sum();
    eprintln!(
        "{bodies} bodies; {envelopes} gas envelopes and their satellites, of which {total} moved, \
         by template {moved:?}"
    );
    assert!(total > 0, "the hold moves the census's envelopes");
}

/// The margin the haze census's edges take, K: a zero-albedo `T_eq` rebuilt from a record's `T_eq`
/// and Bond albedo, not its flux, as the record's own is.
const EDGE_MARGIN_K: f64 = 0.01;

/// One warm sub-Neptune of the census: its zero-albedo `T_eq` (K) and whether its record carries
/// soot and the bright haze.
struct WarmHazes {
    zero_albedo: f64,
    soot: bool,
    bright: bool,
}

/// The sub-Neptunes of one draw with a gas envelope section ([`WarmHazes`]); `None` where the draw
/// finds no planet.
fn sub_neptunes_of(seed: u64) -> Option<Vec<WarmHazes>> {
    let drawn = draw_inputs(galaxy(), grid(), seed).ok()?;
    let system = ViewerSystem::generate(drawn.inputs).ok()?;
    let (ctx, planets) = (system.context(), system.planets());
    let mut out = Vec::new();
    for index in planets.indices() {
        let Ok(record) = planets.body_at(ctx, index, UniverseTime::EPOCH) else {
            continue;
        };
        let (Section::Ok(bulk), Section::Ok(photometry), Section::Ok(envelope)) =
            (record.bulk(), record.photometry(), record.envelope())
        else {
            continue;
        };
        if bulk.class() != crate::planetary::derive::PlanetClass::SubNeptune {
            continue;
        }
        let has = |key: &str| {
            envelope
                .atmosphere()
                .modes()
                .iter()
                .any(|mode| mode.material().substance().key().as_str() == key)
        };
        out.push(WarmHazes {
            zero_albedo: bulk.equilibrium_temperature().value()
                / crate::math::powf(1.0 - photometry.bond_albedo().value(), 0.25),
            soot: has("soot"),
            bright: has("tholin"),
        });
    }
    Some(out)
}

/// The warm sub-Neptunes' hazes by kind (the brief's census): over `RANDOMIZE`'s first 200 draws
/// under every 32 hold, every sub-Neptune whose record carries an envelope, counted by its
/// photochemical hazes of the soot's column. Soot keeps a zero-albedo `T_eq` above 600 K and the
/// bright haze one inside 400–700 K, as `envelope_atmosphere::haze_shares` gives them, and both
/// lie together only inside 600–700 K. The census's counts and ranges are printed.
#[test]
#[ignore = "slow: 200 viewer draws, some minutes"]
fn under_every_32_hold_the_warm_sub_neptunes_take_their_hazes_by_temperature() {
    let draws = with_every_32_hold(|| {
        in_shares(CENSUS_DRAWS, CENSUS_THREADS, |k| {
            sub_neptunes_of(u64::try_from(k).expect("a share's index fits") + 1)
        })
    });
    let mut kinds: std::collections::BTreeMap<&str, (usize, f64, f64)> =
        std::collections::BTreeMap::new();
    let mut warm = 0_usize;
    for (seed, found) in draws.iter().enumerate() {
        for body in found.iter().flatten() {
            // The record's zero-albedo T_eq is its flux's; rebuilt here from its T_eq and Bond
            // albedo, it may differ in its last digits, so the edges take a margin.
            let t = body.zero_albedo;
            if body.soot {
                assert!(
                    t > 600.0 - EDGE_MARGIN_K,
                    "draw {}: soot at {t:.1} K",
                    seed + 1
                );
            }
            if body.bright {
                assert!(
                    t > 400.0 - EDGE_MARGIN_K && t < 700.0 + EDGE_MARGIN_K,
                    "draw {}: the bright haze at {t:.1} K",
                    seed + 1
                );
            }
            let kind = match (body.soot, body.bright) {
                (true, true) => "soot and the bright haze",
                (true, false) => "soot alone",
                (false, true) => "the bright haze alone",
                (false, false) => "neither",
            };
            if (400.0..950.0).contains(&t) {
                warm += 1;
                let entry = kinds.entry(kind).or_insert((0, f64::INFINITY, 0.0));
                entry.0 += 1;
                entry.1 = entry.1.min(t);
                entry.2 = entry.2.max(t);
            }
        }
    }
    eprintln!(
        "{warm} warm sub-Neptunes (zero-albedo T_eq 400–950 K) of the first {CENSUS_DRAWS} draws, \
         by haze kind (count, coolest K, hottest K): {kinds:?}"
    );
    assert!(warm > 0, "the census finds warm sub-Neptunes");
}
