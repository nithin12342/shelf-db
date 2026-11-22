// replication.rs - integration test for the replication adapters (NF-05, NF-06).
//
// The test drives the production adapters: a TcpSender writes real frames over
// loopback, a TcpReceiver reads them, and SectionState merges them. The
// expected result is the fingerprint of the state built in generation order.
// Because the patches are XOR deltas, a fixed-seed shuffle must produce the
// same fingerprint.

use sheaf_runtime::replication::port::{RdmaTransport, ReplicationTransport};
use sheaf_runtime::replication::rdma_unsupported::{RdmaUnsupported, REASON};
use sheaf_runtime::replication::tcp_adapter::{accept, TcpReceiver, TcpSender};
use sheaf_runtime::replication::{frame, Capability, ReplicationError, SectionDelta, SectionState};

fn generator(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

fn deltas(count: usize) -> Vec<SectionDelta> {
    let mut next = generator(0x2545_f491_4f6c_dd1d);
    (0..count)
        .map(|sequence| {
            let cell = next() % 64;
            let mut patch = vec![0u8; 16];
            for word in patch.chunks_mut(8) {
                word.copy_from_slice(&next().to_le_bytes());
            }
            SectionDelta::new(sequence as u64 + 1, cell, patch)
        })
        .collect()
}

#[test]
fn ten_thousand_deltas_over_loopback_reach_the_same_state() {
    let mut receiver = TcpReceiver::bind("127.0.0.1:0").expect("bind");
    let address = receiver.local_addr().expect("address");
    let worker = std::thread::spawn(move || {
        accept(&mut receiver).expect("accept");
        let mut state = SectionState::default();
        let mut received = Vec::new();
        for _ in 0..10_000 {
            let delta = receiver.receive().expect("receive");
            state.apply(&delta);
            received.push(delta);
        }
        (state, received)
    });

    let sender = TcpSender::connect(address).expect("connect");
    let expected_deltas = deltas(10_000);
    let mut expected = SectionState::default();
    for delta in &expected_deltas {
        let ack = sender.send(delta).expect("send");
        assert_eq!(ack.sequence, delta.sequence);
        expected.apply(delta);
    }
    let (state, received) = worker.join().expect("join");

    assert_eq!(state.fingerprint(), expected.fingerprint());
    assert_eq!(state.applied_count(), 10_000);
    assert_eq!(state.section_count(), 64);
    assert_eq!(received, expected_deltas, "every frame arrives intact and in order");
}

#[test]
fn a_reordered_delivery_reaches_the_same_final_state() {
    let source = deltas(10_000);
    let mut expected = SectionState::default();
    expected.apply_all(&source);

    let mut shuffled = source.clone();
    let mut next = generator(0x9e37_79b9_7f4a_7c15);
    for index in (1..shuffled.len()).rev() {
        let swap = (next() as usize) % (index + 1);
        shuffled.swap(index, swap);
    }
    assert_ne!(shuffled, source, "the fixed-seed shuffle must actually reorder");

    let mut reordered = SectionState::default();
    reordered.apply_all(&shuffled);
    assert_eq!(reordered.fingerprint(), expected.fingerprint());
    assert_eq!(reordered.applied_count(), 10_000);
}

#[test]
fn a_repeated_delivery_is_ignored() {
    let source = deltas(200);
    let mut state = SectionState::default();
    for delta in &source {
        state.apply(delta);
        state.apply(delta);
    }
    let mut once = SectionState::default();
    once.apply_all(&source);
    assert_eq!(state.fingerprint(), once.fingerprint());
    assert_eq!(state.applied_count(), 200);
}

#[test]
fn a_truncated_frame_is_an_explicit_error() {
    let delta = SectionDelta::new(1, 2, vec![9; 12]);
    let bytes = frame::encode(&delta).expect("encode");
    assert_eq!(
        frame::decode(&bytes[..bytes.len() - 1]).err(),
        Some(ReplicationError::Framing("frame length does not match its header".into()))
    );
}

#[test]
fn rdma_reports_unsupported_with_a_named_cause() {
    let adapter = RdmaUnsupported;
    match adapter.capability() {
        Capability::Unsupported { reason } => {
            assert_eq!(reason, REASON);
            assert!(!reason.is_empty());
        }
        Capability::Available => panic!("this host must not claim RDMA support"),
    }
    let delta = SectionDelta::new(1, 2, vec![1]);
    assert_eq!(
        adapter.post_write(&delta),
        Err(ReplicationError::Unsupported(REASON.to_owned()))
    );
}

#[test]
fn a_wrong_direction_is_not_a_silent_success() {
    let receiver = TcpReceiver::bind("127.0.0.1:0").expect("bind");
    let delta = SectionDelta::new(1, 2, vec![1]);
    assert!(matches!(
        receiver.send(&delta),
        Err(ReplicationError::Unsupported(_))
    ));
}
