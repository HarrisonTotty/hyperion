//! The phases of a track in the order a single star passes through them (HPT sections 5 and 6,
//! with the rules of section 7.1 for the initial mass), and how each ends.

use crate::stellar::Phase;
use crate::stellar::remnant::collapse::{
    CoreCollapse, ElectronCaptureWindows, IRON_CORE_MC_BAGB, OXYGEN_NEON_CAPTURE_MASS,
    RemnantDraws, core_collapse, electron_capture_remnant,
};
use crate::stellar::remnant::structure::{
    DegenerateCore, OXYGEN_NEON_MC_BAGB, hurley_supernova_remnant, white_dwarf_kind,
};
use crate::stellar::remnant::{
    CompactRemnant, Death, DeathKind, ProgenitorAtDeath, RemnantKind, RemnantRecipe, Stripping,
    SupernovaType,
};
use crate::units::{Megayears, SolarMasses, Years};

use super::super::agb::{self, CoreEnd, EarlyAgb, EarlyAgbEnd, ThermallyPulsingAgb};
use super::super::cheb::CoreHeliumBurning;
use super::super::gb::FirstGiantBranch;
use super::super::helium::{self, HeliumStar};
use super::super::hg::HertzsprungGap;
use super::super::ms::{self, MainSequence};
use super::build::{Builder, Ending, Entry, FLASH_YEARS, Step, segment_coordinate};
use super::model::{HeliumCore, Model, Span};
use super::{Bridges, Coordinate, Fate, IronCore, Junction, Segment};

