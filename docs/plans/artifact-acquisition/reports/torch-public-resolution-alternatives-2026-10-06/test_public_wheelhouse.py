"""Research controls using public APIs and inert local wheels only.

No installed packages, source archives, network requests or pip private imports.
The preflight here demonstrates the admission principle, not production validation.
"""
import base64
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from packaging.metadata import Metadata
from packaging.utils import canonicalize_name, parse_wheel_filename


def wheel(directory, name, requirements=()):
    path = directory / f'{name}-1.0-py3-none-any.whl'
    info = f'{name}-1.0.dist-info'
    metadata = (f'Metadata-Version: 2.1\nName: {name}\nVersion: 1.0\n' +
                ''.join(f'Requires-Dist: {r}\n' for r in requirements) + '\n').encode()
    contents = {
        f'{info}/METADATA': metadata,
        f'{info}/WHEEL': b'Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n',
    }
    record = io.StringIO()
    writer = csv.writer(record, lineterminator='\n')
    for member, body in contents.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(body).digest()).rstrip(b'=').decode()
        writer.writerow([member, 'sha256=' + digest, len(body)])
    writer.writerow([f'{info}/RECORD', '', ''])
    contents[f'{info}/RECORD'] = record.getvalue().encode()
    with zipfile.ZipFile(path, 'w') as archive:
        for member, body in contents.items():
            archive.writestr(member, body)
    return path


def preflight(paths):
    # Deliberately reject every URL, including inactive extras/marker branches.
    # Production must also bind acquisition hashes, tags, custody and resource bounds.
    for path in paths:
        name, version, _, _ = parse_wheel_filename(path.name)
        with zipfile.ZipFile(path) as archive:
            members = [m for m in archive.infolist() if m.filename.endswith('.dist-info/METADATA')]
            if len(members) != 1 or members[0].file_size > 4 * 1024 * 1024:
                raise ValueError('Invalid wheel metadata')
            metadata = Metadata.from_email(archive.read(members[0]), validate=True)
        if canonicalize_name(metadata.name) != name or metadata.version != version:
            raise ValueError('Wheel metadata identity differs')
        for requirement in metadata.requires_dist or []:
            if requirement.url is not None:
                raise ValueError('Dependency direct references refused before pip')


def resolve(directory, output, exact_files=None):
    environment = {k: v for k, v in os.environ.items() if not k.upper().startswith('PIP_')}
    environment['PIP_CONFIG_FILE'] = os.devnull
    command = [sys.executable, '-I', '-m', 'pip', '--isolated', 'install',
               '--dry-run', '--ignore-installed', '--only-binary=:all:',
               '--no-index', '--no-cache-dir', '--disable-pip-version-check',
               '--find-links', str(directory), '--report', str(output),
               *(['--no-deps', *[path.as_uri() for path in exact_files]]
                 if exact_files is not None else ['boundary-root==1.0'])]
    return subprocess.run(command, capture_output=True, text=True, env=environment, timeout=30)


def preflight_then_resolve(directory, output):
    preflight(sorted(directory.iterdir()))
    return resolve(directory, output)


class PublicWheelhouseTests(unittest.TestCase):
    def test_no_index_still_follows_harmless_direct_reference_outside_wheelhouse(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheelhouse = root / 'wheelhouse'
            wheelhouse.mkdir()
            external = root / 'outside-wheelhouse'
            external.mkdir()
            child = wheel(external, 'boundary_child')
            wheel(wheelhouse, 'boundary_root', ['boundary-child @ ' + child.as_uri()])
            report = root / 'report.json'
            result = resolve(wheelhouse, report)
            self.assertEqual(result.returncode, 0, result.stderr)
            items = json.loads(report.read_text())['install']
            self.assertEqual(len(items), 2)
            self.assertIn(child.as_uri(), [i['download_info']['url'] for i in items])
            self.assertFalse((root / 'target').exists())
            print('PUBLIC CONTROL: --no-index followed an inert direct local wheel outside --find-links')

    def test_metadata_direct_sources_refuse_before_public_pip_invocation(self):
        references = [
            'hidden @ https://example.invalid/hidden-1.0.tar.gz',
            'hidden @ git+https://example.invalid/repository.git',
            'hidden @ file:///never-read/hidden-1.0-py3-none-any.whl',
            'hidden @ https://example.invalid/hidden-1.0-py3-none-any.whl ; python_version < "0"',
        ]
        for reference in references:
            with self.subTest(reference=reference), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                wheel(root, 'boundary_root', [reference])
                with patch.object(subprocess, 'run', side_effect=AssertionError('pip invoked')) as child:
                    with self.assertRaisesRegex(ValueError, 'before pip'):
                        preflight_then_resolve(root, root.parent / 'never-created-report.json')
                    child.assert_not_called()

    def test_prevalidated_named_dependency_closure_resolves_with_public_pip(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheelhouse = root / 'wheelhouse'
            wheelhouse.mkdir()
            wheel(wheelhouse, 'boundary_root', ['boundary-child>=1,<2'])
            wheel(wheelhouse, 'boundary_child')
            report = root / 'report.json'
            result = preflight_then_resolve(wheelhouse, report)
            self.assertEqual(result.returncode, 0, result.stderr)
            document = json.loads(report.read_text())
            self.assertEqual(document['version'], '1')
            self.assertEqual({canonicalize_name(i['metadata']['name']) for i in document['install']},
                             {'boundary-root', 'boundary-child'})
            for item in document['install']:
                self.assertTrue(item['download_info']['url'].startswith(wheelhouse.as_uri() + '/'))
                self.assertEqual(len(item['download_info']['archive_info']['hashes']['sha256']), 64)
            print('PUBLIC CONTROL: prevalidated named closure resolves entirely from local wheelhouse')

    def test_finite_validated_recipe_can_report_exact_inputs_without_dependency_resolution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheelhouse = root / 'wheelhouse'
            wheelhouse.mkdir()
            wheels = [wheel(wheelhouse, 'boundary_root', ['boundary-child>=1,<2']),
                      wheel(wheelhouse, 'boundary_child')]
            preflight(wheels)
            report = root / 'report.json'
            result = resolve(wheelhouse, report, exact_files=wheels)
            self.assertEqual(result.returncode, 0, result.stderr)
            items = json.loads(report.read_text())['install']
            self.assertEqual({item['download_info']['url'] for item in items},
                             {path.as_uri() for path in wheels})
            print('PUBLIC CONTROL: fixed exact local inputs produce a report with --no-deps')

    def test_missing_named_closure_fails_without_report_or_source_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheelhouse = root / 'wheelhouse'
            wheelhouse.mkdir()
            wheel(wheelhouse, 'boundary_root', ['boundary-child>=1,<2'])
            report = root / 'report.json'
            result = preflight_then_resolve(wheelhouse, report)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(report.exists())
            self.assertIn('No matching distribution found', result.stderr)
            print('PUBLIC CONTROL: missing local closure refuses; no external source is offered')


if __name__ == '__main__':
    unittest.main(verbosity=2)
