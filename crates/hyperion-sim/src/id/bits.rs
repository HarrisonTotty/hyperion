//! Bit fields of 64-bit identifiers.
//!
//! Bit 63 is the most significant. A field `[hi:lo]` holds bits `hi` down to `lo`, inclusive.
//! Every extraction and insertion in [`crate::id`] goes through [`field`] and [`with_field`].

/// The value of bits `[hi:lo]` of `raw`, shifted down to bit 0.
#[must_use]
pub(super) const fn field(raw: u64, hi: u32, lo: u32) -> u64 {
    debug_assert!(lo <= hi && hi < 64, "a field is [hi:lo] with lo <= hi < 64");
    (raw >> lo) & low_mask(hi - lo + 1)
}

/// `raw` with bits `[hi:lo]` replaced by `value`.
///
/// A value wider than the field is a caller's bug: it fails a debug assertion, and in release
/// only its low bits are written, so a neighbouring field is never touched.
#[must_use]
pub(super) const fn with_field(raw: u64, hi: u32, lo: u32, value: u64) -> u64 {
    debug_assert!(lo <= hi && hi < 64, "a field is [hi:lo] with lo <= hi < 64");
    let mask = low_mask(hi - lo + 1);
    debug_assert!(value <= mask, "the value does not fit the field");
    (raw & !(mask << lo)) | ((value & mask) << lo)
}

/// A mask of the low `width` bits.
const fn low_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1 << width) - 1
    }
}

/// A field `[hi:lo]` of a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Field {
    hi: u32,
    lo: u32,
}

impl Field {
    /// The field `[hi:lo]`.
    #[must_use]
    pub(super) const fn new(hi: u32, lo: u32) -> Self {
        assert!(lo <= hi && hi < 64, "a field is [hi:lo] with lo <= hi < 64");
        Self { hi, lo }
    }

    /// The field of `width` bits whose lowest bit is `lo`.
    #[must_use]
    pub(super) const fn above(lo: u32, width: u32) -> Self {
        Self::new(lo + width - 1, lo)
    }

    /// The highest bit.
    #[must_use]
    pub(super) const fn hi(self) -> u32 {
        self.hi
    }

    /// The lowest bit.
    #[must_use]
    pub(super) const fn lo(self) -> u32 {
        self.lo
    }

    /// The number of bits.
    #[must_use]
    pub(super) const fn width(self) -> u32 {
        self.hi - self.lo + 1
    }

    /// The largest value the field holds.
    #[must_use]
    pub(super) const fn max(self) -> u64 {
        low_mask(self.width())
    }

    /// The field's value in `raw`.
    #[must_use]
    pub(super) const fn get(self, raw: u64) -> u64 {
        field(raw, self.hi, self.lo)
    }

    /// `raw` with this field set to `value`.
    #[must_use]
    pub(super) const fn with(self, raw: u64, value: u64) -> u64 {
        with_field(raw, self.hi, self.lo, value)
    }

    /// Whether every bit of the field is zero in `raw`.
    #[must_use]
    pub(super) const fn is_zero(self, raw: u64) -> bool {
        self.get(raw) == 0
    }

    /// The field's value in `raw`, for a field of at most 8 bits.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the field is at most 8 bits wide, which the assertion checks"
    )]
    pub(super) const fn get_u8(self, raw: u64) -> u8 {
        debug_assert!(self.width() <= 8, "the field is wider than u8");
        self.get(raw) as u8
    }

    /// The field's value in `raw`, for a field of at most 16 bits.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the field is at most 16 bits wide, which the assertion checks"
    )]
    pub(super) const fn get_u16(self, raw: u64) -> u16 {
        debug_assert!(self.width() <= 16, "the field is wider than u16");
        self.get(raw) as u16
    }

    /// The field's value in `raw`, for a field of at most 32 bits.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the field is at most 32 bits wide, which the assertion checks"
    )]
    pub(super) const fn get_u32(self, raw: u64) -> u32 {
        debug_assert!(self.width() <= 32, "the field is wider than u32");
        self.get(raw) as u32
    }
}

