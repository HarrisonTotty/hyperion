//! A body's volatile history (plan 14, P14.T48.b, with P14.T24.b): its wet epoch, the span over
//! which its surface held liquid water, and whether it reached its present state from a colder
//! one (science-p14-t24b-laws Part 3, §3.1–3.7).
//!
//! # The law (§3.3)
//!
//! - **The window.** At a system age t a body is in the water window when its orbit-mean flux lies
//!   between Kopparapu et al.'s (2013, ApJ 765, 131; erratum ApJ 770, 82) early-Mars and
//!   runaway-greenhouse limits at its hosts' state then, by P14.T12.b's multiple-host rule
//!   ([`WindowAt`]). The early-Mars edge is their empirical outer limit, Mars wet at 3.8 Ga under
//!   0.75 L☉; it assumes a thick greenhouse, which the outgassing clause below supplies.
//! - **Whether it has an epoch.** It has a solid surface, keeps an atmosphere (P14.T13.b), holds
//!   water (P14.T13.a's, with H₂O retained by T13.b), and lay in the window at some time at or
//!   after its magma ocean's end before it was ever inside the runaway limit, which loses the
//!   water (T13.c): so an M dwarf's planet inside the runaway limit in its host's pre-main
//!   sequence has none (Luger and Barnes 2015, Astrobiology 15, 119).
//! - **The span.** It starts at the first such time.
//!   - A world wet now (P14.T24.a's liquid-water flag, with water) has an epoch to the present,
//!     unless a runaway came between, which ends it there.
//!   - Any other body's epoch ends at the first of: its present outgassing O (P14.T24.g, at the
//!     heat flow then) falling below [`VOLCANIC_GREENHOUSE_OUTGASSING`], the volcanic greenhouse
//!     failing: early Mars's warming needed a thick CO₂–H₂ greenhouse that its volcanoes fed
//!     (Ramirez et al. 2014, Nature Geoscience 7, 59; Wordsworth et al. 2017, GRL 44, 665), and
//!     Kite 2019 (Space Sci. Rev. 215, 10) reviews the geological constraints; the runaway; or its
//!     flux falling beyond the early-Mars limit, a dimming host. If none comes before the present, the body has
//!     none: its present climate, with its outgassing still above the edge, is frozen or dry, and
//!     under its fainter past host it was colder still.
//! - **The figures.** [`WetEpoch`]: for a world wet now its `effective_flow` is the span and its
//!   runoff [`WET_WORLD_RUNOFF_M_PER_YEAR`]; for a past epoch [`PAST_EPOCH_FLOW_FRACTION`] of the
//!   span and [`PAST_EPOCH_RUNOFF_M_PER_YEAR`]; its paleo-inventory is the water over 4πR².
//! - **From colder** ([`VolatileHistory::from_colder`]): the flux lay beyond the
//!   maximum-greenhouse limit when the magma ocean ended and lies inside it now, as P14.T48.d's
//!   high-obliquity ice belt needs: it "is only reached from more glaciated conditions" (Kilic et
//!   al. 2018, ApJ 864, 106, abstract).
//! - **The start condition** ([`window_entry`]; science-p14-t24b-laws §4.6): the body entered the
//!   window after its magma ocean's end, holding water, before any runaway. It is the epoch's
//!   first half, which needs no liquid-water flag, so P14.T48.a's continental fraction reads it
//!   ahead of the surface conditions with no loop. Every body with an epoch entered the window;
//!   one that entered it and has none is frozen or dry now with its volcanic greenhouse still
//!   running.
//!
//! # Computation
//!
//! Every input is a pure function of the system age. The crossings are found on a fixed grid of
//! [`WINDOW_AGES`] ages from the magma ocean's end to the present, both included, each window read
//! once, then [`CROSSING_HALVINGS`] halvings each, always that many (the sim-determinism skill's
//! fixed counts); the outgassing is read from the window's opening to the first dry age, and at
//! each of the end's halvings inside the window. A body with no magma-ocean draw, a moon or a
//! belt's member, starts at [`UNDRAWN_HISTORY_START`]. A change that starts and ends between two of
//! the grid's ages is not seen. The epoch is continuous in time but at recorded state changes: the
//! liquid-water flag turning, a runaway's onset, and a change seen or missed as the grid's ages
//! move with the present.
//!
//! The outgassing at a past age is [`past_outgassing`](super::past_outgassing): the interior's
//! pure functions then, the regime then of a surface inside the window, the radiogenic term then,
//! and the tide as given.
//!
//! Not modelled: the atmosphere's own history (T13's air is the present one), crustal hydration
//! and the water's escape since, and Venus's disputed early ocean (Way et al. 2016, GRL 43, 8376,
//! for; Turbet et al. 2021, Nature 598, 276, against), which the conservative runaway limit rules
//! out. The faint young Sun's effect inside the window is the window's alone: the
//! generator's climate is not run in the past.

