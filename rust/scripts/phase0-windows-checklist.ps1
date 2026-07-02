$ErrorActionPreference = "Stop"

Write-Host "== dw Rust rewrite / Phase 0 =="
Write-Host "1. Verify Rust toolchain"
cargo --version

Write-Host "2. Workspace status"
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- phase0 status

Write-Host "3. PAT/env auth detection"
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- ado auth-env

Write-Host "4. Optional expanded work item read"
Write-Host "cargo run --manifest-path rust/Cargo.toml -p dw-cli -- ado expanded-work-item --organization https://dev.azure.com/<org> --project <project> --work-item <id>"

Write-Host "5. SQL readonly guard"
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- db guard --sql "select top 1 1 as ok"

Write-Host "6. Capture results in rust/docs/phase0-results.md"
