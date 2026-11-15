// section.rs - 0-cochains and the global-section commit rule (spec Pillar 8).
//
// A section assigns a stalk vector to every 0-cell. It is a 0-cochain
// x in C^0(X; F). The spec defines a commit as a section with zero coboundary
// (delta x = 0), which is the kernel of the sheaf Laplacian L = delta* delta.
//
// This module provides the linear algebra on sections (add, subtract, scale)
// and the global-section test. The merge engine in diff.rs uses these to
// compose branches and to measure the cohomological obstruction.

use crate::complex::Complex;

/// An error produced when a section does not match the complex dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectionError {
    /// The value count does not equal the total 0-cochain length.
    DimensionMismatch { expected: usize, got: usize },
}

/// A 0-cochain: the concatenated stalk values of every vertex.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub values: Vec<f32>,
}

impl Section {
    /// The zero section. It is always a global section because delta 0 = 0.
    pub fn zero(c: &Complex) -> Self {
        Self { values: vec![0.0; c.c0_len()] }
    }

    /// Build a section from explicit values, checking the dimension.
    pub fn new(c: &Complex, values: Vec<f32>) -> Result<Self, SectionError> {
        if values.len() != c.c0_len() {
            return Err(SectionError::DimensionMismatch { expected: c.c0_len(), got: values.len() });
        }
        Ok(Self { values })
    }

    /// A section that fills every lane with the same constant. On a connected
    /// graph with identity restrictions this is a global section.
    pub fn constant(c: &Complex, value: f32) -> Self {
        Self { values: vec![value; c.c0_len()] }
    }

    /// Number of lanes in the section.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True when the section has no lanes.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Lane-wise sum of two sections of equal length.
    pub fn add(&self, other: &Section) -> Section {
        assert_eq!(self.values.len(), other.values.len(), "section length mismatch");
        Section { values: self.values.iter().zip(&other.values).map(|(a, b)| a + b).collect() }
    }

    /// Lane-wise difference self - other.
    pub fn sub(&self, other: &Section) -> Section {
        assert_eq!(self.values.len(), other.values.len(), "section length mismatch");
        Section { values: self.values.iter().zip(&other.values).map(|(a, b)| a - b).collect() }
    }

    /// Multiply every lane by a scalar.
    pub fn scale(&self, w: f32) -> Section {
        Section { values: self.values.iter().map(|v| v * w).collect() }
    }

    /// The coboundary delta x as a 1-cochain.
    pub fn coboundary(&self, c: &Complex) -> Vec<f32> {
        c.coboundary(&self.values)
    }

    /// The Dirichlet energy ||delta x||^2.
    pub fn energy(&self, c: &Complex) -> f32 {
        c.energy(&self.values)
    }

    /// True when this section is a valid commit (delta x = 0 within tol).
    pub fn is_global(&self, c: &Complex, tol: f32) -> bool {
        c.is_global_section(&self.values, tol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Restriction;

    fn line() -> Complex {
        let mut c = Complex::new(vec![1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c
    }

    #[test]
    fn zero_section_is_global() {
        let c = line();
        let z = Section::zero(&c);
        assert!(z.is_global(&c, 1e-6));
        assert_eq!(z.energy(&c), 0.0);
    }

    #[test]
    fn constant_section_on_identity_line_is_global() {
        let c = line();
        let s = Section::constant(&c, 7.0);
        assert!(s.is_global(&c, 1e-6));
    }

    #[test]
    fn linear_algebra_is_lane_wise() {
        let c = line();
        let a = Section::new(&c, vec![1.0, 2.0]).unwrap();
        let b = Section::new(&c, vec![3.0, 4.0]).unwrap();
        assert_eq!(a.add(&b).values, vec![4.0, 6.0]);
        assert_eq!(b.sub(&a).values, vec![2.0, 2.0]);
        assert_eq!(a.scale(2.0).values, vec![2.0, 4.0]);
    }

    #[test]
    fn non_global_section_has_positive_energy() {
        let c = line();
        let s = Section::new(&c, vec![1.0, 3.0]).unwrap();
        assert!(!s.is_global(&c, 1e-6));
        assert_eq!(s.energy(&c), 4.0);
        assert_eq!(s.coboundary(&c), vec![2.0]);
    }

    #[test]
    fn dimension_mismatch_is_rejected() {
        let c = line();
        let err = Section::new(&c, vec![1.0]).unwrap_err();
        assert_eq!(err, SectionError::DimensionMismatch { expected: 2, got: 1 });
    }
}