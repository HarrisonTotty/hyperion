//! The seam between the surface service and plan 14: where a body's coarse-pass inputs come from
//! (rendering plan R09, Provides: `InputsSource`), and the serve gate that holds generated bodies'
//! fields back until its version (P14.T54.a).

use std::fmt;

use hyperion_sim::planetary::SystemContext;
use hyperion_sim::planetary::record::{BodyRecord, RecordSection};
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError, SurfaceInputsError};
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which [`RecordInputs`] serves a generated body's inputs: 24, the
/// serve gate (plan 14's P14.T54.a, "The serve gate"; decision-bump-plan-after-22 §1.4).
///
/// From version 23 the sim's [`CoarseInputs::for_body`] answers `Ok` for a generated body with a
/// solid surface, but the server answers it not modelled until this version, so that R09's and
/// R10's changes to the coarse pass, its quantisation, its synthesis and the payload re-bless at the
/// version in force with no bump until then. The gate opens at the first bump after R09.T16 has
/// landed and R11's liquid members are in the codec, expected to be 24; otherwise the wiring lane
/// raises this constant to the next bump. "Main" may open it at an earlier bump if an R10 task
/// needs a generated world served sooner. Its tripwire,
/// `the_serve_gate_holds_until_its_version`, fails at the version, so that the bump that reaches it
/// opens the gate or raises it.
pub(crate) const GENERATED_SURFACES_FROM: GeneratorVersion = GeneratorVersion::new(24);

/// Where the surface service takes a body's coarse-pass inputs from.
///
/// [`SurfaceService`](super::SurfaceService) calls it on a pool worker, once for each field it
/// computes, with the body's full record at [`RECORD_TIME`](super::RECORD_TIME), a moon's
/// planet's full record taken with it, and its system's context: the arguments of
/// [`CoarseInputs::for_body`] (R09.T10.f), so that every source reads records into inputs.
/// Production uses [`RecordInputs`]; the server's tests use the coarse pass's reference worlds, so
/// that the service and the wire are tested before the serve gate opens
/// (`GENERATED_SURFACES_FROM`).
pub trait InputsSource: fmt::Debug + Send + Sync {
    /// The coarse-pass inputs of the body of `record`, in the system of `ctx`, with `planet` the
    /// record of the planet it orbits if it is a moon, `Some` exactly where
    /// [`CoarseInputs::planet_of`] is.
    ///
    /// # Errors
    ///
    /// [`ReadCoarseInputsError::Section`] for the body's own state: with
    /// [`NoSolidSurface`](hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface)
    /// for a body with no solid surface, and
    /// [`NotModelled`](hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled) or
    /// [`NotResolved`](hyperion_sim::planetary::surface::SurfaceInputsError::NotResolved) naming a
    /// section the pass reads that the record does not model or withholds. The service passes
    /// full records, so the last is a bug there, as is every other variant, a fault of the
    /// service's own reading (a planet's record not given or not the moon's, a record not taken
    /// at the record's time, inputs the builder refuses).
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError>;
}

/// The production source: the body's record, with a moon's planet's, read by
/// [`CoarseInputs::for_body`], behind the serve gate (`GENERATED_SURFACES_FROM`, 24).
///
/// From the gate's version it answers as `for_body` does. Until then it answers as generator
/// version 22 did, whose reading stopped at the crust, which the record did not model: the
/// surface section's tag's refusal ([`SurfaceInputsError::NoSolidSurface`] for a section that is
/// not applicable, a giant's, and [`SurfaceInputsError::NotModelled`] or
/// [`SurfaceInputsError::NotResolved`] naming [`RecordSection::Surface`]), and a moon's missing
/// or wrong planet's record its refusal ([`ReadCoarseInputsError::PlanetMissing`],
/// [`ReadCoarseInputsError::NotItsPlanet`]). Every other answer, every generated body with a solid
/// surface that `for_body` reads and every refusal 22 never gave (another section's, a record
/// not at the record's time, inputs the builder refuses), is [`SurfaceInputsError::NotModelled`]
/// naming [`RecordSection::Surface`], in [`ReadCoarseInputsError::Section`] (P14.T54.a).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordInputs;

impl InputsSource for RecordInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        let read = CoarseInputs::for_body(record, planet, ctx);
        if GENERATOR_VERSION < GENERATED_SURFACES_FROM {
            // The serve gate (P14.T54.a): the reading is the sim's, but no generated field is
            // served until the gate's version, and no refusal 22 never gave.
            return Err(behind_the_gate(&read));
        }
        read
    }
}

