#!/usr/bin/env bash
set -u
cd /workspace/Pumas-Library
export PUMAS_QUALIFIED_UV=/workspace/pumas-reconcile-tools/bin/uv
export PYTHONPATH=/workspace/Pumas-Library/torch-server/tests
export PYTHONDONTWRITEBYTECODE=1
results=/workspace/pumas-parser-fix-evidence
python_bin=/workspace/pumas-parser-test-venv/bin/python
git rev-parse HEAD 'HEAD^{tree}' > "$results/tested-source.txt"
: > "$results/python-gate-results.txt"
run_gate() {
  gate_name=$1
  shift
  "$@" > "$results/$gate_name.log" 2>&1
  gate_result=$?
  printf '%s %s\n' "$gate_name" "$gate_result" >> "$results/python-gate-results.txt"
}
run_gate actual-import-probes "$python_bin" -B "$results/record-import-probes.py"
run_gate python-affected "$python_bin" -B -m unittest -v test_probe_runtime test_consume_selected_wheels test_offline_wheel_selection test_wheel_catalog_owner test_wheel_target test_target_observation test_install_verified_wheels test_qualified_wheel_catalog
run_gate python-full "$python_bin" -B -m unittest discover -s torch-server/tests -v
run_gate ruff "$python_bin" -m ruff check torch-server
run_gate source-unchanged git diff --exit-code 6699ffc89423557f45a596295c3f89698d5d342b -- rust torch-server
cat "$results/python-gate-results.txt"
