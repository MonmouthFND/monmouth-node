//! Q8.24 fixed-point arithmetic for vector operations.
//!
//! Each value is an `i32` where the lower 24 bits are fractional.
//! Dot product results are `i128` in Q16.48 format (product of two Q8.24 values).

/// Maximum vector dimensions (DoS protection).
pub const MAX_VECTOR_DIMENSIONS: u32 = 2048;

/// Errors from verification math operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// Dimension count is zero or exceeds [`MAX_VECTOR_DIMENSIONS`].
    InvalidDimensions(u32),
    /// Vector length does not match the declared dimension count.
    LengthMismatch {
        /// Expected number of elements.
        expected: usize,
        /// Actual number of elements in vector A.
        actual_a: usize,
        /// Actual number of elements in vector B.
        actual_b: usize,
    },
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions(d) => {
                write!(f, "invalid dimensions: {d} (must be 1..={MAX_VECTOR_DIMENSIONS})")
            }
            Self::LengthMismatch { expected, actual_a, actual_b } => {
                write!(f, "length mismatch: expected {expected}, got A={actual_a}, B={actual_b}")
            }
        }
    }
}

impl std::error::Error for VerificationError {}

/// Compute the dot product of two Q8.24 fixed-point vectors.
///
/// Each element is an `i32` in Q8.24 format (8 integer bits, 24 fractional bits).
/// The result is an `i128` in Q16.48 format (the product scale of two Q8.24 values).
///
/// For unit-normalized vectors, the dot product equals cosine similarity.
///
/// # Errors
///
/// Returns [`VerificationError::InvalidDimensions`] if `dimensions` is 0 or
/// exceeds [`MAX_VECTOR_DIMENSIONS`].
///
/// Returns [`VerificationError::LengthMismatch`] if either vector's length
/// does not match `dimensions`.
pub fn dot_product_q8_24(
    dimensions: u32,
    vec_a: &[i32],
    vec_b: &[i32],
) -> Result<i128, VerificationError> {
    if dimensions == 0 || dimensions > MAX_VECTOR_DIMENSIONS {
        return Err(VerificationError::InvalidDimensions(dimensions));
    }

    let n = dimensions as usize;
    if vec_a.len() != n || vec_b.len() != n {
        return Err(VerificationError::LengthMismatch {
            expected: n,
            actual_a: vec_a.len(),
            actual_b: vec_b.len(),
        });
    }

    // Use i128 accumulator for safety.
    // Each element is i32 (Q8.24), so each product is i64 (Q16.48).
    // Summing up to 2048 i64 values can overflow i64, so we use i128.
    let mut dot_product: i128 = 0;
    for i in 0..n {
        dot_product += (vec_a[i] as i64 as i128) * (vec_b[i] as i64 as i128);
    }

    Ok(dot_product)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_dot_product() {
        // 2 dimensions, vectors [1.0, 0.5] dot [0.5, 1.0] in Q8.24
        // 1.0 in Q8.24 = 1 << 24 = 16777216
        // 0.5 in Q8.24 = 1 << 23 = 8388608
        let one_q24: i32 = 1 << 24;
        let half_q24: i32 = 1 << 23;

        let vec_a = vec![one_q24, half_q24];
        let vec_b = vec![half_q24, one_q24];

        let result = dot_product_q8_24(2, &vec_a, &vec_b).unwrap();

        // dot = 1.0*0.5 + 0.5*1.0 = 1.0 in real, which in Q16.48 is:
        // (1<<24)*(1<<23) + (1<<23)*(1<<24) = 2 * (1<<47) = 1<<48
        let expected: i128 =
            (one_q24 as i128) * (half_q24 as i128) + (half_q24 as i128) * (one_q24 as i128);
        assert_eq!(result, expected);
    }

    #[test]
    fn zero_vectors() {
        let vec_a = vec![0i32; 4];
        let vec_b = vec![0i32; 4];
        let result = dot_product_q8_24(4, &vec_a, &vec_b).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(dot_product_q8_24(0, &[], &[]), Err(VerificationError::InvalidDimensions(0)));
    }

    #[test]
    fn rejects_excessive_dimensions() {
        assert_eq!(
            dot_product_q8_24(3000, &[], &[]),
            Err(VerificationError::InvalidDimensions(3000))
        );
    }

    #[test]
    fn rejects_length_mismatch() {
        let result = dot_product_q8_24(2, &[1, 2, 3], &[1, 2]);
        assert!(matches!(result, Err(VerificationError::LengthMismatch { .. })));
    }

    #[test]
    fn negative_values() {
        // -1.0 in Q8.24
        let neg_one_q24: i32 = -(1 << 24);
        let one_q24: i32 = 1 << 24;

        let result = dot_product_q8_24(1, &[neg_one_q24], &[one_q24]).unwrap();

        // -1.0 * 1.0 = -1.0 in Q16.48
        let expected = (neg_one_q24 as i128) * (one_q24 as i128);
        assert_eq!(result, expected);
        assert!(result < 0);
    }

    #[test]
    fn max_dimensions_accepted() {
        let vec_a = vec![0i32; MAX_VECTOR_DIMENSIONS as usize];
        let vec_b = vec![0i32; MAX_VECTOR_DIMENSIONS as usize];
        assert!(dot_product_q8_24(MAX_VECTOR_DIMENSIONS, &vec_a, &vec_b).is_ok());
    }

    #[test]
    fn error_display() {
        let err = VerificationError::InvalidDimensions(0);
        assert!(err.to_string().contains("invalid dimensions"));

        let err = VerificationError::LengthMismatch { expected: 2, actual_a: 3, actual_b: 2 };
        assert!(err.to_string().contains("length mismatch"));
    }
}
