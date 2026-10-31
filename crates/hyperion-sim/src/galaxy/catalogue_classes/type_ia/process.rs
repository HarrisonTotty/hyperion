//! The Type Ia class: the galaxy's recent thermonuclear supernovae, placed on 512 ly cells of their
//! own (plan 09, P09.T34.a).
//!
//! "Type Ia shells are a class of layer D, exactly as recent core collapse is a class of layer E"
//! (brainstorm, "Type Ia shells"): every primary is of 2.5–8 M☉ ([`PRIMARY_MIN`]), and the rate is
//! the delay-time distribution's, applied to each population's own history
//! ([`DelayTimeDistribution`]). The explosions are a Poisson process in space and time, which this
//! module places by thinning on the class grid of P09.T32 ([`ClassProcess`]), as P09.T33.a places
//! the core collapses. Unlike those they have no grid side to agree with: plan 11's binaries redraw
//! any explosion before +H (P09.T35), so the class's test is the supernova test alone, on
//! [`DeathMarks::of_type_ia`].
//!
//! # What the class follows
//!
//! Component c's systems, born and unborn, are its density `n_c(x)`, and each explodes as a Type
//! Ia at the rate the delay-time distribution gives its ages, `r_c` a year
//! ([`DelayTimeDistribution::rate_per_system_per_year`]): the component's layer-D budget, its density × its
//! layer-D share, weighted by its Type Ia rate per layer-D system. So the explosions at x over a
//! span of time are `Σ_c n_c(x) r_c` × the span, every population's. The rate is the epoch's over
//! the whole interval, whose few million years move no population's ages appreciably (P09.T18.b
//! reads the history at the delay itself). From generator version 26 the class follows the field
//! alone (plan 09, P09.T2.c): `r_c` is the field's, its layer-D field factor times the rate over
//! the field's ages (`DelayTimeDistribution::of_field`), so the features' and globulars' hosts
//! are theirs.
//!
//! # The envelope and a candidate
//!
//! A cell's bound is `Σ_c`, in component order, the component's density bound over the cell's
//! closed box ([`Fields::component_bounds`](crate::galaxy::fields::Fields::component_bounds)) ×
//! `r_c` × the Type Ia's thinning cap, W + L + 2H ([`SupernovaInterval::thinning_cap`]), W being
//! the Type Ia's window cap. A candidate, on its own streams (Design note 23):
//!
//! 1. draws its site uniformly in the cell and is thinned on `Σ_c n_c(x) r_c` there ÷ the bound,
//!    the same mark picking its component by the odds of those terms (`class.type_ia.site`);
//! 2. draws its progenitor delay first in that component, [`IaProgenitor::draw_in`], exploding in
//!    the interval (`class.type_ia.progenitor`): its age weighted by ψ, T uniform in whole
//!    seconds, its channel and its two stars;
//! 3. draws its velocity at the site from its component's law, plan 08's draw
//!    (`class.type_ia.velocity`), the one velocity its test and its entry read (rule 6);
//! 4. draws what its explosion leaves, [`IaLeftover::draw`] of its channel
//!    (`class.type_ia.leftover`).
//!
//! The candidates kept are a Poisson process of intensity `Σ_c n_c(x) r_c`, uniform in T over the
//! interval, each with its own component's delays.
//!
//! # The test
//!
//! A candidate is an entry if and only if [`claims`] holds on [`DeathMarks::of_type_ia`] of its
//! progenitor, leftover, site and velocity, in the smooth gas at its site
//! ([`Surroundings::TypeIa`]). Its `v_fast` is |v| + the survivor's speed from the site, at least
//! every member's speed, so the lifetime cap holds every member within 4,096 ly of the site over
//! the entry's lifetime and P09.T32's lookup needs no further rings. The entry's window is the one
//! the test cut ([`claimed_window`]).
//!
//! # The entry
//!
//! Member 0 is the system: the progenitor binary before T, then the supernova and what it leaves
//! at the site, by evaluation time ([`SupernovaEntry::state_at`]); it moves at the drawn velocity
//! throughout. Where the explosion leaves a survivor ([`IaLeftover::has_survivor`]), the entry has
//! member 1: the donor, flung out from T at its speed in an isotropic direction in the
//! progenitor's frame (`class.type_ia.survivor`), or a Type Iax's partly burnt white dwarf, which
//! takes no kick and moves on with member 0. Before T member 1 is the binary's own star, at member
//! 0's place: a consumer that lists members as systems lists it from T. Both members are
//! catalogue systems ([`SystemOrigin::CatalogueSystem`]) whose stars the stellar stage does not
//! model: plan 06 evolves single stars, not a white dwarf's binary nor its explosion, so its star
//! of member 0 would outlive the explosion. They answer for their stars with
//! [`ResolveSystemError::KindNotGenerated`](crate::galaxy::placement::ResolveSystemError).

