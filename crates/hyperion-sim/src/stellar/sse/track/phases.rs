//! The phases of a track in the order a single star passes through them (HPT sections 5 and 6,
//! with the rules of section 7.1 for the initial mass), and how each ends.

use crate::stellar::Phase;
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
use super::{Bridges, Coordinate, Fate, Junction, Segment};

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
        let span = Span {
            start: phase.t_start(),
            end: phase.t_end(),
        };
        if span.years() <= 0.0 {
            // The carbon–oxygen core is at `Mc,SN` already (HPT equation 75, 40–80 M☉).
            return self.early_agb_end(&phase, start, mass, None, previous);
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
            Ending::Nominal => {
                self.early_agb_end(&phase, built.end, built.end_mass, built.segment, previous)
            }
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

    /// What follows the early AGB's nominal end at `age`, with `mass`: the thermal pulses, or a
    /// core-collapse supernova where the carbon–oxygen core reaches `Mc,SN` first.
    fn early_agb_end(
        &self,
        phase: &EarlyAgb,
        age: f64,
        mass: f64,
        segment: Option<Segment>,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let mc_bagb = phase.mc_bagb().value();
        let next = match (phase.end(), phase.thermal_pulses()) {
            (EarlyAgbEnd::ThermalPulses, Some(pulsing)) => Entry::ThermallyPulsingAgb {
                phase: Box::new(pulsing),
                mc_bagb,
                mass,
            },
            (EarlyAgbEnd::ThermalPulses | EarlyAgbEnd::Supernova, _) => {
                let co = phase.mc_sn().value();
                let envelope = (mass - mc_bagb).max(0.0);
                Entry::Dead(self.remnant_fate(
                    age,
                    DeathKind::CoreCollapse {
                        supernova: SupernovaType::of_envelopes(
                            SolarMasses::new(envelope),
                            SolarMasses::new((mc_bagb - co).max(0.0)),
                        ),
                    },
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

    /// The thermally pulsing AGB of a star whose core at the base of the AGB was `mc_bagb`,
    /// entered with `mass`, on [`PULSING_KNOTS`](super::build::PULSING_KNOTS) knots: it ends when
    /// the envelope is gone (a white dwarf) or the core reaches `Mc,SN` first. Each knot carries
    /// the thermal pulses since the phase began.
    pub(super) fn pulsing_agb(
        &self,
        start: f64,
        phase: &ThermallyPulsingAgb,
        mc_bagb: f64,
        mass: f64,
        previous: Option<[f64; 3]>,
    ) -> Step {
        let span = Span {
            start: phase.t_start(),
            end: phase.t_end(),
        };
        let mc_du = phase.mc_du().value();
        if span.years() <= 0.0 || mass <= mc_du {
            return self.pulsing_agb_end(start, mc_bagb, mc_du, mass, None, previous);
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
            (Ending::Nominal, CoreEnd::Supernova) => {
                self.pulsing_agb_end(end, mc_bagb, mc, built.end_mass, built.segment, previous)
            }
            (Ending::Envelope | Ending::Nominal, _) => {
                let white_dwarf = self.white_dwarf(
                    end,
                    white_dwarf_kind(DegenerateCore::CarbonOxygen {
                        mc_bagb: SolarMasses::new(mc_bagb),
                    }),
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
    /// and the envelope still on: carbon ignites in a degenerate core below `Mc,BAGB` = 1.6 M☉ and
    /// leaves nothing, and an oxygen–neon core above it collapses by electron capture (HPT section
    /// 6). A pulsing phase entered with no envelope is a white dwarf at once.
    fn pulsing_agb_end(
        &self,
        age: f64,
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
            self.white_dwarf(
                age,
                white_dwarf_kind(DegenerateCore::CarbonOxygen {
                    mc_bagb: SolarMasses::new(mc_bagb),
                }),
                mass,
                progenitor,
            )
        } else if mc_bagb < OXYGEN_NEON_MC_BAGB.value() {
            no_remnant(age, DeathKind::ThermonuclearDisruption, progenitor)
        } else {
            self.remnant_fate(age, DeathKind::ElectronCapture, mc, progenitor)
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
            self.remnant_fate(age, DeathKind::CoreCollapse { supernova }, mc, progenitor)
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
        }
    }

    /// A death of `kind` at `age` leaving the neutron star or black hole of a carbon–oxygen core
    /// of `co_core` (M☉).
    fn remnant_fate(
        &self,
        age: f64,
        kind: DeathKind,
        co_core: f64,
        progenitor: ProgenitorAtDeath,
    ) -> Fate {
        let remnant = collapse_remnant(self.options.remnant(), SolarMasses::new(co_core));
        let phase = match remnant.kind() {
            RemnantKind::BlackHole => Phase::BlackHole,
            RemnantKind::NeutronStar | RemnantKind::WhiteDwarf => Phase::NeutronStar,
            RemnantKind::None => Phase::NoRemnant,
        };
        Fate {
            death: Death::new(Years::new(age), kind, progenitor),
            remnant,
            phase,
        }
    }
}

/// A death of `kind` at `age` that leaves nothing.
#[must_use]
fn no_remnant(age: f64, kind: DeathKind, progenitor: ProgenitorAtDeath) -> Fate {
    Fate {
        death: Death::new(Years::new(age), kind, progenitor),
        remnant: CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO),
        phase: Phase::NoRemnant,
    }
}

/// The remnant of a core collapse of a carbon–oxygen core of `co_core` under `recipe`: a neutron
/// star or a black hole.
///
/// Under [`RemnantRecipe::Hurley2000`] it is HPT's equation 92 (P06.T11's
/// [`hurley_supernova_remnant`]). **This is the seam of P06.T18.d**: under
/// [`RemnantRecipe::MandelMuller2020`] HPT's remnant stands in until P06.T18.d replaces this arm
/// with Mandel and Müller's (2020) type and mass, which read the star's draws too.
#[must_use]
pub(crate) fn collapse_remnant(recipe: RemnantRecipe, co_core: SolarMasses) -> CompactRemnant {
    match recipe {
        RemnantRecipe::Hurley2000 | RemnantRecipe::MandelMuller2020 => {
            hurley_supernova_remnant(co_core)
        }
    }
}
