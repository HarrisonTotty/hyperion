//! What a body's passes read of its record (P14.T54.a; decision-pre-23-review item 35;
//! decision-p14-t54a-passes §1, §3.1–3.2 and "Cost escalation (after Batch B)", rule (f)).
//!
//! Until 23 every pass read the passes' stand-in surface: a climate with no contrast, no basins,
//! all the condensed water at the surface and the bulk material's regolith. From 23 a body whose
//! system holds its rotation law takes its passes over its record ([`PassRecord::Record`]):
//!
//! - **The spin**, computed once a body from its held law under its seasonal orbit
//!   ([`RecordSpin`]), with its insolation table, and its record's bulk's gravity, radius and
//!   surface material. Each greenhouse step's climate is P14.T24.a's range under it, at θ from
//!   that step's own partition (decision-p14-t54a-passes §3.1; P14.T51.a part 2's "θ from the
//!   previous pass" superseded).
//! - **The context** each pass reads ([`PassContext`], the solver's [`Context`]): what the figures'
//!   first stage of the previous pass's own air gives, K(P) ([`PassRecordOf::context`]): the
//!   basins, the water's surface share, the heat flow, the surface age, the regime, the craters'
//!   crust class, and a moon's tide branch, ocean flag and tidal heat. The first pass reads
//!   [`PassContext::NONE`], C₀, the context K gives a body with no figures, labelled.
//!
//! - **The crust** ([`PassRecordOf::crust`], P14.T51.c's in stages): what its record gives its
//!   crust's basis, once a body before the passes (its origin, its formation time, a moon's side
//!   of its parent's ice line, its host's Mg/Si and its record's bulk), beside what the
//!   derivation gives (its age, its magma ocean, its ΔIW, its ammonia and the formation stand-in's
//!   temperature, the first pass's equilibrium temperature). Each pass takes its lid at its own
//!   climate under the surface age and tide of the context it reads, so that its regolith is its
//!   lid's row; the next pass's ground reads that lid's inertia ([`InertiaLid`]).
//!
//! A body with no rotation law (the public `derive_body` and `atmosphere`, a `generate_planets`
//! system's bodies, `generate`'s parents at its step 7, and a body present at a time but not at
//! its law's) keeps the stand-in ([`PassRecord::StandIn`]): "the passes' stand-in: no rotation
//! law, no basins, the bulk material's regolith; a system's record derives the body under its
//! law, its basins and its lid (P14.T54.a)".

use core::cmp::Ordering;
use core::fmt;

use crate::planetary::chem::partition::{Basins, CrustWater, Ground};
use crate::planetary::derive::atmosphere::{Atmosphere, Gas, SurfaceMaterial, SurfaceState};
use crate::planetary::derive::passes::{Context, floats_agree};
use crate::planetary::derive::{MassFractions, PlanetClass, SnowLineSide};
use crate::planetary::hooks::crust::{
    CrustLid, CrustLithology, CrustOrigin, LidCrust, TidalVolcanism,
};
use crate::planetary::hooks::figures::cover::{self, SurfaceWaterInputs};
use crate::planetary::hooks::figures::{Crust, PartitionGround, PassFigures, TectonicRegime};
use crate::planetary::hooks::surface::{OceanWeight, SeasonalSpin, ZonalInsolation};
use crate::planetary::moons::tides::TidalBranch;
use crate::units::{
    Dex, EarthMasses, EarthRadii, Kelvin, Kilograms, KilogramsPerCubicMetre,
    KilogramsPerSquareMetre, Megayears, Metres, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};

/// A body's bulk before its passes, which no pass moves (lever (e)): its mass, radius, density,
/// surface gravity, class and mass fractions at the time, the derivation's own to the bit, and its
/// volatile inventory's water. What the record's bulk rule reads to give a pass's figures and
/// climate their bulk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PassBulk {
    /// The mass at the time, escape's loss taken.
    pub(crate) mass: EarthMasses,
    /// The radius at the time.
    pub(crate) radius: EarthRadii,
    /// The mean density.
    pub(crate) density: KilogramsPerCubicMetre,
    /// The mean surface gravity.
    pub(crate) surface_gravity: MetresPerSecondSquared,
    /// The class.
    pub(crate) class: PlanetClass,
    /// The mass fractions.
    pub(crate) fractions: MassFractions,
    /// The water of the volatile inventory (P14.T13.a), before escape and the climate divide it.
    pub(crate) inventory_water: Kilograms,
}

