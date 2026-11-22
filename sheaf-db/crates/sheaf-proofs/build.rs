// build.rs - track the Lean-emitted rule table (NF-01).
//
// The build script does not invoke Lean. It declares the generated header and
// manifest as inputs, so a change to either file rebuilds the Rust crate and
// reruns the drift test. The Rust code reads the files with include_str!.

fn main() {
    println!("cargo:rerun-if-changed=../../native/proofs-generated/sheaf_verified_rules.generated.h");
    println!("cargo:rerun-if-changed=../../proofs/generated/verified_rules.txt");
    println!("cargo:rustc-env=SHEAF_LEAN_HEADER=../../native/proofs-generated/sheaf_verified_rules.generated.h");
}
