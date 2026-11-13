//! The hold on the X-ray binaries' high-mass side (P11.T8.b's fix, at [`HIGH_MASS_SIDE_VERSION`];
//! main's ruling of 2026-11-01 that its three hosts against Fortin et al.'s 152 catalogued are a
//! wrong state).
//!
//! Plan 15's task `binary_xrb` scales the class's row to the brainstorm's 10⁴ hosts and weights
//! its kinds by the engine's census shares of the whole class (`hyperion-fit`'s `kind_weights`), so
//! the high-mass side, the young thin disc's, takes the engine's share of all its members: 8.6 ×
//! 10⁻⁴, 7.56 × 10⁻⁴ once the black-hole transients are pinned. The engine makes about 1,160
//! low-mass members (most of them neutron-star transients) for each high-mass one, against the
//! 2.3 the intrinsic 3,000 specify (65 against the 152 that Fortin et al. 2023, A&A 671, A149,
//! catalogue; recalled, not read at source), so the row leaves the high-mass side 7.56 hosts, a
//! twentieth of the catalogued, of which the young features' lists hold 4.53 (0.60 of them) and
//! the class 3.03.
//!
//! From 32 each side of the class is pinned to its own count, as the black-hole transients are
//! (plan 11's design note 11: counts measured and corrected for completeness, the engine giving
//! their shape): task `binary_xrb_32` fits [`XRB_SAMPLER_32`], whose high-mass kinds' weights sum
//! to 3,000 of the row's 10⁴ hosts, about the log-centre (10^3.5 ≈ 3,160) of the Galaxy's
//! intrinsic 10³–10⁴ Be/X-ray binaries (Rappaport and van den Heuvel 1982, IAU Symp. 98, 327;
//! recalled, not read at source), quiescent ones included, since a Be/X host needs no luminosity
//! (research-leans-51's ruling, 2026-11-13). Of them the supergiant hosts are pinned at 40 and the
//! black-hole high-mass hosts at 2, about the catalogued systems: Liu et al.'s (2006, A&A 455,
//! 1165) 32% of 114, about 36, supergiant fast X-ray transients included, a lower limit; and Cyg
//! X-1, the one confirmed, with a second allowed for Cyg X-3's Wolf–Rayet candidate or one hidden
//! behind the bulge (recalled). Those systems are bright and seen across the Galaxy, so that their
//! catalogued counts stand in for intrinsic ones. The Be/X hosts take the rest. The
//! black-hole transients' weights sum to 0.13 and the other low-mass kinds' to the rest, each kind
//! in its census share of its side ([`XrbTableRules::Sided`]), so that the other low-mass kinds
//! fall 33%, from 8,548 hosts with the side at the catalogued 152 to 5,700. The galaxy's high-mass
//! hosts are the class's and the young features' lists' together. With them, a Be/X host's giant
//! outbursts are its active mark's ([`XrbTableRules::marks_active_bex`], `scan::bex_active_mark`):
//! one host in a hundred, so that the galaxy's are about three a year, not 300. The row, the census
//! and every other class are unchanged. Below 32 the class reads [`XRB_SAMPLER`]
//! ([`XrbTableRules::Joint`]) and every Be/X host is active, with no mark drawn, so that every
//! generated value keeps its bits; the tests reach the new arm through
//! `GalaxyRules::every_32_hold()`.
//!
//! The bump to 32 deletes this hold: this module with its tripwire, the galaxy rules'
//! `xrb_tables` field and `every_32_hold`, and [`XRB_SAMPLER_32`] with task `binary_xrb_32`'s
//! block, manifests and lock entry; the scan then draws every Be/X host's active mark. Task
//! `binary_xrb`'s manifest takes `high_mass_hosts = 3000.0`, `supergiant_hosts = 40.0` and
//! `black_hole_high_mass_hosts = 2.0` and is refitted `--since 32`, which gives its sampler items
//! `XRB_SAMPLER_32`'s values.

use crate::GeneratorVersion;
use crate::tables::binary::{ClassSamplerTable, XRB_SAMPLER, XRB_SAMPLER_32, XrbMark, XrbPick};

/// The generator version from which the X-ray binaries' high-mass side is pinned to its intrinsic
/// count and a Be/X host's giant outbursts to its active mark (P11.T8.b's fix).
pub(crate) const HIGH_MASS_SIDE_VERSION: GeneratorVersion = GeneratorVersion::new(32);

