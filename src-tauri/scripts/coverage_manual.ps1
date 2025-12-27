# scripts/coverage_manual.ps1
$ErrorActionPreference = "Stop"

# 1. Setup Environment
$OldPath = $env:PATH
$BuildPath = "$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\CMake\bin;$env:PATH"
$CleanPath = "C:\Windows\System32;C:\Windows"

# 2. Build Instrumented Binaries (Full Environment)
Write-Host "==> [1/3] Building instrumented tests..."
$env:PATH = $BuildPath
# Ensure we clean previous profiles to avoid mixing
if (Test-Path "target/llvm-cov-target/profiles") {
    Remove-Item "target/llvm-cov-target/profiles" -Recurse -Force
}
# Build without running. Note: We use --workspace to cover everything.
# Build without running. Note: We use --workspace to cover everything.
# llvm-cov implicitly runs 'test' by default. --no-run build the binaries.
cargo llvm-cov --workspace --no-run

# 3. Run Binaries (Clean Environment)
Write-Host "==> [2/3] Running tests in isolated environment..."
$env:PATH = $CleanPath

# Find all test binaries in the llvm-cov specific target dir
# Exclude build scripts (build_script_build-*)
$Binaries = Get-ChildItem "target/llvm-cov-target/debug/deps/*.exe" | 
            Where-Object { $_.Name -notmatch "^build_script" -and $_.Name -notmatch "-[0-9a-f]+\.exe$" } 
            # Note: Integration tests are like "commands-hash.exe", unit tests are "fireworks_collaboration-hash.exe"
            # The previous regex might exclude too much. Let's just run everything that looks executable and isn't a build script.
            # Actually, standard cargo test binaries have a hash suffix.
            
$Binaries = Get-ChildItem "target/llvm-cov-target/debug/deps/*.exe" | 
            Where-Object { $_.Name -notmatch "^build_script" }

$ProfileDir = Join-Path (Get-Location) "target/llvm-cov-target/profiles"
New-Item -ItemType Directory -Force -Path $ProfileDir | Out-Null

foreach ($bin in $Binaries) {
    Write-Host " -> Running $($bin.Name)..."
    $env:LLVM_PROFILE_FILE = "$ProfileDir\cov-%p-%m.profraw"
    
    # Run, ignoring failures (we want to collect coverage even if tests fail due to environment)
    try {
        & $bin.FullName
    } catch {
        Write-Warning "Binary $($bin.Name) crashed or failed: $_"
    }
}

# 4. Generate Report (Full Environment)
Write-Host "==> [3/3] Generating coverage report..."
$env:PATH = $BuildPath
# Generate text report for analysis
cargo llvm-cov report --text --output-path coverage_true.txt

Write-Host "==> Done. Report saved to coverage_true.txt"
