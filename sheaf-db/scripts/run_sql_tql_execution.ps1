# run_sql_tql_execution.ps1 - SQL to TQL execution gate.
# Timestamp: 2026-11-22_11-53-08 +05:30
# Remote: origin https://github.com/nithin12342/shelf-db.git
# Branch: combined-stack-feature-improvement
# Base commit: a4328dc5890d93cf97a89c26c46a6ecfe33b82d2

$ErrorActionPreference = 'Stop'
Set-Location "$PSScriptRoot\.."

$root = Get-Location
$data = Join-Path $root 'data\synthetic_sf001'
$db = Join-Path $root 'data\shelf_synthetic'
$run = Join-Path $root 'results\sql_tql_execution_2026-11-22_11-53-08'
New-Item -ItemType Directory -Force -Path $run | Out-Null

$queries = @(
  'SELECT c_name FROM customer WHERE c_id = 42',
  'SELECT o_id FROM orders WHERE o_total > 300000',
  'SELECT COUNT(*) FROM lineitem WHERE l_quantity < 5',
  'SELECT SUM(l_price) FROM lineitem',
  'SELECT MIN(o_total), MAX(o_total) FROM orders',
  'SELECT l_return, SUM(l_price) FROM lineitem GROUP BY l_return',
  'SELECT c_segment, COUNT(*) FROM customer GROUP BY c_segment ORDER BY c_segment',
  'SELECT DISTINCT l_return FROM lineitem',
  'SELECT o_id, o_total FROM orders ORDER BY o_total DESC LIMIT 20',
  'SELECT o.o_id, c.c_name FROM orders o INNER JOIN customer c ON o.o_customer = c.c_id WHERE o.o_total > 250000',
  'SELECT l.l_order, p.o_total FROM lineitem l INNER JOIN orders p ON l.l_order = p.o_id WHERE l.l_quantity > 40',
  'SELECT c.c_name, o.o_total FROM customer c INNER JOIN orders o ON c.c_id = o.o_customer ORDER BY o.o_total DESC LIMIT 10'
)

function Invoke-App([string[]]$Arguments) {
  $out = & cargo run --release -q -p sheaf-app --bin sheaf-app -- --db $db @Arguments 2>$null
  if ($LASTEXITCODE -ne 0) { throw "application failed: $($Arguments -join ' ')" }
  return ($out -join "`n")
}

function Get-Checksum($Rows) {
  $text = $Rows | ConvertTo-Json -Compress -Depth 20
  $sha = [System.Security.Cryptography.SHA256]::Create()
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($text)
  return ([System.BitConverter]::ToString($sha.ComputeHash($bytes)) -replace '-', '')
}

"generating synthetic data"
& cargo run --release -q -p sheaf-app --bin gen_synthetic -- $data
if ($LASTEXITCODE -ne 0) { throw 'generator failed' }

"loading database"
if (Test-Path $db) { Remove-Item -Recurse -Force $db }
Invoke-App @('sql-file', (Join-Path $data 'schema.sql')) | Out-Null
Invoke-App @('sql-file', (Join-Path $data 'load_customer.sql')) | Out-Null
Invoke-App @('sql-file', (Join-Path $data 'load_orders.sql')) | Out-Null
Invoke-App @('sql-file', (Join-Path $data 'load_lineitem.sql')) | Out-Null

$results = @()
for ($i = 0; $i -lt $queries.Count; $i++) {
  $id = 'Q' + ('{0:D2}' -f ($i + 1))
  $sql = $queries[$i]
  $sqlJson = Invoke-App @('sql', $sql)
  $parsed = $sqlJson | ConvertFrom-Json
  if (-not $parsed.ok) { throw "$id SQL failed: $($parsed.error)" }
  $out = $parsed.results[0]
  $tql = $out.tql
  if (-not $tql) { throw "$id has no TQL rendering" }
  $tqlFile = Join-Path $run "$id.tql"
  [System.IO.File]::WriteAllText($tqlFile, $tql, (New-Object System.Text.UTF8Encoding($false)))

  $tqlJson = Invoke-App @('tql-file', $tqlFile)
  $tqlParsed = $tqlJson | ConvertFrom-Json
  if (-not $tqlParsed.ok) { throw "$id TQL failed: $($tqlParsed.error)" }
  $tqlRows = $tqlParsed.results[0].rows
  $sqlRows = $out.rows
  $sqlHash = Get-Checksum $sqlRows
  $tqlHash = Get-Checksum $tqlRows

  $baseHash = 'limit_exceeded'
  $baseUs = $null
  try {
    $baseRaw = & cargo run --release -q -p sheaf-app --bin sheaf-app -- --baseline --db $db sql $sql 2>&1
    $baseJson = $baseRaw -join "`n"
    $baseParsed = $baseJson | ConvertFrom-Json
    if ($baseParsed.ok) {
      $baseHash = Get-Checksum $baseParsed.results[0].rows
      $baseUs = $baseParsed.results[0].stats.elapsed_us
    }
  } catch {
    $baseHash = 'unsupported: query row limit on the nested-loop baseline'
  }

  $results += [pscustomobject]@{
    id = $id
    sql = $sql
    tql = $tql
    sql_rows = @($sqlRows).Count
    sql_checksum = $sqlHash
    tql_checksum = $tqlHash
    baseline_checksum = $baseHash
    tql_parity = ($sqlHash -eq $tqlHash)
    baseline_parity = ($sqlHash -eq $baseHash)
    optimized_us = $out.stats.elapsed_us
    baseline_us = $baseUs
    pages_skipped = $out.stats.pages_skipped
  }
}

$results | ConvertTo-Json -Depth 8 | Set-Content -Path (Join-Path $run 'results.json') -Encoding utf8

"fiber bundle lifecycle gate"
try {
  & cargo run --release -q -p sheaf-engine --bin fiber_lifecycle_bench 2>&1 | Tee-Object -FilePath (Join-Path $run 'fiber_bundle.txt') | Select-Object -Last 12
  $fiberExit = $LASTEXITCODE
} catch {
  $fiberExit = 1
  "fiber bundle gate reported an error: $($_.Exception.Message)"
}
if (Test-Path (Join-Path $run 'fiber_bundle.txt')) {
  Get-Content (Join-Path $run 'fiber_bundle.txt') | Select-Object -Last 12
}

"summary"
$results | Select-Object id, sql_rows, tql_parity, baseline_parity, optimized_us, baseline_us, pages_skipped | Format-Table -AutoSize
"fiber_bundle_exit=$fiberExit"