use std::sync::Arc;

#[cfg(doc)]
use super::PRIMARY_MIN;
use super::{DelayTimeDistribution, IaLeftover, IaProgenitor};
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::bounds::CellBox;
use crate::galaxy::catalogue_classes::core_collapse::class_site_velocity;
use crate::galaxy::catalogue_classes::grid::{CatalogueMember, ClassProcess};
use crate::galaxy::catalogue_classes::ids::CatalogueCellKey;
use crate::galaxy::catalogue_classes::registry::ClassId;
#[cfg(doc)]
use crate::galaxy::catalogue_classes::supernova::claims;
use crate::galaxy::catalogue_classes::supernova::{
    DeathMarks, SupernovaEntry, SupernovaInterval, Surroundings, claimed_window,
};
use crate::galaxy::catalogue_classes::system::{CatalogueMotion, CatalogueSystem};
use crate::galaxy::displaced::ExplosionSite;
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::snr::{ShellEnvironment, ShellWindow};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{CatalogueSystemId, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::isotropic;
use crate::time::UniverseTime;
use crate::units::{MetresPerSecond, PerCubicLightYear, SolarMasses};

/// One of the galaxy's components with Type Ia, as the class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IaComponent {
    id: ComponentId,
    explosions_per_system: f64,
}

impl IaComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its Type Ia per system over the class's interval: its rate per system a year
    /// ([`DelayTimeDistribution::rate_per_system_per_year`]) × the interval's length in Julian years.
    #[must_use]
    pub const fn explosions_per_system(&self) -> f64 {
        self.explosions_per_system
    }
}

/// The Type Ia class's process (plan 09, P09.T34.a): the galaxy's recent Type Ia supernovae
/// (module documentation).
///
/// Built from the galaxy alone each time the production lookup visits the class: it holds the
/// galaxy's delay-time distribution (a quadrature per component and 48 stellar lifetimes, a few
/// milliseconds), the components with Type Ia and their rates, and the class's thinning interval.
///
/// # Examples
///
/// The class's cells about the galactic centre hold recent Type Ia, each resolving by its ID to
/// itself, with a second member wherever a survivor flies off:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::galaxy::snr::ShellEnvironment;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = TypeIaProcess::new(&galaxy);
/// let cap = galaxy.window_caps().cap(ShellEnvironment::TypeIa);
/// let key = CatalogueCellKey::new(ClassId::TYPE_IA, [0, 0, 0])?;
/// for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
///     assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///     assert!(entry.value().supernova().window().duration() <= cap);
///     assert_eq!(entry.members() == 2, entry.value().survivor().is_some());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeIaProcess {
    /// The galaxy's delay-time distribution, which the candidates' progenitors are drawn from,
    /// kept by the galaxy ([`Galaxy::type_ia_delays`]).
    delays: Arc<DelayTimeDistribution>,
    /// The components with Type Ia, in component order.
    components: Vec<IaComponent>,
    /// The class's thinning interval, at the Type Ia's window cap.
    interval: SupernovaInterval,
}

