"""Four public CLI comparisons with direct native tags; no installation."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
from urllib.parse import unquote, urlparse

sys.dont_write_bytecode = True
REPO = Path(sys.argv[1]).resolve()
ROOT = Path(sys.argv[2]).resolve()
ROOT.mkdir()
PRIOR_COMMIT = 'd5317a2e218b79b102b1a55db5b9053724be7e43'
PRIOR_TREE = '138111c43300d6a68170c06efe6d7d81708d97f9'
PRIOR = REPO / 'docs/plans/artifact-acquisition/reports/torch-native-glibc241-diagnostic-2026-10-06'
prior_summary = json.loads((PRIOR / 'summary.json').read_text())
prior_manifest = json.loads((PRIOR / 'published-sha256-manifest.json').read_text())
UV = Path(prior_summary['cases'] and json.loads((PRIOR / 'native-floor42-refusal/invocation.json').read_text())['argv'][0])
PYTHON = Path(prior_summary['interpreter'])
assert UV.is_absolute() and PYTHON.is_absolute()
assert hashlib.sha256(UV.read_bytes()).hexdigest() == prior_summary['uv_sha256']
assert hashlib.sha256(PYTHON.read_bytes()).hexdigest() == prior_summary['interpreter_sha256']
assert subprocess.check_output(['git', 'rev-parse', PRIOR_COMMIT + '^{tree}'], cwd=REPO, text=True).strip() == PRIOR_TREE

def read_prior(relative):
    path = PRIOR / relative
    data = path.read_bytes()
    assert hashlib.sha256(data).hexdigest() == prior_manifest[relative], relative
    repo_path = str(path.relative_to(REPO))
    assert subprocess.check_output(['git', 'show', PRIOR_COMMIT + ':' + repo_path], cwd=REPO) == data
    return data

approved = json.loads(read_prior('native-observation.json'))
wheels = ROOT / 'wheels'
wheels.mkdir()
wheel_bindings = []
for path in sorted((PRIOR / 'wheels').glob('*.whl')):
    relative = 'wheels/' + path.name
    data = read_prior(relative)
    (wheels / path.name).write_bytes(data)
    wheel_bindings.append({'filename': path.name, 'size': len(data),
                           'sha256': hashlib.sha256(data).hexdigest(), 'prior_relative_path': relative})
assert len(wheel_bindings) == 7

controls = [
    ('native-floor42-refusal', 'direct-native-higher-floor-refusal', 1, []),
    ('native-wrongabi-refusal', 'direct-native-python-abi-refusal', 1, []),
    ('native-floor40', 'direct-native-lower-floor', 0, ['native-floor40']),
    ('native-linux-tag', 'direct-native-linux-tag', 0, ['native-linux']),
]
results = []
for prior_case, case, expected, expected_names in controls:
    directory = ROOT / case
    directory.mkdir()
    for leaf in ('config', 'cache', 'tmp'):
        (directory / leaf).mkdir()
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8',
           'XDG_CONFIG_HOME': str(directory / 'config'), 'UV_CACHE_DIR': str(directory / 'cache'),
           'TMPDIR': str(directory / 'tmp')}
    original_bytes = read_prior(prior_case + '/invocation.json')
    original = json.loads(original_bytes)
    argv = original['argv'].copy()
    assert '--python-platform' not in argv
    version_at = argv.index('--python-version')
    assert argv[version_at + 1] == '3.12.14'
    del argv[version_at:version_at + 2]
    assert '--python-platform' not in argv and '--python-version' not in argv
    assert argv[argv.index('--python') + 1] == str(PYTHON)
    argv[argv.index('--find-links') + 1] = str(wheels)
    roots = read_prior(prior_case + '/roots.in')
    (directory / 'roots.in').write_bytes(roots)
    observer_argv = [str(PYTHON), '-B', '-I', str(REPO / 'torch-server/wheel_target.py'), '--observe']
    before_bytes = subprocess.check_output(observer_argv, cwd=directory, env=env, timeout=30)
    before = json.loads(before_bytes)
    assert before == approved, case
    (directory / 'observation-before.json').write_bytes(before_bytes)
    completed = subprocess.run(['strace', '-f', '-e', 'trace=network,process',
                                '-o', str(directory / 'trace'), *argv],
                               cwd=directory, env=env, capture_output=True, timeout=30)
    (directory / 'stdout').write_bytes(completed.stdout)
    (directory / 'stderr').write_bytes(completed.stderr)
    (directory / 'invocation.json').write_text(json.dumps({'argv': argv, 'environment': env,
                                                          'observer_argv': observer_argv}, indent=2) + '\n')
    assert completed.returncode == expected, (case, completed.returncode, completed.stderr)
    after_bytes = subprocess.check_output(observer_argv, cwd=directory, env=env, timeout=30)
    after = json.loads(after_bytes)
    assert after == before == approved
    assert hashlib.sha256(UV.read_bytes()).hexdigest() == prior_summary['uv_sha256']
    assert hashlib.sha256(PYTHON.read_bytes()).hexdigest() == prior_summary['interpreter_sha256']
    (directory / 'observation-after.json').write_bytes(after_bytes)
    trace = (directory / 'trace').read_text()
    assert 'AF_INET' not in trace and 'AF_INET6' not in trace
    selected = []
    packages = []
    if expected == 0:
        lock = tomllib.loads((directory / 'pylock.toml').read_text())
        assert lock['lock-version'] == '1.0' and lock['created-by'] == 'uv'
        packages = [package['name'] for package in lock['packages']]
        assert packages == expected_names
        for package in lock['packages']:
            assert package['version'] == '1.0' and len(package['wheels']) == 1
            wheel = package['wheels'][0]
            url = urlparse(wheel['url'])
            assert url.scheme == 'file' and not url.netloc
            path = Path(unquote(url.path))
            assert path.parent == wheels
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            assert wheel['hashes']['sha256'] == digest
            assert any(row['filename'] == path.name and row['sha256'] == digest for row in wheel_bindings)
            selected.append({'name': package['name'], 'filename': path.name, 'sha256': digest})
    else:
        assert not (directory / 'pylock.toml').exists()
        assert b'No solution found' in completed.stderr
    result = {'case': case, 'prior_case_identifier': prior_case,
              'prior_mode': 'Interpreter platform with --python-version3.12.14; reconstructed cross-target tags.',
              'mode': 'Exact interpreter, neither target override; direct interpreter.tags() and native markers.',
              'exit_code': completed.returncode, 'packages': packages, 'selected_wheels': selected,
              'ip_socket_calls_observed': 0, 'observation_before_after_matches_original': True,
              'argv_sha256': hashlib.sha256(json.dumps(argv, separators=(',', ':')).encode()).hexdigest(),
              'roots_sha256': hashlib.sha256(roots).hexdigest(),
              'prior_invocation_sha256': hashlib.sha256(original_bytes).hexdigest()}
    results.append(result)
    print(json.dumps(result), flush=True)

summary = {'prior_evidence_commit': PRIOR_COMMIT, 'prior_evidence_tree': PRIOR_TREE,
           'approved_reconciliation_commit': '9310c1d6a0dbbef6e602a2a9c464643e21f2d341',
           'approved_reconciliation_tree': '93c3ac702f5f17694f0062b94df2216430a7bfda',
           'uv_version': subprocess.check_output([str(UV), '--version'], text=True).strip(),
           'uv_sha256': prior_summary['uv_sha256'], 'interpreter': str(PYTHON),
           'interpreter_sha256': prior_summary['interpreter_sha256'], 'python_version': '3.12.14',
           'native_libc': approved['target']['libc'], 'wheel_bindings': wheel_bindings, 'cases': results,
           'scope': 'Four public compile diagnostics only. Original evidence retained unchanged; existing no-overrides closure/missing-interpreter controls not repeated. No installation, runtime implementation, catalog capability, adoption, real-provider/native-binary acceptance or enforced network denial.'}
(ROOT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