/// What a body's spun climate reads of its record beyond each pass (P14.T54.a): its spin under its
/// held law at the time ([`SeasonalSpin::of`]), computed once a body, the insolation table of its
/// obliquity (lever (h)), and its record's bulk's surface gravity, radius and surface material,
/// at which the record's conditions are derived.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordSpin<'a> {
    /// How it turns under its seasonal host.
    pub(crate) spin: SeasonalSpin,
    /// The insolation table of its spin's obliquity, over the call chain's hour angles.
    pub(crate) table: &'a ZonalInsolation,
    /// Its record's bulk's mean surface gravity.
    pub(crate) surface_gravity: MetresPerSecondSquared,
    /// Its record's bulk's mean radius.
    pub(crate) radius: Metres,
    /// The material of its record's bulk ([`SurfaceMaterial::of`] of its fractions).
    pub(crate) material: SurfaceMaterial,
}

/// What a body's passes read of its record (P14.T54.a): implemented by the system's records of a
/// planet or member and of a moon.
pub(crate) trait PassRecordOf {
    /// The body's spun climate's record ([`RecordSpin`]), its bulk before its passes `bulk`.
    #[must_use]
    fn spin(&self, bulk: &PassBulk) -> RecordSpin<'_>;

    /// K(P): the context the next pass reads of a pass whose atmosphere is `air` at the
    /// equilibrium temperature `equilibrium`, its bulk before its passes `bulk` (decision-p14-t54a-
    /// passes §1.1): the figures' first stage of its own air, with a moon's tide at that pass. The
    /// pass's own lid joins it after ([`PassContext::with_lid`]).
    #[must_use]
    fn context(&self, bulk: &PassBulk, air: &Atmosphere, equilibrium: Kelvin) -> PassContext;

    /// What the body's record gives its crust's basis (P14.T51.c; [`RecordCrust`]), its bulk
    /// before its passes `bulk`, once a body before the passes: `None` where this version computes
    /// no crust for it (an origin with no formation window, a parent with no formation time).
    #[must_use]
    fn crust(&self, bulk: &PassBulk) -> Option<RecordCrust>;
}

/// What a body's record gives its crust's basis (P14.T54.a; P14.T51.c's `CrustBasisInputs`), the
/// rest coming from its derivation: how it came to be, when it formed, a moon's side of its
/// parent's ice line, its host's Mg/Si, \[α/Fe\] and \[Fe/H\], and its record's bulk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RecordCrust {
    /// How it came to be.
    pub(crate) origin: CrustOrigin,
    /// Its formation time `t_f`, Myr after its host's onset of collapse: a planet's P14.T28.a's
    /// `formed_at`, a giant-impact moon's its parent's, and every other body's word 0 of its
    /// `body.formation` stream in its window ([`formation_time`](crate::planetary::hooks::crust::formation_time)),
    /// drawn once a derivation.
    pub(crate) formation: Megayears,
    /// A moon's side of its parent's ice line (P14.T48.f's `MoonBirth::formed`); `None` for a host
    /// orbiter, which takes its derivation's own.
    pub(crate) snow_line_side: Option<SnowLineSide>,
    /// Its host's Mg/Si by number, its system's `system.abundances` words 0–1 at its \[Fe/H\]
    /// (P14.T50.a's `HostAbundances`), positive.
    pub(crate) host_magnesium_to_silicon: f64,
    /// Its system's \[α/Fe\], dex (0 for none).
    pub(crate) alpha_fe: Dex,
    /// Its host's \[Fe/H\], dex.
    pub(crate) fe_h: Dex,
    /// Its record's bulk's mean radius.
    pub(crate) radius: Metres,
    /// Its mass at the time, its record's bulk's.
    pub(crate) mass: EarthMasses,
    /// Its record's bulk's mass fractions.
    pub(crate) fractions: MassFractions,
}

/// The lid a pass's ground reads for its thermal inertia (P14.T54.a; decision-p14-t54a-passes
/// §1.1, "inertia lid"): the previous pass's own lid, which crust lies at its surface and the
/// lithology it names. A key member of the context, so that a settled pass's ground read the lid
/// its own climate gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InertiaLid {
    /// Which crust, or the bulk material, lies at the surface.
    pub(crate) crust: LidCrust,
    /// The lithology it names, whose row the ground's inertia reads.
    pub(crate) lithology: CrustLithology,
}

