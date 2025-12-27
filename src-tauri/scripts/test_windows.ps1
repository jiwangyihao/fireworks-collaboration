# test_windows.ps1
# Windows Test Runner with DLL Conflict Avoidance
#
# This script builds and runs Rust tests in an isolated environment to prevent
# DLL conflicts between git2-rs dependencies and system libraries (e.g., zlib from Git for Windows).
#
# USAGE:
#   ./scripts/test_windows.ps1                       # Run all tests
#   ./scripts/test_windows.ps1 -TestName git         # Run specific test target (e.g., 'git', 'commands')
#   ./scripts/test_windows.ps1 -TestName credential  # Run credential tests
#   ./scripts/test_windows.ps1 -Filter my_test_fn    # Filter to specific test function
#
# HOW IT WORKS:
#   1. Builds test binaries using the full system PATH (cargo, cmake, etc. available).
#   2. Locates the freshly built test executable in target/debug/deps/.
#   3. Runs the executable with a SANITIZED PATH (only System32) to avoid loading
#      incompatible DLLs from Git for Windows or other tools.

param (
    [string]$TestName = "",
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"

# === Phase 1: Build ===
Write-Host "==> [1/3] Building tests (using full environment)..."

# Build with tauri-core (no WebView2) to avoid DLL conflict on Windows
# tauri-core includes Tauri types but not wry/WebView2 runtime
$cargoArgs = @("test", "--no-run", "--no-default-features", "--features", "tauri-core")
if ($TestName) {
    $cargoArgs += @("--test", $TestName)
}
# Temporarily allow cargo stderr (progress output) without failing
$ErrorActionPreference = "Continue"
cargo @cargoArgs
$buildExitCode = $LASTEXITCODE
$ErrorActionPreference = "Stop"

if ($buildExitCode -ne 0) {
    Write-Error "Build failed!"
    exit $buildExitCode
}

# === Phase 2: Locate Binary ===
Write-Host "==> [2/3] Locating test binary..."

$searchPattern = if ($TestName) { "target/debug/deps/$TestName-*.exe" } else { "target/debug/deps/*.exe" }
# Filter out build_script and pdb companions
$exes = Get-ChildItem $searchPattern -ErrorAction SilentlyContinue | 
        Where-Object { $_.Name -notmatch "^build_script" } |
        Sort-Object LastWriteTime -Descending

if (-not $TestName) {
    # If running all, execute each test binary
    if ($exes.Count -eq 0) {
        Write-Error "No test binaries found!"
        exit 1
    }
    Write-Host "==> Found $($exes.Count) test binaries."
} else {
    $exe = $exes | Select-Object -First 1
    if (-not $exe) {
        Write-Error "Test binary for '$TestName' not found!"
        exit 1
    }
    $exes = @($exe)
    Write-Host "==> Found: $($exe.Name)"
}

# === Phase 3: Execute ===
Write-Host "==> [3/3] Running with CLEAN PATH to avoid DLL conflicts..."

$OldPath = $env:PATH
# Use sanitized PATH that includes:
# - System32/Windows: Core Windows functionality
# - Git\cmd: git.exe (NOT Git\usr\bin or Git\mingw64\bin which contain conflicting DLLs)
$env:PATH = "C:\Windows\System32;C:\Windows;C:\Program Files\Git\cmd"

$runArgs = @("--nocapture")
if ($Filter) {
    $runArgs += $Filter
}

$failCount = 0
foreach ($exe in $exes) {
    Write-Host ""
    Write-Host ">>> Running: $($exe.Name)..."
    & $exe.FullName @runArgs
    if ($LASTEXITCODE -ne 0) {
        $failCount++
    }
}

$env:PATH = $OldPath

if ($failCount -gt 0) {
    Write-Host ""
    Write-Warning "$failCount test suite(s) failed."
    exit 1
}

Write-Host ""
Write-Host "==> All tests passed!"
exit 0