impl Builder<'_> {
    /// The main sequence of a star of `mass`, whose initial mass follows the current one.
    pub(super) fn main_sequence(&self, start: f64, mass: f64, previous: Option<[f64; 3]>) -> Step {
        let c = self.phys.coeffs;
        let duration = |m: f64| ms::t_ms(SolarMasses::new(m), c).value() * 1e6;
        let built = self.fraction_segment(
            Model::MainSequence { fixed: None },
            start,
            mass,
            0.0,
            previous,
            duration,
            |_| f64::INFINITY,
            |m| Model::MainSequence {
                fixed: Some(MainSequence::new(SolarMasses::new(m), c)),
            },
        );
        let (end, end_mass) = (built.segment.end, built.end_mass);
        self.finish(
            Some(built.segment),
            end,
            Entry::HertzsprungGap {
                m0: end_mass,
                mass: end_mass,
            },
            previous,
        )
    }

    /// The Hertzsprung gap of a star of initial mass `m0` and mass `mass`.
    pub(super) fn hertzsprung_gap(
        &self,
        start: f64,
        m0: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let c = self.phys.coeffs;
        let initial = SolarMasses::new(m0);
        let gap = HertzsprungGap::new(initial, c);
        let span = Span {
            start: gap.t_start(),
            end: gap.t_end(),
        };
        let core = HeliumCore::of(initial, c);
        let core_mass = {
            let gap = gap.clone();
            move |age: f64| {
                gap.core_mass(span.at(segment_coordinate(start, span, age)))
                    .value()
            }
        };
        let built = self.envelope_segment(
            Model::HertzsprungGap { gap, core, span },
            start,
            span,
            mass,
            previous,
            self.resolution.knots,
            |age| segment_coordinate(start, span, age),
            |age, m| m - core_mass(age),
            None,
        );
        let next = match built.ending {
            Ending::Nominal if m0 < c.m_fgb().value() => Entry::FirstGiantBranch {
                m0,
                mass: built.end_mass,
            },
            Ending::Nominal => Entry::CoreHeliumBurning {
                m0,
                mass: built.end_mass,
            },
            Ending::Envelope => {
                self.giant_stripped(core, built.end, core_mass(built.end).min(built.end_mass))
            }
        };
        self.finish(built.segment, built.end, next, previous)
    }

    /// The first giant branch of a star of initial mass `m0` below `M_FGB` and mass `mass`.
    pub(super) fn giant_branch(
        &self,
        start: f64,
        m0: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let c = self.phys.coeffs;
        let initial = SolarMasses::new(m0);
        let branch = FirstGiantBranch::new(initial, c);
        let span = Span {
            start: branch.t_start(),
            end: branch.t_hei(),
        };
        let core = HeliumCore::of(initial, c);
        let core_mass = {
            let branch = branch.clone();
            move |age: f64| {
                branch
                    .core_mass(span.at(segment_coordinate(start, span, age)))
                    .value()
            }
        };
        // The branch's progress in the relation's core mass, which sets the luminosity and so the
        // wind: even steps in time would crowd the tip into the last interval.
        let progress = {
            let branch = branch.clone();
            let (mc0, mc1) = (
                branch.relation_core(span.start).value(),
                branch.relation_core(span.end).value(),
            );
            move |age: f64| {
                let clock = span.at(segment_coordinate(start, span, age));
                (branch.relation_core(clock).value() - mc0) / (mc1 - mc0)
            }
        };
        let built = self.envelope_segment(
            Model::FirstGiantBranch { branch, core, span },
            start,
            span,
            mass,
            previous,
            self.resolution.knots,
            progress,
            |age, m| m - core_mass(age),
            None,
        );
        let next = match (built.ending, core) {
            (Ending::Nominal, HeliumCore::Degenerate) => Entry::Flash {
                mass: built.end_mass,
            },
            (Ending::Nominal, HeliumCore::NonDegenerate) => Entry::CoreHeliumBurning {
                m0,
                mass: built.end_mass,
            },
            (Ending::Envelope, _) => {
                self.giant_stripped(core, built.end, core_mass(built.end).min(built.end_mass))
            }
        };
        self.finish(built.segment, built.end, next, previous)
    }

    /// A Hertzsprung-gap or giant-branch star that has lost its envelope at `age`, leaving its
    /// core of `mc`: a helium white dwarf if the core is degenerate, a zero-age naked helium star
    /// otherwise (HPT section 6).
    fn giant_stripped(&self, core: HeliumCore, age: f64, mc: f64) -> Entry {
        match core {
            HeliumCore::Degenerate => Entry::Dead(self.white_dwarf(
                age,
                white_dwarf_kind(DegenerateCore::Helium),
                mc,
                ProgenitorAtDeath::new(
                    SolarMasses::ZERO,
                    SolarMasses::new(mc),
                    SolarMasses::ZERO,
                    Stripping::None,
                ),
            )),
            HeliumCore::NonDegenerate => Entry::HeliumMainSequence {
                mass: mc,
                tau0: 0.0,
            },
        }
    }

    /// The helium flash of a star of `mass` up to `M_HeF`: the bridge of design note 3 from the
    /// tip of the giant branch to the zero-age horizontal branch, then core helium burning at the
    /// current mass (HPT section 7.1). Without bridges core helium burning starts at the tip, with
    /// a declared step.
    pub(super) fn flash(&self, start: f64, mass: f64, previous: Option<[f64; 3]>) -> Step {
        let tip = match (self.options.bridges(), previous) {
            (Bridges::Physical, Some(tip)) => tip,
            (Bridges::Physical | Bridges::Instant, _) => {
                return self.finish(
                    None,
                    start,
                    Entry::CoreHeliumBurning { m0: mass, mass },
                    None,
                );
            }
        };
        let phase = CoreHeliumBurning::new(SolarMasses::new(mass), self.phys.coeffs);
        let span = Span {
            start: phase.t_start(),
            end: phase.t_end(),
        };
        let horizontal = self.state_of(&Model::CoreHeliumBurning { phase, span }, 0.0, mass, start);
        let end = start + FLASH_YEARS;
        let segment = Segment {
            model: Model::FlashBridge {
                from: tip,
                to: horizontal,
            },
            start,
            end,
            coordinate: Coordinate::Linear { nominal_end: end },
            mass,
            knots: Vec::new(),
            junction: Junction {
                continuous: true,
                ..Junction::STEP
            },
            samples: Vec::new(),
            max_before: [0.0; 2],
        };
        Step {
            segment: Some(segment),
            next: Entry::CoreHeliumBurning { m0: mass, mass },
            end,
            end_state: Some(horizontal),
        }
    }

    /// Core helium burning of a star of initial mass `m0` and mass `mass`.
    pub(super) fn core_helium_burning(
        &self,
        start: f64,
        m0: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let phase = CoreHeliumBurning::new(SolarMasses::new(m0), self.phys.coeffs);
        let span = Span {
            start: phase.t_start(),
            end: phase.t_end(),
        };
        let core_mass = {
            let phase = phase.clone();
            move |age: f64| {
                phase
                    .core_mass(span.at(segment_coordinate(start, span, age)))
                    .value()
            }
        };
        let built = self.envelope_segment(
            Model::CoreHeliumBurning { phase, span },
            start,
            span,
            mass,
            previous,
            self.resolution.knots,
            |age| segment_coordinate(start, span, age),
            |age, m| m - core_mass(age),
            None,
        );
        let next = match built.ending {
            Ending::Nominal => Entry::EarlyAgb {
                m0,
                mass: built.end_mass,
            },
            // HPT equation 76: a helium star of the core's mass at the same fractional age.
            Ending::Envelope => Entry::HeliumMainSequence {
                mass: core_mass(built.end).min(built.end_mass),
                tau0: segment_coordinate(start, span, built.end),
            },
        };
        self.finish(built.segment, built.end, next, previous)
    }

    /// The early AGB of a star of initial mass `m0` and mass `mass`.
    pub(super) fn early_agb(
        &self,
        start: f64,
        m0: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let phase = EarlyAgb::new(SolarMasses::new(m0), self.phys.coeffs);
        let mc_bagb = phase.mc_bagb().value();
        // An oxygen–neon core that would pass the white dwarf's cap before the thermal pulses
        // ends the AGB there ([`Builder::oxygen_neon_cap`]).
        let cap = self.oxygen_neon_cap(m0, mc_bagb).filter(|&cap| {
            phase.end() == EarlyAgbEnd::ThermalPulses && cap < phase.mc_du().value()
        });
        let span = Span {
            start: phase.t_start(),
            end: cap.map_or(phase.t_end(), |cap| {
                phase.time_of_co_core_mass(SolarMasses::new(cap))
            }),
        };
        if span.years() <= 0.0 {
            // The carbon–oxygen core is at `Mc,SN` already (HPT equation 75, 40–80 M☉).
            return self.early_agb_end(&phase, start, m0, mass, None, previous);
        }
        let progress = {
            let phase = phase.clone();
            let (co0, co1) = (
                phase.co_core_mass(span.start).value(),
                phase.co_core_mass(span.end).value(),
            );
            move |age: f64| {
                let clock = span.at(segment_coordinate(start, span, age));
                (phase.co_core_mass(clock).value() - co0) / (co1 - co0)
            }
        };
        let built = self.envelope_segment(
            Model::EarlyAgb {
                phase: phase.clone(),
                helium: HeliumStar::new(phase.mc_bagb()),
                span,
            },
            start,
            span,
            mass,
            previous,
            self.resolution.knots,
            progress,
            |_, m| m - mc_bagb,
            None,
        );
        match built.ending {
            Ending::Nominal if let Some(cap) = cap => {
                // The core reaches the cap at the span's end, to rounding.
                let core = cap.min(built.end_mass);
                let white_dwarf = self.white_dwarf(
                    built.end,
                    Phase::OxygenNeonWhiteDwarf,
                    core,
                    ProgenitorAtDeath::new(
                        SolarMasses::new(core),
                        SolarMasses::new(mc_bagb.min(built.end_mass)),
                        SolarMasses::new((built.end_mass - mc_bagb).max(0.0)),
                        Stripping::None,
                    ),
                );
                self.finish(built.segment, built.end, Entry::Dead(white_dwarf), previous)
            }
            Ending::Nominal => self.early_agb_end(
                &phase,
                built.end,
                m0,
                built.end_mass,
                built.segment,
                previous,
            ),
            Ending::Envelope => {
                // The helium giant of the helium core, at the age at which its core has the early
                // AGB's carbon–oxygen core (HPT section 6).
                let clock = span.at(segment_coordinate(start, span, built.end));
                let (star, clock0) = HeliumStar::from_early_agb(&phase, clock);
                let next = Entry::HeliumShellBurning {
                    star: Box::new(star),
                    clock0,
                    mass: mc_bagb,
                };
                self.finish(built.segment, built.end, next, previous)
            }
        }
    }

    /// What follows the early AGB's nominal end at `age` of a star of initial mass `m0`, with
    /// `mass`: the thermal pulses, or an iron core's collapse where the carbon–oxygen core reaches
    /// `Mc,SN` first.
    fn early_agb_end(
        &self,
        phase: &EarlyAgb,
        age: f64,
        m0: f64,
        mass: f64,
        segment: Option<Segment>,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let mc_bagb = phase.mc_bagb().value();
        let next = match (phase.end(), phase.thermal_pulses()) {
            (EarlyAgbEnd::ThermalPulses, Some(pulsing)) => Entry::ThermallyPulsingAgb {
                phase: Box::new(pulsing),
                m0,
                mc_bagb,
                mass,
            },
            (EarlyAgbEnd::ThermalPulses | EarlyAgbEnd::Supernova, _) => {
                let co = phase.mc_sn().value();
                let envelope = (mass - mc_bagb).max(0.0);
                let supernova = SupernovaType::of_envelopes(
                    SolarMasses::new(envelope),
                    SolarMasses::new((mc_bagb - co).max(0.0)),
                );
                Entry::Dead(self.iron_core(
                    age,
                    supernova,
                    co,
                    ProgenitorAtDeath::new(
                        SolarMasses::new(co),
                        SolarMasses::new(mc_bagb),
                        SolarMasses::new(envelope),
                        Stripping::None,
                    ),
                ))
            }
        };
        self.finish(segment, age, next, previous)
    }

    /// The thermally pulsing AGB of a star whose early AGB had the initial mass `m0` and a core at
    /// the base of the AGB of `mc_bagb`, entered with `mass`, on
    /// [`PULSING_KNOTS`](super::build::PULSING_KNOTS) knots: it ends when the envelope is gone (a
    /// white dwarf) or the core reaches `Mc,SN` first. Each knot carries the thermal pulses since
    /// the phase began.
    pub(super) fn pulsing_agb(
        &self,
        start: f64,
        phase: &ThermallyPulsingAgb,
        m0: f64,
        mc_bagb: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let mc_du = phase.mc_du().value();
        // An oxygen–neon core ends the pulses at the white dwarf's cap if it reaches it before
        // `Mc,SN` ([`Builder::oxygen_neon_cap`]).
        let cap = self
            .oxygen_neon_cap(m0, mc_bagb)
            .filter(|&cap| phase.end() == CoreEnd::Supernova && cap > mc_du);
        let span = Span {
            start: phase.t_start(),
            end: cap.map_or(phase.t_end(), |cap| {
                let t = phase.time_of_core_mass(SolarMasses::new(cap));
                if t < phase.t_end() { t } else { phase.t_end() }
            }),
        };
        if span.years() <= 0.0 || mass <= mc_du {
            return self.pulsing_agb_end(start, m0, mc_bagb, mc_du, mass, None, previous);
        }
        let core_mass = |age: f64| {
            phase
                .core_mass(span.at(segment_coordinate(start, span, age)))
                .value()
        };
        let period = |age: f64, m: f64| {
            let mc = core_mass(age);
            phase
                .interpulse_period(SolarMasses::new(mc), SolarMasses::new((m - mc).max(0.0)))
                .value()
        };
        let built = self.envelope_segment(
            Model::ThermallyPulsingAgb {
                phase: phase.clone(),
                span,
            },
            start,
            span,
            mass,
            previous,
            self.resolution.pulsing_knots,
            |age| segment_coordinate(start, span, age),
            |age, m| m - core_mass(age),
            Some(&period),
        );
        let end = built.end;
        let mc = core_mass(end);
        match (built.ending, phase.end()) {
            (Ending::Nominal, CoreEnd::Supernova) if let Some(cap) = cap => {
                // The core reaches the cap at the span's end, to rounding.
                let core = cap.min(built.end_mass);
                let white_dwarf = self.white_dwarf(
                    end,
                    Phase::OxygenNeonWhiteDwarf,
                    core,
                    ProgenitorAtDeath::new(
                        SolarMasses::new(core),
                        SolarMasses::new(core),
                        SolarMasses::new((built.end_mass - mc).max(0.0)),
                        Stripping::None,
                    ),
                );
                self.finish(built.segment, end, Entry::Dead(white_dwarf), previous)
            }
            (Ending::Nominal, CoreEnd::Supernova) => self.pulsing_agb_end(
                end,
                m0,
                mc_bagb,
                mc,
                built.end_mass,
                built.segment,
                previous,
            ),
            (Ending::Envelope | Ending::Nominal, _) => {
                let white_dwarf = self.agb_white_dwarf(
                    end,
                    m0,
                    mc_bagb,
                    mc,
                    ProgenitorAtDeath::new(
                        SolarMasses::new(mc),
                        SolarMasses::new(mc),
                        SolarMasses::ZERO,
                        Stripping::None,
                    ),
                );
                self.finish(built.segment, end, Entry::Dead(white_dwarf), previous)
            }
        }
    }

    /// The thermally pulsing AGB's end at `age` where the core of `mc` reaches `Mc,SN` with `mass`
    /// and the envelope still on, for a star whose early AGB had the initial mass `m0`: carbon
    /// ignites in a degenerate core below `Mc,BAGB` = 1.6 M☉ and leaves nothing, and an
    /// oxygen–neon core above it collapses by electron capture (HPT section 6), under
    /// [`RemnantRecipe::MandelMuller2020`] only inside the electron-capture window
    /// ([`Builder::oxygen_neon_core`]). A pulsing phase entered with no envelope is a white dwarf at
    /// once.
    #[expect(
        clippy::too_many_arguments,
        reason = "the end's age, the star's two masses at the AGB, its core and mass, and the step"
    )]
    fn pulsing_agb_end(
        &self,
        age: f64,
        m0: f64,
        mc_bagb: f64,
        mc: f64,
        mass: f64,
        segment: Option<Segment>,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let core = mc.min(mass);
        let progenitor = ProgenitorAtDeath::new(
            SolarMasses::new(core),
            SolarMasses::new(core),
            SolarMasses::new((mass - mc).max(0.0)),
            Stripping::None,
        );
        let fate = if mass <= mc {
            self.agb_white_dwarf(age, m0, mc_bagb, mass, progenitor)
        } else if mc_bagb < OXYGEN_NEON_MC_BAGB.value() {
            no_remnant(age, DeathKind::ThermonuclearDisruption, progenitor)
        } else {
            self.oxygen_neon_core(age, m0, mc, progenitor)
        };
        self.finish(segment, age, Entry::Dead(fate), previous)
    }

    /// A naked helium star of `mass` on its main sequence from fractional age `tau0`, whose
    /// initial mass follows the current one (HPT section 7.1).
    ///
    /// A helium star lighter than the core at helium ignition of an `M_HeF` star cannot burn
    /// helium and is a helium white dwarf at once, with a declared step. HPT do not print the
    /// rule; the published SSE code applies it (`hrdiag`, stellar type 7, `zpars(10)`).
    pub(super) fn helium_main_sequence(
        &self,
        start: f64,
        mass: f64,
        tau0: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let lightest = self.lightest_helium_star;
        if mass < lightest {
            return self.helium_white_dwarf(start, mass, None);
        }
        let duration = |m: f64| helium::main_sequence_lifetime(SolarMasses::new(m)).value() * 1e6;
        let built = self.fraction_segment(
            Model::HeliumMainSequence { fixed: None },
            start,
            mass,
            tau0,
            previous,
            duration,
            |m| m - lightest,
            |m| Model::HeliumMainSequence {
                fixed: Some(HeliumStar::new(SolarMasses::new(m))),
            },
        );
        let (end, end_mass) = (built.segment.end, built.end_mass);
        if built.ended {
            return self.helium_white_dwarf(end, end_mass, Some(built.segment));
        }
        let star = HeliumStar::new(SolarMasses::new(end_mass));
        let clock0 = star.t_ms();
        self.finish(
            Some(built.segment),
            end,
            Entry::HeliumShellBurning {
                star: Box::new(star),
                clock0,
                mass: end_mass,
            },
            previous,
        )
    }

    /// A helium star of `mass` too light to burn helium, at `age`: a helium white dwarf.
    fn helium_white_dwarf(&self, age: f64, mass: f64, segment: Option<Segment>) -> Step {
        let fate = self.white_dwarf(
            age,
            white_dwarf_kind(DegenerateCore::Helium),
            mass,
            ProgenitorAtDeath::new(
                SolarMasses::ZERO,
                SolarMasses::new(mass),
                SolarMasses::ZERO,
                Stripping::Wind,
            ),
        );
        self.finish(segment, age, Entry::Dead(fate), None)
    }

    /// A naked helium star after its main sequence, from its clock `clock0`, with `mass`: its
    /// carbon–oxygen core grows until it reaches the core limit at the current mass
    /// ([`HeliumStar::core_limit`]).
    pub(super) fn helium_shell_burning(
        &self,
        start: f64,
        star: &HeliumStar,
        clock0: Megayears,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let span = Span {
            start: clock0,
            end: star.t_end(),
        };
        if span.years() <= 0.0 {
            let mc = star.core_limit(SolarMasses::new(mass)).value();
            return self.helium_star_end(start, star, mc.min(mass), mass, None, previous);
        }
        let core_mass = {
            let star = star.clone();
            move |age: f64| {
                star.core_mass_at(span.at(segment_coordinate(start, span, age)))
                    .value()
            }
        };
        let limit = {
            let star = star.clone();
            move |m: f64| star.core_limit(SolarMasses::new(m)).value()
        };
        let progress = {
            let (mc0, mc1) = (core_mass(start), core_mass(start + span.years()));
            let core_mass = core_mass.clone();
            move |age: f64| (core_mass(age) - mc0) / (mc1 - mc0)
        };
        let built = self.envelope_segment(
            Model::HeliumShellBurning {
                star: star.clone(),
                span,
            },
            start,
            span,
            mass,
            previous,
            self.resolution.knots,
            progress,
            |age, m| limit(m) - core_mass(age),
            None,
        );
        let mc = core_mass(built.end).min(limit(built.end_mass));
        self.helium_star_end(built.end, star, mc, built.end_mass, built.segment, previous)
    }

    /// The end at `age` of a helium star whose core has reached its limit `mc`, with `mass` left:
    /// below the Chandrasekhar mass a white dwarf, carbon–oxygen for a helium star lighter than
    /// 1.6 M☉ and oxygen–neon otherwise (HPT sections 6.1 and 6.2.1); from it, carbon ignition
    /// with no remnant below 1.6 M☉ and a core collapse from there up.
    ///
    /// A carbon–oxygen white dwarf has the helium star's whole mass, and an oxygen–neon one the
    /// core's, as in the published SSE code (`hrdiag`, stellar types 8 and 9), since HPT say only
    /// that the star "becomes a CO WD" when its core reaches `Mc,max` (ruling 40 of 2026-09-22).
    /// The two differ only where the shell limit 1.45 M − 0.31 stopped the core, below 0.689 M☉:
    /// there the unburnt helium stays on the dwarf, and since HPT section 6.3 perturbs the helium
    /// giant towards the white dwarf of its core, the luminosity steps at the hand-over by
    /// log₁₀(M ÷ (1.45 M − 0.31)), as in SSE: 0.26–0.34 dex (by Z) for the lightest helium star
    /// that burns helium, of 0.31–0.35 M☉. The step is at the star's death, which the continuity
    /// test excludes.
    fn helium_star_end(
        &self,
        age: f64,
        star: &HeliumStar,
        mc: f64,
        mass: f64,
        segment: Option<Segment>,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let helium_mass = star.mass().value();
        let oxygen_neon = helium_mass >= OXYGEN_NEON_MC_BAGB.value();
        // HPT section 6.1: the helium star's initial mass stands for the core at the base of the
        // AGB.
        let kind = white_dwarf_kind(DegenerateCore::CarbonOxygen {
            mc_bagb: star.mass(),
        });
        let progenitor = ProgenitorAtDeath::new(
            SolarMasses::new(mc.min(mass)),
            SolarMasses::new(mass),
            SolarMasses::ZERO,
            Stripping::Wind,
        );
        let fate = if mc < agb::CHANDRASEKHAR_MSUN {
            let dwarf = if oxygen_neon { mc.min(mass) } else { mass };
            self.white_dwarf(age, kind, dwarf, progenitor)
        } else if oxygen_neon {
            let supernova = SupernovaType::of_envelopes(
                SolarMasses::ZERO,
                SolarMasses::new((mass - mc).max(0.0)),
            );
            self.iron_core(age, supernova, mc, progenitor)
        } else {
            no_remnant(age, DeathKind::ThermonuclearDisruption, progenitor)
        };
        self.finish(segment, age, Entry::Dead(fate), previous)
    }

    // ---------------------------------------------------------------------------------------------
    // Deaths and remnants.

    /// A white dwarf of `phase` and `mass` formed at `age` by the loss of the envelope.
    fn white_dwarf(
        &self,
        age: f64,
        phase: Phase,
        mass: f64,
        progenitor: ProgenitorAtDeath,
    ) -> Fate {
        let _ = self;
        Fate {
            death: Death::new(Years::new(age), DeathKind::EnvelopeLoss, progenitor),
            remnant: CompactRemnant::new(RemnantKind::WhiteDwarf, SolarMasses::new(mass)),
            phase,
            iron_core: None,
        }
    }

    /// The end at `age` of the thermal pulses of a star whose early AGB had the initial mass `m0`
    /// and a core at the base of the AGB of `mc_bagb`, leaving a white dwarf of `mass`: under
    /// [`RemnantRecipe::MandelMuller2020`] a star inside the single-star electron-capture window
    /// collapses by electron capture instead, however its pulses end (plan 06, design note 12).
    #[must_use]
    fn agb_white_dwarf(
        &self,
        age: f64,
        m0: f64,
        mc_bagb: f64,
        mass: f64,
        progenitor: ProgenitorAtDeath,
    ) -> Fate {
        if self.captures_electrons(m0) {
            return electron_capture(age, progenitor);
        }
        self.white_dwarf(
            age,
            white_dwarf_kind(DegenerateCore::CarbonOxygen {
                mc_bagb: SolarMasses::new(mc_bagb),
            }),
            mass,
            progenitor,
        )
    }

    /// Whether a star whose early AGB had the initial mass `m0` dies by electron capture under
    /// the track's remnant recipe whatever its thermal pulses do: never under
    /// [`RemnantRecipe::Hurley2000`], and under [`RemnantRecipe::MandelMuller2020`] inside the
    /// single-star window [`m_cc` − 0.1 M☉, `m_cc`) (plan 06, design note 12;
    /// [`ElectronCaptureWindows::single`]).
    ///
    /// The window is tested in `m0`, the mass the early AGB's `m_c_bagb` reads, which the main
    /// sequence's wind has lowered from the star's initial mass (HPT section 7.1), because
    /// `m_c_bagb` of that mass is what decides an iron core ([`Builder::early_agb`]): the window
    /// then ends exactly where the iron cores begin, with no gap (ruling 45 of 2026-09-22). The
    /// windows are found here, at the end of the pulses, so that a star that never reaches them
    /// does not pay for the root.
    ///
    /// A companion-stripped star's 1 M☉ window waits for P06.T19.c, which decides the provisional
    /// stripped mark (design note 11), and for plan 11: the track never sets
    /// [`Stripping::Companion`].
    #[must_use]
    fn captures_electrons(&self, m0: f64) -> bool {
        match self.options.remnant() {
            RemnantRecipe::Hurley2000 => false,
            RemnantRecipe::MandelMuller2020 => ElectronCaptureWindows::new(self.phys.coeffs)
                .single()
                .contains(SolarMasses::new(m0)),
        }
    }

    /// The largest core, M☉, an oxygen–neon white dwarf of a star whose early AGB had the initial
    /// mass `m0` and a core at the base of the AGB of `mc_bagb` can have, if the track caps it:
    /// under [`RemnantRecipe::MandelMuller2020`], for an oxygen–neon core (`mc_bagb` of 1.6–2.25
    /// M☉) outside the electron-capture window, [`OXYGEN_NEON_CAPTURE_MASS`] (ruling 57 of
    /// 2026-09-22). The AGB ends when the core reaches it, and the envelope goes then.
    ///
    /// Such a star, below the window, loses its envelope before its core reaches the mass at
    /// which electron captures would collapse it: the competition between the core's growth and
    /// the super-AGB wind decides a super-AGB star's fate, and the window of electron capture that
    /// wins it is at most about 0.2 M☉ of initial mass wide (Doherty et al. 2015, MNRAS 446, 2599,
    /// sections 3.2 and 5). HPT's pulses outrun that wind: on the tracks as built they grow such
    /// cores to `Mc,SN` = 1.44 M☉ with up to 5.9 M☉ of envelope still on, which left dwarfs at the
    /// neutron star's radius. Under [`RemnantRecipe::Hurley2000`] there is no cap, as in HPT and
    /// SSE.
    #[must_use]
    fn oxygen_neon_cap(&self, m0: f64, mc_bagb: f64) -> Option<f64> {
        let oxygen_neon =
            (OXYGEN_NEON_MC_BAGB.value()..IRON_CORE_MC_BAGB.value()).contains(&mc_bagb);
        match self.options.remnant() {
            RemnantRecipe::MandelMuller2020 if oxygen_neon && !self.captures_electrons(m0) => {
                Some(OXYGEN_NEON_CAPTURE_MASS.value())
            }
            RemnantRecipe::MandelMuller2020 | RemnantRecipe::Hurley2000 => None,
        }
    }

    /// An oxygen–neon core of `mc` that reaches `Mc,SN` at `age` on the thermal pulses, with its
    /// envelope still on, of a star whose early AGB had the initial mass `m0`.
    ///
    /// Under [`RemnantRecipe::Hurley2000`] it collapses by electron capture into HPT's remnant of
    /// the core (their section 6 and equation 92), as in the published SSE code. Under
    /// [`RemnantRecipe::MandelMuller2020`] a star inside the single-star window
    /// ([`Builder::captures_electrons`]) collapses by electron capture into Mandel and Müller's
    /// 1.26 M☉ neutron star, and one below it ends as an oxygen–neon white dwarf (plan 06, design
    /// note 12: below the window "the star ends as an `ONe` or CO white dwarf through the AGB").
    /// The window is the brainstorm's: electron capture in a window 0.1 M☉ wide in single stars.
    /// Below the window the pulses end at [`Builder::oxygen_neon_cap`] first, so this is only a
    /// guard: the dwarf is held to the cap.
    #[must_use]
    fn oxygen_neon_core(&self, age: f64, m0: f64, mc: f64, progenitor: ProgenitorAtDeath) -> Fate {
        match self.options.remnant() {
            RemnantRecipe::Hurley2000 => {
                let remnant = hurley_supernova_remnant(SolarMasses::new(mc));
                Fate {
                    death: Death::new(Years::new(age), DeathKind::ElectronCapture, progenitor),
                    remnant,
                    phase: collapse_phase(remnant.kind()),
                    iron_core: None,
                }
            }
            RemnantRecipe::MandelMuller2020 => {
                if self.captures_electrons(m0) {
                    electron_capture(age, progenitor)
                } else {
                    let dwarf = mc.min(OXYGEN_NEON_CAPTURE_MASS.value());
                    self.white_dwarf(age, Phase::OxygenNeonWhiteDwarf, dwarf, progenitor)
                }
            }
        }
    }

    /// The collapse at `age` of an iron core whose carbon–oxygen core, as the track's formulae
    /// give it, is `co_core` (M☉), from `progenitor`, whose envelopes make the supernova
    /// `supernova`: under the track's remnant recipe ([`iron_core_fate`]).
    #[must_use]
    fn iron_core(
        &self,
        age: f64,
        supernova: SupernovaType,
        co_core: f64,
        progenitor: ProgenitorAtDeath,
    ) -> Fate {
        iron_core_fate(
            self.options.remnant(),
            age,
            supernova,
            SolarMasses::new(co_core),
            progenitor,
            self.remnant_draws,
        )
    }
}