/// A Type Ia candidate the thinning kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeIaCandidate {
    id: CatalogueSystemId,
    site: GalacticPosition,
    component: ComponentId,
    progenitor: IaProgenitor,
    velocity: GalacticVelocity,
    leftover: IaLeftover,
}

/// A Type Ia entry's survivor, its member 1: the record and motion of the star the explosion left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeIaSurvivor {
    record: SystemRecord,
    motion: CatalogueMotion,
}

impl TypeIaSurvivor {
    /// Its record, of origin [`SystemOrigin::CatalogueSystem`], with member 1's ID, at its epoch
    /// position, its initial mass its own star's: the secondary's for a donor, the primary's for a
    /// Type Iax's partly burnt white dwarf.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// How it moves: with the progenitor until T, at the progenitor's velocity plus its own flight
    /// from T.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }
}

/// A Type Ia entry: a recent thermonuclear supernova (plan 09, P09.T34.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeIaEntry {
    record: SystemRecord,
    component: ComponentId,
    site: ExplosionSite,
    motion: CatalogueMotion,
    progenitor: IaProgenitor,
    leftover: IaLeftover,
    survivor: Option<TypeIaSurvivor>,
    marks: DeathMarks,
    supernova: SupernovaEntry,
}

impl TypeIaEntry {
    /// The record of member 0, the system, of origin [`SystemOrigin::CatalogueSystem`], at its
    /// epoch position, counted in layer D by its primary's initial mass.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The component the progenitor was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// Where and when it explodes.
    #[must_use]
    pub const fn site(&self) -> &ExplosionSite {
        &self.site
    }

    /// How member 0 moves: at its drawn velocity before T and after it.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// The progenitor, drawn delay first.
    #[must_use]
    pub const fn progenitor(&self) -> &IaProgenitor {
        &self.progenitor
    }

    /// What the explosion leaves.
    #[must_use]
    pub const fn leftover(&self) -> &IaLeftover {
        &self.leftover
    }

    /// The survivor, member 1, where the explosion leaves one.
    #[must_use]
    pub const fn survivor(&self) -> Option<&TypeIaSurvivor> {
        self.survivor.as_ref()
    }

    /// What the supernova test read of the explosion.
    #[must_use]
    pub const fn marks(&self) -> &DeathMarks {
        &self.marks
    }

    /// The supernova: its time, kind, window, progenitor and leftover.
    #[must_use]
    pub const fn supernova(&self) -> &SupernovaEntry {
        &self.supernova
    }
}

impl TypeIaProcess {
    /// The process in `galaxy`: its delay-time distribution, its components with Type Ia and the
    /// class's interval.
    ///
    /// # Panics
    ///
    /// Never: the Type Ia's window cap is at most `SHELL_WINDOW_CAP`, 4 Myr, whose interval is a
    /// few 10¹⁴ s.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        // The field's rates and ages: the budget's until P09.T2.c's version 26.
        let delays = Arc::clone(galaxy.type_ia_delays());
        let interval = SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::TypeIa);
        let years = interval.length().as_julian_years_f64();
        let components = galaxy
            .fields()
            .component_ids()
            .filter_map(|id| {
                let explosions =
                    galaxy.field_part(id, MassBand::D, delays.rate_per_system_per_year(id) * years);
                (explosions > 0.0).then_some(IaComponent {
                    id,
                    explosions_per_system: explosions,
                })
            })
            .collect();
        Self {
            delays,
            components,
            interval,
        }
    }

    /// The components with Type Ia, in component order.
    #[must_use]
    pub fn components(&self) -> &[IaComponent] {
        &self.components
    }

    /// The interval over which candidates explode: the Type Ia's, at its cap.
    #[must_use]
    pub const fn interval(&self) -> &SupernovaInterval {
        &self.interval
    }

    /// The galaxy's delay-time distribution.
    #[must_use]
    pub fn delays(&self) -> &DelayTimeDistribution {
        &self.delays
    }

    /// The envelope's terms at `point` into `out`, in the components' order, Type Ia per cubic
    /// light-year over the interval: each component's density × its explosions per system.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, ia) in out.iter_mut().zip(&self.components) {
            *slot = fields.component(ia.id).density(point) * ia.explosions_per_system;
        }
        self.components.len()
    }
}

