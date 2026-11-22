// Timestamp: 2026-11-22_11-53-08 +05:30
// ebpf_policy.rs - integration test for the VFS policy and loader (NF-11).
//
// The test drives the production policy port with 10,000 generated events.
// The expected decision is computed by a separate reference function in this
// test, not by the feature code. It also checks the explicit unsupported
// loader report on this host.

use sheaf_native::ebpf::loader_unsupported::{LoaderError, UnsupportedLoader, REASON};
use sheaf_native::ebpf::policy::{Decision, Op, RuleSet, VfsEvent, VfsPolicy};
use sheaf_native::ebpf::Capability;

const PROTECTED: &[&str] = &["/etc/", "/sys/", "/secure/"];

fn expected(event: &VfsEvent, read_only: bool) -> Decision {
    for prefix in PROTECTED {
        if event.path.starts_with(prefix) {
            return Decision::DenyProtected;
        }
    }
    if read_only && event.op == Op::Write {
        return Decision::DenyReadOnly;
    }
    Decision::Allow
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn events() -> Vec<VfsEvent> {
    let roots = ["/data/", "/etc/", "/home/", "/sys/", "/secure/", "/tmp/"];
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    (0..10_000)
        .map(|id| {
            let op = match rng.next() % 3 {
                0 => Op::Read,
                1 => Op::Write,
                _ => Op::Execute,
            };
            let root = roots[(rng.next() as usize) % roots.len()];
            VfsEvent { op, path: format!("{root}file{id}.txt") }
        })
        .collect()
}

#[test]
fn ten_thousand_events_get_an_explicit_decision() {
    let policy = RuleSet {
        protected_prefixes: PROTECTED.iter().map(|s| s.to_string()).collect(),
        read_only: true,
    };
    let corpus = events();
    let mut allowances = 0usize;
    let mut protected = 0usize;
    let mut read_only = 0usize;
    for event in &corpus {
        let got = policy.decide(event);
        assert_eq!(got, expected(event, true), "event {:?}", event.path);
        match got {
            Decision::Allow => allowances += 1,
            Decision::DenyProtected => protected += 1,
            Decision::DenyReadOnly => read_only += 1,
        }
    }
    assert_eq!(allowances + protected + read_only, 10_000);
    assert!(allowances > 0 && protected > 0 && read_only > 0);
}

#[test]
fn a_read_write_policy_allows_all_writes_outside_protected_prefixes() {
    let policy = RuleSet {
        protected_prefixes: PROTECTED.iter().map(|s| s.to_string()).collect(),
        read_only: false,
    };
    let event = VfsEvent { op: Op::Write, path: "/data/file.txt".into() };
    assert_eq!(policy.decide(&event), Decision::Allow);
    let event = VfsEvent { op: Op::Write, path: "/etc/file.txt".into() };
    assert_eq!(policy.decide(&event), Decision::DenyProtected);
}

#[test]
fn the_loader_reports_unsupported_with_a_named_cause() {
    match UnsupportedLoader::capability() {
        Capability::Unsupported { reason } => assert_eq!(reason, REASON),
        Capability::Available => panic!("this host must not claim eBPF support"),
    }
    let loader = UnsupportedLoader;
    assert_eq!(
        loader.load(),
        Err(LoaderError::Unsupported { reason: REASON.to_owned() })
    );
    let policy = RuleSet { protected_prefixes: vec!["/etc/".into()], read_only: false };
    let decision = loader.dry_run(&policy, &VfsEvent { op: Op::Read, path: "/etc/a".into() });
    assert_eq!(decision, Decision::DenyProtected);
}
