//! The field's share of each component's budget (plan 09, P09.T2.c), on the record from generator
//! version 27.
//!
//! The density fields are each population's budget: everything born in it, the features' members
//! included (brainstorm, "Large features"). The field is what is left once the features take their
//! share φ, and the carved binary classes theirs: [`FeatureShares::component_field_factor`] of each
//! component in each band, and for the young disc `1 − φ(age)` at each age, on
//! [`YoungFieldShare`]'s panels. A [`FieldShare`] holds those factors and the young field's ages;
//! the galaxy builds one, and none for the tests' budget ([`FieldShareRules`]), the one `match` on
//! the rules, and its readers go through the galaxy's accessors below, which give the budget's
//! values, bit for bit, where it holds none.
//!
//! # Where the field applies
//!
//! Placement's thinning, which keeps a candidate the budget's pick gave to a component only in
//! the lowest share of that component's slice of the mark that the field holds
//! (`placement::candidate`); the expected counts that the census decides from; the catalogue
//! classes of the field's recent deaths, core collapses and Type Ia, and the luminous blue
//! variables (P09.T43); and the class table's stay shares. The bounds, and with them every
//! candidate's count, position and marks, stay the budget's, so a survivor of the field is a
//! survivor of the budget, bit for bit. Everything else
//! the fields feed reads the budget: the gas, the substellar abundance, the kinematics, the feature
//! catalogue (whose rates φ is made from), the map, the mean masses and so the system count and
//! the potential, and plan 15's fits. The sky's closed-form light (`sky::luminosity`, `band`,
//! `caps`) reads the budget too: rendering plan R06's R06.T16.a, which rides 27 with this, routes
//! it through the field when it takes the features' members in.
//!
//! # Layer D's ancient Type Ia
//!
//! From generator version 27, under plan 09's P09.T35 carve-out (`displaced::site::CarveOutRules`),
//! layer D also gives up the systems that exploded as a Type Ia
//! before the supernova test's interval and left nothing
//! ([`ancient_share`](super::catalogue_classes::type_ia::ancient_share)): each component's field
//! keeps `1 −` its population's of its layer D ([`Galaxy::layer_d_kept`]). Placement's thinning
//! and the expected counts read it, as they read the field's factors, in the same slice of the
//! mark; the Type Ia class does not, since its rate is per system of the budget, the exploded
//! included.

use super::Galaxy;
use super::Population;
use super::ages::AgeDistribution;
#[cfg(doc)]
use super::ages::FieldShareRules;
use super::catalogue_classes::type_ia::DelayTimeDistribution;
use super::features::shares::{FeatureShares, YoungFieldShare};
use super::fields::{ComponentId, Fields};
use super::imf::MassBand;
use super::params::GalaxyParams;
use crate::units::Years;

/// The number of mass bands: the five stellar ones and the two substellar ones, by
/// [`MassBand::index`].
pub(crate) const BANDS: usize = MassBand::ALL.len() + MassBand::SUBSTELLAR.len();

/// What the field keeps of each component's budget (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldShare {
    /// Per component in the fields' order, per band by [`MassBand::index`]: the share of its
    /// systems, born and unborn alike, that the field keeps.
    factors: Vec<[f64; BANDS]>,
    /// The young thin disc's field, if the galaxy has the component.
    young: Option<YoungField>,
}

/// The young thin disc's field: its ages and the share by age they are made from.
#[derive(Debug, Clone, PartialEq)]
struct YoungField {
    id: ComponentId,
    ages: AgeDistribution,
    share: YoungFieldShare,
}

impl FieldShare {
    /// The field of the galaxy of `params`, whose budget is `fields` and whose features' and
    /// carved classes' shares are `shares`.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose timescale is positive; and in debug builds, if a factor
    /// falls outside [0, 1].
    #[must_use]
    pub(crate) fn new(params: &GalaxyParams, fields: &Fields, shares: &FeatureShares) -> Self {
        let bands = MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR);
        let factors: Vec<[f64; BANDS]> = fields
            .components()
            .iter()
            .map(|component| {
                let mut factor = [0.0; BANDS];
                for band in bands.clone() {
                    let f = shares.component_field_factor(component, band);
                    debug_assert!((0.0..=1.0).contains(&f), "a field factor of {f}");
                    factor[band.index()] = f;
                }
                factor
            })
            .collect();
        let tau = params.sfh_timescale();
        let young = fields
            .component_ids()
            .find(|&id| fields.component(id).population() == Population::YoungThinDisc)
            .map(|id| {
                let share = YoungFieldShare::new(tau);
                let ages = AgeDistribution::young_field(tau, &share)
                    .expect("built parameters have a positive, finite timescale");
                YoungField { id, ages, share }
            });
        Self { factors, young }
    }

    /// The share of component `id`'s systems of `band` that the field keeps, born and unborn
    /// alike.
    ///
    /// # Panics
    ///
    /// If `id` is not one of the galaxy's components.
    #[must_use]
    pub(crate) fn factor(&self, id: ComponentId, band: MassBand) -> f64 {
        self.factors[id.index()][band.index()]
    }

    /// The field's ages of component `id`, if they are not its budget's: the young disc's.
    #[must_use]
    pub(crate) fn ages(&self, id: ComponentId) -> Option<&AgeDistribution> {
        self.young
            .as_ref()
            .filter(|young| young.id == id)
            .map(|young| &young.ages)
    }

    /// The share of component `id`'s systems of `band` and age `age` that the field keeps: its
    /// [`factor`](Self::factor), but for the young disc `1 − φ(age)` scaled so that its mean over
    /// the budget's ages is the factor.
    ///
    /// # Panics
    ///
    /// If `id` is not one of the galaxy's components.
    #[must_use]
    pub(crate) fn keeps(&self, id: ComponentId, band: MassBand, age: Years) -> f64 {
        let factor = self.factor(id, band);
        match self.young.as_ref().filter(|young| young.id == id) {
            Some(young) => young.share.at(age) * (factor / young.share.born_mean()),
            None => factor,
        }
    }

    /// The bytes the field owns on the heap.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.factors.capacity() * size_of::<[f64; BANDS]>()
            + self.young.as_ref().map_or(0, |young| {
                young.ages.heap_bytes() + young.share.heap_bytes()
            })
    }
}

