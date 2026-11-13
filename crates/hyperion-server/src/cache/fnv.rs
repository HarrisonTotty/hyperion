//! FNV-1a's 64-bit hash, which names the server's files on disk and checks their contents.

/// FNV-1a's 64-bit offset basis and prime (Fowler, Noll and Vo; IETF draft-eastlake-fnv-21, §2.1
/// and §6.1.2), for the files' names and checksums: no cryptographic strength is needed, since a
/// name that collides is caught by the key the file holds.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a's 64-bit hash of `bytes`.
#[must_use]
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a's published 64-bit test vectors (draft-eastlake-fnv-21, §8 and its test code).
    #[test]
    fn fnv1a_is_the_published_one() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