use crate::planetary::derive::atmosphere::{Atmosphere, Gas, SurfaceState};
use crate::planetary::derive::habitable_zone::HabitableLimit;
use crate::substance::SubstanceId;
use crate::units::{
    EarthFluxes, Gigayears, Kelvin, Kilograms, KilogramsPerSquareMetre, Metres, Years,
};

#[cfg(test)]
pub(crate) mod tests;

/// The present outgassing O, relative to Earth's now, below which a volcanic greenhouse no longer
/// keeps liquid water at the surface: 0.635, the generator's Mars's O at 3.60 Ga, the Solar System
/// table's Mars at 0.97 Gyr (science-p14-t24b-laws §3.3, whose replica's Mars gives 0.612, ruled
/// as 0.61). `Fitted`: a test refits it to 10⁻⁹. Empirical, calibrated on Mars alone: ±10% in it
/// moves Mars's end to 3.46 and 3.73 Ga.
///
/// Mars's valley networks formed "in the late Noachian and early Hesperian epochs (∼3.6–3.8
/// billion years ago)" (Hoke and Hynek 2009, JGR 114, E08002, abstract), and valley-network
/// activity ran into the Early Hesperian, about 3.5 Ga (Fassett and Head 2008, Icarus 195, 61;
/// from memory). Its real outgassing per unit area was lower than its O suggests.
pub const VOLCANIC_GREENHOUSE_OUTGASSING: f64 = 0.634_741_900_903_217_1;

/// The Earth-equivalent years of channel-forming flow of Mars's epoch, yr: 2.2 × 10⁵, at a runoff
/// of [`PAST_EPOCH_RUNOFF_M_PER_YEAR`] (P14.T48.b). It is where R09's erodibility reaches Mars's
/// 50–350 m of trunk incision (K t ≈ 1.9 m^0.1: R09's Design note 9, from Black et al. 2017,
/// Science 356, 727, Table S2), at the foot of Hoke, Hynek and Tucker 2011's 10⁵–10⁸ yr (EPSL 312, 1, §3 and Table 3),
/// and reads their "each lasting 10⁵–10⁷ years, over the 10⁸ years of valley formation" (§4).
pub const MARS_EFFECTIVE_FLOW: Years = Years::new(2.2e5);

/// A past epoch's Earth-equivalent years of flow per year of its span: 6.56 × 10⁻⁴,
/// [`MARS_EFFECTIVE_FLOW`] over the Solar System table's Mars's 0.335 Gyr, 3.935 to 3.600 Ga
/// (science-p14-t24b-laws §3.3, whose replica gives 5.2 × 10⁻⁴ over 4.02 to 3.60 Ga). `Fitted`: a
/// test refits it to 10⁻⁹.
pub const PAST_EPOCH_FLOW_FRACTION: f64 = 6.564_061_958_818_64e-4;

/// A past epoch's runoff at the reference intermittency, m a⁻¹: 0.1, the plan's Mars (P14.T48.b;
/// science-p14-t24b-laws §3.3), inside 0.073–1.10 m a⁻¹, Hoke, Hynek and Tucker 2011's storm
/// runoff of 0.4–6 cm d⁻¹ (§3) at the 5% reference intermittency (science-hoke.md §3).
pub const PAST_EPOCH_RUNOFF_M_PER_YEAR: f64 = 0.1;

/// A world wet now's mean annual runoff, m a⁻¹: 0.29, Earth's, about 39,300 km³ a⁻¹ of river
/// discharge over its land outside Antarctica (Fekete, Vörösmarty and Grabs 2002, Global
/// Biogeochemical Cycles 16, 1042; from memory, the total not read). It stands until R09's climate
/// supplies its own.
pub const WET_WORLD_RUNOFF_M_PER_YEAR: f64 = 0.29;

