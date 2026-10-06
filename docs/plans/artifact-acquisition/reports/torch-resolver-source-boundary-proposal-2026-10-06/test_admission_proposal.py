"""Inert boundary tests: never fetch, prepare metadata or install build dependencies."""
import io
import unittest
from email.parser import Parser
from types import SimpleNamespace
from unittest.mock import patch

import admission_proposal as proposal
from pip._internal.commands import create_command
from pip._internal.models.link import Link
from pip._internal.models.target_python import TargetPython
from pip._internal.network.session import PipSession
from pip._internal.req.constructors import install_req_from_line, install_req_from_req_string
from pip._internal.resolution.resolvelib.factory import Factory
from pip._vendor.packaging.version import Version
from pip._vendor.requests import Response
from pip._vendor.requests.adapters import BaseAdapter
from pip._vendor.requests.structures import CaseInsensitiveDict

WHEEL = 'https://download.pytorch.org/whl/cpu/torch-2.14.0%2Bcpu-py3-none-any.whl'
DEPENDENCY = 'https://files.pythonhosted.org/packages/helper-1.0-py3-none-any.whl'


def trust(url, name, version):
    return (url, name, version) in {
        (WHEEL, 'torch', '2.14.0+cpu'), (DEPENDENCY, 'helper', '1.0')
    }


class ReachedConstruction(Exception):
    pass


class InertAdapter(BaseAdapter):
    def __init__(self, redirect=None):
        self.redirect = redirect
        self.calls = []

    def send(self, request, **kwargs):
        self.calls.append(request.url)
        response = Response()
        response.request = request
        response.url = request.url
        response.status_code = 302 if self.redirect and len(self.calls) == 1 else 200
        response.headers = CaseInsensitiveDict({'Location': self.redirect} if response.status_code == 302 else {})
        response.raw = io.BytesIO(b'')
        response._content = b''
        return response

    def close(self):
        pass


