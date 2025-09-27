# Adaptive TLS Soak Runner

This utility exercises the adaptive TLS transport end-to-end using long-running push/fetch/clone cycles. It is intended for pre-release stability soak as described in the P3.6 plan.

## Usage

1. Ensure the environment variable `FWC_ADAPTIVE_TLS_SOAK` is set to `1`.
2. Optional environment overrides:
   - `FWC_SOAK_ITERATIONS`: number of iterations (default `10`).
   - `FWC_SOAK_KEEP_CLONES`: set to `1` to keep per-iteration clone folders.
   - `FWC_SOAK_REPORT_PATH`: output path for the JSON report (default `soak-report.json`).
   - `FWC_SOAK_BASE_DIR`: workspace root; when omitted a temp directory is used.
   - `FWC_SOAK_BASELINE_REPORT`: optional path to a previous soak report for comparison.
3. Run the binary:

```powershell
$env:FWC_ADAPTIVE_TLS_SOAK = '1'
cargo run -p fireworks-collaboration --bin adaptive_tls_soak
```

The runner writes a structured report summarising success rates, fallback events, and timing statistics. When `FWC_SOAK_BASELINE_REPORT` is provided, the output also includes a `comparison` section highlighting deltas versus the baseline (success rate, fallback ratio, certificate fingerprint events, and auto-disable counts) along with regression flags. The JSON is suitable for archival or automated threshold checks.

## Report Structure

The report contains:
- `totals`: overall task counts and success rate.
- `timing`: per operation timing samples (min/avg/p50/p95) for connect/TLS/first-byte/total phases.
- `fallback` and `auto_disable`: frequency of fallback transitions and runtime safeguards.
- `thresholds`: pass/fail flags for the ≥99% success rate and ≤5% fake→real fallback ratio.
- `comparison`: optional summary of deltas against a provided baseline report.

## Cleanup

By default clone working directories are removed after each iteration. The configuration directory (used for runtime config and metrics) is kept under the base directory for post-mortem inspection.