/// Earth's reference flood intermittency, the share of the time an Earth river's channel-forming
/// flows run: 0.05 (Kleinhans 2005, JGR 110, E12003, ¶62, after Parker; Hoke, Hynek and Tucker
/// 2011, Table 3 note b, after Wolman and Miller 1960). `effective_flow` is the flowing time at a
/// point over it, and R09's erodibility, a long-term terrestrial calibration, already holds it.
pub const REFERENCE_FLOOD_INTERMITTENCY: f64 = 0.05;

/// How many system ages the window is read at, from the magma ocean's end to the present, both
/// included: 64 (science-p14-t24b-laws §3.3).
pub const WINDOW_AGES: u32 = 64;

/// How many halvings locate each crossing found between two of the grid's ages: 48, which takes a
/// span of 4.5 Gyr ÷ 63 to under a thousandth of a year (science-p14-t24b-laws §3.3).
pub const CROSSING_HALVINGS: u32 = 48;

/// Where the history of a body with no magma-ocean draw starts, a moon's or a belt member's, as
/// a system age, yr: 10^1.5 Myr, the median of P14.T28.a's draw, log-uniform from
/// [`MAGMA_OCEAN_END_EARLIEST`](crate::planetary::params::MAGMA_OCEAN_END_EARLIEST) to
/// [`MAGMA_OCEAN_END_LATEST`](crate::planetary::params::MAGMA_OCEAN_END_LATEST) (the lane's
/// reading, recorded in plan 14's Risks).
///
/// Its window then is a young host's, as a planet's is at its own draw, and not the dark of the
/// system's birth, when no host shines yet, which would read every such body inside the
/// maximum-greenhouse limit now as come from a colder state.
pub const UNDRAWN_HISTORY_START: Years = Years::new(31_622_776.601_683_795);

/// Where a body lies against its hosts' water window at a system age (science-p14-t24b-laws
/// §3.3): its orbit-mean flux over each of the three limits the history reads, at each host's
/// effective temperature then, summed over its hosts, P14.T12.b's multiple-host rule: Σ Sᵢ ÷
/// `S_eff`(`T_eff,i`) (Kaltenegger and Haghighipour 2013, ApJ 777, 165, eqs. 1 and 6), each
/// `S_eff` Kopparapu et al.'s 2013, held to their fit's temperatures.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowAt {
    runaway: f64,
    maximum_greenhouse: f64,
    early_mars: f64,
}

impl WindowAt {
    /// No light at all: beyond every limit.
    pub const DARK: Self = Self {
        runaway: 0.0,
        maximum_greenhouse: 0.0,
        early_mars: 0.0,
    };

    /// The window of a body lit by `lights`: each host's orbit-mean flux at the body, S⊕, with the
    /// host's effective temperature, at the age asked. A host that gives no light adds nothing.
    ///
    /// # Examples
    ///
    /// Earth under the present Sun lies inside the window, Venus inside the runaway limit, and Mars
    /// inside the maximum-greenhouse limit, which the faint young Sun's 0.7 L☉ put it beyond:
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::figures::wet_epoch::WindowAt;
    /// use hyperion_sim::units::{EarthFluxes, Kelvin};
    ///
    /// let sun = |flux: f64| WindowAt::of([(EarthFluxes::new(flux), Kelvin::new(5_772.0))]);
    /// assert!(sun(1.0).in_window() && !sun(1.0).inside_runaway());
    /// assert!(sun(1.91).inside_runaway() && !sun(1.91).in_window());
    /// assert!(sun(0.43).in_window() && sun(0.43).inside_maximum_greenhouse());
    /// assert!(!sun(0.7 * 0.43).inside_maximum_greenhouse());
    /// ```
    #[must_use]
    pub fn of(lights: impl IntoIterator<Item = (EarthFluxes, Kelvin)>) -> Self {
        lights
            .into_iter()
            .filter(|(flux, _)| flux.value() > 0.0)
            .fold(Self::DARK, |sum, (flux, t_eff)| {
                let over = |limit: HabitableLimit| flux.value() / limit.flux(t_eff).value();
                Self {
                    runaway: sum.runaway + over(HabitableLimit::RunawayGreenhouse),
                    maximum_greenhouse: sum.maximum_greenhouse
                        + over(HabitableLimit::MaximumGreenhouse),
                    early_mars: sum.early_mars + over(HabitableLimit::EarlyMars),
                }
            })
    }