impl InertiaLid {
    /// The inertia lid of the pass's lid `lid`.
    #[must_use]
    pub(crate) fn of(lid: &CrustLid) -> Self {
        Self {
            crust: lid.crust(),
            lithology: lid.lithology(),
        }
    }
}

/// How a body's passes take their climate (P14.T54.a; decision-p14-t54a-passes §3.2).
#[derive(Clone, Copy)]
pub(crate) enum PassRecord<'a> {
    /// The passes' stand-in: no rotation law, no basins, the bulk material's regolith; a system's
    /// record derives the body under its law, its basins and its lid (P14.T54.a). Every pass
    /// reads one surface ([`PassSurface::stand_in`](super::atmosphere::PassSurface::stand_in)),
    /// so the passes are T51.h's, a function of the albedo alone.
    StandIn,
    /// The body's record: its spin, and each pass's context from the one before.
    Record(&'a dyn PassRecordOf),
}

impl fmt::Debug for PassRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StandIn => f.write_str("StandIn"),
            Self::Record(_) => f.write_str("Record"),
        }
    }
}

/// A moon's tide as a pass's context holds it (decision-p14-t54a-passes §1.1): its branch, whether
/// it holds a subsurface ocean, and its tidal heat over its surface, at that pass's equilibrium
/// temperature and state (P14.T56.d's `own_tides`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PassTide {
    /// The tide's branch.
    pub(crate) branch: TidalBranch,
    /// Whether its ocean flag holds an ocean under its ice.
    pub(crate) ocean: bool,
    /// Its tidal heat over its surface, W m⁻².
    pub(crate) heat_flux: WattsPerSquareMetre,
}

/// The context a pass reads beside its albedo (P14.T54.a; decision-p14-t54a-passes §1.1): the
/// figures' first stage of the previous pass's own air ([`PassFigures`]), K(P), with a moon's tide
/// at that pass.
///
/// Its members are what a pass reads of it, and what [`Context`] compares:
///
/// | Member | From | Kind |
/// | --- | --- | --- |
/// | figures present | the first stage, `None` for none (a gas envelope, or a class with no solid surface, or a stage the crater contract refused, such as by the air's pressure through its screening) | key |
/// | basins | the relief's hypsometry ([`Basins::of`]), or [`Basins::UNKNOWN`] | known and populations key; weights, means, widths and the full-ocean layer floats |
/// | water share | [`cover::water_surface_share`] of the outgassing | its branch key; float |
/// | heat flow | the first stage's, a moon's tide in it | float |
/// | surface age | the crater contract's | float |
/// | regime | the first stage's | key |
/// | craters' crust class | the impactors' ([`Crust`]) | key |
/// | tide | a moon's branch and ocean flag, and its tidal heat | key; float |
/// | inertia lid | the pass's own lid ([`InertiaLid`]), whose row the next pass's ground reads | key |
/// | `lid_of`'s unresurfaced test | the surface age against the time since the magma ocean, which the next pass's lid takes | key |
///
/// Every member a pass reads is ranked: the lid a pass takes reads the surface age (a float), the
/// tide's branch, the unresurfaced test and the basis, which no pass moves; its ground reads the
/// inertia lid. The rest of the first stage rides along unread by any pass, so that the record's
/// figures reuse the published pass's own K(P*)
/// ([`GlobalFigures::of_first_stage`](crate::planetary::hooks::figures::GlobalFigures::of_first_stage)):
/// the record reads the whole stage only through K(P*), never through the context a pass read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PassContext {
    figures: Option<PassFigures>,
    basins: Basins,
    tide: Option<PassTide>,
    lid: Option<InertiaLid>,
    unresurfaced: Option<bool>,
}

/// How many float members a context holds at most: two populations' weight, mean and width, the
/// full-ocean layer, the water share, the heat flow, the surface age and the tidal heat.
const CONTEXT_FLOATS: usize = 11;

/// The shape of a context's basins, its key's part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BasinsShape {
    /// None known.
    Unknown,
    /// One population.
    One,
    /// Two populations, a mobile lid's.
    Two,
}

