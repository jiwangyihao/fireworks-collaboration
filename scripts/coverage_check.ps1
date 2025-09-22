Param(
  [string]$LcovPath = "lcov.info"
)

if (-not (Test-Path $LcovPath)) {
  Write-Error "LCOV 文件不存在: $LcovPath"
  exit 2
}

$linesTotal = 0
$linesHit = 0
Get-Content $LcovPath | ForEach-Object {
  if ($_ -match '^DA:(\d+),(\d+)') {
    $linesTotal += 1
    if ([int]$Matches[2] -gt 0) { $linesHit += 1 }
  }
}

if ($linesTotal -eq 0) {
  Write-Warning "未检测到任何 DA 行，可能传入了错误的 lcov 文件。"
  exit 0
}

$pct = [math]::Round(($linesHit / $linesTotal) * 100, 2)
$min = $env:FWC_COVERAGE_MIN_LINE
if (-not $min -or -not ($min -as [double])) { $min = 75 }
$enforce = $env:FWC_COVERAGE_ENFORCE -eq '1'

Write-Host "Line Coverage: $pct% ($linesHit/$linesTotal) Threshold: $min% Enforce: $enforce"

if ($pct -lt $min) {
  if ($enforce) {
    Write-Error "覆盖率低于阈值 ($pct% < $min%)"
    exit 1
  } else {
    Write-Warning "覆盖率低于阈值 ($pct% < $min%) (软门控)"
  }
}
exit 0