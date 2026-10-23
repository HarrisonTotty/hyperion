//! A solid body under a supercritical envelope (plan 14, P14.T53.b part 2;
//! decision-absurd-states-24 R2): the record's surface section does not apply where a condensable
//! above its critical temperature holds a liquid-like reservoir of at least
//! [`THIN_ENVELOPE_FRACTION`] of the body's mass over its ground.
//!
//! - **The line.** Above its critical temperature `T_c` a fluid has no interface, and its
//!   gas-like and liquid-like regimes are divided by no first-order singularity. Their line here
//!   is the critical isochore, which approximates the Widom line (Simeoni, Bryk, Gorelli, Krisch,
//!   Ruocco, Santoro and Scopigno 2010, Nat. Phys. 6, 503–507): p* = `p_c` + `s_c` (`T_s` − `T_c`),
//!   the saturation curve's tangent at its critical point continued above it
//!   ([`PhaseDiagram::critical_isochore_pressure`]), at the published mean surface temperature
//!   `T_s` (decision-p14-t51a-partition §5).
//! - **The reservoir.** A condensable whose own partial pressure `p_i` exceeds p* holds a
//!   liquid-like column between p* and the lesser of `p_i` and its melting pressure at `T_s`
//!   ([`PhaseDiagram::melting_pressure`], where the reference gives a rising branch there; else
//!   `p_i`): the fluid part of it, a pure substance being solid at and above its melting pressure
//!   ([`PhaseDiagram::phase_at`]). Its share of the body's mass is the column's weight over
//!   the sphere, 4πR² Δp ÷ (g M) = 4π G Δp ÷ g², with g = G M ÷ R² the bulk's surface gravity.
//! - **The section.** Where any one condensable's share is at least [`THIN_ENVELOPE_FRACTION`],
//!   the share that makes a hydrogen envelope a sub-Neptune's (P14.T16.a), the body's surface
//!   does not apply: its record publishes the surface [`Section::NotApplicable`] and the envelope
//!   [`Section::NotModelled`], the gas-envelope branch's tags, until part 3 models the envelope.
//!   R09's coarse pass then refuses the body by its tag, as it refuses a giant
//!   (decision-p14-t35e-wire §2.4).
//!
//! The test reads only `T_s` at or above `T_c`: below it the partition holds a condensable at its
//! saturation, and a saturated `p_i` lies just above the line's continuation below `T_c`, so the
//! line would fire there on every saturated air. At `T_c` the line is `p_c` itself.
//!
//! What stays at 24 and is part 3's: the partition's cap at p*, the reservoir's mass as a surface
//! condensate, the figure's datum (`derive/figure.rs`), the photometry's template and the body's
//! internal state (decision-absurd-states-24 R2.3).
//!
//! It is in force under the escape's batch rules ([`EscapeRules::Batch`], from its hold's
//! version 24) and never under the hold, so that version 23's record is unchanged to the bit.
//! P14.T54.b deletes the hold and keeps the test.
//!
//! [`PhaseDiagram::critical_isochore_pressure`]: crate::substance::diagram::PhaseDiagram::critical_isochore_pressure
//! [`PhaseDiagram::melting_pressure`]: crate::substance::diagram::PhaseDiagram::melting_pressure
//! [`PhaseDiagram::phase_at`]: crate::substance::diagram::PhaseDiagram::phase_at
//! [`Section::NotApplicable`]: crate::planetary::record::Section::NotApplicable
//! [`Section::NotModelled`]: crate::planetary::record::Section::NotModelled

use core::f64::consts::PI;

use super::{Atmosphere, Gas, PartialPressures};
use crate::planetary::chem::escape::EscapeRules;
use crate::planetary::derive::PlanetClass;
use crate::planetary::params::THIN_ENVELOPE_FRACTION;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kelvin, MetresPerSecondSquared, Pascals};

/// A condensable at or above its critical temperature at a body's surface, against its critical
/// isochore: its liquid-like reservoir, if any, and that reservoir's share of the body's mass (the
/// [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SupercriticalReservoir {
    gas: Gas,
    temperature_ratio: f64,
    partial_pressure: Pascals,
    isochore_pressure: Pascals,
    melting_pressure: Option<Pascals>,
    share: f64,
}

