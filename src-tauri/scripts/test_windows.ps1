# test_windows.ps1
# Windows Test Runner using cargo-nextest
#
# USAGE:
#   ./scripts/test_windows.ps1                       # Run all tests
#   ./scripts/test_windows.ps1 -TestName git         # Run specific test target
#   ./scripts/test_windows.ps1 -Filter my_test_fn    # Filter to specific test function
#
# PREREQUISITES:
#   cargo install cargo-nextest --version 0.9.114 --locked

param (
    [string]$TestName = "",
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"

# Check if nextest is installed
if (-not (Get-Command cargo-nextest -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-nextest..." -ForegroundColor Yellow
    cargo install cargo-nextest --version 0.9.114 --locked
}

Write-Host "==> Running tests with nextest..." -ForegroundColor Cyan

$nexttestArgs = @("nextest", "run")

if ($TestName) {
    $nexttestArgs += @("--test", $TestName)
}

if ($Filter) {
    $nexttestArgs += @("-E", "test(/$Filter/)")
}

# Run with sanitized PATH to avoid DLL conflicts
$OldPath = $env:PATH
$env:PATH = "C:\Windows\System32;C:\Windows;C:\Program Files\Git\cmd;$($env:USERPROFILE)\.cargo\bin"

try {
    cargo @nexttestArgs
    $exitCode = $LASTEXITCODE
} finally {
    $env:PATH = $OldPath
}

if ($exitCode -ne 0) {
    Write-Warning "Tests failed with exit code $exitCode"
    exit $exitCode
}

Write-Host ""
Write-Host "==> All tests passed!" -ForegroundColor Green
exit 0
