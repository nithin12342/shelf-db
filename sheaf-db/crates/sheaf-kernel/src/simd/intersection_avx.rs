// crates/sheaf-kernel/src/simd/intersection_avx.rs — AVX2 vectorized sorted u64 set intersection
// Fast path for sorted slice intersection: 4x u64 parallel comparison using 256-bit registers.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[inline]
pub fn intersect_count_avx2(a: &[u64], b: &[u64]) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && a.len() >= 4 && b.len() >= 4 {
            unsafe {
                return intersect_count_avx2_inner(a, b);
            }
        }
    }
    intersect_count_scalar(a, b)
}

#[inline]
pub fn intersect_count_scalar(a: &[u64], b: &[u64]) -> usize {
    let (mut i, mut j, mut count) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        let va = a[i];
        let vb = b[j];
        if va == vb {
            count += 1;
            i += 1;
            j += 1;
        } else if va < vb {
            i += 1;
        } else {
            j += 1;
        }
    }
    count
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn intersect_count_avx2_inner(a: &[u64], b: &[u64]) -> usize {
    let (mut i, mut j, mut count) = (0, 0, 0);
    let a_len = a.len();
    let b_len = b.len();

    while i + 4 <= a_len && j + 4 <= b_len {
        let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(j) as *const __m256i);

        let max_a = a[i + 3];
        let max_b = b[j + 3];

        let eq0 = _mm256_cmpeq_epi64(va, _mm256_permute4x64_epi64(vb, 0x00));
        let eq1 = _mm256_cmpeq_epi64(va, _mm256_permute4x64_epi64(vb, 0x55));
        let eq2 = _mm256_cmpeq_epi64(va, _mm256_permute4x64_epi64(vb, 0xAA));
        let eq3 = _mm256_cmpeq_epi64(va, _mm256_permute4x64_epi64(vb, 0xFF));

        let any_eq = _mm256_or_si256(_mm256_or_si256(eq0, eq1), _mm256_or_si256(eq2, eq3));
        let mask = _mm256_movemask_epi8(any_eq);
        if mask != 0 {
            let m0 = _mm256_movemask_epi8(eq0) != 0;
            let m1 = _mm256_movemask_epi8(eq1) != 0;
            let m2 = _mm256_movemask_epi8(eq2) != 0;
            let m3 = _mm256_movemask_epi8(eq3) != 0;
            count += m0 as usize + m1 as usize + m2 as usize + m3 as usize;
        }

        if max_a <= max_b {
            i += 4;
        }
        if max_b <= max_a {
            j += 4;
        }
    }

    count + intersect_count_scalar(&a[i..], &b[j..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intersect_avx2() {
        let a = [1, 3, 5, 7, 9, 11, 13, 15];
        let b = [3, 7, 9, 15, 20];
        assert_eq!(intersect_count_avx2(&a, &b), 4);
        assert_eq!(intersect_count_avx2(&a, &b), intersect_count_scalar(&a, &b));
    }
}