/// The discrete key of a [`PassContext`] (decision-p14-t54a-passes §1.5): every member that is not
/// a float, and every branch that sets a float member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PassContextKey {
    /// Whether the figures are present.
    pub(crate) figures: bool,
    /// Whether the basins are known, and how many populations they hold.
    pub(crate) basins: BasinsShape,
    /// Whether the volcanic greenhouse runs, the water share's outgassing branch.
    pub(crate) volcanic: bool,
    /// The tectonic regime.
    pub(crate) regime: Option<TectonicRegime>,
    /// The craters' crust class.
    pub(crate) crust: Option<Crust>,
    /// A moon's tide branch and ocean flag.
    pub(crate) tide: Option<(TidalBranch, bool)>,
    /// The inertia lid.
    pub(crate) lid: Option<InertiaLid>,
    /// `lid_of`'s unresurfaced test on its surface age, where the body has a crust's basis and
    /// the context a surface age.
    pub(crate) unresurfaced: Option<bool>,
}

impl PassContext {
    /// C₀, the first pass's stand-in context (decision-p14-t54a-passes §1.2), labelled: the
    /// context K gives a body with no figures, member for member. No basins known, all the
    /// condensed water at the surface, no heat flow (the cosmic background's floor), no surface
    /// age, no tide (a quiet one) and no lid: its pass takes no lid of its own, and its ground
    /// takes its bulk material's inertia.
    pub(crate) const NONE: Self = Self {
        figures: None,
        basins: Basins::UNKNOWN,
        tide: None,
        lid: None,
        unresurfaced: None,
    };

    /// The context of the first stage `figures` and a moon's tide `tide` (K(P)), with no lid yet
    /// ([`with_lid`](Self::with_lid)).
    ///
    /// # Panics
    ///
    /// In a debug build, if a float member is not finite: [`Context::rank`] orders floats by
    /// `total_cmp`, whose order of a NaN, by its sign, is not portable across targets, so the
    /// planetary context holds none (the C1 review's DD1).
    #[must_use]
    pub(crate) fn of(figures: Option<&PassFigures>, tide: Option<PassTide>) -> Self {
        let context = Self {
            basins: figures.map_or(Basins::UNKNOWN, PassFigures::basins),
            figures: figures.copied(),
            tide,
            lid: None,
            unresurfaced: None,
        };
        debug_assert!(
            {
                let (floats, n) = context.floats();
                floats[..n].iter().all(|x| x.is_finite())
            },
            "a context's floats are finite: {context:?}"
        );
        context
    }

    /// The figures' first stage it holds, `None` for none.
    #[must_use]
    pub(crate) const fn figures(&self) -> Option<&PassFigures> {
        self.figures.as_ref()
    }

    /// The context with the pass's own lid `lid` (the next pass's ground's) and `lid_of`'s
    /// unresurfaced test on its own surface age, `unresurfaced`, which the next pass's lid takes:
    /// K(P) whole (decision-p14-t54a-passes §1.1).
    #[must_use]
    pub(crate) const fn with_lid(
        self,
        lid: Option<InertiaLid>,
        unresurfaced: Option<bool>,
    ) -> Self {
        Self {
            lid,
            unresurfaced,
            ..self
        }
    }

