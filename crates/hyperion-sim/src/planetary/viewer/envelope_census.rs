//! The envelope column read whole, on `RANDOMIZE`'s draws (P14.T24.h.b, the lanes
//! `fix-dark-sub-neptune`, `fix-dark-sub-neptune-2` and `fix-dark-sub-neptune-3`;
//! `envelope_bond`'s "The column read whole", from 32): the warm soot-hazed sub-Neptune the planet
//! viewer drew teal at 29 and near black once its air came, its photometry against its disc twin;
//! and the census of the warm sub-Neptunes' hazes, soot and the bright haze, by kind.

use super::tests::{galaxy, grid};
use super::*;
use crate::planetary::derive::photometry::Bands;
use crate::planetary::record::Section;
use crate::planetary::testing::in_shares;

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

/// [`DARK_SUB_NEPTUNE`]'s geometric albedo in B, V and R as R08's disc twin draws its disc at 33.
///
/// Measured by the landing lane of train 33 (2026-11-18), on R08's disc table that halves its view
/// steps under a thin opaque slab (`thick/discBake.ts`): the client's `test/discAirCases.ts`
/// (`bakedDiscAir`, `integratedRadianceAt` at phase 0 from 10⁹ m, `geometricAlbedoOf`, its table's
/// disc) on the record's air as `body_detail` sends it, assembled by `assembleMedium` under the
/// reference Sun, its upper medium drawn on its floor's datum. Its soot shares its column with the
/// bright haze (its zero-albedo `T_eq` 672 K, s 0.72): soot τ(550) 2.35, the bright haze's slab
/// 2.80 and its coagulated mode 12.41 beneath. Before that fix the table's 48 steps fell across the
/// slab by chance, and with the mode, which lifts the table's floor from 6.8 bar to 390 Pa, it read
/// 0.0263, 0.0393 and 0.0471 (the lane `fix-dark-sub-neptune-3`, 2026-11-13), 0.67 and 0.39 stops
/// under the draw's p in B and V; it now meets the full passes' march inside the disc to about
/// 0.005 stop, and the march's air beyond the datum's limb, which the table's disc does not draw,
/// is the rest ([`DARK_SUB_NEPTUNE_MARCH`]). With the soot alone (the lane `fix-dark-sub-neptune`'s
/// rules) the old table gave 0.0224, 0.0166 and 0.0122.
const DARK_SUB_NEPTUNE_TWIN: Bands = Bands {
    b: 0.0409,
    v: 0.0512,
    r: 0.0555,
};

/// [`DARK_SUB_NEPTUNE`]'s disc as the client's full passes draw it from 32, routed through
/// its thick table: [`DARK_SUB_NEPTUNE_TWIN`]'s harness's `routed` (measured by the lane
/// `fix-dark-sub-neptune-3`, 2026-11-13, and reproduced unchanged at 33, 2026-11-18). Without the
/// coagulated mode it gave 0.0424, 0.0569 and 0.0647.
const DARK_SUB_NEPTUNE_MARCH: Bands = Bands {
    b: 0.0434,
    v: 0.0549,
    r: 0.0604,
};

/// The Bond albedo under which the owner's draw is still dark (research-leans-50: "stays dark
/// (`A_B` under about 0.1)"): a GJ 436 b or GJ 3470 b analogue, not a GJ 1214 b one.
const DARK: f64 = 0.1;

/// A third of a stop, the brief's agreement: a factor of 2^(1/3).
const THIRD_OF_A_STOP: f64 = 1.0 / 3.0;

/// Each band's p over `twin`'s, in stops: log₂ of the ratio, B, V and R.
fn stops(p: Bands, twin: Bands) -> [f64; 3] {
    [p.b / twin.b, p.v / twin.v, p.r / twin.r].map(crate::math::log2)
}

/// The warm soot-hazed sub-Neptune's photometry meets its disc twin: each of B, V and R within a
/// third of a stop of the client's full passes ([`DARK_SUB_NEPTUNE_MARCH`]) and of its table's disc
/// ([`DARK_SUB_NEPTUNE_TWIN`]; +0.03, +0.01 and −0.13 stops at 33); to 31
/// its R lay a stop under the twin: its column started below its soot, which lies above 1 Pa, and
/// the δ-Eddington held its dark layers at R = 0, so that its p was Neptune's row scaled by a Bond
/// albedo of 0.027 that the gas alone returned. Its soot keeps 0.72 of its column and the bright
/// haze the rest (its zero-albedo `T_eq` 672 K), its q weighed toward the bright haze's by that
/// haze's share of the hazes' light, and it stays dark: its Bond albedo under [`DARK`], 0.048 with
/// the coagulated mode (research-leans-52).
#[test]
fn the_dark_sub_neptune_meets_its_full_passes_and_stays_dark() {
    let (whole, bond, zero_albedo) = {
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
    };
    let twin = stops(whole, DARK_SUB_NEPTUNE_TWIN);
    let march = stops(whole, DARK_SUB_NEPTUNE_MARCH);
    eprintln!(
        "p {whole:?}, {twin:+.2?} stops (B, V, R) over the table's disc and {march:+.2?} over the \
         full passes, its Bond albedo {bond:.4}, its zero-albedo T_eq {zero_albedo:.1} K"
    );
    for (band, stop) in ["B", "V", "R"].into_iter().zip(march) {
        assert!(stop.abs() <= THIRD_OF_A_STOP, "{band}: {stop:+.3} stops");
    }
    for (band, stop) in ["B", "V", "R"].into_iter().zip(twin) {
        assert!(
            stop.abs() <= THIRD_OF_A_STOP,
            "{band}: {stop:+.3} stops over the table's disc"
        );
    }
    assert!(bond < DARK, "{bond}");
    assert!((zero_albedo - 672.0).abs() < 1.0, "{zero_albedo}");
}

/// The draws the census takes: `RANDOMIZE`'s draw seeds 1 to 200 of the viewer tests' fixture.
const CENSUS_DRAWS: usize = 200;

/// The threads the census generates its draws on (its `threads-required` in
/// `.config/nextest.toml`).
const CENSUS_THREADS: usize = 16;

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

/// The warm sub-Neptunes' hazes by kind (the brief's census): over `RANDOMIZE`'s first 200 draws,
/// every sub-Neptune whose record carries an envelope, counted by its
/// photochemical hazes of the soot's column. Soot keeps a zero-albedo `T_eq` above 600 K and the
/// bright haze one inside 400–700 K, as `envelope_atmosphere::haze_shares` gives them, and both
/// lie together only inside 600–700 K. The census's counts and ranges are printed.
#[test]
#[ignore = "slow: 200 viewer draws, some minutes"]
fn the_warm_sub_neptunes_take_their_hazes_by_temperature() {
    let draws = in_shares(CENSUS_DRAWS, CENSUS_THREADS, |k| {
        sub_neptunes_of(u64::try_from(k).expect("a share's index fits") + 1)
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