    /// Σ Sᵢ ÷ `S_rg`(`T_eff,i`), the flux over the runaway-greenhouse limit.
    #[must_use]
    pub const fn runaway(&self) -> f64 {
        self.runaway
    }

    /// Σ Sᵢ ÷ `S_mg`(`T_eff,i`), the flux over the maximum-greenhouse limit.
    #[must_use]
    pub const fn maximum_greenhouse(&self) -> f64 {
        self.maximum_greenhouse
    }

    /// Σ Sᵢ ÷ `S_em`(`T_eff,i`), the flux over the early-Mars limit.
    #[must_use]
    pub const fn early_mars(&self) -> f64 {
        self.early_mars
    }

    /// Whether the body lies inside the runaway-greenhouse limit, where it loses its water.
    #[must_use]
    pub fn inside_runaway(&self) -> bool {
        self.runaway >= 1.0
    }

    /// Whether the body lies in the water window: inside the early-Mars limit and outside the
    /// runaway-greenhouse limit.
    #[must_use]
    pub fn in_window(&self) -> bool {
        self.early_mars >= 1.0 && self.runaway < 1.0
    }

    /// Whether the body lies inside the maximum-greenhouse limit, the conservative outer edge.
    #[must_use]
    pub fn inside_maximum_greenhouse(&self) -> bool {
        self.maximum_greenhouse >= 1.0
    }
}

/// The span over which a body's surface held liquid (P14.T48.b), with what R09's erosion reads of
/// it, as times before the present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetEpoch {
    liquid: SubstanceId,
    start: Gigayears,
    end: Gigayears,
    effective_flow: Years,
    runoff_m_per_year: f64,
    paleo_inventory: KilogramsPerSquareMetre,
}

impl WetEpoch {
    /// The liquid that flowed, a registry row: water, `H2O`, the only one this rule computes (a
    /// later rule may add a Titan's methane with no change of shape).
    #[must_use]
    pub const fn liquid(&self) -> SubstanceId {
        self.liquid
    }

    /// When the epoch began, thousands of millions of years before the present, at or after
    /// [`end`](Self::end).
    #[must_use]
    pub const fn start(&self) -> Gigayears {
        self.start
    }

    /// When it ended, thousands of millions of years before the present: zero for a world wet now.
    #[must_use]
    pub const fn end(&self) -> Gigayears {
        self.end
    }

    /// The time channel-forming flow ran at a typical point of the surface over the epoch, over
    /// [`REFERENCE_FLOOD_INTERMITTENCY`], in Earth-equivalent years: R09's stream-power solver's
    /// t. The span for a world wet now, and [`PAST_EPOCH_FLOW_FRACTION`] of it for a past epoch.
    #[must_use]
    pub const fn effective_flow(&self) -> Years {
        self.effective_flow
    }

    /// The epoch's mean annual runoff at the reference intermittency, m a⁻¹, which weights R09's
    /// drainage area on a world dry now: [`WET_WORLD_RUNOFF_M_PER_YEAR`] for a world wet now and
    /// [`PAST_EPOCH_RUNOFF_M_PER_YEAR`] for a past epoch.
    #[must_use]
    pub const fn runoff_m_per_year(&self) -> f64 {
        self.runoff_m_per_year
    }

    /// The liquid inventory then, its mass over the body's whole surface, M ÷ 4πR², kg m⁻²:
    /// P14.T13.a's water, which R09 places as a paleo-sea or spreads through closed basins. Its
    /// loss to space and to crustal hydration since is not modelled.
    #[must_use]
    pub const fn paleo_inventory(&self) -> KilogramsPerSquareMetre {
        self.paleo_inventory
    }
}

/// Whether a body entered its hosts' water window holding water (P14.T48.b's start condition,
/// [`window_entry`]), which P14.T48.a's continental fraction reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WindowEntry {
    /// It lay in the window at some time from its magma ocean's end, holding water, before it was
    /// ever inside the runaway limit.
    Entered,
    /// It never did.
    Never,
}

