//! Plan 09's feature members in the census (rendering plan R06, R06.T16.a; Design note 10).
//!
//! From plan 09's P09.T2.c the field gives up to the features the share φ of each population that
//! sits in them, so the clusters', associations' and nurseries' stars are the features' members,
//! no longer field stars. The census takes them exactly where the galaxy's own field gives them up
//! ([`takes_feature_members`]): the sky and the field flip together, whatever generator version
//! holds φ, and a member is never counted twice, once as a member and once spread through the
//! field.
//!
//! A census's jobs each carry the same sources ([`SkyContext::sources`]): a server's are plan 09's
//! [`FeatureMemberSource`] over its shared feature caches (P09.T40.a). [`census_members`] takes
//! one shell's members of the shell's layer:
//!
//! - **Which members.** Those of the layer's mass band that may lie in the shell at some time from
//!   its outer edge's light time before the observer's time to it ([`SkySource`]). The source pads
//!   each feature by its own members' speed bound over that window (P09.T23.b), never by the cell
//!   rule, and draws only the candidates within the shell so padded.
//! - **Each member.** Its line is its epoch position at its own velocity, the cluster's bulk
//!   motion plus its own (P09.T10), from which it is found at its retarded time. It is the shell's
//!   if its apparent position, brought nearer by the farthest its layer's stars lie from their
//!   barycentres, lies between the shell's edges, so that no star of it is nearer than a shell
//!   done before it. It is always generated, from its own composition and multiplicity
//!   ([`MemberRecord::stars`]): it has no density component, so no envelope bound skips it, and a
//!   helium-rich member, which the brightness envelope does not bound, is never skipped.
//! - **Its stars.** As a grid system's, each measured at its retarded time
//!   ([`census_record`](super::census_record)'s path, a star past the cut with no extinction
//!   taking no sightline), and kept only nearer than the radius its layer is complete to towards
//!   its band texel once the census is final ([`Completeness::radius_at`]): the band's boundary,
//!   beyond which the band holds the layer's light.
//!
//! # The band and the caps
//!
//! They read the budget, each population's whole density, the members' included (decided at
//! R06.T16.a's re-validation as a lean, 2026-11-01): the members' light fainter than the cut and
//! beyond each cap reaches the band, and their counts brighter than the cut each cap's expected
//! count beyond, as the field's do, in expectation, spread as the field is rather than gathered at
//! the features. The listed members lie within the band's boundary and above the cut, where the
//! band holds no light, so no star's light is counted twice. A globular's unresolved light is
//! therefore in the band where the halo's is, not at the globular.

use core::num::NonZeroU32;

use crate::galaxy::Galaxy;
use crate::galaxy::features::members::MemberRecord;
use crate::galaxy::features::members::source::FeatureMemberSource;
use crate::galaxy::imf::MassBand;
use crate::math;
use crate::observe::{Drift, Observer, retarded};
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::cell::{Bound, CensusTallies, Counts, SkyStar, layer_offset_bound, system_stars};
use super::query::{CensusPlan, Completeness, Shell, SkyContext, SkyQuery, light_time};

/// A source of systems beyond the grid as the sky's census reads them (R06.T16.a): each with its
/// own composition, multiplicity and velocity, from which its stars and its line are drawn.
///
/// The census retards each member a source gives and keeps it only in its shell, so a source may
/// give more than the shell holds, never fewer. Its order is its own, but the census's merge is
/// total, so no answer depends on it. Plan 09's feature members are the one source until the
/// galactic centre's and the global list's join (plan 09's P09.T29 and plan 10's P10.T10.b, from
/// generator version 27), whose members move on orbits rather than lines.
pub trait SkySource {
    /// Gives `each` every member of `seen`'s shell's band whose light can reach `observer` from
    /// between the shell's inner and outer distances, of the members `seen`'s part holds: each
    /// that lies so at some time from the outer distance's light time before the observer's time
    /// to it, born by the observer's time. The parts of one number hold each member once between
    /// them.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        seen: ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(&MemberRecord),
    );
}

/// One of the parts a shell's members are drawn in, so that they can be several jobs (R06.T16.a's
/// cost): the `index`-th of `of`. Every member is in one part of each number of parts, so the
/// census of a shell is the same however many parts it is taken in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemberPart {
    index: u32,
    of: NonZeroU32,
}

