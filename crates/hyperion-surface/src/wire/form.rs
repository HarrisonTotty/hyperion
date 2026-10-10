//! The wire form of every value a block carries: one table per record, from which both the
//! writer and the reader are generated (plan R09, T3).
//!
//! [`Wire`] writes a value little-endian and reads it back; [`FixedWire`] adds the length of a
//! value whose form is always the same length, which a record's table sums, so that a record's
//! stride is its table's and nothing else's. A record's table ([`wire_table!`]) lists its fields
//! in their wire order with their types: the writer destructures the record exhaustively, so a
//! field added to the record without a line here fails to compile, and the reader builds the
//! record in the same order. Adding a field to a record, or a part to the header, is one line of
//! its table and the part's own `Wire` form.

use hyperion_base::units::{
    Gigayears, Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, Pascals, PerSquareKilometre, SquareMetres,
};

use super::DecodeBlockError;
use crate::craters::{CraterParams, CraterParamsParts, Screening};
use crate::field::{
    BodyRef, BoundaryKind, ClimateCell, ClimateModelKind, Crust, FieldHeaderParts, FlowDirection,
    LogArea, LogPrecipitation, LogSteepness, Morphology, PrecipitationSource, SurfaceClass,
    SynthesisCell, Wind,
};
use crate::spheroid::Spheroid;
use crate::synth::BandSpectrum;

/// A value with a wire form, written little-endian and read back.
pub(super) trait Wire: Sized {
    /// Writes the value.
    fn put(&self, out: &mut Vec<u8>);

    /// Reads a value.
    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError>;
}

/// A value whose wire form is always [`BYTES`](Self::BYTES) long.
pub(super) trait FixedWire: Wire {
    /// The length of the value's wire form, bytes.
    const BYTES: usize;
}

/// The wire form of a record: its fields in their wire order, each with its type, the writer
/// destructuring the record so that every field must be listed, and, for `fixed`, its length the
/// sum of its fields'.
macro_rules! wire_table {
    ($record:ident { $($field:ident: $ty:ty),+ $(,)? }) => {
        impl Wire for $record {
            fn put(&self, out: &mut Vec<u8>) {
                let $record { $($field),+ } = self;
                $(<$ty as Wire>::put($field, out);)+
            }

            fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
                Ok($record { $($field: <$ty as Wire>::read(r)?),+ })
            }
        }
    };
    (fixed $record:ident { $($field:ident: $ty:ty),+ $(,)? }) => {
        wire_table!($record { $($field: $ty),+ });

        impl FixedWire for $record {
            const BYTES: usize = 0 $(+ <$ty as FixedWire>::BYTES)+;
        }
    };
}

// One cell's synthesis record (Design note 17's 21 bytes), in its declared order.
wire_table!(fixed SynthesisCell {
    elevation_mm: i32,
    boundary_distance_km: i16,
    plate: u8,
    crust: Crust,
    boundary: BoundaryKind,
    boundary_obliquity: u8,
    flow: FlowDirection,
    drainage: LogArea,
    steepness: LogSteepness,
    water_surface_mm: i32,
    ice: u8,
    class: SurfaceClass,
    crater_state: u8,
});

// One climate record (Design note 17's 34 bytes), in its declared order.
wire_table!(fixed ClimateCell {
    sea_level_temperature: i16,
    month_anomaly: [i8; 12],
    month_precipitation: [LogPrecipitation; 12],
    wind: [Wind; 4],
});

// A wind: its azimuth, then its speed's code.
wire_table!(fixed Wind {
    azimuth: u8,
    speed: u8,
});

// The field header's parts, in their declared order (block 0 alone).
wire_table!(FieldHeaderParts {
    body: BodyRef,
    radius: Metres,
    figure: Spheroid,
    sea_level: Metres,
    lapse_rate_k_per_m: f64,
    spectrum: BandSpectrum,
    craters: CraterParams,
    climate_model: ClimateModelKind,
    precipitation: PrecipitationSource,
    realised_sigma_h: Metres,
    realised_relief: Metres,
    months: u8,
    reference_temperature: Kelvin,
    temperature_step: u8,
    anomaly_step: u8,
    surface_age: Gigayears,
    surface_pressure: Pascals,
    albedo_scale: Option<f64>,
});