impl SupercriticalReservoir {
    /// The column of `gas` at the partial pressure `partial` over a surface at the mean
    /// temperature `temperature` under the gravity `gravity`.
    ///
    /// `None` for a gas with no phase diagram or no critical slope, at a temperature below its
    /// critical one or not a number, and for a partial pressure or a gravity that is not finite and
    /// positive. Its share is 0 where the partial pressure lies at or below the critical
    /// isochore, or the melting pressure at or below it.
    #[must_use]
    pub(crate) fn of(
        gas: Gas,
        temperature: Kelvin,
        partial: Pascals,
        gravity: MetresPerSecondSquared,
    ) -> Option<Self> {
        let (p_i, g) = (partial.value(), gravity.value());
        if !(p_i.is_finite() && p_i > 0.0 && g.is_finite() && g > 0.0) {
            return None;
        }
        let diagram = gas.substance().substance().phase_diagram()?;
        let isochore = diagram.critical_isochore_pressure(temperature)?;
        let melting = diagram.melting_pressure(temperature);
        let top = melting.map_or(p_i, |melting| melting.value().min(p_i));
        let column = top - isochore.value();
        let share = if column > 0.0 {
            4.0 * PI * GRAVITATIONAL_CONSTANT * column / (g * g)
        } else {
            0.0
        };
        Some(Self {
            gas,
            temperature_ratio: temperature.value() / diagram.critical_temperature().value(),
            partial_pressure: partial,
            isochore_pressure: isochore,
            melting_pressure: melting,
            share,
        })
    }

    /// The condensable.
    #[cfg_attr(not(test), expect(dead_code, reason = "the census's tests read it"))]
    #[must_use]
    pub(crate) const fn gas(&self) -> Gas {
        self.gas
    }

    /// `T_s` ÷ `T_c`, at least 1.
    #[cfg_attr(not(test), expect(dead_code, reason = "the census's tests read it"))]
    #[must_use]
    pub(crate) const fn temperature_ratio(&self) -> f64 {
        self.temperature_ratio
    }

    /// Its own partial pressure `p_i`.
    #[cfg_attr(not(test), expect(dead_code, reason = "the census's tests read it"))]
    #[must_use]
    pub(crate) const fn partial_pressure(&self) -> Pascals {
        self.partial_pressure
    }

    /// Its critical isochore's pressure p* at `T_s`.
    #[cfg_attr(not(test), expect(dead_code, reason = "the census's tests read it"))]
    #[must_use]
    pub(crate) const fn isochore_pressure(&self) -> Pascals {
        self.isochore_pressure
    }

    /// Its melting pressure at `T_s` on the melting curve's rising branch, `None` where its
    /// reference gives none there.
    #[cfg_attr(not(test), expect(dead_code, reason = "the census's tests read it"))]
    #[must_use]
    pub(crate) const fn melting_pressure(&self) -> Option<Pascals> {
        self.melting_pressure
    }

    /// The liquid-like reservoir's share of the body's mass, 4π G Δp ÷ g², not negative: 0 where
    /// there is none.
    #[must_use]
    pub(crate) const fn share(&self) -> f64 {
        self.share
    }

    /// Whether the reservoir hides the body's surface: its share at least
    /// [`THIN_ENVELOPE_FRACTION`].
    #[must_use]
    pub(crate) fn hides_the_surface(&self) -> bool {
        self.share >= THIN_ENVELOPE_FRACTION
    }
}

/// Each condensable of `partials` at or above its critical temperature at the mean surface
/// temperature `temperature`, under the gravity `gravity`, in [`Gas::ALL`]'s order
/// ([`SupercriticalReservoir::of`]), whatever its share.
pub(crate) fn critical_columns(
    temperature: Kelvin,
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
) -> impl Iterator<Item = SupercriticalReservoir> + '_ {
    Gas::ALL.into_iter().filter_map(move |gas| {
        SupercriticalReservoir::of(gas, temperature, partials.of(gas), gravity)
    })
}

/// The reservoir that hides the surface ([`SupercriticalReservoir::hides_the_surface`]) with the
/// largest share among [`critical_columns`]', the first in [`Gas::ALL`]'s order of equal shares;
/// `None` where none does. It reads no rules: [`Atmosphere::supercritical_reservoir`] is the
/// record's, under the escape's hold.
#[must_use]
pub(crate) fn hiding_reservoir(
    temperature: Kelvin,
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
) -> Option<SupercriticalReservoir> {
    critical_columns(temperature, partials, gravity)
        .filter(SupercriticalReservoir::hides_the_surface)
        .fold(None, |largest, reservoir| match largest {
            Some(largest) if largest.share() >= reservoir.share() => Some(largest),
            _ => Some(reservoir),
        })
}

