//! The golden detections of a pinned observer over a pinned century (plan 12, P12.T13): "what
//! went off?" (`alerts_in`, P12.T5.d) from the Sun-like point over the century after the epoch.
//!
//! The galaxy is the Milky Way fixture with its full potential and its global list, as the server
//! builds them, and the walk is the field's as built: the features' lists are not walked
//! (P09.T36). The sensor detects at 10⁻⁹ W m⁻² in V (magnitude 1.2) and 10⁻⁸ W m⁻² in
//! X-rays, resolving at a hundred times each, which keeps the accreting white dwarfs' walk
//! interactive (5,544 ly) and sends the supernova classes, whose reach is the whole cube, to the
//! background; the local rows are those of the observed 10 ly query about the observer at the
//! epoch. The golden holds the census of every class and source, and each detection's event,
//! host, class, kind, phase, band, emitted and arrival times, light age, flux, bearing,
//! resolvability and its light's knots, so that a change to the walks, the light curves, plan 07's
//! lines of sight or the arrival's closed form shows (Design notes 5–7).

use hyperion_sim::alerts::local::LocalEventsCache;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, DetectionLight, SensorHorizon, alerts_in,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::global_list::GlobalList;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, range_query_observed};
use hyperion_sim::observe::NoStarsCache;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The universe of the golden: the Milky Way fixture at this seed.
const SEED: u64 = 0x1213_0000_0000_0002;

/// The observer: the Sun-like point, ly.
const SUN: [f64; 3] = [0.0, 26_000.0, 20.0];

/// The century's length, Julian years from the epoch.
const CENTURY: i64 = 100;

/// The sensor's detection limits, W m⁻²: V, then X-rays (module documentation).
const LIMITS: [(AlertBand, f64); 2] = [
    (AlertBand::Photometric(Band::V), 1e-9),
    (AlertBand::XRay, 1e-8),
];

/// The local rows' radius, ly.
const LOCAL_RADIUS_LY: f64 = 10.0;

fn time_line(label: &str, t: UniverseTime) -> String {
    format!("{label} = {}s {}ns", t.seconds(), t.subsec_nanos())
}

/// P12.T13: the detections of the Sun-like observer over the century after the epoch.
#[test]
fn alerts_of_a_century_are_the_golden_detections() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential();
    let list = GlobalList::build(&galaxy);
    let sun = GalacticPosition::from_light_years(SUN).expect("inside the root cube");
    let around = RangeQuery::builder(sun, LightYears::new(LOCAL_RADIUS_LY))
        .mode(QueryMode::ObservedFrom(sun))
        .build()
        .expect("a query at the epoch");
    let rows = range_query_observed(
        &galaxy,
        &mut NoCache::new(),
        &mut NoStarsCache,
        &[],
        &around,
    )
    .expect("no centre member lies near the Sun");
    let end = UniverseTime::from_julian_years(CENTURY).expect("on the clock");
    let century = TimeWindow::new(UniverseTime::EPOCH, end).expect("a window forward");
    let w = WattsPerSquareMetre::new;
    let sensor = SensorHorizon::new(LIMITS.map(|(band, detect)| {
        let limits = BandLimits::new(w(detect), w(100.0 * detect)).expect("limits in order");
        (band, limits)
    }))
    .expect("two bands");
    let query = AlertQuery::builder(sun, century, sensor)
        .build()
        .expect("a query of the clock window");
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_global_list(&list);
    let result = alerts_in(
        &galaxy,
        &mut ctx,
        &query,
        rows.observed(),
        &mut NoStarsCache,
        &mut LocalEventsCache::default(),
    );

    let mut g = GoldenWriter::new();
    g.header(GENERATOR_VERSION.get());
    g.line(&format!(
        "{} local rows within {LOCAL_RADIUS_LY} ly",
        rows.observed().len()
    ));
    for census in result.census() {
        g.line(&format!("{} {:?}", census.class(), census.status()));
        g.f64(
            &format!("{}.reach_ly", census.class()),
            census.reach().value(),
        );
        if let Some(hosts) = census.expected_hosts() {
            g.f64(&format!("{}.expected_hosts", census.class()), hosts);
        }
    }
    for source in result.sources() {
        g.line(&format!("{:?} {:?}", source.source(), source.status()));
    }
    g.line(&format!("{} detections", result.detections().len()));
    for (i, d) in result.detections().iter().enumerate() {
        let class = d
            .class()
            .map_or_else(|| "none".to_owned(), |c| c.to_string());
        g.line(&format!(
            "d{i} = {} of {} ({class}) {:?} {:?} in {:?}, resolvable {}",
            d.event(),
            d.host(),
            d.kind(),
            d.phase(),
            d.band(),
            d.is_resolvable()
        ));
        g.line(&time_line(&format!("d{i}.emitted"), d.emitted()));
        g.line(&time_line(&format!("d{i}.arrival"), d.arrival()));
        let age = d.retardation().light_age();
        g.line(&format!(
            "d{i}.light_age = {}s {}ns",
            age.seconds(),
            age.subsec_nanos()
        ));
        g.f64(&format!("d{i}.flux_w_m2"), d.flux().value());
        let bearing = d.bearing();
        g.line(&format!("d{i}.bearing_frame = {:?}", bearing.frame()));
        g.f64(&format!("d{i}.azimuth_deg"), bearing.azimuth().value());
        g.f64(&format!("d{i}.elevation_deg"), bearing.elevation().value());
        match d.light() {
            DetectionLight::Lit(light) => {
                for (k, knot) in light.knots().into_iter().enumerate() {
                    g.line(&time_line(&format!("d{i}.knot{k}"), knot));
                }
            }
            DetectionLight::Unlit => g.line(&format!("d{i}.light = unlit")),
        }
    }
    golden!("alerts/century", g.as_str());
}