impl ClassProcess for TypeIaProcess {
    type Candidate = TypeIaCandidate;
    type Entry = TypeIaEntry;

    fn class(&self) -> ClassId {
        ClassId::TYPE_IA
    }

    /// Σ, over the components with Type Ia in order, its density bound over the cell's closed box
    /// ([`Fields::component_bounds`](crate::galaxy::fields::Fields::component_bounds)) × its
    /// explosions per system: each term bounds the matching weight of the site's thinning, in the
    /// same arithmetic, so their sum bounds the weights' running total.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let mut bounds = [0.0; MAX_COMPONENTS];
        galaxy.fields().component_bounds(&cube, &mut bounds);
        PerCubicLightYear::new(self.components.iter().fold(0.0, |sum, ia| {
            sum + bounds[ia.id.index()] * ia.explosions_per_system
        }))
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<TypeIaCandidate> {
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let seed = galaxy.seed();
        let key = ObjectKey::from(SystemId::from(id));
        // 1. The site, thinned on the Type Ia's density there, the mark picking the component.
        let mut stream = Stream::open(seed, tags::CLASS_TYPE_IA_SITE, key);
        let site =
            cell.position_from_words([stream.next_u64(), stream.next_u64(), stream.next_u64()]);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &PointLy::from(&site), &mut weights);
        let component =
            self.components[stream.mark().pick_weighted(&weights[..n], bound.value())?].id;
        // 2. The progenitor, delay first, exploding in the interval.
        let mut stream = Stream::open(seed, tags::CLASS_TYPE_IA_PROGENITOR, key);
        let progenitor = IaProgenitor::draw_in(
            &self.delays,
            &mut stream,
            component,
            self.interval.start(),
            self.interval.end(),
        )?;
        // 3. The one velocity.
        let velocity =
            class_site_velocity(galaxy, component, &site, tags::CLASS_TYPE_IA_VELOCITY, key);
        // 4. What the explosion leaves.
        let mut stream = Stream::open(seed, tags::CLASS_TYPE_IA_LEFTOVER, key);
        let leftover = IaLeftover::draw(progenitor.channel(), &mut stream);
        Some(TypeIaCandidate {
            id,
            site,
            component,
            progenitor,
            velocity,
            leftover,
        })
    }

    fn test(&self, galaxy: &Galaxy, candidate: TypeIaCandidate) -> Option<TypeIaEntry> {
        let site = ExplosionSite::new(candidate.site, candidate.progenitor.explosion());
        let marks = candidate.marks(galaxy, &site);
        let window = claimed_window(galaxy, &site, &marks, Surroundings::TypeIa)?;
        Some(candidate.into_entry(galaxy, site, marks, window))
    }

    fn members(&self, entry: &TypeIaEntry) -> u8 {
        1 + u8::from(entry.survivor.is_some())
    }

    /// Member 0's system, or member 1's, the survivor's; neither has stars of the stellar stage's
    /// (module documentation).
    ///
    /// # Panics
    ///
    /// For member 1 of an entry without a survivor, which [`members`](Self::members) never
    /// counts.
    fn system(&self, _: &Galaxy, member: &CatalogueMember<TypeIaEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        if member.member() == 0 {
            return CatalogueSystem::new(entry.record, entry.motion);
        }
        let survivor = entry
            .survivor
            .expect("an entry has member 1 only where it has a survivor");
        CatalogueSystem::new(survivor.record, survivor.motion)
    }
}

