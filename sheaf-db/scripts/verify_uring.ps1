# verify_uring.ps1 - NF-14 host guard.
# Timestamp: 2026-11-22_09-02-56 +05:30
# Remote: origin https://github.com/nithin12342/shelf-db.git
# Branch: combined-stack-feature-improvement
# Base commit: 3b7e332

# This host is Windows. Linux io_uring cannot be verified here. The Rust test
# verifies that the platform reports Unsupported with a named cause.

Write-Output "UNSUPPORTED: io_uring fixed-buffer writes require Linux. This host is Windows."
Write-Output "Run cargo test -p sheaf-native --test uring_linux to verify the unsupported report."
