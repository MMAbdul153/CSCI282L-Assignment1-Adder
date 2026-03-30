#!/usr/bin/env bash
# Run all Cobra tests and report pass/fail.
# Usage:  bash run_tests.sh
#
# Tests with a .expected file are checked automatically.
# Tests without .expected are just run (smoke-tested).
# Tests with .input file pass that content as the command-line argument.
# Error tests (exit code != 0) are checked against stderr .expected.

PASS=0
FAIL=0
SKIP=0

run_one() {
    local num=$1
    local snek="test/test${num}.snek"
    local asm="test/test${num}.s"
    local bin="test/test${num}.run"
    local exp="test/test${num}.expected"
    local inp_file="test/test${num}.input"

    [[ -f "$snek" ]] || return

    # Compile snek → assembly
    cargo run -q -- "$snek" "$asm" 2>/dev/null
    if [[ $? -ne 0 ]]; then
        echo "[FAIL] test${num}: compiler error"
        FAIL=$((FAIL+1))
        return
    fi

    # Assemble + link
    nasm -f elf64 "$asm" -o runtime/our_code.o 2>/dev/null &&
    ar rcs runtime/libour_code.a runtime/our_code.o 2>/dev/null &&
    rustc -L runtime/ runtime/start.rs -o "$bin" 2>/dev/null
    if [[ $? -ne 0 ]]; then
        echo "[FAIL] test${num}: assemble/link error"
        FAIL=$((FAIL+1))
        return
    fi

    # Determine input arg (if any)
    local arg=""
    [[ -f "$inp_file" ]] && arg=$(cat "$inp_file")

    # Run the binary
    local actual_out actual_err exit_code
    actual_out=$("$bin" $arg 2>/tmp/snek_stderr)
    exit_code=$?
    actual_err=$(cat /tmp/snek_stderr)

    # If no expected file, just smoke-test (compile + run without crash except type errors)
    if [[ ! -f "$exp" ]]; then
        echo "[OK]   test${num}: ran (no expected file)"
        PASS=$((PASS+1))
        return
    fi

    local expected
    expected=$(cat "$exp")

    # For error tests the expected output is in stderr
    if [[ $exit_code -ne 0 ]]; then
        if [[ "$actual_err" == "$expected" ]]; then
            echo "[OK]   test${num}: runtime error matches"
            PASS=$((PASS+1))
        else
            echo "[FAIL] test${num}: expected stderr '${expected}' got '${actual_err}'"
            FAIL=$((FAIL+1))
        fi
    else
        if [[ "$actual_out" == "$expected" ]]; then
            echo "[OK]   test${num}: ${actual_out}"
            PASS=$((PASS+1))
        else
            echo "[FAIL] test${num}: expected '${expected}' got '${actual_out}'"
            FAIL=$((FAIL+1))
        fi
    fi
}

# Run all numbered tests found in the test directory
for snek in test/test*.snek; do
    num=$(basename "$snek" .snek | sed 's/test//')
    run_one "$num"
done

echo ""
echo "Results: ${PASS} passed, ${FAIL} failed, ${SKIP} skipped"
[[ $FAIL -eq 0 ]]   # exit 0 if all passed
