use sheaf_app::{model::Value, topological::execute};
use sheaf_native::hypercell::{solve_fallback, CsrCoboundary};

fn program(faces: usize, dimension: usize) -> (String, CsrCoboundary, Vec<f32>) {
    let mut source = String::new();
    let mut initial = Vec::new();
    let mut starts = Vec::new();
    let mut matrices = Vec::new();
    for face in 0..faces {
        let width = dimension + face % 2;
        source += &format!("DEFINE CELL TYPE F{face} (dim: 0, discrete: {{ label: string }}, continuous: Float32[{width}]);\n");
    }
    source += &format!("DEFINE HYPERCELL TYPE H (dim: 3, discrete: {{ tag: string }}, continuous: Float32[{dimension}]);\n");
    for face in 0..faces {
        let width = dimension + face % 2;
        let matrix: Vec<Vec<f32>> = (0..dimension).map(|row| (0..width).map(|col|
            if row == col { (face + 1) as f32 } else { 0.25 }).collect()).collect();
        source += &format!("DEFINE MORPHISM M{face}: F{face} -> H USING PROJECTION {{ label -> tag }} AND MATRIX {matrix:?};\n");
        matrices.push(matrix);
    }
    for face in 0..faces {
        let width = dimension + face % 2;
        let stalk: Vec<_> = (0..width).map(|j| (face + j + 1) as f32 * 0.25).collect();
        source += &format!("INSERT CELL F{face} ({}) DISCRETE {{ label: \"face{face}\" }} CONTINUOUS {stalk:?};\n", face + 1);
        starts.push(initial.len());
        initial.extend(stalk);
    }
    let boundary = (0..faces).map(|f| format!("F{f}({})", f + 1)).collect::<Vec<_>>().join(", ");
    source += &format!("INSERT HYPERCELL H (100) OVER BOUNDARY [{boundary}] DISCRETE {{ tag: \"hyper\" }} CONTINUOUS {:?};\n", vec![0.0; dimension]);
    let overs = (0..faces).map(|f| format!("p{f}: F{f}")).collect::<Vec<_>>().join(", ");
    source += &format!("MATCH HYPERCELL h: H OVER ({overs}) WHERE NOT p0.label == \"no\" AND h.tag == \"hyper\" RESTRICT TO BOUNDARY (p0) SOLVE HARMONIC (REGULARIZATION: 0.25, MAX_ITERATIONS: 128, ENERGY_THRESHOLD: 1000.0) EMIT SECTION {{ p0.label, p1.label, h.tag, SECTION_ENERGY, COHOMOLOGY_STATE }};");
    let mut csr = CsrCoboundary { cols: initial.len(), row_offsets: vec![0], columns: vec![], weights: vec![] };
    for row in 0..dimension {
        for face in 0..faces {
            for (col, weight) in matrices[face][row].iter().enumerate() {
                csr.columns.push(starts[face] + col);
                csr.weights.push(if face % 2 == 0 { *weight } else { -*weight });
            }
        }
        csr.row_offsets.push(csr.columns.len());
    }
    (source, csr, initial)
}

#[test]
fn variable_stalk_dimensions_two_and_four_boundaries_use_declared_morphisms() {
    for faces in [2, 4] {
        for dimension in [1, 2, 5, 9] {
            let (source, matrix, initial) = program(faces, dimension);
            let expected = solve_fallback(&matrix, &initial, 0.25, 128, 0.000001).unwrap();
            let output = execute(&source).unwrap();
            assert_eq!(output.rows.len(), 1);
            assert_eq!(output.columns, ["p0.label", "h.tag", "SECTION_ENERGY", "COHOMOLOGY_STATE"]);
            let Value::Float(energy) = output.rows[0][2] else { panic!("numeric energy required") };
            assert!((energy - expected.energy as f64).abs() < 0.000001);
            assert_eq!(output.rows[0][0], Value::Text("face0".into()));
            let diagnostic = output.plan.last().unwrap();
            assert!(diagnostic.contains(&format!("cols={}", matrix.cols)));
            assert!(diagnostic.contains("projections: p0="));
            assert!(!diagnostic.contains("p1="));
            assert!(diagnostic.contains(&format!("converged={}", expected.converged)));
            assert_eq!(output.tql.as_deref(), Some(source.as_str()));
            assert_eq!(output.stats.scanned_rows, 1);
            assert_eq!(output.stats.candidate_rows, 1);
            assert_eq!(output.stats.output_rows, 1);
        }
    }
}

