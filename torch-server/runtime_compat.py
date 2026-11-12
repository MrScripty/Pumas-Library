"""Check that an installed Torch runtime can run this sidecar.

Pumas runs the sidecar shipped with the running Pumas against the interpreter
and packages of an installed runtime. A runtime installed by an older Pumas may
lack a package this sidecar imports when it starts, or carry a release older
than the code here uses. Pumas runs this check before every launch and refuses
the launch with the reasons it prints, instead of letting the sidecar fail on
an import.

A floor is the oldest release this sidecar's code works with. Newer releases
are always accepted: floors are never pins.

This module uses only the standard library and the syntax of the oldest
interpreter it must still diagnose, so it can report an interpreter that is
itself too old. It writes nothing.
"""

import importlib.metadata
import importlib.util
import json
import re
import sys

# The sidecar's own minimum interpreter.
MINIMUM_PYTHON = (3, 10)

# Every third-party module the sidecar imports when it starts (serve.py and the
# route modules it loads), as (import name, distribution, floor). A floor of
# None accepts any release. The other floors match requirements.txt, whose
# FastAPI floor is past a known advisory. Adapter packages are imported when a
# model of that adapter loads; a missing one fails that load, not startup.
# tests/test_runtime_compat.py keeps this list equal to the startup imports.
STARTUP_REQUIREMENTS = (
    ("torch", "torch", None),
    ("fastapi", "fastapi", "0.109.1"),
    # The request models use the Pydantic 2 validator API.
    ("pydantic", "pydantic", "2.0"),
    ("uvicorn", "uvicorn", "0.24.0"),
    ("psutil", "psutil", "5.9.0"),
)

_RELEASE = re.compile(r"\d+(?:\.\d+)*")


def release(version):
    """The numeric release of a version string, or None when it has none."""
    match = _RELEASE.match(version.strip())
    if match is None:
        return None
    return tuple(int(part) for part in match.group().split("."))


def _padded(left, right):
    width = max(len(left), len(right))
    return left + (0,) * (width - len(left)), right + (0,) * (width - len(right))


def requirement_problem(import_name, distribution, floor):
    """Why the runtime cannot satisfy one requirement, or None when it can."""
    try:
        installed = importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return "{} is not installed".format(distribution)
    if importlib.util.find_spec(import_name) is None:
        return "{} {} is installed but its module {} cannot be found".format(
            distribution, installed, import_name
        )
    if floor is None:
        return None
    observed = release(installed)
    if observed is None:
        return "{} reports version {!r}, which is not a release number".format(
            distribution, installed
        )
    observed, minimum = _padded(observed, release(floor))
    if observed < minimum:
        return "{} {} is older than {}, the oldest release this sidecar supports".format(
            distribution, installed, floor
        )
    return None


def check():
    """Every reason the running interpreter cannot run this sidecar."""
    if sys.version_info[:2] < MINIMUM_PYTHON:
        return [
            "Python {}.{} is older than {}.{}, the oldest this sidecar supports".format(
                sys.version_info[0], sys.version_info[1], *MINIMUM_PYTHON
            )
        ]
    problems = []
    for requirement in STARTUP_REQUIREMENTS:
        problem = requirement_problem(*requirement)
        if problem is not None:
            problems.append(problem)
    return problems


def main():
    problems = check()
    print(json.dumps({"compatible": not problems, "problems": problems}), flush=True)


if __name__ == "__main__":
    main()
