//! The envelope column read whole, on `RANDOMIZE`'s draws (P14.T24.h.b, the lane
//! `fix-dark-sub-neptune`; `envelope_bond`'s "The column read whole", held at 32): the warm
//! soot-hazed sub-Neptune the planet viewer drew teal at 29 and near black once its air came, its
//! photometry against its disc twin; and the census of what the hold moves.

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

/// [`DARK_SUB_NEPTUNE`]'s disc as R08's disc twin draws it under 32's rules (the lane's
/// measurement, 2026-11-10): the client's `test/discAirCases.ts` (`bakedDiscAir`,
/// `integratedRadianceAt` at phase 0 from 10⁹ m, `geometricAlbedoOf`, its table's disc) on the
/// record's air as `body_detail` sends it, assembled by `assembleMedium` under the reference Sun
/// and drawn on its envelope's closed route above its floor. At 31 it is 0.0226, 0.0169 and 0.0124;
/// the full passes' march, unrouted, gives 0.0274, 0.0213 and 0.0154.
const DARK_SUB_NEPTUNE_TWIN: Bands = Bands {
    b: 0.0224,
    v: 0.0166,
    r: 0.0122,
};

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

/// Each band's p over the twin's, in stops: log₂ of the ratio, B, V and R.
fn stops(p: Bands) -> [f64; 3] {
    let twin = DARK_SUB_NEPTUNE_TWIN;
    [p.b / twin.b, p.v / twin.v, p.r / twin.r].map(crate::math::log2)
}

/// The warm soot-hazed sub-Neptune's photometry meets its disc twin under every 32 hold, each of B,
/// V and R within a third of a stop, where at 31 its V and B lie a stop above it: its column
/// started below its soot, which lies above 1 Pa, and the δ-Eddington held its dark layers at R
/// = 0, so that its p was Neptune's row scaled by a Bond albedo of 0.027 that the gas alone
/// returned.
#[test]
fn under_every_32_hold_the_dark_sub_neptune_meets_its_disc_twin() {
    let held = photometry_of(DARK_SUB_NEPTUNE);
    let whole = with_every_32_hold(|| photometry_of(DARK_SUB_NEPTUNE));
    let (at_31, at_32) = (stops(held), stops(whole));
    eprintln!(
        "at 31: p {held:?}, {at_31:+.2?} stops; under every 32 hold: p {whole:?}, {at_32:+.2?} \
         stops (B, V, R)"
    );
    for (band, stop) in ["B", "V", "R"].into_iter().zip(at_32) {
        assert!(stop.abs() <= THIRD_OF_A_STOP, "{band}: {stop:+.3} stops");
    }
    assert!(at_31[0] > 2.0 * THIRD_OF_A_STOP && at_31[1] > 2.0 * THIRD_OF_A_STOP);
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
