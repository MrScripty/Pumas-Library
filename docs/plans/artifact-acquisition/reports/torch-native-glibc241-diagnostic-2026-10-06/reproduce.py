"""Read-only repository diagnostic using inert, newly generated local wheels.

This never constructs a CompleteCatalog or installs a package. Native resolution
is tested only through the unchanged, qualified public uv executable.
"""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tomllib
import zipfile

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[5]
ROOT = Path(sys.argv[1]).resolve()
ROOT.mkdir()
UV = Path(sys.argv[2]).resolve()
sys.path.insert(0, str(REPO / 'torch-server/tests'))
from test_install_verified_wheels import make_wheel

spec = importlib.util.spec_from_file_location('diagnostic_selection', REPO / 'torch-server/offline_wheel_selection.py')
selection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selection)
observation = selection.catalog.target_owner.capture_observation()
(ROOT / 'native-observation.json').write_text(json.dumps(observation, indent=2) + '\n')
assert observation['target']['libc'] == {'family': 'glibc', 'version': '2.41'}
assert hashlib.sha256(UV.read_bytes()).hexdigest() == 'abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f'
try:
    selection.resolver_platform(selection.catalog.target_owner.WheelTarget(observation['target']))
except selection.catalog.Refused as error:
    refusal = str(error)
else:
    raise AssertionError('Frozen adapter must refuse native 2.41')

wheels = ROOT / 'wheels'
wheels.mkdir()

def fixture(name, tag='py3-none-any', **kwargs):
    row = make_wheel(wheels, name, **kwargs)
    path = wheels / row['url'].rsplit('/', 1)[-1]
    if tag == 'py3-none-any':
        return path
    with zipfile.ZipFile(path) as archive:
        members = {info.filename: archive.read(info) for info in archive.infolist()}
    wheel = next(name for name in members if name.endswith('/WHEEL'))
    record = next(name for name in members if name.endswith('/RECORD'))
    members[wheel] = members[wheel].replace(b'Tag: py3-none-any', ('Tag: ' + tag).encode())
    rows = []
    for name, data in members.items():
        if name != record:
            digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip('=')
            rows.append(f'{name},sha256={digest},{len(data)}\n')
    members[record] = (''.join(rows) + f'{record},,\n').encode()
    path.unlink()
    path = path.with_name(path.name.replace('py3-none-any', tag))
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            archive.writestr(name, data)
    return path

python_version = observation['target']['python']
fixture('native_floor41', 'cp312-cp312-manylinux_2_41_x86_64', extras=['tools'],
        requires=['native_child; extra == "tools"',
                  f'native_patch; python_full_version == "{python_version}"',
                  'native_linux; sys_platform == "linux"',
                  'absent_windows; sys_platform == "win32"'])
fixture('native_floor40', 'cp312-cp312-manylinux_2_40_x86_64')
fixture('native_floor42', 'cp312-cp312-manylinux_2_42_x86_64')
fixture('native_linux', 'cp312-cp312-linux_x86_64')
fixture('native_wrongabi', 'cp313-cp313-manylinux_2_41_x86_64')
fixture('native_child')
fixture('native_patch')
cases = [
    ('native-floor41-closure', 'native_floor41[tools]==1.0', None, True, 0),
    ('native-floor41-no-overrides', 'native_floor41[tools]==1.0', None, False, 0),
    ('explicit-floor41', 'native_floor41[tools]==1.0', 'x86_64-manylinux_2_41', True, 2),
    ('wheel-style-floor41', 'native_floor41[tools]==1.0', 'manylinux_2_41_x86_64', True, 2),
    ('explicit-floor40-vs41', 'native_floor41[tools]==1.0', 'x86_64-manylinux_2_40', True, 1),
    ('generic-linux-vs41', 'native_floor41[tools]==1.0', 'linux', True, 1),
    ('generic-gnu-vs41', 'native_floor41[tools]==1.0', 'x86_64-unknown-linux-gnu', True, 1),
    ('native-floor42-refusal', 'native_floor42==1.0', None, True, 1),
    ('native-wrongabi-refusal', 'native_wrongabi==1.0', None, True, 1),
    ('native-floor40', 'native_floor40==1.0', None, True, 0),
    ('explicit-floor40', 'native_floor40==1.0', 'x86_64-manylinux_2_40', True, 0),
    ('native-linux-tag', 'native_linux==1.0', None, True, 0),
    ('native-missing-interpreter', 'native_floor41[tools]==1.0', None, False, 2),
]
results = []
for label, requirement, platform, full_patch, expected in cases:
    directory = ROOT / label
    directory.mkdir()
    for leaf in ('config', 'cache', 'tmp'):
        (directory / leaf).mkdir()
    (directory / 'roots.in').write_text(requirement + '\n')
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8',
           'XDG_CONFIG_HOME': str(directory / 'config'),
           'UV_CACHE_DIR': str(directory / 'cache'), 'TMPDIR': str(directory / 'tmp')}
    command = [str(UV), '--no-config', '--no-cache', '--offline', '--no-python-downloads',
               '--no-managed-python', '--color', 'never', 'pip', 'compile', 'roots.in',
               '--no-index', '--no-build', '--no-sources', '--keyring-provider', 'disabled',
               '--python', observation['interpreter'], '--find-links', str(wheels),
               '--format', 'pylock.toml', '--generate-hashes', '--no-header', '--no-annotate',
               '-o', 'pylock.toml']
    if label == 'native-missing-interpreter':
        command[command.index('--python') + 1] = str(directory / 'missing-python')
    if full_patch:
        command += ['--python-version', python_version]
    if platform:
        command += ['--python-platform', platform]
    traced = ['strace', '-f', '-e', 'trace=network,process', '-o', str(directory / 'trace'), *command]
    completed = subprocess.run(traced, cwd=directory, env=env, capture_output=True, timeout=30)
    (directory / 'stdout').write_bytes(completed.stdout)
    (directory / 'stderr').write_bytes(completed.stderr)
    (directory / 'invocation.json').write_text(json.dumps({'argv': command, 'environment': env}, indent=2) + '\n')
    assert completed.returncode == expected, (label, completed.returncode, completed.stderr)
    packages = []
    if expected == 0:
        lock = tomllib.loads((directory / 'pylock.toml').read_text())
        packages = [row['name'] for row in lock['packages']]
        if 'floor41' in label:
            assert set(packages) == {'native-floor41', 'native-child', 'native-patch', 'native-linux'}
        for package in lock['packages']:
            for wheel in package['wheels']:
                path = Path(wheel['url'].removeprefix('file://'))
                assert path.parent == wheels and hashlib.sha256(path.read_bytes()).hexdigest() == wheel['hashes']['sha256']
    trace = (directory / 'trace').read_text()
    assert 'AF_INET' not in trace and 'AF_INET6' not in trace, label
    result = {'case': label, 'exit_code': completed.returncode, 'packages': packages,
              'ip_socket_calls_observed': 0}
    results.append(result)
    print(json.dumps(result), flush=True)

summary = {'uv_version': subprocess.check_output([str(UV), '--version'], text=True).strip(),
           'uv_sha256': hashlib.sha256(UV.read_bytes()).hexdigest(),
           'interpreter': observation['interpreter'], 'interpreter_sha256': observation['interpreter_sha256'],
           'python_version': python_version, 'native_libc': observation['target']['libc'],
           'frozen_adapter_refusal': refusal, 'cases': results,
           'scope': 'Public CLI diagnostic only; no installation, catalog capability, adoption, provider qualification or enforced network denial.'}
(ROOT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