/// What [`window_entry`] reads of a body: its present air and water, its magma ocean's end, its
/// age, and its window as a pure function of a system age (P14.T48.b's start condition).
///
/// Plain data with public fields, as [`VolatileHistoryInputs`]: the window is read at the grid's
/// system ages from the history's start, in order, until it opens or a runaway comes.
#[derive(Debug, Clone, Copy)]
pub struct WindowEntryInputs<'a, W> {
    /// The body's atmosphere now (P14.T13), as [`VolatileHistoryInputs::atmosphere`].
    pub atmosphere: &'a Atmosphere,
    /// Its water, P14.T13.a's inventory, kg, finite and non-negative.
    pub water: Kilograms,
    /// Its mean radius, the bulk section's, m, finite and positive.
    pub radius: Metres,
    /// The system age at which its young magma ocean ended; `None` for a body with no draw, whose
    /// history starts at [`UNDRAWN_HISTORY_START`].
    pub molten_until: Option<Years>,
    /// Its age now, the system's, yr.
    pub age: Years,
    /// Where it lay against its hosts' window at a system age.
    pub window: W,
}

/// Whether the body `inputs` describe entered the water window holding water (P14.T48.b's start
/// condition; science-p14-t24b-laws §4.6): it can hold surface water (a solid surface, an
/// atmosphere kept, water, H₂O retained), and on the history's grid of [`WINDOW_AGES`] system ages
/// from its magma ocean's end to the present the window opens before any age inside the runaway
/// limit.
///
/// It is [`VolatileHistory::derive`]'s own condition for an epoch, read with no liquid-water flag:
/// a body with an epoch entered the window. The window is read from the grid's first age until it
/// opens or a runaway comes, at most [`WINDOW_AGES`] times.
///
/// # Examples
///
/// A world under a steady Sun-like star at 1 S⊕, holding Earth's water, entered the window; the
/// same world at 2 S⊕ lay inside the runaway limit from the start:
///
/// ```
/// use hyperion_sim::planetary::hooks::figures::wet_epoch::{
///     WindowAt, WindowEntry, WindowEntryInputs, window_entry,
/// };
/// use hyperion_sim::units::{EarthFluxes, Kelvin, Kilograms, Metres, Years};
/// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
/// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// # use hyperion_sim::stellar::Composition;
/// # use hyperion_sim::stellar::draws::UnitUniform;
/// # use hyperion_sim::time::UniverseTime;
/// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// # use hyperion_sim::units::{
/// #     Dex, EarthMasses, GravitationalParameter, Megayears, Radians, SolarLuminosities,
/// #     SolarMasses, SolarRadii,
/// # };
/// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
/// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// # let disc = disc.profile().expect("a median disc exists");
/// # let a = Metres::new(METRES_PER_AU);
/// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
/// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
/// # let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, UnitUniform::new(0.384).expect("inside (0, 1)"))?;
/// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
/// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
/// # let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
/// let entry = |flux: f64| {
///     window_entry(&WindowEntryInputs {
///         atmosphere: derived.atmosphere(),
///         water: Kilograms::new(1.4e21),
///         radius: Metres::new(6.371e6),
///         molten_until: Some(Years::new(3.2e7)),
///         age: Years::new(4.57e9),
///         window: |_: Years| WindowAt::of([(EarthFluxes::new(flux), Kelvin::new(5_772.0))]),
///     })
/// };
/// assert_eq!(entry(1.0), WindowEntry::Entered);
/// assert_eq!(entry(2.0), WindowEntry::Never);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn window_entry<W>(inputs: &WindowEntryInputs<'_, W>) -> WindowEntry
where
    W: Fn(Years) -> WindowAt,
{
    let Some(span) = history_span(inputs.molten_until, inputs.age) else {
        return WindowEntry::Never;
    };
    if !holds_water(inputs.atmosphere, inputs.water, inputs.radius) {
        return WindowEntry::Never;
    }
    let windows = (0..WINDOW_AGES).map(|k| (inputs.window)(grid_age(span, k)));
    if opening(windows).is_some() {
        WindowEntry::Entered
    } else {
        WindowEntry::Never
    }
}