/// Whether a supercritical envelope hides a solid body's surface on the record (P14.T53.b part 2):
/// under the escape's batch rules ([`EscapeRules::Batch`], from its hold's version 24), and never
/// under the hold, to generator version 23.
#[must_use]
pub(crate) fn supercritical_section_in_force() -> bool {
    EscapeRules::in_force() == EscapeRules::Batch
}

/// Whether a present body of class `class` whose atmosphere is `air`, under the bulk's surface
/// gravity `gravity`, has its surface section on the record: a class with a solid surface
/// ([`PlanetClass::has_solid_surface`]) not under a supercritical envelope
/// ([`Atmosphere::supercritical_reservoir`]). Where it has not, its envelope section applies
/// instead (P14.T48.e; P14.T53.b part 2), so that exactly one of the two does.
#[must_use]
pub(crate) fn has_surface_section(
    class: PlanetClass,
    air: &Atmosphere,
    gravity: MetresPerSecondSquared,
) -> bool {
    class.has_solid_surface() && air.supercritical_reservoir(gravity).is_none()
}

impl Atmosphere {
    /// The supercritical reservoir that hides the body's surface under the bulk's surface gravity
    /// `gravity` (P14.T53.b part 2; the [module documentation](self)): the condensable at or above
    /// its critical temperature at the mean surface temperature whose liquid-like column above its
    /// critical isochore holds the largest share of the body's mass, at least
    /// [`THIN_ENVELOPE_FRACTION`]. `None` where none does, for a gas envelope, whose partial
    /// pressures are all zero, and under the escape's hold, to generator version 23
    /// ([`supercritical_section_in_force`]).
    #[must_use]
    pub(crate) fn supercritical_reservoir(
        &self,
        gravity: MetresPerSecondSquared,
    ) -> Option<SupercriticalReservoir> {
        if !supercritical_section_in_force() {
            return None;
        }
        hiding_reservoir(
            self.surface_temperature(),
            self.partial_pressures(),
            gravity,
        )
    }

    /// Each condensable of the air at or above its critical temperature at the mean surface
    /// temperature, under the gravity `gravity`, whatever its share and the rules
    /// ([`critical_columns`]): what a census states of its margins.
    #[cfg(test)]
    pub(crate) fn critical_columns(
        &self,
        gravity: MetresPerSecondSquared,
    ) -> impl Iterator<Item = SupercriticalReservoir> + '_ {
        critical_columns(
            self.surface_temperature(),
            self.partial_pressures(),
            gravity,
        )
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::{assert_same_bits, bits};

    use super::*;
    use crate::planetary::chem::escape::with_rules;

    /// Earth's standard gravity, m s⁻², for the hand-built airs.
    const G: MetresPerSecondSquared = MetresPerSecondSquared::new(9.806_65);

    /// `gas` alone at `pressure`.
    fn alone(gas: Gas, pressure: f64) -> PartialPressures {
        PartialPressures::from_fn(|g| Pascals::new(if g == gas { pressure } else { 0.0 }))
    }

    /// The partial pressure above `gas`'s isochore at `t` whose column holds `share` of the mass
    /// under [`G`].
    fn at_share(gas: Gas, t: Kelvin, share: f64) -> f64 {
        let diagram = gas.substance().substance().phase_diagram().unwrap();
        let isochore = diagram.critical_isochore_pressure(t).unwrap().value();
        isochore + share * G.value() * G.value() / (4.0 * PI * GRAVITATIONAL_CONSTANT)
    }