impl TypeIaCandidate {
    /// The marks of the candidate's explosion at `site`, [`DeathMarks::of_type_ia`] with its one
    /// velocity.
    #[must_use]
    fn marks(&self, galaxy: &Galaxy, site: &ExplosionSite) -> DeathMarks {
        DeathMarks::of_type_ia(
            galaxy,
            SystemId::from(self.id),
            &self.progenitor,
            &self.leftover,
            site,
            self.velocity,
        )
    }

    /// The entry of the candidate, whose explosion at `site` the test kept with `marks` and the
    /// shell `window` it cut.
    ///
    /// # Panics
    ///
    /// Never: member 1 of a valid ID is a valid ID.
    #[must_use]
    fn into_entry(
        self,
        galaxy: &Galaxy,
        site: ExplosionSite,
        marks: DeathMarks,
        window: ShellWindow,
    ) -> TypeIaEntry {
        let explosion = site.death();
        let motion = CatalogueMotion::broken(self.site, explosion, self.velocity, self.velocity);
        let record = self.record(
            SystemId::from(self.id),
            motion.position_at(UniverseTime::EPOCH),
            self.progenitor.primary_mass(),
        );
        let survivor = self.survivor_mass().map(|mass| {
            let flight = self.survivor_flight(galaxy);
            let motion = CatalogueMotion::broken(
                self.site,
                explosion,
                self.velocity,
                self.velocity + flight,
            );
            let member =
                CatalogueSystemId::new(self.id.class(), self.id.cell(), self.id.index(), 1)
                    .expect("member 1 of a valid ID is a valid ID");
            TypeIaSurvivor {
                record: self.record(
                    SystemId::from(member),
                    motion.position_at(UniverseTime::EPOCH),
                    mass,
                ),
                motion,
            }
        });
        let supernova = SupernovaEntry::new(record.id(), explosion, marks.kind(), window)
            .with_progenitor(self.progenitor)
            .with_leftover(self.leftover);
        TypeIaEntry {
            record,
            component: self.component,
            site,
            motion,
            progenitor: self.progenitor,
            leftover: self.leftover,
            survivor,
            marks,
            supernova,
        }
    }

    /// The initial mass of the star the explosion leaves, if it leaves one: a donor's, the
    /// secondary's, or a Type Iax's partly burnt white dwarf's, the primary's.
    #[must_use]
    fn survivor_mass(&self) -> Option<SolarMasses> {
        match self.leftover {
            IaLeftover::Nothing => None,
            IaLeftover::SurvivingDonor { .. } | IaLeftover::HydrogenDonor { .. } => {
                Some(self.progenitor.secondary_mass())
            }
            IaLeftover::PartlyBurntDwarf => Some(self.progenitor.primary_mass()),
        }
    }

    /// The survivor's velocity relative to the progenitor from T: its speed from the site
    /// ([`IaLeftover::speed`]) along an isotropic direction (`class.type_ia.survivor`, two words),
    /// or none for a Type Iax's partly burnt dwarf, which moves on with the progenitor.
    #[must_use]
    fn survivor_flight(&self, galaxy: &Galaxy) -> GalacticVelocity {
        let Some(speed) = self.leftover.speed() else {
            return GalacticVelocity::default();
        };
        let key = ObjectKey::from(SystemId::from(self.id));
        let direction = isotropic(&mut Stream::open(
            galaxy.seed(),
            tags::CLASS_TYPE_IA_SURVIVOR,
            key,
        ));
        let speed = MetresPerSecond::from(speed).value();
        GalacticVelocity::new(direction.components().map(|c| c * speed))
    }

    /// A record of the candidate's with `id`, at `position` at the epoch, its primary of `mass`:
    /// the progenitor's population and age.
    #[must_use]
    fn record(&self, id: SystemId, position: GalacticPosition, mass: SolarMasses) -> SystemRecord {
        SystemRecord::from_parts(
            id,
            position,
            SystemOrigin::CatalogueSystem(ClassId::TYPE_IA),
            self.progenitor.population(),
            mass,
            self.progenitor.age_at_epoch(),
        )
    }
}

#[cfg(test)]
mod tests;
