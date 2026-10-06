"""Unactivated design candidate: pip 26.2.1 admission, not production tooling.

Never invoke pip's CLI here. Tests intercept candidate construction and adapters.
Private seams require an explicit contract exception before repository activation.
"""
from contextlib import contextmanager
import inspect
import re
from urllib.parse import unquote, urlparse
from unittest.mock import patch
import pip
from pip._internal.exceptions import InstallationError
from pip._internal.network.session import PipSession
from pip._internal.resolution.resolvelib.factory import Factory
from pip._vendor.packaging.utils import canonicalize_name, parse_wheel_filename


class AdmissionRefused(InstallationError):
    def __init__(self):
        super().__init__("Resolver source admission refused; no candidate fallback is permitted")


def check_support():
    # Deliberately exact: other versions need independent qualification.
    if pip.__version__ != "26.2.1":
        raise AdmissionRefused()
    try:
        candidate_shape = tuple(inspect.signature(Factory._make_base_candidate_from_link).parameters)
        send_shape = tuple(inspect.signature(PipSession.send).parameters)
    except (AttributeError, TypeError, ValueError):
        raise AdmissionRefused() from None
    if candidate_shape != ("self", "link", "template", "name", "version") or send_shape != (
        "self", "request", "kwargs"
    ):
        raise AdmissionRefused()


def candidate_allowed(link, template, name, version, trust):
    if template.editable or not link.is_wheel or link.is_vcs or link.is_file:
        return False
    # Final local installation already refuses Requires-Dist direct references.
    # Root explicit references stay admitted; metadata references fail earlier.
    if template.req is not None and template.req.url and template.comes_from is not None:
        return False
    try:
        parsed = urlparse(link.url)
        if parsed.fragment and not re.fullmatch(r"sha256=[0-9a-fA-F]{64}", parsed.fragment):
            return False
        wheel_name, wheel_version, _, _ = parse_wheel_filename(unquote(link.filename))
        if name is not None and wheel_name != canonicalize_name(name):
            return False
        if version is not None and wheel_version != version:
            return False
        return trust(link.url_without_fragment, wheel_name, str(wheel_version))
    except (ValueError, TypeError):
        return False


def request_allowed(url, trust):
    try:
        parsed = urlparse(url)
        if (parsed.scheme != "https" or parsed.port not in (None, 443)
            or parsed.username is not None or parsed.password is not None
            or parsed.query or parsed.fragment
            or any(character.isspace() or ord(character) < 32 for character in url)):
            return False
        if parsed.hostname == "pypi.org" and parsed.path.startswith("/simple/"):
            return True
        if parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}:
            return parsed.path.startswith("/whl/")
        if parsed.hostname == "files.pythonhosted.org":
            return parsed.path.startswith("/packages/")
        # A policy-approved explicit wheel may serve PEP658 .metadata as well.
        if parsed.path.endswith(".metadata"):
            url = url[:-len(".metadata")]
        filename = unquote(urlparse(url).path.rsplit("/", 1)[-1])
        name, version, _, _ = parse_wheel_filename(filename)
        return trust(url, name, str(version))
    except (ValueError, TypeError):
        return False


@contextmanager
def admitted_resolution(trust):
    check_support()
    original_candidate = Factory._make_base_candidate_from_link
    original_send = PipSession.send
    state = {"refused": False}

    def refuse():
        state["refused"] = True
        raise AdmissionRefused()

    def candidate(factory, link, template, name, version):
        if not candidate_allowed(link, template, name, version, trust):
            refuse()
        return original_candidate(factory, link, template, name, version)

    def send(session, request, **kwargs):
        if not request_allowed(request.url, trust):
            refuse()
        return original_send(session, request, **kwargs)

    with patch.object(Factory, "_make_base_candidate_from_link", candidate), patch.object(
        PipSession, "send", send
    ):
        yield state
