//! The water ice an ice bulk shows through a dark lag (plan 14, P14.T53.c's exposed share;
//! decision-p14-t54a-ganymede §C3.1 and §C3.4), in force from generator version 25
//! (decision-bump-plan-after-22 V3.9, W3 (a)).
//!
//! On a body whose bulk is ice under a lid that is not `H2O` (`cometary`, a chondrite,
//! `phyllosilicate`, a silicate), the lid's row is the surface its albedo reads, and the crust's
//! water stays in the bulk, unlisted ([`CrustWater::Unlisted`]). The cover's ice area there
//! ([`cover::ice_fraction`], uncapped for an ice-rich crust) is where the crust's ice is stable:
//! the ice table beneath the lag, not ice at the surface. Ice shows through such a lag where
//! something fresh exposes it (an impact, a slump, a collapsing cliff) or where frost gathers on
//! the coldest crests, until the lag regrows over it: dirty ice sublimes from the dark ground,
//! which keeps a lag just thick enough to throttle it, and condenses in bright patches (Spencer
//! 1987, Icarus 69, 297; Moore et al. 2004, "Callisto", in *Jupiter: The Planet, Satellites and
//! Magnetosphere*, Cambridge University Press, ch. 17, pp. 397–426). The share this balance leaves
//! exposed is small, and it is not the share of the surface cold enough for ice:
//!
//! - **Comets.** 67P's exposed water ice, over 600 bright spots catalogued from 2014 to 2016, came
//!   to 46,714 m², 0.1% of its nucleus (Fornasier et al. 2023, A&A 672, A136, §7); Tempel 1's lies
//!   in patches of 3–6% ice, far less than its outgassing needs (Sunshine et al. 2006, Science
//!   311, 1453), over 0.5% of its observed side (the University of Maryland's release 06014 of 2
//!   February 2006).
//! - **Callisto.** Bright frost on the crests over a dark, carbonaceous-chondrite-like lag: about
//!   10% of the surface is ice by its reflectance spectra (Spencer 1987, Icarus 70, 99), about 3%
//!   by its geometric albedo split between its bright and dark units (Moore et al. 2004).
//! - **Ceres.** Water ice shows at nine sites, each under 7 km², poleward of 30° in fresh craters
//!   near their rims' shadows (Combe et al. 2019, Icarus 318, 22), Juling's wall's 3.6–5.5 km²
//!   among them (Raponi et al. 2018, Sci. Adv. 4, eaao3757): about 10⁻⁵ of the surface, over a
//!   crust that holds ice beneath.
//!
//! So where the condensates list no `H2O` solid, the record's water ice under such a lid (W3 (a)'s
//! item, [`GlobalFigures::with_surface_under`]) is its lag's exposed share of the surface
//! ([`DarkLag::exposed_share`]), held to the cover's ice area, where alone the ice can last: an
//! empirical share by the lag's class until a lag budget (delivery, gardening and sublimation)
//! exists, with P14.T55.c, as the cold traps' exposed fraction is
//! ([`cold_traps::exposed_fraction`]). The measured shares are of their bodies' surfaces (Tempel
//! 1's of its observed side), and their ice lies in local cold spots (bright frost stays cold by
//! its albedo, Ceres's exposures by their craters' shadows), not in proportion to a zonal cold
//! area, so the share is not scaled by the cover's. Each class is calibrated on one or two bodies
//! (67P and Tempel 1, Callisto, Ceres), and its share elsewhere is an extrapolation until that
//! budget exists. It draws nothing. The passes' P14.T53.e rule 5 reads the same item ([`shown`]).
//! The crust and the condensates are unchanged, and the albedo still reads the lid's row alone:
//! the area mix does not weigh the exposed ice.
//!
//! **The belt test with it.** P14.T48.d's equatorial ice belt asks for ice over part of the
//! surface, not all of it ([`regime`]'s forcing). From generator version 25 it reads an ice cover
//! within [`WHOLE_COVER_ROUNDING`] of 1 as whole, so that a list the surface scales, which sums
//! back to 1 only to rounding (0.999 999 999 999 999 9), is not taken for part of the surface
//! (decision-p14-t54a-ganymede, "§C3.1 The count is 34").
//!
//! [`CrustWater::Unlisted`]: crate::planetary::chem::partition::CrustWater::Unlisted
//! [`cover::ice_fraction`]: super::cover::ice_fraction
//! [`GlobalFigures::with_surface_under`]: super::GlobalFigures::with_surface_under
//! [`cold_traps::exposed_fraction`]: crate::planetary::hooks::cold_traps::exposed_fraction
//! [`regime`]: crate::planetary::hooks::surface::regime

