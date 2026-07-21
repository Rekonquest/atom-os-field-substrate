# ATOM OS Field Substrate - Quick Test Script for Windows
# Run this in PowerShell from the repository root

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "ATOM OS Field Substrate - Quick Test Suite" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

$passed = 0
$failed = 0

function Run-Test {
    param(
        [string]$Name,
        [string]$Command
    )
    
    Write-Host "Running: $Name" -ForegroundColor Yellow
    Write-Host "Command: $Command"
    
    try {
        $result = Invoke-Expression -Command $Command -ErrorAction Stop 2>&1
        Write-Host "✓ PASS: $Name" -ForegroundColor Green
        $passed++
    } catch {
        Write-Host "✗ FAIL: $Name" -ForegroundColor Red
        Write-Host "Error: $_" -ForegroundColor Red
        $failed++
    }
    Write-Host ""
}

# Change to substrate directory
Set-Location substrate

Write-Host "=== Workspace Tests ===" -ForegroundColor Cyan
Run-Test "field-core unit tests" "cargo test -p field-core --release"
Run-Test "kernel-glue unit tests" "cargo test -p kernel-glue --release"

Write-Host "=== Integration Tests ===" -ForegroundColor Cyan
Run-Test "closed-energy integration" "cargo run -p closed-energy --release"
Run-Test "ipc-energy integration" "cargo run -p ipc-energy --release"
Run-Test "scheduler-determinism integration" "cargo run -p scheduler-determinism --release"

# Back to root
Set-Location ..

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Test Summary" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Passed: $passed" -ForegroundColor Green
Write-Host "Failed: $failed" -ForegroundColor Red
Write-Host ""

if ($failed -eq 0) {
    Write-Host "All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Some tests failed" -ForegroundColor Red
    exit 1
}