/// Fails (at compile time, when called in a `const`) unless `fields`, listed from high to low,
/// cover bits 63 to 0 exactly once each: the bit budget of a layout.
pub(super) const fn assert_tiles_64(fields: &[Field]) {
    assert!(!fields.is_empty(), "a layout has fields");
    assert!(fields[0].hi() == 63, "a layout starts at bit 63");
    let mut i = 1;
    while i < fields.len() {
        assert!(
            fields[i].hi() + 1 == fields[i - 1].lo(),
            "a layout's fields are contiguous and do not overlap"
        );
        i += 1;
    }
    assert!(fields[fields.len() - 1].lo() == 0, "a layout ends at bit 0");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_reads_the_inclusive_bit_range() {
        let raw = 0xF0E1_D2C3_B4A5_9687;
        assert_eq!(field(raw, 63, 60), 0xF);
        assert_eq!(field(raw, 63, 0), raw);
        assert_eq!(field(raw, 0, 0), 1);
        assert_eq!(field(raw, 3, 0), 0x7);
        assert_eq!(field(raw, 15, 8), 0x96);
        assert_eq!(field(raw, 47, 32), 0xD2C3);
        assert_eq!(field(raw, 62, 61), 0b11);
    }

    #[test]
    fn with_field_replaces_only_the_range() {
        let raw = 0xFFFF_FFFF_FFFF_FFFF;
        assert_eq!(with_field(raw, 63, 61, 0), 0x1FFF_FFFF_FFFF_FFFF);
        assert_eq!(with_field(raw, 7, 4, 0x5), 0xFFFF_FFFF_FFFF_FF5F);
        assert_eq!(with_field(0, 63, 0, 42), 42);
        assert_eq!(with_field(0, 60, 58, 0b101), 0b101 << 58);
        for (hi, lo) in [(63, 0), (63, 63), (0, 0), (40, 3), (57, 44)] {
            let value = field(0x0123_4567_89AB_CDEF, hi, lo);
            assert_eq!(field(with_field(raw, hi, lo, value), hi, lo), value);
            assert_eq!(
                with_field(raw, hi, lo, value) | (low_mask(hi - lo + 1) << lo),
                raw,
                "bits outside [{hi}:{lo}] are untouched"
            );
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "does not fit the field")]
    fn with_field_rejects_a_wide_value_in_debug() {
        let _ = with_field(0, 3, 0, 16);
    }

    #[test]
    fn fields_know_their_width_and_convert() {
        let f = Field::new(27, 13);
        assert_eq!(f.width(), 15);
        assert_eq!(f.max(), 0x7FFF);
        assert_eq!(Field::above(13, 15), f);
        assert_eq!(f.get_u16(f.with(0, 0x7FFF)), 0x7FFF);
        assert!(f.is_zero(!f.with(0, f.max())));
        assert_eq!(Field::new(7, 0).get_u8(0x1FF), 0xFF);
        assert_eq!(Field::new(31, 0).get_u32(u64::MAX), u32::MAX);
    }

    #[test]
    fn tiling_accepts_a_complete_layout() {
        assert_tiles_64(&[Field::new(63, 61), Field::new(60, 13), Field::new(12, 0)]);
        assert_tiles_64(&[Field::new(63, 0)]);
    }

    #[test]
    #[should_panic(expected = "contiguous")]
    fn tiling_rejects_a_gap() {
        assert_tiles_64(&[Field::new(63, 61), Field::new(59, 0)]);
    }

    #[test]
    #[should_panic(expected = "ends at bit 0")]
    fn tiling_rejects_a_short_layout() {
        assert_tiles_64(&[Field::new(63, 61), Field::new(60, 1)]);
    }
}
