use p3_field::PrimeField64;
use p3_goldilocks::Goldilocks;

use crate::params::{
    INPUT_BYTES_PER_ELEMENT, OUTPUT_BYTES, OUTPUT_BYTES_PER_ELEMENT, OUTPUT_ELEMENTS, RATE,
};

/// Encode arbitrary bytes into Goldilocks field elements.
///
/// Each element holds 7 bytes (little-endian). The last element may hold fewer bytes,
/// zero-padded in the high positions.
#[allow(dead_code)] // Used in tests; will be used by future BAO streaming code.
pub(crate) fn bytes_to_elements(bytes: &[u8]) -> Vec<Goldilocks> {
    bytes
        .chunks(INPUT_BYTES_PER_ELEMENT)
        .map(|chunk| {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            Goldilocks::new(u64::from_le_bytes(buf))
        })
        .collect()
}

/// Encode exactly `count` elements from a byte slice into a fixed-size array,
/// writing into `out[offset..]`. Returns the number of bytes consumed.
///
/// This is the hot-path version used by the sponge absorb.
pub(crate) fn bytes_to_rate_block(bytes: &[u8], out: &mut [Goldilocks; RATE]) -> usize {
    let mut consumed = 0;
    for elem in out.iter_mut() {
        if consumed >= bytes.len() {
            *elem = Goldilocks::new(0);
        } else {
            let end = (consumed + INPUT_BYTES_PER_ELEMENT).min(bytes.len());
            let mut buf = [0u8; 8];
            buf[..end - consumed].copy_from_slice(&bytes[consumed..end]);
            *elem = Goldilocks::new(u64::from_le_bytes(buf));
            consumed = end;
        }
    }
    consumed
}

/// Convert output field elements to a hash byte array.
///
/// Uses the full canonical u64 representation (8 bytes LE) per element.
pub(crate) fn hash_to_bytes(elements: &[Goldilocks; OUTPUT_ELEMENTS]) -> [u8; OUTPUT_BYTES] {
    let mut out = [0u8; OUTPUT_BYTES];
    for (i, elem) in elements.iter().enumerate() {
        let val = elem.as_canonical_u64();
        out[i * OUTPUT_BYTES_PER_ELEMENT..(i + 1) * OUTPUT_BYTES_PER_ELEMENT]
            .copy_from_slice(&val.to_le_bytes());
    }
    out
}

/// Convert a hash byte array back into Goldilocks elements.
///
/// Inverse of `hash_to_bytes`. Used by `parent_cv` in the hazmat module.
pub(crate) fn bytes_to_cv(bytes: &[u8; OUTPUT_BYTES]) -> [Goldilocks; OUTPUT_ELEMENTS] {
    let mut out = [Goldilocks::new(0); OUTPUT_ELEMENTS];
    for (i, elem) in out.iter_mut().enumerate() {
        let start = i * OUTPUT_BYTES_PER_ELEMENT;
        let val = u64::from_le_bytes(
            bytes[start..start + OUTPUT_BYTES_PER_ELEMENT]
                .try_into()
                .unwrap(),
        );
        *elem = Goldilocks::new(val);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::RATE_BYTES;

    #[test]
    fn roundtrip_hash_bytes() {
        let elements = [
            Goldilocks::new(1),
            Goldilocks::new(2),
            Goldilocks::new(3),
            Goldilocks::new(4),
            Goldilocks::new(5),
            Goldilocks::new(6),
            Goldilocks::new(7),
            Goldilocks::new(8),
        ];
        let bytes = hash_to_bytes(&elements);
        let recovered = bytes_to_cv(&bytes);
        assert_eq!(elements, recovered);
    }

    #[test]
    fn bytes_to_elements_empty() {
        let result = bytes_to_elements(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn bytes_to_elements_short() {
        let result = bytes_to_elements(&[0x42]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_canonical_u64(), 0x42);
    }

    #[test]
    fn bytes_to_elements_exact_7() {
        let input = [1, 2, 3, 4, 5, 6, 7];
        let result = bytes_to_elements(&input);
        assert_eq!(result.len(), 1);
        let expected = u64::from_le_bytes([1, 2, 3, 4, 5, 6, 7, 0]);
        assert_eq!(result[0].as_canonical_u64(), expected);
    }

    #[test]
    fn bytes_to_elements_8_bytes_splits() {
        let input = [1, 2, 3, 4, 5, 6, 7, 8];
        let result = bytes_to_elements(&input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn rate_block_fill() {
        let input = vec![0xAB; RATE_BYTES];
        let mut block = [Goldilocks::new(0); RATE];
        let consumed = bytes_to_rate_block(&input, &mut block);
        assert_eq!(consumed, RATE_BYTES);
        // All 8 elements should be non-zero
        for elem in &block {
            assert_ne!(elem.as_canonical_u64(), 0);
        }
    }

    #[test]
    fn seven_byte_values_fit_in_goldilocks() {
        // Max 7-byte value: 2^56 - 1 = 0x00FF_FFFF_FFFF_FFFF
        let max_7byte: u64 = (1u64 << 56) - 1;
        // Goldilocks prime: p = 2^64 - 2^32 + 1 = 0xFFFF_FFFF_0000_0001
        let p: u64 = 0xFFFF_FFFF_0000_0001;
        assert!(max_7byte < p, "7-byte max must be less than Goldilocks prime");
    }
}
