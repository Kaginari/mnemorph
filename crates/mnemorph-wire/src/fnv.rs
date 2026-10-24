//! FNV-1a, 64 bits: the content address the wire uses for drafts and courier envelopes. Small
//! enough to own, so the wire stays free of dependencies.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// The FNV-1a 64-bit hash of `bytes`.
#[must_use]
pub fn fnv64a(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(OFFSET, |h, &b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    use super::fnv64a;

    #[test]
    fn known_vectors() {
        assert_eq!(fnv64a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv64a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv64a(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