/// What [`VolatileHistory::derive`] reads of a body: its present air, water and liquid-water
/// flag, its magma ocean's end and its age, and two pure functions of a system age, its window
/// then and its present outgassing then.
///
/// Plain data with public fields: the functions are called at system ages from the history's
/// start to `age`, in a fixed order. A body whose water is not finite and positive, or whose
/// radius is not, has no epoch, and one whose start is not before `age`, or either not finite, has
/// no history at all.
#[derive(Debug, Clone, Copy)]
pub struct VolatileHistoryInputs<'a, W, O> {
    /// The body's atmosphere now (P14.T13): its state says whether it has a solid surface and
    /// keeps an atmosphere (T13.b), and its retention whether it keeps water.
    pub atmosphere: &'a Atmosphere,
    /// Its water, P14.T13.a's inventory, kg, finite and non-negative.
    pub water: Kilograms,
    /// Its mean radius, the bulk section's, m, finite and positive: the paleo-inventory is spread
    /// over its sphere.
    pub radius: Metres,
    /// Whether water can be liquid at its surface now, P14.T24.a's
    /// [`liquid_water`](crate::planetary::hooks::surface::SurfaceConditions::liquid_water).
    pub liquid_water: bool,
    /// The system age at which its young magma ocean ended (P14.T28.a's `molten_until`), where its
    /// history starts; `None` for a body with no draw, a moon or a belt's member, whose history
    /// starts at [`UNDRAWN_HISTORY_START`].
    pub molten_until: Option<Years>,
    /// Its age now, the system's, yr.
    pub age: Years,
    /// Where it lay against its hosts' window at a system age.
    pub window: W,
    /// Its present outgassing O at a system age, relative to Earth's now
    /// ([`past_outgassing`](super::past_outgassing)).
    pub outgassing: O,
}

/// A body's volatile history (P14.T48.b): its wet epoch, and whether it reached its present state
/// from a colder one.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VolatileHistory {
    wet_epoch: Option<WetEpoch>,
    from_colder: bool,
}

impl VolatileHistory {
    /// No epoch, and not from a colder state.
    pub const NONE: Self = Self {
        wet_epoch: None,
        from_colder: false,
    };