/// The X-ray binaries' sampler a galaxy's class draws from (the galaxy rules' `xrb_tables` field,
/// the hold of [`HIGH_MASS_SIDE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum XrbTableRules {
    /// Version 31's and before: [`XRB_SAMPLER`], the kinds weighted by the engine's shares of the
    /// whole class, the black-hole transients pinned, and every Be/X host active.
    Joint,
    /// Version 32's: [`XRB_SAMPLER_32`], each side pinned to its own count, and a Be/X host active
    /// by its mark.
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

    /// Whether a Be/X host draws its active mark (`scan::bex_active_mark`), so that only one in a
    /// hundred has giant outbursts: from 32. Below, every Be/X host has them, and no mark is
    /// drawn.
    #[must_use]
    pub(crate) const fn marks_active_bex(self) -> bool {
        match self {
            Self::Joint => false,
            Self::Sided => true,
        }
    }
}

/// The galaxy's expected hosts on `side` under `process`, before the class's test and the young
/// lists' thinning by their members' ages: the class's and the young features' lists' together,
/// each component's catalogue hosts over the share of them the catalogue keeps. The tests' one
/// reading of the galaxy's high-mass hosts (here and `fitted_tables`).
#[cfg(test)]
pub(super) fn galaxy_hosts_on(
    galaxy: &crate::galaxy::Galaxy,
    process: &super::XrayBinaries,
    side: super::XrbSide,
) -> f64 {
    let row =
        crate::tables::binary::CLASS_SHARES.row(crate::stellar::binary::CarvedClass::XrayBinary);
    let fields = galaxy.fields();
    process
        .components()
        .iter()
        .filter(|c| c.side() == side)
        .map(|c| {
            let component = fields.component(c.id());
            c.hosts_per_system() * component.count_with_unborn()
                / super::feature::catalogue_share(galaxy, row, component)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

    use super::*;
    use crate::galaxy::Galaxy;
    use crate::galaxy::catalogue_classes::binary::{XrayBinaries, XrbSide};
    use crate::galaxy::catalogue_classes::testing::HIGH_MASS_XRB;
    use crate::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::rules::GalaxyRules;
    use crate::rng::ObjectKey;
    use crate::stellar::binary::events::{BEX_ACTIVE_SHARE, BEX_GIANT_OUTBURST_RATE_PER_YEAR};
    use crate::stellar::binary::scan::{XrbScanMarks, bex_active_mark, scan_cell};
    use crate::tables::binary::{BLACK_HOLE_TRANSIENT_SHARE, XrbHostKind};
    use crate::{GENERATOR_VERSION, Seed};

    /// The Galaxy's intrinsic high-mass X-ray binaries that task `binary_xrb_32`'s manifest pins
    /// the high-mass side to: 3,000, about the log-centre of the 10³–10⁴ Be/X-ray binaries of
    /// Rappaport and van den Heuvel (1982, IAU Symp. 98, 327; recalled, not read at source;
    /// research-leans-51).
    const PINNED_SIDE_HOSTS: f64 = 3_000.0;

    /// The high-mass kinds the manifest pins inside the side, with their hosts: the supergiant
    /// hosts' 40 and the black-hole high-mass hosts' 2, about the catalogued systems (Liu et al.
    /// 2006, A&A 455, 1165, about 36 supergiant ones; Cyg X-1 and a second allowed; recalled).
    const PINNED_PERSISTENT_KINDS: [(XrbHostKind, f64); 2] = [
        (XrbHostKind::Supergiant, 40.0),
        (XrbHostKind::BlackHoleHighMass, 2.0),
    ];

    /// The Galaxy's giant outbursts of Be/X-ray binaries a year that P11.T9.b's and P12.T5.c's
    /// rate is held inside: 1–10, the few a year MAXI and Swift/BAT see (recalled, rough;
    /// research-leans-51).
    const GIANT_OUTBURSTS_PER_YEAR: (f64, f64) = (1.0, 10.0);

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

    /// The galaxy's expected high-mass hosts of each high-mass kind under `process`, in
    /// [`XrbHostKind::HIGH_MASS`]' order: the side's hosts in its sampler's weights.
    fn galaxy_high_mass_kinds(galaxy: &Galaxy, process: &XrayBinaries) -> [f64; 3] {
        let high = galaxy_hosts_on(galaxy, process, XrbSide::HighMass);
        let weights = process
            .sampler()
            .table()
            .pick(XrbPick::HighMassKind)
            .weights();
        let sum: f64 = weights.iter().sum();
        std::array::from_fn(|k| high * weights[k] / sum)
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
        assert!(!XrbTableRules::Joint.marks_active_bex());
        assert!(XrbTableRules::Sided.marks_active_bex());
    }

    /// From 32 the class draws from [`XRB_SAMPLER_32`]: the galaxy's expected high-mass hosts, the
    /// class's and the young features' lists' together, are the intrinsic 3,000, the supergiant
    /// hosts 40 and the black-hole high-mass hosts 2 of them, the Be/X hosts the rest; its hosts
    /// are still the row's 10⁴ and its black-hole transients 1,300, and the other low-mass kinds
    /// keep their shares of their side in 31's table, the census being the same. The marks are
    /// refitted with their members weighted to the new kinds' weights, so the pooled marks move
    /// too.
    #[test]
    fn from_32_the_high_mass_side_holds_the_intrinsic_hosts() {
        let galaxy = galaxy_32();
        let process = XrayBinaries::new(galaxy);
        assert_eq!(process.sampler().table(), &XRB_SAMPLER_32);
        let low = galaxy_hosts_on(galaxy, &process, XrbSide::LowMass);
        let high = galaxy_hosts_on(galaxy, &process, XrbSide::HighMass);
        let catalogued = process.expected_hosts_on(galaxy, XrbSide::HighMass);
        let kinds = galaxy_high_mass_kinds(galaxy, &process);
        println!(
            "from 32: {:.1} hosts, {high:.2} high-mass (the catalogue's {catalogued:.2}, a share \
             of {:.4}; by kind {kinds:.2?}), against the intrinsic {PINNED_SIDE_HOSTS}",
            low + high,
            catalogued / high
        );
        assert!(((low + high) / 1.0e4 - 1.0).abs() < 1e-9, "{}", low + high);
        assert!((high / PINNED_SIDE_HOSTS - 1.0).abs() < 1e-9, "{high}");
        assert!(catalogued > 0.0 && catalogued < high, "{catalogued}");
        let pinned: f64 = PINNED_PERSISTENT_KINDS.iter().map(|&(_, h)| h).sum();
        for (k, kind) in XrbHostKind::HIGH_MASS.into_iter().enumerate() {
            let wanted = PINNED_PERSISTENT_KINDS
                .iter()
                .find(|&&(pinned_kind, _)| pinned_kind == kind)
                .map_or(PINNED_SIDE_HOSTS - pinned, |&(_, h)| h);
            assert!(
                (kinds[k] / wanted - 1.0).abs() < 1e-9,
                "{kind:?}: {} against {wanted}",
                kinds[k]
            );
        }
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
        // The other low-mass kinds in the same ratios as 31's, the pinned transients apart.
        let ratios = |w: &[f64]| -> Vec<f64> {
            let kept = |k: &usize| *k != transients;
            let sum: f64 = (0..w.len()).filter(kept).map(|k| w[k]).sum();
            (0..w.len()).filter(kept).map(|k| w[k] / sum).collect()
        };
        let (old, new) = (
            ratios(&weights(&XRB_SAMPLER, XrbPick::LowMassKind)),
            ratios(&low_32),
        );
        for (a, b) in old.iter().zip(&new) {
            assert!((a - b).abs() <= 1e-12 * a.max(*b), "{old:?} {new:?}");
        }
        let share_32: f64 = weights(&XRB_SAMPLER_32, XrbPick::HighMassKind).iter().sum();
        assert!(
            (share_32 - PINNED_SIDE_HOSTS / 1.0e4).abs() < 1e-15,
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

    /// From 32 the galaxy's Be/X giant outbursts a year, its Be/X hosts × [`BEX_ACTIVE_SHARE`] ×
    /// [`BEX_GIANT_OUTBURST_RATE_PER_YEAR`], are inside the few a year seen
    /// ([`GIANT_OUTBURSTS_PER_YEAR`]; P11.T9.b, P12.T5.c), where every host active would give some
    /// 300. The hosts drawn lie between the catalogue's expected Be/X hosts, which the class's
    /// test does not refuse (a Be/X host is marked detached, with no rate to floor), and those
    /// with the young lists' added before their thinning by their members' ages, which can only
    /// lower it; both bounds are inside.
    #[test]
    fn from_32_the_galaxy_s_be_x_giant_outbursts_are_a_few_a_year() {
        let galaxy = galaxy_32();
        let process = XrayBinaries::new(galaxy);
        let be_x = galaxy_high_mass_kinds(galaxy, &process)[0];
        let weights = process
            .sampler()
            .table()
            .pick(XrbPick::HighMassKind)
            .weights();
        let catalogue_be_x = process.expected_hosts_on(galaxy, XrbSide::HighMass) * weights[0]
            / weights.iter().sum::<f64>();
        let per_host = BEX_ACTIVE_SHARE * BEX_GIANT_OUTBURST_RATE_PER_YEAR;
        let (fewest, most_per_year) = (catalogue_be_x * per_host, be_x * per_host);
        let all_active = be_x * BEX_GIANT_OUTBURST_RATE_PER_YEAR;
        println!(
            "from 32: {catalogue_be_x:.1}–{be_x:.1} Be/X hosts, {fewest:.2}–{most_per_year:.2} \
             giant outbursts a year ({all_active:.0} with every host active)"
        );
        let (least, most) = GIANT_OUTBURSTS_PER_YEAR;
        for per_year in [fewest, most_per_year] {
            assert!((least..=most).contains(&per_year), "{per_year}");
        }
        assert!(all_active > most, "{all_active}");
    }

    /// The active mark accepts its share of hosts: over 2 × 10⁵ hosts' keys, [`BEX_ACTIVE_SHARE`]
    /// of them by chi-square at the project's α, and the same mark for the same key.
    #[test]
    fn the_active_mark_accepts_one_host_in_a_hundred() {
        let seed = Seed::new(0x0b17_08b0_0000_0032);
        let n = 200_000_u32;
        let active = (0..n)
            .filter(|&k| bex_active_mark(seed, ObjectKey::system(0x1234_0000_0000 + u64::from(k))))
            .count();
        let active = u64::try_from(active).expect("a count under 2⁶⁴");
        let total = f64::from(n);
        let fit = chi_square_gof(
            &[active, u64::from(n) - active],
            &[BEX_ACTIVE_SHARE * total, (1.0 - BEX_ACTIVE_SHARE) * total],
        );
        println!(
            "{active} of {n} active against {BEX_ACTIVE_SHARE}, p {:.3}",
            fit.p_value
        );
        assert_p_value(
            "the active share against BEX_ACTIVE_SHARE",
            fit.p_value,
            ALPHA,
        );
        let key = ObjectKey::system(0x1234_0000_0007);
        assert_eq!(bex_active_mark(seed, key), bex_active_mark(seed, key));
    }

    /// The X-ray binaries the plane's cells within some 8,000 ly of the centre hold under
    /// `galaxy`'s rules, drawn through `process`.
    fn plane_hosts(galaxy: &Galaxy, process: &XrayBinaries) -> Vec<XrbScanMarks> {
        let mut hosts = Vec::new();
        for x in -16..16 {
            for y in -16..16 {
                let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, 0])
                    .expect("a cell of the disc");
                scan_cell(galaxy, process, key, &mut hosts);
            }
        }
        hosts
    }

    /// From 32 a Be/X host is active exactly where its mark says, and no host of another kind is;
    /// under 31's rules every Be/X host is active, with no mark drawn: there drawn through
    /// [`HIGH_MASS_XRB`], since 31's class holds some three high-mass hosts in the galaxy.
    #[test]
    fn from_32_a_be_x_host_is_active_by_its_mark() {
        let galaxy = galaxy_32();
        let hosts = plane_hosts(galaxy, &XrayBinaries::new(galaxy));
        let be_x: Vec<&XrbScanMarks> = hosts
            .iter()
            .filter(|h| h.kind() == XrbHostKind::BeX)
            .collect();
        println!("{} hosts, {} Be/X", hosts.len(), be_x.len());
        assert!(be_x.len() >= 20, "{} Be/X hosts", be_x.len());
        for host in &hosts {
            let wanted = host.kind() == XrbHostKind::BeX
                && bex_active_mark(galaxy.seed(), host.host().key());
            assert_eq!(host.is_bex_active(), wanted, "{host:?}");
        }
        let joint = galaxy
            .clone()
            .with_rules(GalaxyRules::of_version(GENERATOR_VERSION));
        let hosts_31 = plane_hosts(&joint, &XrayBinaries::of_table(&joint, &HIGH_MASS_XRB));
        let be_x_31 = hosts_31
            .iter()
            .filter(|h| h.kind() == XrbHostKind::BeX)
            .count();
        assert!(be_x_31 >= 20, "{be_x_31} Be/X hosts at 31");
        for host in &hosts_31 {
            assert_eq!(
                host.is_bex_active(),
                host.kind() == XrbHostKind::BeX,
                "below 32 every Be/X host is active, and no other: {host:?}"
            );
        }
    }
}
