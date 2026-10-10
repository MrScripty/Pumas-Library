"""Private-session lifecycle fixture, never installed or native ASR qualification."""

import argparse
import asyncio
import os
from pathlib import Path
import sys
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from owned_worker import bootstrap

parser = argparse.ArgumentParser()
for root in ("code", "packages", "model"):
    parser.add_argument(f"--{root}-root-fd", required=True, type=int)
parser.add_argument("--phase-fd", required=True, type=int)
parser.add_argument("--hold", choices=("hello", "load", "none"), required=True)
args = parser.parse_args()


def controlled(model_fd):
    from installed_cohere_source import _InstalledCohereSourceOwner
    from tests.owned_channel_fixture import create_channel

    channel = create_channel(
        SimpleNamespace(
            fixture_source_fd=model_fd,
            unqualified=False,
            hold_use_until_cancel=False,
            hold_load_until_cancel=args.hold == "load",
        )
    )
    source = _InstalledCohereSourceOwner._capture(
        channel.actor.manager, model_fd, "library/speech", "controlled-selected"
    )
    channel._session_source = source
    original_prepare = channel._gate.prepare_from_parent

    def prepare(payload, manager):
        source.validate()
        if payload["source_id"] != source.source_id:
            raise ValueError("controlled original source correlation changed")
        return original_prepare({**payload, "source_id": "fixture-selected"}, manager)

    channel._gate.prepare_from_parent = prepare
    # The existing fixture invokes this only after consuming selected bytes and
    # placing its controlled object in load custody, immediately before its hold.
    channel._gate._load_entered = lambda: os.write(args.phase_fd, b"L")
    original_handle = channel._handle

    async def handle(writer, request, record):
        if request["operation"] == "hello":
            os.write(args.phase_fd, b"H")
            if args.hold == "hello":
                await asyncio.Future()
        await original_handle(writer, request, record)

    channel._handle = handle
    return channel


asyncio.run(bootstrap(args, channel_factory=controlled))
