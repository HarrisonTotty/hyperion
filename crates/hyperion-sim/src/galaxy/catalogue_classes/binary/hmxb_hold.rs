//! The hold on the X-ray binaries' high-mass side (P11.T8.b's fix, at [`HIGH_MASS_SIDE_VERSION`];
//! main's ruling of 2026-11-01 that its three hosts against Fortin et al.'s 152 are a wrong state).
//!
//! Plan 15's task `binary_xrb` scales the class's row to the brainstorm's 10⁴ hosts and weights
//! its kinds by the engine's census shares of the whole class (`hyperion-fit`'s `kind_weights`), so
//! the high-mass side, the young thin disc's, takes the engine's share of all its members: 8.6 ×
//! 10⁻⁴, 7.56 × 10⁻⁴ once the black-hole transients are pinned. The engine makes about 1,160
//! low-mass members (most of them neutron-star transients) for each high-mass one, against the
//! specified 65 (the brainstorm's 10⁴ against the 152 that Fortin et al. 2023, A&A 671, A149,
//! catalogue; recalled, not read at source), so the row leaves the high-mass side 7.56 hosts, a
//! twentieth of the catalogued, of which the young features' lists hold 4.53 (0.60 of them) and
//! the class 3.03.
//!
//! From 32 each side of the class is pinned to its own observed count, as the black-hole
//! transients are (plan 11's design note 11: counts are observed, the engine gives their shape):
//! task `binary_xrb_32` fits [`XRB_SAMPLER_32`], whose high-mass kinds' weights sum to 152 of the
//! row's 10⁴, the black-hole transients' to 0.13 and the other low-mass kinds' to the rest, each
//! kind in its census share of its side ([`XrbTableRules::Sided`]). The galaxy's high-mass hosts,
//! the class's and the young features' lists' together, are then the 152 catalogued, a count
//! uncorrected for completeness and so a lower limit on the class's every host (lean,
//! 2026-11-10). The row, the
//! census and every other class are unchanged. Below 32 the class reads [`XRB_SAMPLER`]
//! ([`XrbTableRules::Joint`]), so that every generated value keeps its bits; the tests reach the
//! new arm through `GalaxyRules::every_32_hold()`.
//!
//! The bump to 32 deletes this hold: this module with its tripwire, the galaxy rules'
//! `xrb_tables` field and `every_32_hold`, and [`XRB_SAMPLER_32`] with task `binary_xrb_32`'s
//! block, manifests and lock entry. Task
//! `binary_xrb`'s manifest takes `high_mass_hosts = 152.0` and is refitted `--since 32`, which
//! gives its sampler items `XRB_SAMPLER_32`'s values.

use crate::GeneratorVersion;
use crate::tables::binary::{ClassSamplerTable, XRB_SAMPLER, XRB_SAMPLER_32, XrbMark, XrbPick};

/// The generator version from which the X-ray binaries' high-mass side is pinned to its observed
/// count (P11.T8.b's fix).
pub(crate) const HIGH_MASS_SIDE_VERSION: GeneratorVersion = GeneratorVersion::new(32);

/// The X-ray binaries' sampler a galaxy's class draws from (the galaxy rules' `xrb_tables` field,
/// the hold of [`HIGH_MASS_SIDE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum XrbTableRules {
    /// Version 31's and before: [`XRB_SAMPLER`], the kinds weighted by the engine's shares of the
    /// whole class, the black-hole transients pinned.
    Joint,
    /// Version 32's: [`XRB_SAMPLER_32`], each side pinned to its own observed count.
    Sided,
}

