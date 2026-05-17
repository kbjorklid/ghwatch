$ErrorActionPreference = 'Stop'

Set-Location $env:CLAUDE_PROJECT_DIR

# Format in-place (always succeeds)
cargo fmt 2>&1 | Out-Null

# Run clippy — treat warnings as errors
$clippy = cargo clippy -- -D warnings 2>&1
$clippyExit = $LASTEXITCODE

if ($clippyExit -ne 0) {
    $output = @{
        decision = "block"
        reason   = "cargo clippy found issues that must be fixed before finishing:`n$clippy"
    } | ConvertTo-Json -Compress
    Write-Output $output
    exit 0
}

Write-Output '{"decision":"approve"}'