    /// A moon's tide at the pass it is of.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "a pass's lid reads its branch through `tidal`; the tests read it whole"
        )
    )]
    #[must_use]
    pub(crate) const fn tide(&self) -> Option<PassTide> {
        self.tide
    }

    /// Whether the tide makes the body volcanic, its lid's (P14.T51.c): a moon on P14.T56.d's
    /// heated branch is; every other moon, and a body with no tide, is not.
    #[must_use]
    pub(crate) fn tidal(&self) -> TidalVolcanism {
        match self.tide.map(|tide| tide.branch) {
            Some(TidalBranch::Heated) => TidalVolcanism::Volcanic,
            Some(TidalBranch::Cold | TidalBranch::Ocean) | None => TidalVolcanism::Quiet,
        }
    }

    /// The inertia lid the pass's ground reads: the previous pass's own lid, `None` for none (C₀,
    /// a body with no crust), whose ground takes its bulk material's constant.
    #[must_use]
    pub(crate) const fn lid(&self) -> Option<InertiaLid> {
        self.lid
    }

    /// The basins the liquids fill: the relief's hypsometry, or none known.
    #[must_use]
    pub(crate) const fn basins(&self) -> Basins {
        self.basins
    }

    /// The share of the condensed water at the surface, 0 to 1: the outgassing's
    /// ([`cover::water_surface_share`]), all of it with no figures.
    #[must_use]
    pub(crate) fn water_share(&self) -> f64 {
        self.figures
            .as_ref()
            .map_or(1.0, PassFigures::water_surface_share)
    }

    /// The heat flow at the surface, the night floor's: the first stage's, none with no figures.
    #[must_use]
    pub(crate) fn heat_flow(&self) -> WattsPerSquareMetre {
        self.figures
            .as_ref()
            .map_or(WattsPerSquareMetre::new(0.0), PassFigures::heat_flow)
    }

    /// The surface age, `None` with no figures.
    #[must_use]
    pub(crate) fn surface_age(&self) -> Option<Years> {
        self.figures.as_ref().map(PassFigures::surface_age)
    }

    /// The crust that caps T24.b's ice on a body whose bulk is of `material`: the craters' crust
    /// class, or with no figures the passes' stand-in's by the material (ice uncapped, rock
    /// capped).
    #[must_use]
    pub(crate) fn crust(&self, material: SurfaceMaterial) -> Crust {
        self.figures.as_ref().map_or(
            match material {
                SurfaceMaterial::Ice => Crust::IceRich,
                SurfaceMaterial::Rock => Crust::Silicate,
            },
            PassFigures::crust,
        )
    }

    /// The surface water and the potential ocean it fills of an air whose water vapour is
    /// `vapour`, gated by `water` ([`cover::water_under`]) over this context's basins and water
    /// share: the one expression a greenhouse step's θ, a pass's published partition and the
    /// record's read (decision-p14-t54a-passes §3.1).
    #[must_use]
    pub(crate) fn surface_water(
        &self,
        water: &WaterGate,
        vapour: Pascals,
    ) -> (KilogramsPerSquareMetre, f64) {
        cover::water_under(
            &SurfaceWaterInputs {
                state: water.state,
                retained: water.retained,
                water: water.water,
                radius: water.radius,
                surface_gravity: water.surface_gravity,
                vapour,
                share: self.water_share(),
            },
            &self.basins,
        )
    }

    /// θ of an air whose water vapour is `vapour`, gated by `water`: the weight of the potential
    /// ocean it fills ([`OceanWeight::of_potential_ocean`]; [`surface_water`](Self::surface_water)).
    /// 0 where all the water is airborne, as at a greenhouse's first step.
    #[must_use]
    pub(crate) fn ocean_weight(&self, water: &WaterGate, vapour: Pascals) -> OceanWeight {
        OceanWeight::of_potential_ocean(self.surface_water(water, vapour).1)
    }

    /// The partition's ground under this context of the atmosphere `air` of a body whose water is
    /// gated by `water` (rule (f); [`PartitionGround`]): its basins and water share, the surface
    /// water and potential ocean of `air`'s vapour, the crust that caps the ice on a bulk of
    /// `material`, and whether an ice bulk's crust is listed, `crust_water`.
    #[must_use]
    pub(crate) fn partition_ground(
        &self,
        air: &Atmosphere,
        water: &WaterGate,
        material: SurfaceMaterial,
        crust_water: CrustWater,
    ) -> PartitionGround {
        let (surface_water, potential_ocean) =
            self.surface_water(water, air.partial_pressures().of(Gas::Water));
        PartitionGround {
            ground: Ground::new(water.surface_gravity, self.basins, self.water_share()),
            surface_water,
            potential_ocean,
            crust: self.crust(material),
            crust_water,
        }
    }

    /// The float members in their fixed order, and how many there are: the basins' populations'
    /// weight, mean and width and their full-ocean layer where known, then the water share, the
    /// heat flow, the surface age (0 with none) and the tidal heat (0 with none). Two contexts of
    /// one key hold the same count.
    #[must_use]
    fn floats(&self) -> ([f64; CONTEXT_FLOATS], usize) {
        let mut floats = [0.0; CONTEXT_FLOATS];
        let mut n = 0;
        let mut push = |x: f64| {
            floats[n] = x;
            n += 1;
        };
        if let Some((hypsometry, full_ocean)) = self.basins.known() {
            for population in hypsometry.populations() {
                push(population.weight());
                push(population.mean().value());
                push(population.width().value());
            }
            push(full_ocean.value());
        }
        push(self.water_share());
        push(self.heat_flow().value());
        push(self.surface_age().map_or(0.0, Years::value));
        push(self.tide.map_or(0.0, |tide| tide.heat_flux.value()));
        (floats, n)
    }
}

impl Context for PassContext {
    type Key = PassContextKey;

