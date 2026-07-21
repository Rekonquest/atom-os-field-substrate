#!/bin/bash

# ATOM OS Field Substrate - Complete Test Suite
# Run this script in the repository root to verify all components

set -e

echo "=========================================="
echo "ATOM OS Field Substrate - Test Suite"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track results
PASSED=0
FAILED=0

# Function to run a test and track results
run_test() {
    local test_name="$1"
    local command="$2"
    
    echo -e "${YELLOW}Running: ${test_name}${NC}"
    echo "Command: $command"
    
    if eval "$command" > /tmp/test_output.log 2>&1; then
        echo -e "${GREEN}✓ PASS: ${test_name}${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}✗ FAIL: ${test_name}${NC}"
        echo "Output:"
        cat /tmp/test_output.log
        FAILED=$((FAILED + 1))
    fi
    echo ""
}

# Change to substrate directory
cd substrate

echo "=== Phase 1: Workspace Tests ==="
run_test "field-core unit tests" "cargo test -p field-core --release"
run_test "field-std tests" "cargo test -p field-std --release"
run_test "kernel-glue unit tests" "cargo test -p kernel-glue --release"

echo "=== Phase 2: Host Harness Tests ==="
run_test "host-harness falsify" "cargo run -p host-harness --release -- falsify --output /tmp/falsify-report.txt"

echo "=== Phase 3: Integration Tests ==="
run_test "closed-energy integration" "cargo run -p closed-energy --release"
run_test "ipc-energy integration" "cargo run -p ipc-energy --release"
run_test "scheduler-determinism integration" "cargo run -p scheduler-determinism --release"

# Change back to root
cd ..

echo "=== Phase 4: Patch Verification ==="
# Check if atom-os-kernel is available for patch testing
if [ -d "../atom-os-kernel" ]; then
    echo "Found local atom-os-kernel, testing patch..."
    cd ../atom-os-kernel
    
    # Save current state
    git stash push --include-untracked --message "Before patch test" || true
    
    # Apply patch
    if git apply --check ../../ATOM\ OS/patches/0001-atom-os-field-substrate.patch 2>/dev/null; then
        echo -e "${GREEN}✓ Patch applies cleanly to atom-os-kernel${NC}"
        PASSED=$((PASSED + 1))
        
        # Actually apply and test compile
        git apply ../../ATOM\ OS/patches/0001-atom-os-field-substrate.patch
        if cargo check -p kernel-kit 2>&1 | grep -q "Finished"; then
            echo -e "${GREEN}✓ Patched kernel-kit compiles${NC}"
            PASSED=$((PASSED + 1))
        else
            echo -e "${RED}✗ Patched kernel-kit does not compile${NC}"
            FAILED=$((FAILED + 1))
        fi
        
        # Revert patch
        git apply --reverse ../../ATOM\ OS/patches/0001-atom-os-field-substrate.patch
    else
        echo -e "${RED}✗ Patch does not apply cleanly${NC}"
        FAILED=$((FAILED + 1))
    fi
    
    # Restore state
    git stash pop || true
    cd ../../ATOM\ OS
else
    echo "atom-os-kernel not found, skipping patch verification"
fi

echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed${NC}"
    exit 1
fi
