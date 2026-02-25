#!/bin/bash
# Test script to run only error_recovery tests

set -e

echo "Testing error_recovery module..."
cargo test --lib --test-threads=1 -- error_recovery 2>&1 | tee test_output.log

echo ""
echo "Test Summary:"
grep -E "(test result:|running)" test_output.log || true
