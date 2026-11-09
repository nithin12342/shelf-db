// crates/sheaf-storage/src/lsp/fiber_epoch.rs — epoch stamping for fiber
// artifacts (N3 / METHOD-032, REQ-037 / SPEC-037; extended by N5).
//
// A fiber (a precomputed answer artifact) is built against an EPOCH: the
// engine generation it was derived from plus the fingerprint of the source
// data. Loading it against a different epoch FAILS LOUD - it never
// answers. The caller then falls back to a scan, and that fallback must
// return the exact same aggregate. Without the stamp, a fiber built before
// a compaction silently answered with pre-compaction numbers.

use std::fmt;

/// The epoch a fiber was built at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epoch {
    /// Engine generation the fiber was derived from.
    pub generation: u64,
    /// Fingerprint of the source data the fiber was derived from.
    pub src_fingerprint: u64,
}

impl Epoch {
    pub fn new(generation: u64, src_fingerprint: u64) -> Self {
        Epoch { generation, src_fingerprint }
    }

    /// True when a fiber built at `self` may be used at epoch `current`.
    pub fn matches(&self, current: &Epoch) -> bool {
        self.generation == current.generation && self.src_fingerprint == current.src_fingerprint
    }

    /// Validate a fiber's epoch against the current one. A mismatch is an
    /// error, never a silent pass.
    pub fn validate(&self, current: &Epoch) -> Result<(), EpochMismatch> {
        if self.generation != current.generation {
            return Err(EpochMismatch::EpochAdvanced {
                built: self.generation,
                current: current.generation,
            });
        }
        if self.src_fingerprint != current.src_fingerprint {
            return Err(EpochMismatch::SourceChanged {
                built: self.src_fingerprint,
                current: current.src_fingerprint,
            });
        }
        Ok(())
    }
}

/// Why a fiber may not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpochMismatch {
    /// The engine compacted forward: the fiber predates the current epoch.
    EpochAdvanced { built: u64, current: u64 },
    /// The source data changed under the fiber.
    SourceChanged { built: u64, current: u64 },
}

impl fmt::Display for EpochMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EpochMismatch::EpochAdvanced { built, current } => write!(
                f,
                "fiber epoch {built} is older than the current epoch {current} (compaction advanced)"
            ),
            EpochMismatch::SourceChanged { built, current } => write!(
                f,
                "fiber source fingerprint {built:#x} != current {current:#x} (source changed)"
            ),
        }
    }
}

/// What the caller must do with a fiber at this epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiberDecision {
    /// The fiber is valid: use it.
    Use,
    /// The fiber is stale: fall back to a scan (which must return the
    /// exact same aggregate).
    FallbackToScan(EpochMismatch),
}

/// Decide whether a fiber built at `built` may be used at `current`.
pub fn decide(built: &Epoch, current: &Epoch) -> FiberDecision {
    match built.validate(current) {
        Ok(()) => FiberDecision::Use,
        Err(e) => FiberDecision::FallbackToScan(e),
    }
}

/// Fingerprint of a BCSF base (N5): the CRC32C of the cell dimensions plus
/// the boundary count. It identifies the base a fiber was derived from, so
/// a fiber built against a different base is detected as stale.
pub fn base_fingerprint(reader: &crate::bcsf::reader::BcsfReader) -> u64 {
    let dims_crc = crate::lsp::ring_wal::crc_hw(&reader.dims) as u64;
    let boundary: u64 = reader.indices.indices.len() as u64;
    dims_crc ^ boundary.rotate_left(17)
}