impl MemberPart {
    /// The one part that holds every member.
    pub const WHOLE: Self = Self {
        index: 0,
        of: NonZeroU32::MIN,
    };

    /// Each of `of` parts, in order.
    pub fn all(of: NonZeroU32) -> impl Iterator<Item = Self> {
        (0..of.get()).map(move |index| Self { index, of })
    }

    /// Its place among its parts, from 0.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// How many parts it is one of.
    #[must_use]
    pub const fn of(self) -> NonZeroU32 {
        self.of
    }
}

impl SkySource for FeatureMemberSource<'_> {
    /// The members the source would draw for a sphere padded by each feature's own speed bound
    /// over the light's window, less the cells wholly inside `inner` so padded.
    ///
    /// # Panics
    ///
    /// If `outer`'s light time before the observer's time is off the clock, which no cap's is: an
    /// observer is within ±1,000 years of the epoch and a cap within the root cube's diagonal.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        (shell, part): ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(&MemberRecord),
    ) {
        let t = observer.time();
        let earliest = t
            .checked_sub(light_time(shell.2.value()))
            .expect("an observer's time less a cap's light time is on the clock");
        self.members_in_shell_over(
            galaxy,
            observer.position(),
            (earliest, t),
            (shell, (part.index, part.of.get())),
            each,
        );
    }
}

/// Whether the census takes plan 09's feature members in `galaxy`: exactly where its field gives
/// them up, the share φ of each population that sits in the features (plan 09's P09.T2.c, the
/// galaxy rules' `field_share`).
///
/// One switch for the sky and the field, so they flip together whatever generator version holds
/// φ: a server gives a census its feature sources where this holds ([`SkyContext::sources`]), and
/// none elsewhere, where the field keeps every population's whole budget and a member would be
/// counted twice. No version of its own: φ's hold's tripwire covers both.
#[must_use]
pub fn takes_feature_members(galaxy: &Galaxy) -> bool {
    galaxy.field_share().is_some()
}

/// The feature members' stars of `shell`, one of `plan`'s, of the members `part` holds, kept for
/// `query`, appended to `out` (which is not cleared); their counts are returned, for the caller to
/// add up with [`CensusTallies::add`] beside the shell's cells' (see the [module](self)
/// documentation).
///
/// Every member of the shell's layer that `ctx`'s sources give and the shell holds is generated
/// and tallied in it: `generated` per member whose state exists at its emitted time, `accepted`
/// per star kept. For a query with a cone, a member whose stars all lie outside the cone a plan
/// opens its cells by is not generated. A context with no sources gives nothing, and tallies that leave the members out
/// ([`CensusTallies::feature_members_absent`]), as its cells' do; one with them, tallies that say
/// they are in. The stars of every shell of a plan are its members' stars, each once: a census of
/// its shells done, merged ([`merge_shells`](super::merge_shells)), lists each where its cells'
/// stars are listed.
///
/// # Panics
///
/// If `shell` is not one of `plan`'s; as [`MemberRecord::stars`], for a member a source gives of
/// another galaxy.
pub fn census_members(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    plan: &CensusPlan,
    (shell, part): (Shell, MemberPart),
    query: &SkyQuery,
    out: &mut Vec<SkyStar>,
) -> CensusTallies {
    let mut counts = Counts::default();
    let sources = ctx.sources;
    if sources.is_empty() {
        return counts.tallies;
    }
    counts.tallies.note_feature_members();
    let layer = shell.layer();
    let Some(band) = MassBand::ALL.into_iter().find(|band| band.layer() == layer) else {
        // The brown dwarfs: no feature holds a member of theirs.
        return counts.tallies;
    };
    let (inner, outer) = plan.shell_span(shell);
    let observer = query.observer();
    let [x, y, z] = observer.position().to_light_years_f64();
    let from_centre = math::hypot(math::hypot(x, y), z);
    let offset = layer_offset_bound(galaxy, layer, LightYears::new(from_centre + outer.value()));
    let complete = plan.complete();
    let span = (inner, outer, offset);
    for source in sources {
        let seen = ((band, inner, outer + offset), part);
        source.members_seen(galaxy, observer, seen, &mut |member| {
            census_member(
                galaxy,
                ctx,
                member,
                query,
                (span, &complete),
                &mut counts,
                out,
            );
        });
    }
    counts.tallies
}