/// What [`RecordInputs`] answers behind the serve gate for `for_body`'s reading `read`: its
/// refusal where generator version 22 gave it, the surface section's tag's and a moon's planet's,
/// which `for_body` checks before the crust; otherwise not modelled, naming the surface section.
fn behind_the_gate(read: &Result<CoarseInputs, ReadCoarseInputsError>) -> ReadCoarseInputsError {
    let not_modelled =
        ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(RecordSection::Surface));
    match read {
        Err(
            refused @ (ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
                | SurfaceInputsError::NotModelled(RecordSection::Surface)
                | SurfaceInputsError::NotResolved(RecordSection::Surface),
            )
            | ReadCoarseInputsError::PlanetMissing(_)
            | ReadCoarseInputsError::NotItsPlanet { .. }),
        ) => *refused,
        Ok(_)
        | Err(
            ReadCoarseInputsError::Section(
                SurfaceInputsError::NotModelled(_) | SurfaceInputsError::NotResolved(_),
            )
            | ReadCoarseInputsError::NotAtRecordTime
            | ReadCoarseInputsError::SeasonalOrbit(_)
            | ReadCoarseInputsError::Build(_),
        ) => not_modelled,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::placement::CellKey;
    use hyperion_sim::id::Layer;
    use hyperion_sim::planetary::derive::PlanetClass;
    use hyperion_sim::planetary::generate;
    use hyperion_sim::planetary::record::{BulkProperties, Section};
    use hyperion_sim::planetary::surface::RECORD_TIME;
    use hyperion_sim::units::{Dex, SolarMasses, Years};

    use super::*;

    /// The serve gate's tripwire: it holds until [`GENERATED_SURFACES_FROM`], and the bump that
    /// reaches it opens it or raises it (plan 14's P14.T54.a, "The serve gate").
    #[test]
    fn the_serve_gate_holds_until_its_version() {
        assert!(
            GENERATOR_VERSION < GENERATED_SURFACES_FROM,
            "generator version {} reaches the serve gate's {}: if R09.T16 has landed and R11's \
             liquid members are in the codec, open the gate (delete `GENERATED_SURFACES_FROM`, \
             its arm in `RecordInputs::inputs` and this test, and let the server's surface tests \
             serve a generated body's field, bit for bit with the sim's pass); otherwise raise \
             the constant to the next bump (plan 14's P14.T54.a, \"The serve gate\")",
            GENERATOR_VERSION.get(),
            GENERATED_SURFACES_FROM.get()
        );
    }

    /// Behind the gate, a generated rocky planet the sim reads `Ok` answers that its surface is
    /// not modelled, naming the section, and a giant still that it has no solid surface.
    #[test]
    fn a_generated_body_the_sim_reads_is_not_served_behind_the_gate() {
        let cell = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell in range");
        let (mut rocky, mut giant) = (0, 0);
        for index in 1..16 {
            let ctx = SystemContext::builder()
                .system(cell.candidate_id(index).expect("an index in range"))
                .star(SolarMasses::new(1.0))
                .fe_h(Dex::ZERO)
                .age_at_epoch(Years::new(4.57e9))
                .build()
                .expect("a Sun-like context");
            let system = generate(Seed::new(0x5eed_0009_0010), &ctx);
            let snapshot = system.snapshot_at(&ctx, RECORD_TIME);
            for record in snapshot.bodies() {
                if CoarseInputs::planet_of(record).is_some() {
                    continue;
                }
                let class = record.bulk().ok().map(BulkProperties::class);
                match (record.surface(), class) {
                    (Section::Ok(_), Some(PlanetClass::Rocky)) => {
                        let Ok(_) = CoarseInputs::for_body(record, None, &ctx) else {
                            continue;
                        };
                        assert_eq!(
                            RecordInputs.inputs(record, None, &ctx),
                            Err(ReadCoarseInputsError::Section(
                                SurfaceInputsError::NotModelled(RecordSection::Surface)
                            ))
                        );
                        rocky += 1;
                    }
                    (Section::NotApplicable, Some(PlanetClass::GasGiant)) => {
                        assert_eq!(
                            RecordInputs.inputs(record, None, &ctx),
                            Err(ReadCoarseInputsError::Section(
                                SurfaceInputsError::NoSolidSurface
                            ))
                        );
                        giant += 1;
                    }
                    _ => {}
                }
            }
            if rocky > 0 && giant > 0 {
                return;
            }
        }
        panic!("15 Sun-like systems hold {rocky} rocky planets the sim reads and {giant} giants");
    }

    /// Behind the gate only the refusals generator version 22 gave pass, the surface section's
    /// tag's and a moon's planet's; every other refusal `for_body` can give from 23, another
    /// section's, a record not at the record's time or inputs the builder refuses, answers that
    /// the surface is not modelled, as 22's reading, which stopped at the crust, did.
    #[test]
    fn behind_the_gate_only_22_s_refusals_pass() {
        use hyperion_sim::planetary::surface::BuildCoarseInputsError;

        let ctx = SystemContext::builder()
            .system(
                CellKey::new(Layer::C, [0, 812, 0])
                    .expect("a cell in range")
                    .candidate_id(1)
                    .expect("an index in range"),
            )
            .star(SolarMasses::new(1.0))
            .fe_h(Dex::ZERO)
            .age_at_epoch(Years::new(4.57e9))
            .build()
            .expect("a Sun-like context");
        let system = generate(Seed::new(0x5eed_0009_0010), &ctx);
        let snapshot = system.snapshot_at(&ctx, RECORD_TIME);
        let body = snapshot
            .bodies()
            .first()
            .expect("a Sun-like system holds a body")
            .identity()
            .id();
        let section = ReadCoarseInputsError::Section;
        let not_modelled = section(SurfaceInputsError::NotModelled(RecordSection::Surface));
        for passes in [
            section(SurfaceInputsError::NoSolidSurface),
            not_modelled,
            section(SurfaceInputsError::NotResolved(RecordSection::Surface)),
            ReadCoarseInputsError::PlanetMissing(body),
            ReadCoarseInputsError::NotItsPlanet {
                planet: Some(body),
                given: body,
            },
            ReadCoarseInputsError::NotItsPlanet {
                planet: None,
                given: body,
            },
        ] {
            assert_eq!(behind_the_gate(&Err(passes)), passes);
        }
        for held in [
            section(SurfaceInputsError::NotModelled(RecordSection::Bulk)),
            section(SurfaceInputsError::NotResolved(RecordSection::Orbit)),
            ReadCoarseInputsError::NotAtRecordTime,
            ReadCoarseInputsError::Build(BuildCoarseInputsError::Missing("palette")),
        ] {
            assert_eq!(behind_the_gate(&Err(held)), not_modelled, "{held:?}");
        }
    }
}