class AdmissionTests(unittest.TestCase):
    def factory(self):
        opts, _ = create_command('install', isolated=True).parse_args([
            '--dry-run', '--ignore-installed', '--report', '/owned/report.json',
            '--only-binary=:all:', 'torch==2.14.0+cpu'
        ])
        factory = object.__new__(Factory)
        factory._finder = SimpleNamespace(format_control=opts.format_control, target_python=TargetPython())
        factory._build_failures = {}
        factory._link_candidate_cache = {}
        factory._editable_candidate_cache = {}
        return factory

    def constructor(self, *args, **kwargs):
        raise ReachedConstruction()

    def test_original_pip_reaches_source_constructor_despite_public_flags(self):
        parent = install_req_from_line('torch @ ' + WHEEL)
        child = install_req_from_req_string(
            'hidden-build @ https://example.invalid/hidden-build-1.0.tar.gz',
            comes_from=parent, isolated=True
        )
        with patch('pip._internal.resolution.resolvelib.factory.LinkCandidate', self.constructor):
            with self.assertRaises(ReachedConstruction):
                list(self.factory()._make_requirements_from_install_req(child, []))

    def test_real_requirement_boundary_refuses_inert_metadata_direct_sources(self):
        metadata = Parser().parsestr('''Metadata-Version: 2.1
Name: torch
Version: 2.14.0+cpu
Requires-Dist: hidden-build @ https://example.invalid/hidden-build-1.0.tar.gz
Requires-Dist: hidden-vcs @ git+https://example.invalid/repository.git
Requires-Dist: helper @ https://files.pythonhosted.org/packages/helper-1.0-py3-none-any.whl

''')
        parent = install_req_from_line('torch @ ' + WHEEL)
        for dependency in metadata.get_all('Requires-Dist'):
            with self.subTest(dependency=dependency), patch(
                'pip._internal.resolution.resolvelib.factory.LinkCandidate', self.constructor
            ), proposal.admitted_resolution(trust) as state:
                child = install_req_from_req_string(dependency, comes_from=parent, isolated=True)
                with self.assertRaises(proposal.AdmissionRefused):
                    list(self.factory()._make_requirements_from_install_req(child, []))
                self.assertTrue(state['refused'])
                # The constructor sentinel would fail the assertion if reached.

    def test_candidate_refusals_precede_constructor(self):
        fixtures = [
            ('https://example.invalid/helper-1.0-py3-none-any.whl', False, None, None),
            ('https://example.invalid/helper-1.0.zip', False, None, None),
            ('file:///not-read/helper-1.0-py3-none-any.whl', False, None, None),
            ('git+https://example.invalid/repository.git', False, None, None),
            (DEPENDENCY, True, None, None),
            (DEPENDENCY, False, 'other', None),
            (DEPENDENCY, False, 'helper', Version('2.0')),
            (DEPENDENCY + '#subdirectory=fixture', False, None, None),
            (DEPENDENCY + '#sha256=bad', False, None, None),
        ]
        for url, editable, name, version in fixtures:
            with self.subTest(url=url, editable=editable, name=name, version=version):
                template = install_req_from_line('helper @ ' + url)
                template.editable = editable
                with patch('pip._internal.resolution.resolvelib.factory.LinkCandidate', self.constructor), patch(
                    'pip._internal.resolution.resolvelib.factory.EditableCandidate', self.constructor
                ), proposal.admitted_resolution(trust):
                    with self.assertRaises(proposal.AdmissionRefused):
                        self.factory()._make_base_candidate_from_link(
                            Link(url), template, name, version
                        )

    def test_approved_wheels_and_hash_fragment_reach_constructor(self):
        for url, name, version in [
            (WHEEL, 'torch', Version('2.14.0+cpu')),
            (DEPENDENCY, 'helper', Version('1.0')),
            (DEPENDENCY + '#sha256=' + 'a' * 64, 'helper', Version('1.0')),
        ]:
            with self.subTest(url=url), patch(
                'pip._internal.resolution.resolvelib.factory.LinkCandidate', self.constructor
            ), proposal.admitted_resolution(trust) as state:
                with self.assertRaises(ReachedConstruction):
                    self.factory()._make_base_candidate_from_link(
                        Link(url), install_req_from_line(name + ' @ ' + url), name, version
                    )
                self.assertFalse(state['refused'])

    def test_redirect_refusals_precede_adapter_dispatch(self):
        rejected = [
            'https://example.invalid/hidden.tar.gz',
            'http://download.pytorch.org/whl/cpu/fixture',
            'file:///not-read/fixture',
            'https://user:secret@download.pytorch.org/whl/cpu/fixture',
            'https://download.pytorch.org/whl/cpu/fixture?token=secret',
        ]
        for redirect in rejected:
            with self.subTest(redirect=redirect), PipSession() as session:
                adapter = InertAdapter(redirect)
                session.mount('https://', adapter)
                session.mount('http://', adapter)
                session.mount('file://', adapter)
                with proposal.admitted_resolution(trust) as state:
                    with self.assertRaises(proposal.AdmissionRefused):
                        session.get(WHEEL)
                    self.assertTrue(state['refused'])
                self.assertEqual(adapter.calls, [WHEEL])

    def test_approved_redirect_dispatches_without_real_network(self):
        with PipSession() as session:
            adapter = InertAdapter(DEPENDENCY)
            session.mount('https://', adapter)
            with proposal.admitted_resolution(trust):
                self.assertEqual(session.get(WHEEL).status_code, 200)
            self.assertEqual(adapter.calls, [WHEEL, DEPENDENCY])

    def test_cached_https_adapter_is_refused_before_cache_or_dispatch(self):
        import tempfile
        with tempfile.TemporaryDirectory() as cache, PipSession(cache=cache) as session:
            adapter = session.get_adapter('https://example.invalid/fixture')
            self.assertEqual(type(adapter).__name__, 'CacheControlAdapter')
            with patch.object(adapter, 'send', side_effect=AssertionError('cache/send reached')) as send:
                with proposal.admitted_resolution(trust):
                    with self.assertRaises(proposal.AdmissionRefused):
                        session.get('https://example.invalid/fixture')
                send.assert_not_called()

    def test_unsupported_version_and_missing_seam_refuse_before_activation(self):
        for version in ['24.0', '26.2.2']:
            with patch.object(proposal.pip, '__version__', version):
                with self.assertRaises(proposal.AdmissionRefused):
                    with proposal.admitted_resolution(trust):
                        self.fail('unsupported worker activated')
        with patch.object(Factory, '_make_base_candidate_from_link', None):
            with self.assertRaises(proposal.AdmissionRefused):
                with proposal.admitted_resolution(trust):
                    self.fail('missing worker activated')
        with patch.object(Factory, '_make_base_candidate_from_link', lambda factory, link: None):
            with self.assertRaises(proposal.AdmissionRefused):
                with proposal.admitted_resolution(trust):
                    self.fail('incompatible worker activated')

    def test_refusal_message_excludes_rejected_url_and_credentials(self):
        self.assertEqual(str(proposal.AdmissionRefused()),
                         'Resolver source admission refused; no candidate fallback is permitted')


if __name__ == '__main__':
    unittest.main(verbosity=2)