/// One member's stars for `query`, in the shell between `inner` and `outer` whose layer's stars
/// lie at most `offset` from their barycentres, each kept within the radius `complete` gives its
/// layer towards it.
fn census_member(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    member: &MemberRecord,
    query: &SkyQuery,
    ((inner, outer, offset), complete): ((LightYears, LightYears, LightYears), &Completeness),
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    let record = member.record();
    if query.exclude() == Some(record.id()) {
        return;
    }
    let observer = query.observer();
    let r = retarded(
        observer,
        &Drift::new(*record.epoch_position(), member.velocity()),
    );
    // Its nearest star, held at nought so that the first shell, from nought, holds a member about
    // the observer.
    let nearest = (observer
        .position()
        .distance_to(r.apparent_position())
        .value()
        / METRES_PER_LIGHT_YEAR
        - offset.value())
    .max(0.0);
    if nearest < inner.value() || nearest >= outer.value() {
        return;
    }
    if let Some(region) = query.cone_region() {
        let [x, y, z] = observer.position().to_light_years_f64();
        let [ax, ay, az] = r.apparent_position().to_light_years_f64();
        if !region.meets_ball([ax - x, ay - y, az - z], offset.value()) {
            return;
        }
    }
    let layer = record.layer();
    let stars = member.stars(galaxy);
    system_stars(
        galaxy,
        ctx,
        (record, &stars, &r),
        query,
        (Bound::Applied, &|apparent, d| {
            d < complete.radius_at(layer, apparent).value()
        }),
        counts,
        out,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    use hyperion_testkit::float::bits;

    use super::*;
    use crate::Seed;
    use crate::coords::{GalacticPosition, UnitVector};
    use crate::galaxy::ages::FieldShareRules;
    use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord, NoFeatureCache};
    use crate::galaxy::features::cluster::ClusterModel;
    use crate::galaxy::features::members::source::KeepInteriors;
    use crate::galaxy::features::members::{FeatureInterior, local_offset};
    use crate::galaxy::features::{FeatureId, FeatureKind, FeatureProcess};
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::CellKey;
    use crate::galaxy::query::LayerPads;
    use crate::galaxy::rules::GalaxyRules;
    use crate::id::{Layer, SystemId, SystemIdKind};
    use crate::sky::caps::{CAPPED_LAYERS, LayerCap};
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::census::cell::{CellOffsets, census_cell};
    use crate::sky::census::merge::{SkyCensus, merge_shells, sky_order};
    use crate::sky::census::query::{
        CellSlab, Cone, MAX_N_MAX, census_plan, census_plan_with_edges,
    };
    use crate::sky::luminosity::LuminosityTables;
    use crate::sky::testing::{SUN_LY, milky_way_dark_context, milky_way_envelope};
    use crate::time::UniverseTime;
    use crate::units::{Degrees, Magnitudes, Years};

    /// The sky fixture's universe (`galaxy::features::centre::testing::milky_way_galaxy`'s).
    const SEED: u64 = 0x0926_0000;

    /// The parts each shell's members are taken in by [`census`].
    const PARTS: NonZeroU32 = NonZeroU32::new(16).expect("not zero");

    /// Every 26 hold, with the field giving up φ whichever version holds it (P09.T2.c).
    fn phi_rules() -> GalaxyRules {
        GalaxyRules {
            field_share: FieldShareRules::Phi,
            ..GalaxyRules::every_26_hold_with_its_retargets()
        }
    }

    /// The fixture under `rules`, with its kinematic tables, so that its members move.
    fn fixture(rules: GalaxyRules) -> Galaxy {
        Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
            .expect("the fixture builds")
            .with_full_potential()
    }

    /// The fixture under [`phi_rules`]: its census takes the features' members.
    fn phi_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| fixture(phi_rules()))
    }

    /// The fixture under every 26 hold but φ's: its field keeps every population's whole budget.
    fn budget_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            fixture(GalaxyRules {
                field_share: FieldShareRules::None,
                ..phi_rules()
            })
        })
    }

    /// One job of a census: a slab's cells, or a part of a shell's members.
    enum Job {
        Cells(Box<CellSlab>),
        Members(Shell, MemberPart),
    }

    /// The census of `query` (whose caps are forced) in `galaxy` over every shell of its plan, its
    /// cells and, if `members`, its feature members through a [`FeatureMemberSource`] in
    /// [`PARTS`] parts a shell, on threads that take the jobs in turn, each with its own context,
    /// merged to the plan's whole completeness.
    fn census(galaxy: &Galaxy, query: &SkyQuery, members: bool) -> SkyCensus {
        let tables = LuminosityTables::dark(galaxy);
        let envelope = milky_way_envelope();
        let offsets = CellOffsets::build(galaxy);
        // A forced plan reads neither the tables nor the noise.
        let plan = census_plan(
            galaxy,
            &tables,
            envelope,
            query,
            &mut NoiseCache::with_capacity(0),
        );
        let mut jobs: Vec<Job> = Vec::new();
        for shell in plan.shells() {
            jobs.extend(MemberPart::all(PARTS).map(|part| Job::Members(shell, part)));
            jobs.extend(
                plan.shell_slabs(shell)
                    .map(|slab| Job::Cells(Box::new(slab))),
            );
        }
        let interiors = KeepInteriors::new(galaxy);
        let next = AtomicUsize::new(0);
        let parts = Mutex::new(Vec::new());
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
                    let sources: [&dyn SkySource; 1] = [&source];
                    let mut ctx = SkyContext {
                        tables: &tables,
                        envelope,
                        offsets: &offsets,
                        noise: NoiseCache::with_capacity(1 << 14),
                        cells: &NoSkyCellCache,
                        sources: if members { &sources } else { &[] },
                        modifiers: &NoModifiers,
                    };
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(k) else { break };
                        let mut stars = Vec::new();
                        let tallies = match job {
                            Job::Cells(slab) => {
                                let mut tallies: Option<CensusTallies> = None;
                                for key in slab.cells() {
                                    let cell =
                                        census_cell(galaxy, &mut ctx, key, query, &mut stars);
                                    match &mut tallies {
                                        Some(sum) => sum.add(&cell),
                                        None => tallies = Some(cell),
                                    }
                                }
                                let Some(tallies) = tallies else { continue };
                                tallies
                            }
                            Job::Members(shell, part) => census_members(
                                galaxy,
                                &mut ctx,
                                &plan,
                                (*shell, *part),
                                query,
                                &mut stars,
                            ),
                        };
                        parts
                            .lock()
                            .expect("no job panics")
                            .push((k, stars, tallies));
                    }
                });
            }
        });
        let mut parts = parts.into_inner().expect("no job panics");
        parts.sort_by_key(|(k, _, _)| *k);
        merge_shells(
            parts
                .into_iter()
                .map(|(_, stars, tallies)| (stars, tallies)),
            query.n_max(),
            plan.complete(),
        )
    }

    /// The builder of a query from `at` (ly) to `cut` in `galaxy`, listing up to [`MAX_N_MAX`].
    fn query_at(galaxy: &Galaxy, at: [f64; 3], cut: f64) -> crate::sky::census::SkyQueryBuilder {
        let position = GalacticPosition::from_light_years(at).expect("in the cube");
        let observer = Observer::new(position, UniverseTime::EPOCH).expect("on the clock");
        SkyQuery::builder(observer, Magnitudes::new(cut))
            .pads(LayerPads::of(galaxy))
            .n_max(NonZeroU32::new(MAX_N_MAX).expect("not zero"))
    }

    /// `query` with its caps forced to `radii`, ly, and every other layer's to nought.
    fn forced_to(query: SkyQuery, radii: &[(Layer, f64)]) -> SkyQuery {
        let radii: Vec<(Layer, LightYears)> = radii
            .iter()
            .map(|&(layer, r)| (layer, LightYears::new(r)))
            .collect();
        query
            .with_caps_forced_per_layer(&radii)
            .expect("valid caps")
    }

    /// A query from `at` (ly) to `cut` in `galaxy`, its caps forced to `radii`.
    fn forced(galaxy: &Galaxy, at: [f64; 3], cut: f64, radii: &[(Layer, f64)]) -> SkyQuery {
        forced_to(
            query_at(galaxy, at, cut).build().expect("a valid query"),
            radii,
        )
    }

    /// The Pleiades-like open cluster of the fixture: 2,135 M☉ at 164 Myr, 804 ly from the Sun,
    /// the nearest of its kind (surveyed 2026-11-01).
    fn pleiades_like(galaxy: &Galaxy) -> FeatureRecord {
        let sun = GalacticPosition::from_light_years(SUN_LY).expect("in the cube");
        FeatureCatalogue::near(galaxy, &sun, LightYears::new(1_000.0), &NoFeatureCache)
            .find(|feature| {
                feature.kind_at(Years::ZERO) == Some(FeatureKind::OpenCluster)
                    && ClusterModel::from_record(galaxy, feature).is_some_and(|model| {
                        (1e8..3e8).contains(&model.age().value())
                            && (1_000.0..5_000.0).contains(&model.mass().value())
                    })
            })
            .expect("the fixture's Pleiades-like cluster")
    }

    /// Plain caps of `radius` for every layer.
    fn caps_of(radius: f64) -> Vec<LayerCap> {
        CAPPED_LAYERS
            .iter()
            .map(|&layer| LayerCap::forced(layer, LightYears::new(radius)))
            .collect()
    }

    /// The members' stars of every shell of `plan`, each shell in `parts` parts, in the census's
    /// order, with their tallies added.
    fn members_of(
        galaxy: &Galaxy,
        plan: &CensusPlan,
        query: &SkyQuery,
        parts: NonZeroU32,
    ) -> (Vec<SkyStar>, CensusTallies) {
        let interiors = KeepInteriors::new(galaxy);
        let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
        let sources: [&dyn SkySource; 1] = [&source];
        let mut ctx = SkyContext {
            sources: &sources,
            ..milky_way_dark_context()
        };
        let (mut stars, mut tallies) = (Vec::new(), None::<CensusTallies>);
        for shell in plan.shells() {
            for part in MemberPart::all(parts) {
                let part = census_members(galaxy, &mut ctx, plan, (shell, part), query, &mut stars);
                match &mut tallies {
                    Some(sum) => sum.add(&part),
                    None => tallies = Some(part),
                }
            }
        }
        stars.sort_by(sky_order);
        (stars, tallies.expect("a plan of shells"))
    }

    /// The brute force of [`members_of`] for plain caps of `radius`: every member of every band
    /// placed within `radius` and a margin of the observer, each found at its retarded time and
    /// every star of it measured, kept within `radius`.
    fn every_member(galaxy: &Galaxy, query: &SkyQuery, radius: f64) -> Vec<SkyStar> {
        // Far above a member's drift over the light's travel and its stars' offsets.
        const MARGIN_LY: f64 = 10.0;
        let observer = query.observer();
        let centre = observer.position();
        let mut ctx = milky_way_dark_context();
        let mut counts = Counts::default();
        let (mut stars, mut members) = (Vec::new(), Vec::new());
        let reach = LightYears::new(radius + MARGIN_LY);
        for feature in FeatureCatalogue::near(galaxy, centre, reach, &NoFeatureCache) {
            let Some(interior) = FeatureInterior::of(galaxy, &feature) else {
                continue;
            };
            let local = local_offset(feature.position(), centre);
            for band in MassBand::ALL {
                for cell in interior.grid().cells_touching(&local, reach) {
                    interior.members_in_cell(galaxy, band, cell, &mut members);
                    for (member, _) in &members {
                        let record = member.record();
                        let drift = Drift::new(*record.epoch_position(), member.velocity());
                        let r = retarded(observer, &drift);
                        system_stars(
                            galaxy,
                            &mut ctx,
                            (record, &member.stars(galaxy), &r),
                            query,
                            (Bound::Ignored, &|_, d| d < radius),
                            &mut counts,
                            &mut stars,
                        );
                    }
                }
            }
        }
        stars.sort_by(sky_order);
        stars
    }

    /// Each star's system and index, and its V, extinction and distance by their bits.
    fn star_bits(stars: &[SkyStar]) -> Vec<(SystemId, u8, [u64; 3])> {
        stars
            .iter()
            .map(|s| {
                (
                    s.system(),
                    s.star().get(),
                    [s.v().value(), s.a_v().value(), s.distance().value()].map(bits),
                )
            })
            .collect()
    }

    /// Whether `star` is a feature member's.
    fn is_member(star: &SkyStar) -> bool {
        matches!(star.system().kind(), SystemIdKind::FeatureMember(_))
    }

    /// The census takes feature members exactly where the field gives them up, and then each
    /// member once (R06.T16.a), each property on one build of the fixture in turn:
    ///
    /// - the switch: no members under the held arm of φ, members under its arm;
    /// - a context's sources say whether a census's tallies leave the members out, its cells'
    ///   and its members' alike, and one without them takes no member;
    /// - inside the Pleiades-like cluster, to 12 ly at V 9, the members' stars of shells to 4, 8
    ///   and 12 ly, in one part or three, are those of one shell to 12 ly, bit for bit and tally
    ///   for tally, and those of a brute force over every member placed near, each star measured.
    #[test]
    fn the_census_takes_each_member_once_where_the_field_gives_them_up() {
        let galaxy = phi_galaxy();
        assert!(takes_feature_members(galaxy));
        let held = Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                field_share: FieldShareRules::None,
                ..phi_rules()
            },
        )
        .expect("the fixture builds");
        assert!(!takes_feature_members(&held));

        let cluster = pleiades_like(galaxy);
        let [cx, cy, cz] = cluster.position().to_light_years_f64();
        let query = forced(galaxy, [cx + 3.0, cy, cz], 9.0, &[]);
        let shelled = census_plan_with_edges(&query, caps_of(12.0), &[4, 8]);
        let whole = census_plan_with_edges(&query, caps_of(12.0), &[]);
        assert!(shelled.shells().count() > whole.shells().count());

        // A cell's tallies with the sources and without, and a member census without them.
        let interiors = KeepInteriors::new(galaxy);
        let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
        let sources: [&dyn SkySource; 1] = [&source];
        let key = CellKey::containing(Layer::C, query.observer().position()).expect("a cell");
        let mut out = Vec::new();
        let mut without = milky_way_dark_context();
        assert!(census_cell(galaxy, &mut without, key, &query, &mut out).feature_members_absent());
        let shell = whole.shells().next().expect("a shell");
        let none = census_members(
            galaxy,
            &mut without,
            &whole,
            (shell, MemberPart::WHOLE),
            &query,
            &mut out,
        );
        assert_eq!(none, CensusTallies::default());
        assert!(none.feature_members_absent());
        assert!(!out.iter().any(is_member), "no member without the sources");
        let mut with = SkyContext {
            sources: &sources,
            ..milky_way_dark_context()
        };
        assert!(!census_cell(galaxy, &mut with, key, &query, &mut out).feature_members_absent());

        let one = NonZeroU32::MIN;
        let three = NonZeroU32::new(3).expect("not zero");
        let (stars, tallies) = members_of(galaxy, &whole, &query, one);
        assert!(!tallies.feature_members_absent());
        assert!(
            stars.len() > 10,
            "{} members' stars in the cluster",
            stars.len()
        );
        assert!(stars.iter().all(is_member));
        for (plan, parts) in [(&shelled, one), (&shelled, three), (&whole, three)] {
            let (each, each_tallies) = members_of(galaxy, plan, &query, parts);
            assert_eq!(star_bits(&each), star_bits(&stars), "{parts} parts");
            for layer in Layer::ALL {
                let (got, want) = (each_tallies.layer(layer), tallies.layer(layer));
                assert_eq!(
                    (got.generated(), got.accepted(), got.without_photometry()),
                    (want.generated(), want.accepted(), want.without_photometry()),
                    "{layer:?}, {parts} parts"
                );
            }
        }
        assert_eq!(
            star_bits(&every_member(galaxy, &query, 12.0)),
            star_bits(&stars)
        );
    }

    /// The listed stars of `census` within `angle` (rad) of `axis` from `observer`: its members',
    /// then the field's.
    fn in_cone(
        census: &SkyCensus,
        observer: &Observer,
        axis: [f64; 3],
        angle: f64,
    ) -> (usize, usize) {
        let from = observer.position().to_light_years_f64();
        let cos = math::cos(angle);
        let (mut members, mut field) = (0, 0);
        for star in census.listed() {
            let to = star.apparent().to_light_years_f64();
            let d: [f64; 3] = core::array::from_fn(|k| to[k] - from[k]);
            let length = math::hypot(math::hypot(d[0], d[1]), d[2]);
            if (d[0] * axis[0] + d[1] * axis[1] + d[2] * axis[2]) / length >= cos {
                if is_member(star) {
                    members += 1;
                } else {
                    field += 1;
                }
            }
        }
        (members, field)
    }

    /// The Pleiades-like cluster of the fixture seen from 400 ly, towards the Sun from it, is a
    /// clump of bright stars (R06.T16.a): within twice its half-mass radius of its centre, its
    /// members brighter than V 6.5 outnumber the field's there at least three times over, and
    /// number at least six, as the real Pleiades' naked-eye stars do from 444 ly. The census is a
    /// cone's about the cluster, the field to 3,000 ly. Measured 2026-11-01: 107 members and 9
    /// field stars within 4.5° of the 2,135 M☉ cluster of 164 Myr, half-mass radius 15.9 ly.
    #[test]
    #[ignore = "slow: a cone's census of the field to 3,000 ly, some thousands of members"]
    fn a_pleiades_like_cluster_is_a_clump_of_bright_stars_from_400_ly() {
        let galaxy = phi_galaxy();
        let cluster = pleiades_like(galaxy);
        let model = ClusterModel::from_record(galaxy, &cluster).expect("a cluster");
        let at = cluster.position().to_light_years_f64();
        let towards: [f64; 3] = core::array::from_fn(|k| SUN_LY[k] - at[k]);
        let norm = math::hypot(math::hypot(towards[0], towards[1]), towards[2]);
        let u = towards.map(|c| c / norm);
        let observer_at: [f64; 3] = core::array::from_fn(|k| at[k] + 400.0 * u[k]);
        let axis = u.map(|c| -c);
        let angle = math::atan(2.0 * model.half_mass_radius().value() / 400.0);
        let cone = Cone::new(
            UnitVector::from_components(axis).expect("a direction"),
            Degrees::new(3.0_f64.max(1.5 * angle.to_degrees())),
        )
        .expect("a cone");
        let query = forced_to(
            query_at(galaxy, observer_at, 6.5)
                .cone(cone)
                .build()
                .expect("a valid query"),
            &[
                (Layer::A, 11.0),
                (Layer::B, 68.0),
                (Layer::C, 1_000.0),
                (Layer::D, 2_000.0),
                (Layer::E, 3_000.0),
            ],
        );
        let census = census(galaxy, &query, true);
        let (members, field) = in_cone(&census, query.observer(), axis, angle);
        eprintln!(
            "{:.0} M_sun at {:.0} Myr, half-mass radius {:.1} ly: within {:.2} deg, {members} \
             members and {field} field stars to V 6.5",
            model.mass().value(),
            model.age().value() / 1e6,
            model.half_mass_radius().value(),
            angle.to_degrees()
        );
        assert!(members >= 6, "{members} members brighter than V 6.5");
        assert!(
            members >= 3 * field.max(1),
            "{members} members against {field} field stars"
        );
    }

    /// The globular core's sky from its centre lists stars to V 6.5 within a factor of two of the
    /// brainstorm's about 4 × 10⁵, its 47 Tuc row (R06.T16.a).
    ///
    /// The count is set by the stars within some tens of light-years of the centre, so the core is
    /// the fixture's globular of at least 47 Tuc's mass whose central density is nearest 47 Tuc's:
    /// 7.8 × 10⁵ M☉ (Baumgardt and Hilker 2018, MNRAS 478, 1520), and log ρ₀ = 4.88 L☉ pc⁻³
    /// (Harris 1996, 2010 edition) at a mass-to-light ratio of about 2 in V (Baumgardt and Hilker 2018), 1.5 × 10⁵ M☉ pc⁻³ or
    /// 4.4 × 10³ M☉ ly⁻³. Measured 2026-11-01: the 1.33 × 10⁶ M☉ globular of 3,222 M☉ ly⁻³ lists
    /// 2.98 × 10⁵ (its census some 6,500 CPU-s on a pod of 16 vCPU), where the 7.6 × 10⁵ M☉ one
    /// nearest 47 Tuc's mass, of 1,581 M☉ ly⁻³, lists 1.24 × 10⁵.
    #[test]
    #[ignore = "slow: generates some 10⁶ members of a globular's core, some 6,500 CPU-s"]
    fn a_globular_cores_sky_lists_some_four_hundred_thousand_stars_to_v_6_5() {
        /// 47 Tuc's mass, M☉ (Baumgardt and Hilker 2018).
        const TUC_47_MSUN: f64 = 7.8e5;
        /// 47 Tuc's central density, M☉ ly⁻³: 10^4.88 L☉ pc⁻³ (Harris 1996, 2010 edition) at
        /// a mass-to-light ratio of 2 in V, over 34.6 ly³ a cubic parsec.
        const TUC_47_RHO0: f64 = 4.4e3;
        let galaxy = phi_galaxy();
        let (globular, model) = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .filter_map(|feature| {
                ClusterModel::from_record(galaxy, &feature).map(|model| (feature, model))
            })
            .filter(|(_, model)| model.mass().value() >= TUC_47_MSUN)
            .min_by(|a, b| {
                let off = |m: &ClusterModel| math::ln(m.central_density() / TUC_47_RHO0).abs();
                off(&a.1).total_cmp(&off(&b.1))
            })
            .expect("a globular as heavy as 47 Tuc");
        // Each layer's cap holds its stars brighter than V 6.5 from the centre: M dwarfs within
        // 10 ly, K dwarfs within 40 ly, the turn-off's and the giants' within the tidal radius.
        let query = forced(
            galaxy,
            globular.position().to_light_years_f64(),
            6.5,
            &[
                (Layer::A, 10.0),
                (Layer::B, 40.0),
                (Layer::C, 500.0),
                (Layer::D, 500.0),
                (Layer::E, 500.0),
            ],
        );
        let census = census(galaxy, &query, true);
        let stars = census.listed().len() + census.overflow().len();
        let members = census
            .listed()
            .iter()
            .chain(census.overflow())
            .filter(|star| is_member(star))
            .count();
        eprintln!(
            "{:.3e} M_sun, central density {:.0} M_sun/ly^3: {stars} stars to V 6.5, {members} \
             members",
            model.mass().value(),
            model.central_density()
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of stars, for comparison"
        )]
        let stars_f = stars as f64;
        assert!(
            (2e5..=8e5).contains(&stars_f),
            "{stars} stars to V 6.5 from the core"
        );
    }

    /// Near the Sun the census's count to V 6.5 is the count of the field and the members together
    /// (R06.T16.a, the band's and the caps' lean), against the same seed's census with the field
    /// the whole budget, φ's held arm, over the same caps: never below it by more than its Poisson
    /// error, so that the members make up what φ takes from the field, and never above it by more
    /// than its compound error, the features' members coming in clumps, each feature one Poisson
    /// event of as many stars as it lists (decided as a lean, 2026-11-01; near the Sun one cluster
    /// of 2.2 × 10⁴ M☉ at 485 ly lists 345 of the 365 members' stars). Measured 2026-11-01: 3,430
    /// stars with the members, 3,065 without, 3,242 of the budget.
    #[test]
    #[ignore = "slow: three censuses near the Sun to 1,000 ly"]
    fn near_the_sun_the_count_to_v_6_5_is_the_fields_and_the_members_together() {
        let radii = [
            (Layer::A, 11.0),
            (Layer::B, 68.0),
            (Layer::C, 250.0),
            (Layer::D, 500.0),
            (Layer::E, 1_000.0),
        ];
        let phi = phi_galaxy();
        let query = forced(phi, SUN_LY, 6.5, &radii);
        let together = census(phi, &query, true);
        let field = census(phi, &query, false).listed().len();
        let budget = census(
            budget_galaxy(),
            &forced(budget_galaxy(), SUN_LY, 6.5, &radii),
            false,
        )
        .listed()
        .len();
        let mut clumps: BTreeMap<FeatureId, u64> = BTreeMap::new();
        for star in together.listed() {
            if let SystemIdKind::FeatureMember(member) = star.system().kind() {
                *clumps.entry(FeatureId::new(member.feature())).or_default() += 1;
            }
        }
        let squares: u64 = clumps.values().map(|n| n * n).sum();
        let listed = together.listed().len();
        eprintln!(
            "to V 6.5: {listed} with the members ({clumps:?} by feature), {field} without, \
             {budget} of the budget"
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of stars, for comparison"
        )]
        let (listed, budget, squares) = (listed as f64, budget as f64, squares as f64);
        assert!(
            listed >= budget - 3.0 * budget.sqrt(),
            "{listed} against the budget's {budget}"
        );
        assert!(
            listed <= budget + 3.0 * (budget + squares).sqrt(),
            "{listed} against the budget's {budget}, the clumps' squares {squares}"
        );
    }
}