use crate::planetary::derive::atmosphere::SurfaceMaterial;
use crate::planetary::hooks::crust::CrustLithology;

#[cfg(test)]
mod tests;

/// The exposed share of a comet's dust mantle, [`DarkLag::Cometary`]: 10⁻³ of the surface.
///
/// It is 67P's 0.1%, its exposed water ice over the whole mission and nucleus, the bright spots
/// being partly ice (Fornasier et al. 2023, A&A 672, A136, §7). The comets' range runs from 10⁻⁴,
/// under Tempel 1's ice (patches over 0.5% of its observed side, University of Maryland release
/// 06014, at 3–6% ice, Sunshine et al. 2006, Science 311, 1453: 1.5–3 × 10⁻⁴ of that side), to
/// 10⁻², an order above 67P's detected 0.1%, whose bright spots are a few percent of its exposed
/// ice-enriched blocks (Fornasier et al. 2023, §9), and decision-p14-t54a-ganymede §C3.4's "under
/// 1%". Calibrated on two active nuclei a few km across, it is an extrapolation on every other body
/// with a `cometary` lid.
pub(crate) const COMETARY_EXPOSED_SHARE: f64 = 1e-3;

/// The exposed share of a carbonaceous lag, [`DarkLag::Carbonaceous`]: 0.1 of the surface.
///
/// It is Callisto's 10% areal ice by its reflectance spectra (Spencer 1987, Icarus 70, 99). The
/// range runs from 0.03, (0.22 − 0.2) ÷ (0.8 − 0.2): Callisto's whole-disc geometric albedo of
/// 0.22, the opposition effect included (Moore et al. 2004, §17.2.3, after Buratti 1991), split
/// linearly between its bright units' 0.8 and its dark units' 0.2 (§17.3), to 0.3, the least
/// ice-bearing (ice and rock mixed) area of Calvin and Clark's best fits (1991, Icarus 89, 305, as
/// Moore et al. 2004, §17.2.1, give them). Calibrated on Callisto alone, 2,410 km in radius, it
/// is an extrapolation on the small carbonaceous bodies the lid also covers, which keep less of
/// their sublimed water to frost their crests.
pub(crate) const CARBONACEOUS_EXPOSED_SHARE: f64 = 0.1;

/// The exposed share of any other rock lag, [`DarkLag::Rock`]: 10⁻⁵ of the surface.
///
/// It is Ceres's: some 30 km² of exposed ice over its 2.77 × 10⁶ km² (4π R² at its 469.72 km mean
/// radius, Park et al. 2019, Icarus 319, 812), the lane's reading within Combe et al. 2019's nine
/// sites, each under 7 km² (Icarus 318, 22), Juling's 5.5 km² among them. The range runs from
/// 1.3 × 10⁻⁶, Juling's 3.6 km² alone (Raponi et al. 2018, Sci. Adv. 4, eaao3757), to 2.3 × 10⁻⁵,
/// all nine sites at their 7 km² bound. Calibrated on Ceres alone, it is an extrapolation on every
/// other rock lid over an ice bulk.
pub(crate) const ROCK_EXPOSED_SHARE: f64 = 1e-5;

/// The rounding within which the belt test reads an ice cover as whole from generator version 25
/// (P14.T53.c): 10⁻¹², the sim's tests' bound on a list the surface scales, whose shares sum back
/// to 1 only to rounding ([`GlobalFigures::ice_fraction`](super::GlobalFigures::ice_fraction)).
pub(crate) const WHOLE_COVER_ROUNDING: f64 = 1e-12;