/// A death of `kind` at `age` that leaves nothing.
#[must_use]
fn no_remnant(age: f64, kind: DeathKind, progenitor: ProgenitorAtDeath) -> Fate {
    Fate {
        death: Death::new(Years::new(age), kind, progenitor),
        remnant: CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO),
        phase: Phase::NoRemnant,
        iron_core: None,
    }
}

/// An electron-capture supernova at `age` from `progenitor`, leaving Mandel and Müller's 1.26 M☉
/// neutron star ([`electron_capture_remnant`]).
#[must_use]
fn electron_capture(age: f64, progenitor: ProgenitorAtDeath) -> Fate {
    Fate {
        death: Death::new(Years::new(age), DeathKind::ElectronCapture, progenitor),
        remnant: electron_capture_remnant(),
        phase: Phase::NeutronStar,
        iron_core: None,
    }
}

/// The phase of the neutron star or black hole a collapse leaves, or `NoRemnant`. A collapse never
/// leaves a white dwarf.
#[must_use]
const fn collapse_phase(kind: RemnantKind) -> Phase {
    match kind {
        RemnantKind::BlackHole => Phase::BlackHole,
        RemnantKind::NeutronStar | RemnantKind::WhiteDwarf => Phase::NeutronStar,
        RemnantKind::None => Phase::NoRemnant,
    }
}