// The datum: the equatorial radius a, then the polar radius c.
wire_table!(fixed Spheroid {
    equatorial_radius_m: f64,
    polar_radius_m: f64,
});

// The crater contract's parts, which `CraterParams::new` validates.
wire_table!(CraterParamsParts {
    n_1km: PerSquareKilometre,
    screening: Screening,
    gravity: MetresPerSecondSquared,
    k_target: f64,
    impact_velocity: Option<MetresPerSecond>,
});

/// The integers' forms: their little-endian bytes.
macro_rules! wire_integer {
    ($($ty:ty),+) => {
        $(
            impl Wire for $ty {
                fn put(&self, out: &mut Vec<u8>) {
                    out.extend_from_slice(&self.to_le_bytes());
                }

                fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
                    Ok(<$ty>::from_le_bytes(r.array()?))
                }
            }

            impl FixedWire for $ty {
                const BYTES: usize = core::mem::size_of::<$ty>();
            }
        )+
    };
}

wire_integer!(u8, i8, u16, i16, u32, i32, u64);

impl Wire for f64 {
    /// The value's IEEE 754 bits, little-endian.
    #[expect(
        clippy::disallowed_methods,
        reason = "the payload carries the field's f64 values exactly, as their bits: this \
                  serialises them and never hashes them, which is what the ban guards against"
    )]
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        Ok(f64::from_le_bytes(r.array()?))
    }
}

impl FixedWire for f64 {
    const BYTES: usize = 8;
}

/// The forms of the units: their values' `f64`.
macro_rules! wire_unit {
    ($($ty:ty),+) => {
        $(
            impl Wire for $ty {
                fn put(&self, out: &mut Vec<u8>) {
                    self.value().put(out);
                }

                fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
                    Ok(<$ty>::new(f64::read(r)?))
                }
            }

            impl FixedWire for $ty {
                const BYTES: usize = 8;
            }
        )+
    };
}

wire_unit!(
    Metres,
    Kelvin,
    Gigayears,
    Pascals,
    MetresPerSecond,
    MetresPerSecondSquared,
    PerSquareKilometre,
    SquareMetres,
    KilogramsPerSquareMetre,
    KilogramsPerCubicMetre
);

/// The forms of the field's one-byte enums: their codes, an unknown code refused.
macro_rules! wire_coded {
    ($($ty:ty),+) => {
        $(
            impl Wire for $ty {
                fn put(&self, out: &mut Vec<u8>) {
                    out.push(u8::from(*self));
                }

                fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
                    <$ty>::try_from(u8::read(r)?).map_err(DecodeBlockError::Code)
                }
            }

            impl FixedWire for $ty {
                const BYTES: usize = 1;
            }
        )+
    };
}

wire_coded!(
    Crust,
    BoundaryKind,
    FlowDirection,
    Morphology,
    ClimateModelKind,
    PrecipitationSource
);

/// The forms of the field's scaled and open codes: the code's integer.
macro_rules! wire_scaled {
    ($($ty:ty: $code:ty),+) => {
        $(
            impl Wire for $ty {
                fn put(&self, out: &mut Vec<u8>) {
                    self.get().put(out);
                }

                fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
                    Ok(<$ty>::new(<$code>::read(r)?))
                }
            }

            impl FixedWire for $ty {
                const BYTES: usize = <$code as FixedWire>::BYTES;
            }
        )+
    };
}

wire_scaled!(LogArea: u16, LogSteepness: u8, LogPrecipitation: u8, SurfaceClass: u8);