#[test]
fn where_filter_and_restrict_do_not_change_the_solve() {
    let (source, _, _) = program(4, 3);
    let restricted = execute(&source).unwrap();
    let unrestricted = execute(&source.replace("RESTRICT TO BOUNDARY (p0)", "")).unwrap();
    assert_eq!(unrestricted.columns.len(), restricted.columns.len() + 1);
    assert_eq!(unrestricted.rows[0][3], restricted.rows[0][2]);
    assert!(unrestricted.plan.last().unwrap().contains("p3="));
    let filtered = execute(&source.replace("NOT p0.label", "p0.label")).unwrap();
    assert!(filtered.rows.is_empty());
    assert_eq!(filtered.stats.scanned_rows, 1);
    assert_eq!(filtered.stats.candidate_rows, 0);
}

#[test]
fn malformed_references_dimensions_fields_and_morphisms_return_errors() {
    let (source, _, _) = program(2, 2);
    let cases = [
        source.replace("F0(1)", "F0(999)"),
        source.replace("F0(1)", "F0(2)"),
        source.replace("F1 (2)", "F1 (1)"),
        source.replace("Float32[2]", "Float32[1]"),
        source.replace("label -> tag", "missing -> tag"),
        source.replace("label: \"face0\"", "label: 42"),
        source.replace("label: \"face0\"", "missing: \"face0\""),
        source.replace("dim: 3", "dim: 256"),
        source.replace("dim: 0", "dim: 1"),
        source.replace("F0 (1)", "F0 (-1)"),
        source.replace("MAX_ITERATIONS: 128", "MAX_ITERATIONS: 1.5"),
        source.replace("MAX_ITERATIONS: 128", "MAX_ITERATIONS: 999999999"),
        source.replace("p0.label, p1.label", "p0.label.extra, p1.label"),
        source.replace("p0: F0", "h: F0"),
        source.replace("BOUNDARY (p0)", "BOUNDARY (p0, p0)"),
        source.replace("[[1.0, 0.25], [0.25, 1.0]]", "[[1.0, 0.25]]"),
    ];
    for bad in cases { assert!(execute(&bad).is_err(), "unexpected acceptance: {bad}"); }
    let no_morph = source.lines().filter(|line| !line.starts_with("DEFINE MORPHISM M0:")).collect::<Vec<_>>().join("\n");
    assert!(execute(&no_morph).unwrap_err().contains("morphism"));
    let declaration = source.lines().find(|line| line.starts_with("DEFINE MORPHISM M0:")).unwrap();
    let ambiguous = source.replace(declaration, &format!("{declaration}\n{}", declaration.replace("M0:", "Other:")));
    assert!(execute(&ambiguous).unwrap_err().contains("ambiguous"));
}

#[test]
fn convergence_and_state_are_reported_without_a_proof_claim() {
    let (source, _, _) = program(4, 5);
    let output = execute(&source.replace("MAX_ITERATIONS: 128", "MAX_ITERATIONS: 1")).unwrap();
    assert!(output.plan.last().unwrap().contains("converged=false"));
    assert_eq!(output.rows[0][3], Value::Text("NOT_CONVERGED".into()));
    let (source, _, _) = program(2, 1);
    let output = execute(&source.replace("ENERGY_THRESHOLD: 1000.0", "ENERGY_THRESHOLD: 0.0")).unwrap();
    assert_eq!(output.rows[0][3], Value::Text("UNBOUNDED".into()));
    assert!(output.plan.iter().any(|s| s.contains("not a cohomology proof")));
}

#[test]
fn bounded_input_and_complete_program_are_required() {
    assert!(execute(&" ".repeat(65_537)).unwrap_err().contains("65536"));
    assert!(execute("").is_err());
    let (source, _, _) = program(2, 1);
    let deep = source.replace("WHERE NOT", &format!("WHERE {}", "NOT ".repeat(1000)));
    assert!(execute(&deep).unwrap_err().contains("expression limit"));
    let query = source.lines().last().unwrap();
    assert!(execute(query).is_err());
    assert!(execute(&format!("{source}{query}")).is_err());
    let mut too_many = String::from("DEFINE CELL TYPE F (dim: 0, discrete: {}, continuous: Float32[1]);\n");
    for id in 0..513 { too_many += &format!("INSERT CELL F ({id}) DISCRETE {{}} CONTINUOUS [0];\n"); }
    too_many += &source;
    assert!(execute(&too_many).unwrap_err().contains("cell limit"));
}