    /// The history of the body `inputs` describe, by the [module](self) documentation's law.
    ///
    /// # Examples
    ///
    /// A Mars-like world under a star that brightened from 0.7 of its light lay beyond the
    /// maximum-greenhouse limit when its magma ocean ended, and lies inside it now: it reached its
    /// present state from a colder one.
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::figures::wet_epoch::{
    ///     VolatileHistory, VolatileHistoryInputs, WindowAt,
    /// };
    /// use hyperion_sim::units::{EarthFluxes, Kelvin, Kilograms, Metres, Years};
    /// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// # use hyperion_sim::stellar::Composition;
    /// # use hyperion_sim::stellar::draws::UnitUniform;
    /// # use hyperion_sim::time::UniverseTime;
    /// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// # use hyperion_sim::units::{
    /// #     Dex, EarthMasses, GravitationalParameter, Megayears, Radians, SolarLuminosities,
    /// #     SolarMasses, SolarRadii,
    /// # };
    /// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
    /// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
    /// # let disc = disc.profile().expect("a median disc exists");
    /// # let a = Metres::new(1.524 * METRES_PER_AU);
    /// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
    /// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
    /// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0934)?, orientation, Radians::ZERO)?;
    /// # let mars = PlacedBody::new(EarthMasses::new(0.107), orbit, a, UnitUniform::new(0.5).expect("inside (0, 1)"))?;
    /// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
    /// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
    /// # let derived = derive_body(&mars, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
    /// // 0.43 S⊕ now, under a star that brightened linearly from 0.7 of its present light.
    /// let flux = |age: Years| 0.43 * (0.7 + 0.3 * age.value() / 4.57e9);
    /// let history = VolatileHistory::derive(&VolatileHistoryInputs {
    ///     atmosphere: derived.atmosphere(),
    ///     water: Kilograms::new(1.4e20),
    ///     radius: Metres::new(3.39e6),
    ///     liquid_water: false,
    ///     molten_until: Some(Years::new(3.2e7)),
    ///     age: Years::new(4.57e9),
    ///     window: |age: Years| WindowAt::of([(EarthFluxes::new(flux(age)), Kelvin::new(5_772.0))]),
    ///     outgassing: |age: Years| 1.2 - 0.6 * age.value() / 1e9,
    /// });
    /// assert!(history.from_colder());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn derive<W, O>(inputs: &VolatileHistoryInputs<'_, W, O>) -> Self
    where
        W: Fn(Years) -> WindowAt,
        O: Fn(Years) -> f64,
    {
        Self::derive_under(inputs, VOLCANIC_GREENHOUSE_OUTGASSING)
    }

    /// [`derive`](Self::derive) with the volcanic greenhouse failing below `threshold` in place of
    /// [`VOLCANIC_GREENHOUSE_OUTGASSING`], for the tests of the end's sensitivity.
    #[must_use]
    pub(crate) fn derive_under<W, O>(
        inputs: &VolatileHistoryInputs<'_, W, O>,
        threshold: f64,
    ) -> Self
    where
        W: Fn(Years) -> WindowAt,
        O: Fn(Years) -> f64,
    {
        // A body still molten now, or a history with no length, has neither.
        let Some((start, age)) = history_span(inputs.molten_until, inputs.age) else {
            return Self::NONE;
        };
        let from_colder = !(inputs.window)(start).inside_maximum_greenhouse()
            && (inputs.window)(age).inside_maximum_greenhouse();
        let wet_epoch = if holds_water(inputs.atmosphere, inputs.water, inputs.radius) {
            wet_epoch(inputs, (start, age), threshold)
        } else {
            None
        };
        Self {
            wet_epoch,
            from_colder,
        }
    }

    /// The span over which the surface held liquid water; `None` for a body that never did by the
    /// law, or that had a wet climate only while frozen or dry now, with its volcanic greenhouse
    /// still above the edge.
    #[must_use]
    pub const fn wet_epoch(&self) -> Option<&WetEpoch> {
        self.wet_epoch.as_ref()
    }

    /// Whether the body reached its present state from a colder one: its flux lay beyond the
    /// maximum-greenhouse limit when its magma ocean ended and lies inside it now. The table's
    /// Mars's is true and Earth's false.
    #[must_use]
    pub const fn from_colder(&self) -> bool {
        self.from_colder
    }
}

/// The history's span of system ages, from its start (the magma ocean's end, or
/// [`UNDRAWN_HISTORY_START`] with no draw) to the present `age`; `None` for a body still molten
/// now, or for either not finite.
#[must_use]
fn history_span(molten_until: Option<Years>, age: Years) -> Option<(Years, Years)> {
    let start = molten_until.unwrap_or(UNDRAWN_HISTORY_START);
    (start.value().is_finite() && age.value().is_finite() && start < age).then_some((start, age))
}

/// Whether a body of atmosphere `air` holding `water` over a radius `radius` can hold surface
/// water at all: a solid surface, an atmosphere kept, water, and H₂O retained
/// (science-p14-t24b-laws §3.3's first three conditions), over a radius its paleo-inventory can be
/// spread on.
#[must_use]
fn holds_water(air: &Atmosphere, water: Kilograms, radius: Metres) -> bool {
    let solid = match air.state() {
        SurfaceState::GasEnvelope | SurfaceState::MagmaOcean => false,
        SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => true,
    };
    let (water, r) = (water.value(), radius.value());
    solid
        && air.keeps_atmosphere()
        && water.is_finite()
        && water > 0.0
        && air.retention().retains(Gas::Water)
        && r.is_finite()
        && r > 0.0
}

/// The grid's `k`-th system age of [`WINDOW_AGES`] evenly spaced from `start` to `age`, the last
/// exactly the present.
#[must_use]
fn grid_age((start, age): (Years, Years), k: u32) -> Years {
    if k + 1 >= WINDOW_AGES {
        age
    } else {
        let (start, age) = (start.value(), age.value());
        Years::new(start + (age - start) * f64::from(k) / f64::from(WINDOW_AGES - 1))
    }
}

/// The index of the first of the grid's windows `windows` that lies in the water window, if the
/// window opens before any lies inside the runaway limit, which loses the water: a window that
/// opens only after one is no epoch's. The windows are read in order, and no further than that.
#[must_use]
fn opening(windows: impl IntoIterator<Item = WindowAt>) -> Option<usize> {
    for (k, window) in windows.into_iter().enumerate() {
        if window.in_window() {
            return Some(k);
        }
        if window.inside_runaway() {
            return None;
        }
    }
    None
}

/// The first system age, to [`CROSSING_HALVINGS`] halvings, at which `changed` holds between
/// `low`, where it does not, and `high`, where it does: always that many halvings.
#[must_use]
fn crossing(low: Years, high: Years, changed: impl Fn(Years) -> bool) -> Years {
    let (mut low, mut high) = (low.value(), high.value());
    for _ in 0..CROSSING_HALVINGS {
        let mid = low + (high - low) * 0.5;
        if changed(Years::new(mid)) {
            high = mid;
        } else {
            low = mid;
        }
    }
    Years::new(high)
}

/// How a wet epoch ends.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ending {
    /// It runs to the present: a world wet now.
    Present,
    /// It ended at this system age.
    At(Years),
}