/// The dark lag over an ice bulk's ice, by the lid's class: what sets the share of its ice
/// exposed.
///
/// It is the lid over an ice bulk that never lost its ice, not P14.T56.g's rock lag of a heated
/// shell ([`SurfaceLag`](crate::planetary::derive::SurfaceLag)), whose surface is rock and takes
/// none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum DarkLag {
    /// A comet's dust mantle, `cometary`: an icy body's never-melted ice and dust, over which its
    /// ice shows in metre-scale patches.
    Cometary,
    /// A carbonaceous lag, `chondrite_ci` or `chondrite_cm`: an icy body's rock altered in place,
    /// Callisto's kind of dark material (carbonaceous-chondrite-like, Spencer 1987, Icarus 70, 99;
    /// Fe-serpentines of primitive carbonaceous chondrites' matrices, Calvin and Clark 1991,
    /// Icarus 89, 305), frosted on its crests.
    Carbonaceous,
    /// Any other rock: `phyllosilicate`, Ceres's, `chondrite_cv`, an ordinary or enstatite
    /// chondrite, a silicate or graphite, its ice exposed only where something fresh cuts it or a
    /// crater's shadow cold-traps it.
    Rock,
}

impl DarkLag {
    /// The lag over the body whose bulk is of `material` under the lid `lid` (the record's, its
    /// published pass's own, as [`CrustWater::of_record`] reads it): none off an ice bulk, and none
    /// under an `H2O` lid or where no lid is known, where the crust is its own ice and lists it;
    /// otherwise the lid's class.
    ///
    /// [`CrustWater::of_record`]: crate::planetary::chem::partition::CrustWater::of_record
    #[must_use]
    pub(crate) const fn of(material: SurfaceMaterial, lid: Option<CrustLithology>) -> Option<Self> {
        let Some(lid) = lid else {
            return None;
        };
        match (material, lid) {
            (SurfaceMaterial::Rock, _) | (SurfaceMaterial::Ice, CrustLithology::Water) => None,
            (SurfaceMaterial::Ice, CrustLithology::Cometary) => Some(Self::Cometary),
            (SurfaceMaterial::Ice, CrustLithology::ChondriteCi | CrustLithology::ChondriteCm) => {
                Some(Self::Carbonaceous)
            }
            (
                SurfaceMaterial::Ice,
                CrustLithology::Graphite
                | CrustLithology::Feldspathic
                | CrustLithology::Basalt
                | CrustLithology::IronRichBasalt
                | CrustLithology::Komatiite
                | CrustLithology::ReducedBasalt
                | CrustLithology::Granite
                | CrustLithology::Ultramafic
                | CrustLithology::Phyllosilicate
                | CrustLithology::ChondriteCv
                | CrustLithology::ChondriteH
                | CrustLithology::ChondriteL
                | CrustLithology::ChondriteLl
                | CrustLithology::ChondriteE,
            ) => Some(Self::Rock),
        }
    }

    /// The share of the surface that shows its ice through this lag, 0 to 1:
    /// [`COMETARY_EXPOSED_SHARE`], [`CARBONACEOUS_EXPOSED_SHARE`] or [`ROCK_EXPOSED_SHARE`].
    #[must_use]
    pub(crate) const fn exposed_share(self) -> f64 {
        match self {
            Self::Cometary => COMETARY_EXPOSED_SHARE,
            Self::Carbonaceous => CARBONACEOUS_EXPOSED_SHARE,
            Self::Rock => ROCK_EXPOSED_SHARE,
        }
    }
}

/// The water ice the record lists for T24.b's cover of ice area `ice`, 0 to 1, over a body under
/// the lag `lag` ([`DarkLag::of`]), where the condensates list no `H2O` solid (W3 (a)'s item): the
/// lag's exposed share, held to `ice`, and `ice` as given with no lag.
#[must_use]
pub(crate) fn shown(lag: Option<DarkLag>, ice: f64) -> f64 {
    match lag {
        Some(lag) => {
            let share = lag.exposed_share();
            if ice > share { share } else { ice }
        }
        None => ice,
    }
}

/// Whether the ice cover `ice`, 0 to 1, covers part of the surface but not all of it, as the belt
/// test asks ([`regime`](crate::planetary::hooks::surface::regime)): above 0, and below 1 by more
/// than [`WHOLE_COVER_ROUNDING`].
#[must_use]
pub(crate) fn covers_part(ice: f64) -> bool {
    ice > 0.0 && ice < 1.0 - WHOLE_COVER_ROUNDING
}
