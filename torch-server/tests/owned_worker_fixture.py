"""Controlled native factory through the shipping bootstrap, never installed."""

import argparse
import asyncio
import importlib
import os
from pathlib import Path
import sys
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from owned_worker import StartupRefusal, bootstrap

parser = argparse.ArgumentParser()
for root in ("code", "packages", "model"):
    parser.add_argument(f"--{root}-root-fd", required=True, type=int)
parser.add_argument("--hold-use-until-cancel", action="store_true")
parser.add_argument("--hold-load-until-cancel", action="store_true")
parser.add_argument("--assert-retained-after-close", action="store_true")
parser.add_argument("--assert-excluded-venv", action="store_true")
args = parser.parse_args()


def controlled(model_fd):
    if args.assert_excluded_venv:
        assert importlib.import_module("selected_package").VALUE == "selected"
        try:
            importlib.import_module("venv.escape")
        except StartupRefusal as error:
            assert error.code == "ambient_import_refused"
        else:
            raise AssertionError("unselected code-role venv became importable")
    from tests.owned_channel_fixture import create_channel

    return create_channel(
        SimpleNamespace(
            fixture_source_fd=model_fd,
            unqualified=False,
            hold_use_until_cancel=args.hold_use_until_cancel,
            hold_load_until_cancel=args.hold_load_until_cancel,
        )
    )


async def run():
    await bootstrap(args, channel_factory=controlled)
    if args.assert_retained_after_close:
        import owned_worker

        for descriptor in owned_worker._BOOTSTRAP_CUSTODY[0]:
            os.fstat(descriptor)
        assert importlib.import_module("retained_probe").VALUE == "retained"
        print("controlled roots retained after channel close", file=sys.stderr, flush=True)


try:
    asyncio.run(run())
except StartupRefusal as error:
    print(error.code, file=sys.stderr)
    raise SystemExit(2) from None