/// The wet epoch of the body `inputs` describe, which can hold water, over the system ages `span`
/// (its history's start and the present), its volcanic greenhouse failing below `threshold`.
///
/// The grid's windows are read once each. Its scan for the end reads the outgassing from the
/// window's opening to the first dry age, and stops there; each crossing takes its full count of
/// halvings.
#[must_use]
fn wet_epoch<W, O>(
    inputs: &VolatileHistoryInputs<'_, W, O>,
    span: (Years, Years),
    threshold: f64,
) -> Option<WetEpoch>
where
    W: Fn(Years) -> WindowAt,
    O: Fn(Years) -> f64,
{
    let window = &inputs.window;
    let greenhouse = |t: Years| (inputs.outgassing)(t) >= threshold;
    let ages: Vec<Years> = (0..WINDOW_AGES).map(|k| grid_age(span, k)).collect();
    let windows: Vec<WindowAt> = ages.iter().map(|&t| window(t)).collect();
    let opened = opening(windows.iter().copied())?;
    let runaway = |from: usize| {
        windows
            .iter()
            .enumerate()
            .skip(from)
            .find_map(|(k, w)| w.inside_runaway().then_some(k))
    };
    let start = if opened == 0 {
        ages[0]
    } else {
        crossing(ages[opened - 1], ages[opened], |t| window(t).in_window())
    };
    let ending = if inputs.liquid_water {
        match runaway(opened) {
            Some(k) => Ending::At(crossing(ages[k - 1], ages[k], |t| {
                window(t).inside_runaway()
            })),
            None => Ending::Present,
        }
    } else {
        let wet = |t: Years| window(t).in_window() && greenhouse(t);
        // The first of the grid's ages from the window's opening at which the surface is dry; with
        // none before the present, there is no epoch.
        let dried =
            (opened..ages.len()).find(|&k| !(windows[k].in_window() && greenhouse(ages[k])))?;
        let low = if dried == opened {
            // Dry at the grid's first age in the window: the end lies after the start, if the
            // greenhouse held there at all.
            if !wet(start) {
                return None;
            }
            start
        } else {
            ages[dried - 1]
        };
        Ending::At(crossing(low, ages[dried], |t| !wet(t)))
    };
    let (age, start) = (span.1.value(), start.value());
    let (end_ago, effective_flow, runoff_m_per_year) = match ending {
        Ending::Present => (0.0, age - start, WET_WORLD_RUNOFF_M_PER_YEAR),
        Ending::At(end) => (
            age - end.value(),
            PAST_EPOCH_FLOW_FRACTION * (end.value() - start),
            PAST_EPOCH_RUNOFF_M_PER_YEAR,
        ),
    };
    let r = inputs.radius.value();
    let gyr = |years: f64| Gigayears::new(years / 1e9);
    Some(WetEpoch {
        liquid: Gas::Water.substance(),
        start: gyr(age - start),
        end: gyr(end_ago),
        effective_flow: Years::new(effective_flow),
        runoff_m_per_year,
        paleo_inventory: KilogramsPerSquareMetre::new(
            inputs.water.value() / (4.0 * core::f64::consts::PI * r * r),
        ),
    })
}
