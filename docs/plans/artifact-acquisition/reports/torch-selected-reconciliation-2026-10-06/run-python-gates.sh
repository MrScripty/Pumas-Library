#!/usr/bin/env bash
set -u
cd /workspace/Pumas-Library
export PUMAS_QUALIFIED_UV=/workspace/pumas-reconcile-tools/bin/uv
export PYTHONPATH=/workspace/pumas-reconcile-python-full:/workspace/pumas-reconcile-tools:torch-server/tests
python3 -m unittest -v test_probe_runtime test_consume_selected_wheels test_offline_wheel_selection test_wheel_catalog_owner test_wheel_target test_target_observation test_install_verified_wheels test_qualified_wheel_catalog
python3 -m unittest discover -s torch-server/tests -v
