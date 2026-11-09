// crates/sheaf-storage/src/lsp/arrow_ingest.rs — zero-poset flat ingest.
// Node H2 of IMPLEMENTATION_PLAN_HARDWARE_MAPPED_2026-10-27.md.
// Ingests a TSV into flat columnar buffers without building a cell complex.
// The fiber bundle is woven in the background afterwards.
pub struct ArrowBuffers {
    pub columns: Vec<Vec<u8>>,
}

pub fn ingest_flat(_tsv_path: &str) -> std::io::Result<ArrowBuffers> {
    // Placeholder: real implementation parses the TSV into column buffers.
    Ok(ArrowBuffers { columns: Vec::new() })
}