/// The collapse at `age` of an iron core from `progenitor`, whose envelopes make the supernova
/// `supernova`, under `recipe` (P06.T18.d; **the seam that P06.T10.e left for it**).
///
/// - Under [`RemnantRecipe::Hurley2000`] the remnant is HPT's equation 92 of the carbon–oxygen
///   core `co_core` (P06.T11's [`hurley_supernova_remnant`]), a neutron star or a black hole,
///   and the death is always [`DeathKind::CoreCollapse`], as in the published SSE code.
/// - Under [`RemnantRecipe::MandelMuller2020`] the star's remnant `draws` decide the type and
///   mass from the progenitor's carbon–oxygen and helium cores
///   ([`core_collapse`](crate::stellar::remnant::collapse::core_collapse), Mandel and Müller
///   2020, section 3, with Belczynski et al.'s 2016 pair instability first). A neutron star, or a
///   black hole with some fallback, ends a [`DeathKind::CoreCollapse`] supernova of `supernova`'s
///   type; complete fallback, and the collapse after pulsational pair instability, is a
///   [`DeathKind::DirectCollapse`] with no supernova; a pair-instability supernova is
///   [`DeathKind::PairInstability`] and leaves nothing.
///
/// The fate records `supernova`, so that the remnant stage of P06.T29 can redraw the remnant on
/// the built track ([`Track::fate_with`](super::Track::fate_with)).
#[must_use]
pub(super) fn iron_core_fate(
    recipe: RemnantRecipe,
    age: f64,
    supernova: SupernovaType,
    co_core: SolarMasses,
    progenitor: ProgenitorAtDeath,
    draws: RemnantDraws,
) -> Fate {
    let (kind, remnant) = match recipe {
        RemnantRecipe::Hurley2000 => (
            DeathKind::CoreCollapse { supernova },
            hurley_supernova_remnant(co_core),
        ),
        RemnantRecipe::MandelMuller2020 => {
            let outcome = core_collapse(
                progenitor.co_core_mass(),
                progenitor.helium_core_mass(),
                draws,
            );
            let kind = match outcome {
                CoreCollapse::NeutronStar { .. } | CoreCollapse::BlackHole { .. } => {
                    DeathKind::CoreCollapse { supernova }
                }
                CoreCollapse::DirectCollapse { .. } | CoreCollapse::PulsationalPairInstability => {
                    DeathKind::DirectCollapse
                }
                CoreCollapse::PairInstabilitySupernova => DeathKind::PairInstability,
            };
            (kind, outcome.remnant())
        }
    };
    Fate {
        death: Death::new(Years::new(age), kind, progenitor),
        remnant,
        phase: collapse_phase(remnant.kind()),
        iron_core: Some(IronCore { supernova, co_core }),
    }
}
