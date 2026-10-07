"""Run qualification against one frozen source; retain commands and raw results."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
tree = subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], cwd=root, text=True).strip()
env = os.environ.copy()
env.update(RUSTUP_HOME='/workspace/.pumas-tools/rustup', CARGO_HOME='/workspace/.pumas-tools/cargo',
    ORT_SKIP_DOWNLOAD='1', CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0',
    CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0',
    CARGO_TARGET_DIR='/workspace/pumas-selected-reconcile-saved-target',
    XDG_CONFIG_HOME='/workspace/pumas-selected-reconcile-saved-config',
    PUMAS_QUALIFIED_UV='/workspace/pumas-reconcile-tools/bin/uv',
    PYTHONDONTWRITEBYTECODE='1',
    PYTHONPATH='/workspace/pumas-reconcile-python-full:/workspace/pumas-reconcile-tools:'+str(root/'torch-server/tests'),
    PUMAS_SELECTED_LIFECYCLE_EVIDENCE=str(out/'publication'))
env['PATH'] = '/workspace/.pumas-tools/cargo/bin:'+env['PATH']
cargo = ['cargo']
common = ['--offline', '--locked', '--manifest-path', 'rust/Cargo.toml']
workspace = ['--workspace', '--exclude', 'pumas_rustler']
rust = [
 ('graphs', ['python3', 'scripts/release/check-dependency-features.py', '--s3']),
 ('lifecycle', cargo+['test']+common+['-p','pumas-app-manager','--features','test-support','selected_runtime_','--','--include-ignored','--test-threads=1','--nocapture']),
 ('rust-affected', cargo+['test']+common+['-p','pumas-app-manager','--features','test-support','version_manager::installer::torch::torch_','--','--include-ignored','--test-threads=1','--skip','selected_runtime_','--nocapture']),
 ('workspace-test', cargo+['test']+common+workspace+['--','--test-threads=1']),
 ('workspace-check', cargo+['check']+common+workspace+['--all-targets','--all-features']),
 ('workspace-clippy', cargo+['clippy']+common+workspace+['--all-targets','--all-features','--','-D','warnings']),
 ('no-default', cargo+['check']+common+workspace+['--no-default-features']),
 ('doc', cargo+['test']+common+workspace+['--doc']),
 ('rustfmt', cargo+['fmt','--manifest-path','rust/Cargo.toml','--all','--','--check']),
 ('evidence-verification', ['python3','docs/plans/artifact-acquisition/reports/torch-selected-lifecycle-2026-10-06/verify_evidence.py',str(out/'publication'),str(root)]),
 ('prior-evidence', ['python3','docs/plans/artifact-acquisition/reports/torch-selected-lifecycle-2026-10-06/verify_evidence.py'])]
python = [
 ('python-affected', ['python3','-m','unittest','-v','test_probe_runtime','test_consume_selected_wheels','test_offline_wheel_selection','test_wheel_catalog_owner','test_wheel_target','test_target_observation','test_install_verified_wheels','test_qualified_wheel_catalog']),
 ('python-full', ['python3','-m','unittest','discover','-s','torch-server/tests','-v']),
 ('ruff', ['/workspace/pumas-reconcile-tools/bin/ruff','check','--no-cache','torch-server'])]
lane = sys.argv[1]
rows = []
for name, command in rust if lane == 'rust' else python:
    start = time.time()
    with (out/(name+'.log')).open('w') as log:
        result = subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
    rows.append(dict(gate=name,command=command,exit_code=result.returncode,elapsed_seconds=round(time.time()-start,3),source=source,tree=tree))
    (out/(lane+'-results.json')).write_text(json.dumps(rows,indent=2)+'\n')
    print(name,result.returncode,flush=True)
    if result.returncode and name != 'python-full':
        sys.exit(result.returncode)
changed = subprocess.run(['git','diff','--exit-code',source,'--','rust','torch-server'],cwd=root,capture_output=True)
(out/(lane+'-source-unchanged.log')).write_bytes(changed.stdout+changed.stderr)
sys.exit(changed.returncode)
