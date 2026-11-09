param([string]$Dir = (Join-Path $PSScriptRoot "results"))
$files = Get-ChildItem "$Dir\pl_run*.txt" | Sort-Object Name
$runs = $files | ForEach-Object { Get-Content $_.FullName }

# Each entry is: label | regex with ONE capture group for the value.
# The regex must be anchored enough that "set_kb=32" cannot match
# "set_kb=32768", and must find the value even when several key=value
# pairs share one output line.
$specs = @(
  @{ L = "L1 latency  (32 KiB set)";  R = 'A_latency_ns set_kb=32 pages_4k=\d+ ns=([0-9.]+)' },
  @{ L = "L1 latency  (32 KiB set)";  R = 'A_latency_ns set_kb=32 pages_4k=\d+ ns=[0-9.]+ cycles=([0-9.]+)' },
  @{ L = "L2 latency  (128 KiB set)"; R = 'A_latency_ns set_kb=128 pages_4k=\d+ ns=([0-9.]+)' },
  @{ L = "L2 latency  (128 KiB set)"; R = 'A_latency_ns set_kb=128 pages_4k=\d+ ns=[0-9.]+ cycles=([0-9.]+)' },
  @{ L = "L2 latency  (1 MiB set)";  R = 'A_latency_ns set_kb=1024 pages_4k=\d+ ns=([0-9.]+)' },
  @{ L = "L2 latency  (1 MiB set)";  R = 'A_latency_ns set_kb=1024 pages_4k=\d+ ns=[0-9.]+ cycles=([0-9.]+)' },
  @{ L = "L3 latency  (4 MiB set)";  R = 'A_latency_ns set_kb=4096 pages_4k=\d+ ns=([0-9.]+)' },
  @{ L = "L3 latency  (4 MiB set)";  R = 'A_latency_ns set_kb=4096 pages_4k=\d+ ns=[0-9.]+ cycles=([0-9.]+)' },
  @{ L = "DRAM latency (128 MiB set)"; R = 'A_latency_ns set_kb=131072 pages_4k=\d+ ns=([0-9.]+)' },
  @{ L = "DRAM latency (128 MiB set)"; R = 'A_latency_ns set_kb=131072 pages_4k=\d+ ns=[0-9.]+ cycles=([0-9.]+)' },
  @{ L = "TLB few-pages (64)";    R = 'A_tlb_few_pages_64_ns=([0-9.]+)' },
  @{ L = "TLB many-pages (16384)";R = 'A_tlb_many_pages_16384_ns=([0-9.]+)' },
  @{ L = "TLB penalty";           R = 'A_tlb_penalty_ns=([0-9.]+)' },
  @{ L = "clock before sweep";    R = 'A_clock_GHz_before=([0-9.]+)' },
  @{ L = "clock after sweep";     R = 'A_clock_GHz_after=([0-9.]+)' },
  @{ L = "read 1-thread";         R = 'B_read_1t_mean_GBps=([0-9.]+)' },
  @{ L = "read 2-thread";         R = 'B_read_GBps threads=2 gbs=([0-9.]+)' },
  @{ L = "read 4-thread";         R = 'B_read_GBps threads=4 gbs=([0-9.]+)' },
  @{ L = "read 8-thread";         R = 'B_read_GBps threads=8 gbs=([0-9.]+)' },
  @{ L = "clock (section C)";     R = 'C_clock_GHz=([0-9.]+)' },
  @{ L = "clock crosscheck add";  R = 'C_clock_crosscheck_add_ratio=([0-9.]+)' },
  @{ L = "clock crosscheck imul"; R = 'C_clock_crosscheck_imul_ratio=([0-9.]+)' },
  @{ L = "flops avx512 (1T)";     R = 'C_flops avx512 gflops=([0-9.]+)' },
  @{ L = "flops all-threads";     R = 'C_flops_allthreads gflops=([0-9.]+)' },
  @{ L = "write 4 KiB";           R = 'D_write_4KiB_p50_us=([0-9.]+)' },
  @{ L = "fsync 4 KiB";           R = 'D_fsync_4KiB_p50_us=([0-9.]+)' },
  @{ L = "write 1 MiB";           R = 'D_write_1MiB_p50_us=([0-9.]+)' },
  @{ L = "seq read (uncached)";   R = 'D_seq_read_uncached_GBps=([0-9.]+)' },
  @{ L = "rand 4K read (uncached)"; R = 'D_rand4k_read_uncached_p50_us=([0-9.]+)' },
  @{ L = "quad L3 TLB-hit";       R = 'E_quad_L3_tlbhit_ns=([0-9.]+)' },
  @{ L = "quad L3 TLB-miss";      R = 'E_quad_L3_tlbmiss_ns=([0-9.]+)' },
  @{ L = "quad DRAM TLB-miss";    R = 'E_quad_DRAM_tlbmiss_ns=([0-9.]+)' },
  @{ L = "page walk";             R = 'E_page_walk_ns=([0-9.]+)' },
  @{ L = "DRAM access component"; R = 'E_dram_access_component_ns=([0-9.]+)' },
  @{ L = "DRAM vs CAS floor (x)"; R = 'E_dram_vs_ddr4_cas_floor_x=([0-9.]+)' },
  @{ L = "BDP bytes";             R = 'E_bdp_bytes_at_dram_latency=([0-9.]+)' },
  @{ L = "DRAM 1t % of 1ch peak"; R = 'E_dram_1t_pct_of_1ch_peak=([0-9.]+)' },
  @{ L = "DRAM all % of 1ch peak";R = 'E_dram_all_pct_of_1ch_peak=([0-9.]+)' },
  @{ L = "seq read % of PCIe4x4";R = 'E_seq_read_pct_of_pcie4_x4=([0-9.]+)' },
  @{ L = "storage/DRAM ratio";    R = 'E_storage_over_dram_x=([0-9.]+)' },
  @{ L = "same-window read";      R = 'G_same_window_read_GBps=([0-9.]+)' },
  @{ L = "same-window gflops";    R = 'G_same_window_gflops=([0-9.]+)' },
  @{ L = "same-window balance";   R = 'G_same_window_machine_balance_flop_per_byte=([0-9.]+)' }
)

foreach ($s in $specs) {
  $vals = @()
  foreach ($r in $runs) {
    foreach ($line in $r) {
      $m = [regex]::Match($line, $s.R)
      if ($m.Success) { $vals += [double]$m.Groups[1].Value; break }
    }
  }
  if ($vals.Count -gt 0) {
    $q = @($vals | Sort-Object)
    $med = $q[[int][math]::Floor($q.Count / 2)]
    Write-Output ("{0,-30} med={1,-12} min={2,-10} max={3,-10} runs={4}" -f $s.L, $med, $q[0], $q[$q.Count-1], ($vals -join " / "))
  }
}
