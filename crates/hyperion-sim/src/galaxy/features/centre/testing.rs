//! A Milky Way galaxy and its centre, each built once, for tests (plan 09, P09.T24–T27).

use std::sync::OnceLock;

use crate::Seed;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;

use super::{CentreModel, CentreProfile};

/// The Milky Way fixture (`GalaxyParams::milky_way_like`) at seed `0x0926_0000`, built on first
/// use.
///
/// # Panics
///
/// Never: the fixture builds.
#[must_use]
pub fn milky_way_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
            .expect("the fixture builds")
    })
}

/// The centre of [`milky_way_galaxy`], built on first use.
///
/// # Panics
///
/// Never: the fixture's centre inverts.
#[must_use]
pub fn milky_way_centre() -> &'static CentreModel {
    static CENTRE: OnceLock<CentreModel> = OnceLock::new();
    CENTRE
        .get_or_init(|| CentreModel::new(milky_way_galaxy()).expect("the fixture's centre inverts"))
}

/// The Milky Way fixture's centre profile in its galaxy's potential
/// (`CentreProfile::from_params`), built on first use: what [`milky_way_centre`]'s profile is,
/// bit for bit, without building the galaxy.
///
/// # Panics
///
/// Never: the fixture's profile builds.
#[must_use]
pub fn milky_way_profile() -> &'static CentreProfile {
    static PROFILE: OnceLock<CentreProfile> = OnceLock::new();
    PROFILE.get_or_init(|| {
        CentreProfile::from_params(&GalaxyParams::milky_way_like()).expect("the fixture's profile")
    })
}
