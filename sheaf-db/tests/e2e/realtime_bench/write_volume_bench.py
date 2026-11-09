#!/usr/bin/env python
"""write_volume_bench.py - Measure 1,000,000 writes for grouped vs ungrouped commits."""

import os
import tempfile
import time

RECORDS = 1_000_000
SAMPLE_UNGROUPED = 10_000  # Sample ungrouped due to physical fsync floor (~10 us each)

def test_ungrouped():
    """Simulate ungrouped commit: write and flush per record."""
    tmp = tempfile.NamedTemporaryFile(delete=False)
    payload = b"X" * 33  # 33-byte cell frame matching ShelfDB WAL
    t0 = time.time()
    for _ in range(SAMPLE_UNGROUPED):
        tmp.write(payload)
        tmp.flush()
        os.fsync(tmp.fileno())
    elapsed = time.time() - t0
    tmp.close()
    os.remove(tmp.name)
    per_op_us = (elapsed / SAMPLE_UNGROUPED) * 1e6
    proj_1m_s = per_op_us * 1e-6 * RECORDS
    return per_op_us, proj_1m_s

def test_grouped():
    """Simulate grouped commit: 1,000,000 writes with batched epoch flushes."""
    tmp = tempfile.NamedTemporaryFile(delete=False)
    payload = b"X" * 33
    batch_size = 10_000
    t0 = time.time()
    for i in range(RECORDS):
        tmp.write(payload)
        if (i + 1) % batch_size == 0:
            tmp.flush()
            os.fsync(tmp.fileno())
    tmp.flush()
    os.fsync(tmp.fileno())
    elapsed = time.time() - t0
    tmp.close()
    os.remove(tmp.name)
    per_op_ns = (elapsed / RECORDS) * 1e9
    throughput = RECORDS / elapsed
    return elapsed, per_op_ns, throughput

def main():
    print("=== High Volume Write Benchmark (1,000,000 Records) ===")
    ungrouped_us, proj_ungrouped_s = test_ungrouped()
    print(f"Ungrouped Commit (per-op sync): {ungrouped_us:.2f} us/op (Projected 1M: {proj_ungrouped_s:.2f} s)")
    grouped_s, grouped_ns, tput = test_grouped()
    print(f"Grouped Commit (epoch sync):   {grouped_ns:.1f} ns/op ({grouped_s:.2f} s for 1M, {tput:,.0f} ops/s)")
    ratio = (ungrouped_us * 1000.0) / grouped_ns
    print(f"Grouped Commit Speedup:        {ratio:.1f}x faster")

if __name__ == "__main__":
    main()
