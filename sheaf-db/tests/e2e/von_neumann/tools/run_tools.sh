# Rebuild and run the two standalone projection tools.
# They are NOT cargo targets: they measure the cost of work the engine does,
# so they live here as plain rustc programs.
#
# Before you begin: a rustc toolchain on the PATH.
# Verify: the output shows HASHBUILD, HASHPROBE, SEQSCAN, ANSWERONLY,
# FIBER_TOP10_SCAN and FIBER_TOP10_PRESORTED lines.
set -e
cd "$(dirname "$0")"

for tool in cpu_floor q1_decompose q2_decompose q2_projection crc_pass arch_gains; do
  rustc -O "$tool.rs" -o "$tool.exe"
  "./$tool.exe"
  echo "--- $tool done ---"
done

echo "PROJECTION TOOLS: COMPLETE"