impl<T: FixedWire + Copy + Default, const N: usize> Wire for [T; N] {
    fn put(&self, out: &mut Vec<u8>) {
        for value in self {
            value.put(out);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        let mut values = [T::default(); N];
        for value in &mut values {
            *value = T::read(r)?;
        }
        Ok(values)
    }
}

impl<T: FixedWire + Copy + Default, const N: usize> FixedWire for [T; N] {
    const BYTES: usize = N * T::BYTES;
}

/// The presence byte of an absent `Option`.
pub(super) const ABSENT: u8 = 0;

/// The presence byte of a present `Option`.
pub(super) const PRESENT: u8 = 1;

impl<T: Wire> Wire for Option<T> {
    /// A presence byte, [`ABSENT`] or [`PRESENT`], and the value when present.
    fn put(&self, out: &mut Vec<u8>) {
        match self {
            None => out.push(ABSENT),
            Some(value) => {
                out.push(PRESENT);
                value.put(out);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        match u8::read(r)? {
            ABSENT => Ok(None),
            PRESENT => Ok(Some(T::read(r)?)),
            code => Err(DecodeBlockError::Tag {
                part: "presence",
                code,
            }),
        }
    }
}

impl Wire for BodyRef {
    /// The system's raw ID, then the body's index.
    fn put(&self, out: &mut Vec<u8>) {
        self.raw_system_id().put(out);
        self.body_index().put(out);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        let raw_system_id = u64::read(r)?;
        Ok(Self::new(raw_system_id, u16::read(r)?))
    }
}

impl FixedWire for BodyRef {
    const BYTES: usize = 10;
}

impl Wire for BandSpectrum {
    /// The exponent β, then the degree-1 variance V₁.
    fn put(&self, out: &mut Vec<u8>) {
        self.exponent().put(out);
        self.unit_degree_variance().put(out);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        let exponent = f64::read(r)?;
        Self::new(exponent, SquareMetres::read(r)?).map_err(DecodeBlockError::Spectrum)
    }
}

impl Wire for CraterParams {
    /// Its parts' table.
    fn put(&self, out: &mut Vec<u8>) {
        self.parts().put(out);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        Self::new(CraterParamsParts::read(r)?).map_err(DecodeBlockError::CraterParams)
    }
}

/// The screening tag of [`Screening::None`].
pub(super) const SCREENING_NONE: u8 = 0;

/// The screening tag of [`Screening::Atmosphere`].
pub(super) const SCREENING_ATMOSPHERE: u8 = 1;

/// The screening tag of [`Screening::Cutoff`].
pub(super) const SCREENING_CUTOFF: u8 = 2;

impl Wire for Screening {
    /// A tag, then the variant's parts: none for [`Screening::None`], the column mass and the
    /// projectile density for [`Screening::Atmosphere`], the diameter for [`Screening::Cutoff`].
    fn put(&self, out: &mut Vec<u8>) {
        match self {
            Self::None => out.push(SCREENING_NONE),
            Self::Atmosphere {
                column_mass,
                projectile_density,
            } => {
                out.push(SCREENING_ATMOSPHERE);
                column_mass.put(out);
                projectile_density.put(out);
            }
            Self::Cutoff { diameter } => {
                out.push(SCREENING_CUTOFF);
                diameter.put(out);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeBlockError> {
        match u8::read(r)? {
            SCREENING_NONE => Ok(Self::None),
            SCREENING_ATMOSPHERE => Ok(Self::Atmosphere {
                column_mass: KilogramsPerSquareMetre::read(r)?,
                projectile_density: KilogramsPerCubicMetre::read(r)?,
            }),
            SCREENING_CUTOFF => Ok(Self::Cutoff {
                diameter: Metres::read(r)?,
            }),
            code => Err(DecodeBlockError::Tag {
                part: "screening",
                code,
            }),
        }
    }
}

/// A cursor over a block's bytes.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// A cursor at the start of `bytes`.
    #[must_use]
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// The bytes read so far.
    #[must_use]
    pub(super) fn position(&self) -> usize {
        self.at
    }

    /// Whether every byte has been read.
    #[must_use]
    pub(super) fn is_empty(&self) -> bool {
        self.at == self.bytes.len()
    }

    /// The truncation of a read of `n` more bytes.
    #[must_use]
    fn truncated(&self, n: u64) -> DecodeBlockError {
        let needed = u64::try_from(self.at)
            .ok()
            .and_then(|at| at.checked_add(n))
            .and_then(|needed| usize::try_from(needed).ok())
            .unwrap_or(usize::MAX);
        DecodeBlockError::Truncated {
            needed,
            available: self.bytes.len(),
        }
    }

    /// Checks that `count` values of `each` bytes remain, before anything is allocated for them.
    pub(super) fn need(&self, count: u64, each: usize) -> Result<(), DecodeBlockError> {
        let remaining = u64::try_from(self.bytes.len() - self.at).unwrap_or(u64::MAX);
        let total = u64::try_from(each).ok().and_then(|e| e.checked_mul(count));
        match total {
            Some(t) if t <= remaining => Ok(()),
            _ => Err(self.truncated(total.unwrap_or(u64::MAX))),
        }
    }

    /// The next `n` bytes.
    pub(super) fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeBlockError> {
        let bytes = self.bytes;
        let Some(end) = self.at.checked_add(n).filter(|&end| end <= bytes.len()) else {
            return Err(self.truncated(u64::try_from(n).unwrap_or(u64::MAX)));
        };
        let taken = &bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    /// The next `N` bytes, as an array.
    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeBlockError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }
}

/// A field enum's code as the error of an unknown one, for the tests' expectations.
#[cfg(test)]
pub(super) fn unknown(kind: &'static str, code: u8) -> DecodeBlockError {
    DecodeBlockError::Code(crate::field::DecodeFieldCodeError { kind, code })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The records' strides are Design note 17's: 21 bytes a synthesis record and 34 a climate
    /// record; a field added to either moves them, and the payload's size with them.
    #[test]
    fn wire_records_have_design_note_seventeens_strides() {
        assert_eq!(SynthesisCell::BYTES, 21);
        assert_eq!(ClimateCell::BYTES, 34);
        assert_eq!(Wind::BYTES, 2);
        assert_eq!(Spheroid::BYTES, 16);
    }

    /// Every value of the forms comes back from its bytes to the bit, a −0.0 included, and an
    /// unknown code or tag is refused.
    #[test]
    fn wire_forms_read_back_what_they_write() {
        fn round_trip<T: Wire + PartialEq + std::fmt::Debug>(value: &T) {
            let mut out = Vec::new();
            value.put(&mut out);
            let mut r = Reader::new(&out);
            let back = T::read(&mut r).unwrap();
            assert_eq!(&back, value);
            assert!(r.is_empty(), "{value:?} left bytes");
            let mut again = Vec::new();
            back.put(&mut again);
            assert_eq!(again, out, "{value:?} came back with other bits");
        }
        round_trip(&-0.0_f64);
        round_trip(&f64::MIN_POSITIVE);
        round_trip(&Some(Metres::new(-3.5)));
        round_trip(&None::<f64>);
        round_trip(&BodyRef::new(u64::MAX, 7));
        for screening in [
            Screening::None,
            Screening::Atmosphere {
                column_mass: KilogramsPerSquareMetre::new(10_300.0),
                projectile_density: KilogramsPerCubicMetre::new(3_000.0),
            },
            Screening::Cutoff {
                diameter: Metres::new(1.6),
            },
        ] {
            round_trip(&screening);
        }
        round_trip(
            &[Wind {
                azimuth: 9,
                speed: 200,
            }; 4],
        );
        let read = |bytes: &[u8]| Crust::read(&mut Reader::new(bytes));
        assert_eq!(read(&[3]), Ok(Crust::Province));
        assert_eq!(read(&[4]), Err(unknown("Crust", 4)));
        assert_eq!(
            Screening::read(&mut Reader::new(&[3])),
            Err(DecodeBlockError::Tag {
                part: "screening",
                code: 3
            })
        );
        assert_eq!(
            Option::<f64>::read(&mut Reader::new(&[2])),
            Err(DecodeBlockError::Tag {
                part: "presence",
                code: 2
            })
        );
        assert_eq!(
            u32::read(&mut Reader::new(&[1, 2])),
            Err(DecodeBlockError::Truncated {
                needed: 4,
                available: 2
            })
        );
    }
}