    /// A methane air at 200 K: just under its isochore it holds no reservoir; a column just under
    /// a thousandth of the mass keeps the surface, and one just over it hides it; the column stops
    /// at the melting pressure.
    #[test]
    fn a_body_just_under_the_line_keeps_its_surface_and_one_just_over_it_does_not() {
        let t = Kelvin::new(200.0);
        let methane = Gas::Methane
            .substance()
            .substance()
            .phase_diagram()
            .unwrap();
        let isochore = methane.critical_isochore_pressure(t).unwrap().value();
        let under = alone(Gas::Methane, isochore * (1.0 - 1e-9));
        let column = critical_columns(t, &under, G).next().unwrap();
        assert_eq!(column.gas(), Gas::Methane);
        assert_same_bits(column.share(), 0.0);
        let hidden = hiding_reservoir(t, &under, G);
        assert!(hidden.is_none(), "{hidden:?}");
        let just_under = alone(Gas::Methane, at_share(Gas::Methane, t, 0.999_999e-3));
        let column = critical_columns(t, &just_under, G).next().unwrap();
        assert!(
            column.share() > 0.99e-3 && !column.hides_the_surface(),
            "{column:?}"
        );
        let hidden = hiding_reservoir(t, &just_under, G);
        assert!(hidden.is_none(), "{hidden:?}");
        let just_over = alone(Gas::Methane, at_share(Gas::Methane, t, 1.000_001e-3));
        let hiding = hiding_reservoir(t, &just_over, G).unwrap();
        assert_eq!(hiding.gas(), Gas::Methane);
        assert!(hiding.share() < 1.01e-3, "{hiding:?}");
        assert!((hiding.temperature_ratio() - 200.0 / 190.564).abs() < 1e-12);
        let melting = hiding.melting_pressure().unwrap().value();
        assert!(melting > hiding.partial_pressure().value(), "{hiding:?}");
        assert!(hiding.isochore_pressure().value() > methane.critical_pressure().value());
        // Above its melting pressure the fluid is solid, so the column stops there.
        let deep = alone(Gas::Methane, 10.0 * melting);
        let capped = hiding_reservoir(t, &deep, G).unwrap();
        let column = melting - isochore;
        let share = 4.0 * PI * GRAVITATIONAL_CONSTANT * column / (G.value() * G.value());
        assert_same_bits(capped.share(), share);
    }

    /// Below its critical temperature a condensable holds no supercritical column, whatever its
    /// partial pressure; at it the line is the critical pressure.
    #[test]
    fn below_the_critical_temperature_it_never_fires() {
        for gas in [Gas::Methane, Gas::Nitrogen, Gas::CarbonDioxide, Gas::Water] {
            let diagram = gas.substance().substance().phase_diagram().unwrap();
            let t_c = diagram.critical_temperature();
            for below in [1e-9, 1.0, 50.0] {
                let t = Kelvin::new(t_c.value() - below);
                for pressure in [1e5, 1e8, 1e11] {
                    let air = alone(gas, pressure);
                    assert_eq!(critical_columns(t, &air, G).count(), 0, "{gas:?} at {t:?}");
                }
            }
            let at = critical_columns(t_c, &alone(gas, 1e11), G).next().unwrap();
            assert_eq!(
                bits(at.isochore_pressure().value()),
                bits(diagram.critical_pressure().value()),
                "{gas:?}"
            );
            assert_eq!(bits(at.temperature_ratio()), bits(1.0), "{gas:?}");
        }
    }

    /// Of two condensables that hide the surface, the larger share is the body's; a gravity, a
    /// temperature or a pressure that is not finite and positive holds none.
    #[test]
    fn the_largest_reservoir_is_the_body_s_and_odd_inputs_hold_none() {
        let t = Kelvin::new(250.0);
        let nitrogen = at_share(Gas::Nitrogen, t, 2e-3);
        let methane = at_share(Gas::Methane, t, 5e-3);
        let air = PartialPressures::from_fn(|gas| {
            Pascals::new(match gas {
                Gas::Nitrogen => nitrogen,
                Gas::Methane => methane,
                _ => 0.0,
            })
        });
        let columns: Vec<Gas> = critical_columns(t, &air, G).map(|c| c.gas()).collect();
        assert_eq!(columns, [Gas::Methane, Gas::Nitrogen]);
        assert_eq!(hiding_reservoir(t, &air, G).unwrap().gas(), Gas::Methane);
        for gravity in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let gravity = MetresPerSecondSquared::new(gravity);
            let hidden = hiding_reservoir(t, &air, gravity);
            assert!(hidden.is_none(), "{gravity:?}: {hidden:?}");
        }
        let hidden = hiding_reservoir(Kelvin::new(f64::NAN), &air, G);
        assert!(hidden.is_none(), "{hidden:?}");
        let odd = PartialPressures::from_fn(|_| Pascals::new(f64::NAN));
        let hidden = hiding_reservoir(t, &odd, G);
        assert!(hidden.is_none(), "{hidden:?}");
        let hidden = hiding_reservoir(t, &PartialPressures::default(), G);
        assert!(hidden.is_none(), "{hidden:?}");
    }

    /// The record's test is in force under the escape's batch rules alone.
    #[test]
    fn the_section_is_in_force_under_the_batch_alone() {
        assert!(!with_rules(
            EscapeRules::Held,
            supercritical_section_in_force
        ));
        assert!(with_rules(
            EscapeRules::Batch,
            supercritical_section_in_force
        ));
    }
}