impl XrbTableRules {
    /// The arm at `version`: [`Sided`](Self::Sided) from [`HIGH_MASS_SIDE_VERSION`].
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= HIGH_MASS_SIDE_VERSION.get() {
            Self::Sided
        } else {
            Self::Joint
        }
    }

    /// The class's sampler: [`XRB_SAMPLER`], or from 32 [`XRB_SAMPLER_32`].
    #[must_use]
    pub(crate) const fn sampler(self) -> &'static ClassSamplerTable<XrbMark, XrbPick> {
        match self {
            Self::Joint => &XRB_SAMPLER,
            Self::Sided => &XRB_SAMPLER_32,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::galaxy::Galaxy;
    use crate::galaxy::catalogue_classes::binary::feature::catalogue_share;
    use crate::galaxy::catalogue_classes::binary::{XrayBinaries, XrbSide};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::rules::GalaxyRules;
    use crate::stellar::binary::CarvedClass;
    use crate::tables::binary::{BLACK_HOLE_TRANSIENT_SHARE, CLASS_SHARES, XrbHostKind};
    use crate::{GENERATOR_VERSION, Seed};

    /// Fortin et al.'s (2023, A&A 671, A149) catalogue of the Galaxy's high-mass X-ray binaries:
    /// 152 (recalled, not read at source), which task `binary_xrb_32`'s manifest pins the
    /// high-mass side to.
    const CATALOGUED_HIGH_MASS: f64 = 152.0;

    /// The Milky Way fixture under every 32 hold, built once.
    fn galaxy_32() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params_with_rules(
                Seed::new(0x0b17_08b0_0000_0032),
                GalaxyParams::milky_way_like(),
                GalaxyRules::every_32_hold(),
            )
            .expect("the Milky Way fixture builds")
        })
    }

    /// The hold is deleted by the bump to 32, with everything its module documentation lists.
    #[test]
    fn the_high_mass_side_is_held_only_until_the_32_bump() {
        assert!(
            GENERATOR_VERSION.get() < HIGH_MASS_SIDE_VERSION.get(),
            "the bump to 32 deletes the high-mass side's hold (module documentation)"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).xrb_tables,
            XrbTableRules::Joint
        );
        assert_eq!(
            GalaxyRules::every_32_hold().xrb_tables,
            XrbTableRules::Sided
        );
        assert_eq!(XrbTableRules::Joint.sampler(), &XRB_SAMPLER);
        assert_eq!(XrbTableRules::Sided.sampler(), &XRB_SAMPLER_32);
    }

    /// From 32 the class draws from [`XRB_SAMPLER_32`]: the galaxy's expected high-mass hosts, the
    /// class's and the young features' lists' together, are Fortin et al.'s 152 catalogued, its
    /// hosts still the row's 10⁴ and its black-hole transients 1,300; each side's kinds keep their
    /// shares of the side in 31's table, the census being the same. The marks are refitted with
    /// their members weighted to the new kinds' weights, so the pooled low-mass marks move too.
    #[test]
    fn from_32_the_high_mass_side_holds_the_catalogued_hosts() {
        let galaxy = galaxy_32();
        let process = XrayBinaries::new(galaxy);
        assert_eq!(process.sampler().table(), &XRB_SAMPLER_32);
        let row = CLASS_SHARES.row(CarvedClass::XrayBinary);
        let fields = galaxy.fields();
        let row_hosts_on = |side: XrbSide| -> f64 {
            process
                .components()
                .iter()
                .filter(|c| c.side() == side)
                .map(|c| {
                    let component = fields.component(c.id());
                    c.hosts_per_system() * component.count_with_unborn()
                        / catalogue_share(galaxy, row, component)
                })
                .sum()
        };
        let (low, high) = (
            row_hosts_on(XrbSide::LowMass),
            row_hosts_on(XrbSide::HighMass),
        );
        let catalogued = process.expected_hosts_on(galaxy, XrbSide::HighMass);
        println!(
            "from 32: {:.1} hosts, {high:.2} high-mass (the catalogue's {catalogued:.2}, a share \
             of {:.4}), against Fortin et al.'s {CATALOGUED_HIGH_MASS}",
            low + high,
            catalogued / high
        );
        assert!(((low + high) / 1.0e4 - 1.0).abs() < 1e-9, "{}", low + high);
        assert!((high / CATALOGUED_HIGH_MASS - 1.0).abs() < 1e-9, "{high}");
        assert!(catalogued > 0.0 && catalogued < high, "{catalogued}");
        let weights =
            |table: &ClassSamplerTable<XrbMark, XrbPick>, pick| table.pick(pick).weights().to_vec();
        let low_32 = weights(&XRB_SAMPLER_32, XrbPick::LowMassKind);
        let transients = XrbHostKind::LOW_MASS
            .iter()
            .position(|&k| k == XrbHostKind::BlackHoleTransient)
            .expect("a low-mass kind");
        let low_sum: f64 = low_32.iter().sum();
        let black_hole_transients = low * low_32[transients] / low_sum;
        assert!(
            (black_hole_transients / 1_300.0 - 1.0).abs() < 1e-9,
            "{black_hole_transients}"
        );
        assert!((low_32[transients] - BLACK_HOLE_TRANSIENT_SHARE).abs() < 1e-15);
        // Each side's kinds in the same ratios as 31's, the pinned transients apart.
        let ratios = |w: &[f64], skip: Option<usize>| -> Vec<f64> {
            let kept = |k: &usize| Some(*k) != skip;
            let sum: f64 = (0..w.len()).filter(kept).map(|k| w[k]).sum();
            (0..w.len()).filter(kept).map(|k| w[k] / sum).collect()
        };
        for (pick, skip) in [
            (XrbPick::LowMassKind, Some(transients)),
            (XrbPick::HighMassKind, None),
        ] {
            let (old, new) = (
                ratios(&weights(&XRB_SAMPLER, pick), skip),
                ratios(&weights(&XRB_SAMPLER_32, pick), skip),
            );
            for (a, b) in old.iter().zip(&new) {
                assert!(
                    (a - b).abs() <= 1e-12 * a.max(*b),
                    "{pick:?}: {old:?} {new:?}"
                );
            }
        }
        let share_32: f64 = weights(&XRB_SAMPLER_32, XrbPick::HighMassKind).iter().sum();
        assert!(
            (share_32 - CATALOGUED_HIGH_MASS / 1.0e4).abs() < 1e-15,
            "{share_32}"
        );
        // Under 31's rules the same galaxy's class reads 31's table, as a galaxy built under them
        // does: no memo of the galaxy holds the class, so `with_rules` may change its tables.
        let joint = galaxy
            .clone()
            .with_rules(GalaxyRules::of_version(GENERATOR_VERSION));
        assert_eq!(XrayBinaries::new(&joint).sampler().table(), &XRB_SAMPLER);
        let built = Galaxy::from_params(
            Seed::new(0x0b17_08b0_0000_0032),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture builds");
        assert_eq!(XrayBinaries::new(&joint), XrayBinaries::new(&built));
        assert_eq!(
            XrayBinaries::new(&built.with_rules(GalaxyRules::every_32_hold())),
            process
        );
    }
}
