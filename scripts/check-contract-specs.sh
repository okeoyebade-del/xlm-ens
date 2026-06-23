#!/usr/bin/env bash
set -euo pipefail

# Script to check contract spec changes against a baseline.
# Prints a markdown table of changes and exits with non-zero if breaking changes are found.

BASELINE_DIR="${1:-specs/baseline}"
TEMP_DIR="${2:-/tmp/contract_specs_new}"
MARKDOWN_FILE="${3:-artifacts/contract-spec-report.md}"

echo "Checking contract spec changes against baseline..."
echo "Baseline directory: $BASELINE_DIR"
echo "Temporary directory for new specs: $TEMP_DIR"
echo "Markdown report will be written to: $MARKDOWN_FILE"

# Ensure the temporary directory exists
mkdir -p "$TEMP_DIR"

# List of contracts (must match the names of the WASM files without the .wasm extension)
CONTRACTS=(
    xlm_ns_registry
    xlm_ns_registrar
    xlm_ns_resolver
    xlm_ns_auction
    xlm_ns_subdomain
    xlm_ns_nft
    xlm_ns_bridge
)

# Ensure jq is available
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed." >&2
    exit 1
fi

# Generate new specs for each contract (if WASM exists)
echo "Generating new specs..."
for contract in "${CONTRACTS[@]}"; do
    wasm_file="target/wasm32-unknown-unknown/release/${contract}.wasm"
    if [[ ! -f "$wasm_file" ]]; then
        echo "WARNING: WASM not found for $contract, skipping spec generation." >&2
        continue
    fi
    # Generate spec and save to temporary directory
    soroban contract spec --wasm "$wasm_file" --output json > "$TEMP_DIR/${contract}.json"
    echo "Generated spec for $contract"
done

# Compare each new spec with the baseline
{
    echo "| Contract | Change Type | Details |"
    echo "|----------|-------------|---------|"
    BREAKING_FOUND=0
    ADDITIVE_FOUND=0
    for contract in "${CONTRACTS[@]}"; do
        baseline_file="$BASELINE_DIR/${contract}.json"
        new_file="$TEMP_DIR/${contract}.json"

        if [[ ! -f "$baseline_file" ]]; then
            echo "| $contract | ⚠️ BASELINE MISSING | Baseline spec not found at $baseline_file |"
            continue
        fi

        if [[ ! -f "$new_file" ]]; then
            echo "| $contract | ❌ NEW SPEC MISSING | New spec not generated (WASM missing?) |"
            BREAKING_FOUND=1
            continue
        fi

        # Sort JSON keys for stable comparison
        baseline_sorted=$(jq -S . "$baseline_file" 2>/dev/null || echo "")
        new_sorted=$(jq -S . "$new_file" 2>/dev/null || echo "")

        if [[ "$baseline_sorted" == "$new_sorted" ]]; then
            echo "| $contract | ✅ UNCHANGED | No changes detected |"
            continue
        fi

        # Compute diff to see what changed
        diff_output=$(diff -u <(echo "$baseline_sorted") <(echo "$new_sorted") || true)
        # Count lines starting with '-' (removed) and '+' (added)
        removed_lines=$(echo "$diff_output" | grep -E '^-' | grep -v '^---' | wc -l)
        added_lines=$(echo "$diff_output" | grep -E '^+' | grep -v '^\+\+\+' | wc -l)

        if [[ $removed_lines -gt 0 && $added_lines -eq 0 ]]; then
            change_type="🔴 BREAKING (removals)"
            BREAKING_FOUND=1
        elif [[ $removed_lines -eq 0 && $added_lines -gt 0 ]]; then
            change_type="🟢 ADDITIVE (additions)"
            ADDITIVE_FOUND=1
        else
            change_type="🔴 BREAKING (modifications and/or additions+removals)"
            BREAKING_FOUND=1
        fi
        # Provide a brief detail: number of lines removed/added
        details="-${removed_lines} +${added_lines} lines"
        echo "| $contract | $change_type | $details |"
    done
} > "$MARKDOWN_FILE"

echo "Report written to $MARKDOWN_FILE"
cat "$MARKDOWN_FILE"

if [[ $BREAKING_FOUND -eq 1 ]]; then
    echo "::error::Breaking changes detected in contract specs."
    exit 1
else
    echo "No breaking changes detected."
fi