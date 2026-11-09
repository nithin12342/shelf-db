# fiber_progress_monitor.ps1 — 10-minute progress checkpoints for the bench run.
# Appends one status line per checkpoint; exits (and thus alerts) when the run
# finishes. Purely read-only against the run's output file.
param(
  [string]$OutFile = (Join-Path $env:TEMP "sheaf_progress.out"),
  [string]$StatusLog = (Join-Path $env:TEMP "bench_progress.log"),
  [int]$IntervalSec = 600
)

$start = Get-Date
$tick = 0
while ($true) {
  Start-Sleep -Seconds $IntervalSec
  $tick++
  $now = Get-Date
  $elapsed = [math]::Round(($now - $start).TotalMinutes, 1)
  $txt = ""
  if (Test-Path $OutFile) { $txt = Get-Content $OutFile -Raw -ErrorAction SilentlyContinue }
  $done = ($txt -match "SAVED .*RUN_SUMMARY")
  # current phase: last engine/bin header seen
  $phases = [regex]::Matches($txt, "(?m)^===== (.+?) =====") | ForEach-Object { $_.Groups[1].Value }
  $phase = if ($phases) { $phases[-1] } else { "starting" }
  # last gate/status line of interest
  $last = ($txt -split "`n" | Where-Object { $_ -match "gate_|PASS|FAIL|SKIP|ERROR|SAVED|:" } | Select-Object -Last 1)
  if ($null -eq $last) { $last = "" }
  $line = "[{0}] +{1}min phase={2} last={3}" -f $now.ToString("HH:mm:ss"), $elapsed, $phase, ($last -replace "\s+", " ").Trim()
  Add-Content -Path $StatusLog -Value $line
  if ($done) {
    Add-Content -Path $StatusLog -Value "[$($now.ToString('HH:mm:ss'))] RUN COMPLETE after $($elapsed) min"
    Write-Output "RUN COMPLETE after $elapsed min"
    # final headline block for immediate reporting
    $m = [regex]::Matches($txt, "(?m)^(sqlite|duckdb|redis|mongodb|mariadb|pg|neo4j|scylladb) \[([^\]]+)\]: (\{.*\})")
    foreach ($x in $m) { Write-Output ("HEADLINE {0} {1} {2}" -f $x.Groups[1].Value, $x.Groups[2].Value, $x.Groups[3].Value) }
    $s = [regex]::Matches($txt, "(?m)^-- ([a-z0-9_]+): (.*)$")
    foreach ($x in $s) { Write-Output ("SHELF {0} :: {1}" -f $x.Groups[1].Value, $x.Groups[2].Value.Trim()) }
    break
  }
}