impl Galaxy {
    /// The field's share of each component's budget (P09.T2.c); `None` for the tests' budget
    /// ([`FieldShareRules`]), where the field is the budget.
    #[must_use]
    pub(crate) fn field_share(&self) -> Option<&FieldShare> {
        self.field_share.as_deref()
    }

    /// The field's ages of component `id`: its budget's, [`Component::ages`], for the tests'
    /// budget and for every component but the young disc; the young field's, `1 − φ(age)` times
    /// the budget's renormalised. The ages placed systems are drawn from, once the
    /// thinning has kept them.
    ///
    /// [`Component::ages`]: super::fields::Component::ages
    ///
    /// # Panics
    ///
    /// If `id` is not one of the galaxy's components.
    #[must_use]
    pub fn field_ages(&self, id: ComponentId) -> &AgeDistribution {
        match self.field_share().and_then(|field| field.ages(id)) {
            Some(ages) => ages,
            None => self.fields.component(id).ages(),
        }
    }

    /// `value`, a quantity of component `id`'s budget in `band`, as the field's: times the field's
    /// [`factor`](FieldShare::factor), and itself for the tests' budget.
    ///
    /// # Panics
    ///
    /// If `id` is not one of the galaxy's components.
    #[must_use]
    pub(crate) fn field_part(&self, id: ComponentId, band: MassBand, value: f64) -> f64 {
        match self.field_share() {
            Some(field) => value * field.factor(id, band),
            None => value,
        }
    }

    /// The field's share of component `id`'s systems in `band`: the share matrix's
    /// [`component_share`](super::shares::ShareMatrix::component_share), taken to the field
    /// (P09.T2.c), and in layer D from version 27 times what the field keeps of it, 1
    /// less its population's ancient Type Ia (P09.T35). A layer's placed density is the sum of this
    /// times each component's density.
    ///
    /// # Panics
    ///
    /// If `id` is not one of the galaxy's components.
    #[must_use]
    pub fn field_component_share(&self, band: MassBand, id: ComponentId) -> f64 {
        let share = self.shares.component_share(band, self.fields.component(id));
        let field = self.field_part(id, band, share);
        match band {
            MassBand::D => self.layer_d_kept(id).map_or(field, |kept| field * kept),
            MassBand::A
            | MassBand::B
            | MassBand::C
            | MassBand::E
            | MassBand::BrownDwarf
            | MassBand::RoguePlanet => field,
        }
    }

    /// What the field keeps of component `id`'s layer D from generator version 27: 1 less its
    /// population's ancient Type Ia, the share of its layer D that exploded before the supernova
    /// test's interval and left nothing (plan 09, P09.T35; module documentation). In a galaxy that
    /// holds plan 08's displaced classes it is the share of the stay's slice, `1 − a ÷ stay_D`,
    /// since the classes take the rest of layer D's budget first (P08.T12.a). `None` under the
    /// carve-out's held arm, where layer D is the budget's.
    ///
    /// # Panics
    ///
    /// From version 27, if `id` is not one of the galaxy's components.
    #[must_use]
    pub(crate) fn layer_d_kept(&self, id: ComponentId) -> Option<f64> {
        self.layer_d_kept.as_ref().map(|kept| kept[id.index()])
    }
}

/// What the field keeps of each component's layer D under the carve-out's new arm, in the fields'
/// order (plan 09, P09.T35): 1 less its population's
/// [`ancient_share`](DelayTimeDistribution::ancient_share), read from `galaxy`'s delay-time
/// distribution, a quadrature per component and 48 stellar lifetimes when the galaxy is built.
///
/// # Panics
///
/// In debug builds, if a share it keeps falls outside [0, 1].
#[must_use]
pub(crate) fn layer_d_kept(galaxy: &Galaxy) -> Box<[f64]> {
    let delays = DelayTimeDistribution::from_galaxy(galaxy);
    galaxy
        .fields()
        .components()
        .iter()
        .map(|component| {
            let kept = 1.0 - delays.ancient_share(component.population());
            debug_assert!((0.0..=1.0).contains(&kept), "layer D keeps {kept}");
            kept
        })
        .collect()
}

#[cfg(test)]
mod hold;