    fn key(&self) -> Self::Key {
        let figures = self.figures.as_ref();
        PassContextKey {
            figures: figures.is_some(),
            basins: match self.basins.known() {
                None => BasinsShape::Unknown,
                Some((hypsometry, _)) => match hypsometry.populations().len() {
                    1 => BasinsShape::One,
                    _ => BasinsShape::Two,
                },
            },
            volcanic: figures.is_none_or(PassFigures::volcanic_greenhouse),
            regime: figures.map(PassFigures::tectonic_regime),
            crust: figures.map(PassFigures::crust),
            tide: self.tide.map(|tide| (tide.branch, tide.ocean)),
            lid: self.lid,
            unresurfaced: self.unresurfaced,
        }
    }

    fn agrees(&self, other: &Self) -> bool {
        if self.key() != other.key() {
            return false;
        }
        let ((a, n), (b, m)) = (self.floats(), other.floats());
        n == m
            && a[..n]
                .iter()
                .zip(&b[..m])
                .all(|(&a, &b)| floats_agree(a, b))
    }

    fn rank(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key()).then_with(|| {
            let ((a, n), (b, m)) = (self.floats(), other.floats());
            a[..n]
                .iter()
                .zip(&b[..m])
                .map(|(a, b)| a.total_cmp(b))
                .find(|order| order.is_ne())
                .unwrap_or_else(|| n.cmp(&m))
        })
    }
}

/// What gates a body's surface water at a pass (decision-p14-t54a-passes §3.1;
/// [`cover::surface_water_of`]): the state that holds it, whether the body keeps its water
/// against escape, its volatile inventory's water, and its record's bulk's radius and gravity.
///
/// At a greenhouse step the state is step 3's, the climate's division, which the step's partition
/// cannot move; at the pass's published partition, and on the record, its air's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WaterGate {
    /// The state that gates the water: only a temperate world or a snowball holds it.
    pub(crate) state: SurfaceState,
    /// Whether the body keeps its water against escape, not by an ice crust's ice alone
    /// ([`Atmosphere::retains_against_escape`]).
    pub(crate) retained: bool,
    /// P14.T13.a's water.
    pub(crate) water: Kilograms,
    /// The record's bulk's mean radius.
    pub(crate) radius: Metres,
    /// The record's bulk's mean surface gravity.
    pub(crate) surface_gravity: MetresPerSecondSquared,
}

impl WaterGate {
    /// The gate of the atmosphere `air`, its own state and retention, of a body whose
    /// inventory's water is `water`, of the record's bulk's radius and gravity in `spin`.
    #[must_use]
    pub(crate) fn of_air(air: &Atmosphere, water: Kilograms, spin: &RecordSpin<'_>) -> Self {
        Self {
            state: air.state(),
            retained: air.retains_against_escape(Gas::Water),
            water,
            radius: spin.radius,
            surface_gravity: spin.surface_gravity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C1 review's DD2: on the planetary context, `rank` is `Equal` exactly where every member
    /// is the same bits, which the 2-cycle stop and the bracket's reuse of an end read; and
    /// `agrees` holds for the same bits, never across keys.
    #[test]
    fn a_context_ranks_equal_exactly_at_the_same_members() {
        let none = PassContext::NONE;
        assert_eq!(none.rank(&none), Ordering::Equal);
        assert!(none.agrees(&none));
        let tide = |branch, ocean, heat: f64| {
            PassContext::of(
                None,
                Some(PassTide {
                    branch,
                    ocean,
                    heat_flux: WattsPerSquareMetre::new(heat),
                }),
            )
        };
        let a = tide(TidalBranch::Ocean, true, 0.01);
        let b = tide(TidalBranch::Ocean, true, 0.01);
        let c = tide(TidalBranch::Ocean, true, 0.01_f64.next_up());
        let d = tide(TidalBranch::Ocean, false, 0.01);
        assert_eq!(a.rank(&b), Ordering::Equal);
        assert_ne!(a.rank(&c), Ordering::Equal);
        assert_eq!(a.rank(&c), c.rank(&a).reverse());
        assert!(a.agrees(&c), "one ulp apart agrees");
        assert_ne!(a.rank(&d), Ordering::Equal);
        assert!(!a.agrees(&d), "another key never agrees");
        assert_ne!(none.rank(&a), Ordering::Equal);
        assert!(!none.agrees(&a));
        // A tide 10⁻⁸ apart is not the same context.
        let far = tide(TidalBranch::Ocean, true, 0.01 * (1.0 + 1e-8));
        assert!(!a.agrees(&far));
    }
}
