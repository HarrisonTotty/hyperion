//! The central black hole's own events: its X-ray flares and its tidal disruptions of stars (plan
//! 09, P09.T30.a).
//!
//! Both kinds are plan 06's Poisson bins ([`PoissonBins`](crate::events::PoissonBins)) on the black
//! hole's own event key. The subject is the black hole's ID, `0xF000_0007_0000_0000`
//! ([`CentreMemberId::CENTRAL_BLACK_HOLE`], Design note 23), and each kind has its event tag in
//! plan 09's block, so each is a pure function of the universe's seed, its tag and the time,
//! whatever was asked before:
//!
//! - [`flares`](mod@flares): Sgr A*-like X-ray flares, 1.1 a day, in bins of a day, under
//!   [`CENTRE_FLARE`](crate::id::event_tags::CENTRE_FLARE) (`0x0200`), each with a mean
//!   luminosity, a duration, an energy and a light curve (Neilsen et al. 2013);
//! - [`disruptions`](mod@disruptions): tidal disruptions at Stone and Metzger's (2016) rate for the black hole's
//!   mass, in bins of 1,024 years, under
//!   [`TIDAL_DISRUPTION`](crate::id::event_tags::TIDAL_DISRUPTION) (`0x0201`), each an ID and a
//!   time.
//!
//! Nothing reads them yet: P09.T30.b gives each disruption its victim and composes the black
//! hole's luminosity, and plan 12's alerts list both kinds.

pub mod disruptions;
pub mod flares;

pub use disruptions::{
    BuildTidalDisruptionModelError, TidalDisruption, TidalDisruptionModel, disruption_rate,
    tidal_disruption, tidal_disruptions,
};
pub use flares::{CentreFlare, CentreFlareModel, flare, flares, flares_active_at};

use crate::id::{CentreMemberId, EventSubject, SystemId};

/// The subject of the black hole's events: its own ID, `0xF000_0007_0000_0000`, the centre's
/// member 0 (Design note 23).
#[must_use]
pub fn subject() -> EventSubject {
    SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).into()
}

/// The Milky Way fixture's black hole, 4.30 × 10⁶ M☉, as the centre's member 0 reads it.
#[cfg(test)]
fn milky_way_mass() -> crate::units::SolarMasses {
    crate::galaxy::params::GalaxyParams::milky_way_like()
        .black_hole()
        .mass()
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::Seed;
    use crate::events::TimeWindow;
    use crate::time::{SourceHorizon, Span, UniverseTime};
    use crate::units::SolarMasses;

    /// The subject is the black hole's ID, member 0 of the centre's feature-level list.
    #[test]
    fn the_subject_is_the_black_hole_s_own_id() {
        assert_eq!(
            subject(),
            EventSubject::System(SystemId::from_raw(0xF000_0007_0000_0000).unwrap())
        );
        let model = CentreFlareModel::new(Seed::new(3));
        assert_eq!(model.series().subject(), subject());
        assert_eq!(
            TidalDisruptionModel::new(Seed::new(3), SolarMasses::new(4.3e6))
                .unwrap()
                .series()
                .subject(),
            subject()
        );
    }

    /// `of(&Galaxy)` reads the galaxy's seed and its black hole's mass.
    #[test]
    fn of_a_galaxy_reads_its_seed_and_its_black_hole() {
        let seed = Seed::new(0x0930_0000_0000_0001);
        let params = crate::galaxy::params::GalaxyParams::milky_way_like();
        let galaxy = crate::galaxy::Galaxy::from_params(seed, params).unwrap();
        assert_eq!(CentreFlareModel::of(&galaxy), CentreFlareModel::new(seed));
        assert_eq!(
            TidalDisruptionModel::of(&galaxy).unwrap(),
            TidalDisruptionModel::new(seed, milky_way_mass()).unwrap()
        );
    }

    /// Pinned: the flare law's template constant and the two inverse transforms at u = 0, ½ and
    /// 1 − 2⁻⁵³; the fixture's black-hole mass and the disruption rate at three masses; at the
    /// fixture's mass and one seed, every flare over three days (ID, onset, mean luminosity,
    /// duration, energy, peak luminosity, and the light curve at a quarter, half and three quarters
    /// of the flare) and every disruption in the source horizon (ID, time).
    #[test]
    fn centre_events_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.line("");
        w.line("# the flare law");
        w.f64("peak over mean", flares::peak_over_mean());
        for (name, u) in [
            ("0", 0.0),
            ("1/2", 0.5),
            ("1 - 2^-53", 1.0 - f64::EPSILON / 2.0),
        ] {
            w.f64(
                &format!("mean luminosity at u = {name} (W)"),
                flares::mean_luminosity_watts_at(u),
            );
            w.f64(
                &format!("duration at u = {name} (s)"),
                flares::duration_seconds_at(u),
            );
        }
        let mass = milky_way_mass();
        w.line("");
        w.line("# the disruption rate (per s)");
        w.f64("the fixture's black-hole mass (solar masses)", mass.value());
        for (name, m) in [
            ("1e5 solar masses", 1e5),
            ("the fixture's mass", mass.value()),
            ("1e8 solar masses", 1e8),
        ] {
            w.f64(
                &format!("at {name}"),
                disruption_rate(SolarMasses::new(m)).value(),
            );
        }
        let seed = Seed::new(0x0930_a0a0_0000_0001);
        w.line("");
        w.line("# flares over three days from the epoch");
        let flare_model = CentreFlareModel::new(seed);
        let days = TimeWindow::new(
            UniverseTime::EPOCH,
            UniverseTime::new(3 * flares::BIN_SECONDS, 0).unwrap(),
        )
        .unwrap();
        let mut listed = Vec::new();
        flares(&flare_model, days, &mut listed);
        for f in listed {
            w.line(&format!("{} at {}", f.id(), f.onset()));
            w.f64("  mean luminosity (W)", f.mean_luminosity_watts());
            w.f64("  duration (s)", f.duration_seconds());
            w.line(&format!("  duration {}", f.duration()));
            w.f64("  energy (J)", f.energy_joules());
            w.f64("  peak luminosity (W)", f.peak_luminosity_watts());
            for share in [0.25, 0.5, 0.75] {
                let t = f
                    .onset()
                    .checked_add(Span::from_seconds_f64(share * f.duration_seconds()).unwrap())
                    .unwrap();
                w.f64(
                    &format!("  luminosity at {share} of the flare (W)"),
                    f.luminosity_watts_at(t),
                );
            }
        }
        w.line("");
        w.line("# tidal disruptions in the source horizon at the fixture's mass");
        let model = TidalDisruptionModel::new(seed, mass).unwrap();
        let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
        let mut events = Vec::new();
        tidal_disruptions(&model, horizon, &mut events);
        for e in events {
            w.line(&format!("{} at {}", e.id(), e.time()));
        }
        golden!("galaxy/features/centre_events", w.as_str());
    }
}
