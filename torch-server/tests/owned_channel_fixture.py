"""Real-process controlled backend; explicitly not installed Torch qualification.

Only the test supervisor launches this module. It reads selected bytes through
an inherited directory descriptor and validates held fixed code files. No wire
path, digest, manifest or boolean can install its source or create authority.
System interpreter/stdlib/native ASR are not qualified by this fixture.
"""

import argparse
import asyncio
import hashlib
import os
from pathlib import Path
import stat
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

# Explicit controlled CPU implementation. Never installed by shipping factory.
torch = types.ModuleType("torch")


class Device:
    def __init__(self, value):
        if value != "cpu":
            raise ValueError("Controlled CPU only")
        self.type = value

    def __str__(self):
        return self.type


torch.device = Device
torch.cuda = types.SimpleNamespace(synchronize=lambda device: None)
sys.modules["torch"] = torch
psutil = types.ModuleType("psutil")
sys.modules["psutil"] = psutil

from model_manager import LoadedModel, ModelManager  # noqa: E402
from native_speech_result import NativeSpeechResult  # noqa: E402
from owned_audio import OwnedAudioActor, OwnedAudioError, OwnedLoadPlan  # noqa: E402
from private_owned_channel import PrivateOwnedChannel  # noqa: E402
from loaders.cohere_asr_loader import _finish_reason  # noqa: E402

MEMBERS = (
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
)
CODE = (
    "owned_worker.py",
    "tests/owned_worker_fixture.py",
    "owned_audio.py",
    "model_manager.py",
    "device_manager.py",
    "private_owned_channel.py",
    "owned_model_operations.py",
    "speech_binding.py",
    "speech_operations.py",
    "native_speech_result.py",
    "audio_input.py",
    "audio_contract.py",
    "loaders/cohere_asr_loader.py",
    "loaders/__init__.py",
    "tests/owned_channel_fixture.py",
)


def identity(fd):
    value = os.fstat(fd)
    return value.st_dev, value.st_ino, value.st_size


def read(fd):
    os.lseek(fd, 0, os.SEEK_SET)
    parts = []
    while block := os.read(fd, 65536):
        parts.append(block)
        if sum(map(len, parts)) > 1024 * 1024:
            raise OwnedAudioError("artifact_custody_unavailable")
    return b"".join(parts)


class Snapshot:
    def __init__(self, source_fd):
        self.root = source_fd
        if set(os.listdir(source_fd)) != set(MEMBERS):
            raise OwnedAudioError("artifact_custody_unavailable")
        self.assets = {}
        self.code = {}
        for name in MEMBERS:
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=source_fd)
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                os.close(fd)
                raise OwnedAudioError("artifact_custody_unavailable")
            self.assets[name] = (fd, identity(fd), hashlib.sha256(read(fd)).hexdigest())
        for name in CODE:
            fd = os.open(ROOT / name, os.O_RDONLY | os.O_NOFOLLOW)
            self.code[name] = (fd, identity(fd), hashlib.sha256(read(fd)).hexdigest())

    def validate_and_read(self):
        if set(os.listdir(self.root)) != set(MEMBERS):
            raise OwnedAudioError("artifact_custody_unavailable")
        for name, (fd, expected, digest) in self.code.items():
            current = os.stat(ROOT / name, follow_symlinks=False)
            if (current.st_dev, current.st_ino, current.st_size) != expected or hashlib.sha256(
                read(fd)
            ).hexdigest() != digest:
                raise OwnedAudioError("native_runtime_unqualified")
        values = {}
        for name, (fd, expected, digest) in self.assets.items():
            current = os.stat(name, dir_fd=self.root, follow_symlinks=False)
            value = read(fd)
            if (
                (current.st_dev, current.st_ino, current.st_size) != expected
                or identity(fd) != expected
                or hashlib.sha256(value).hexdigest() != digest
            ):
                raise OwnedAudioError("artifact_custody_unavailable")
            values[name] = value
        return values


class Devices:
    def resolve_device(self, value):
        return Device(value)


class Gate:
    def __init__(self, snapshot, *, hold_load=False):
        self.snapshot = snapshot
        self.hold_load = hold_load
        self.actor = None

    def prepare_from_parent(self, payload, manager):
        if payload["source_id"] != "fixture-selected" or payload["model_id"] != "library/speech":
            raise OwnedAudioError("invalid_load_plan")
        return OwnedLoadPlan._from_native_gate(
            self,
            runtime_instance_id=manager.runtime_instance_id,
            model_id=payload["model_id"],
            custody=types.SimpleNamespace(released=False, loaded=None),
        )

    def admit(self, plan, runtime_instance_id):
        if plan.runtime_instance_id != runtime_instance_id or plan._custody.released:
            raise OwnedAudioError("invalid_load_plan")

    def load(self, plan, device):
        values = self.snapshot.validate_and_read()
        # All five selected members are actually consumed, with concrete digest
        # provenance; no tensors/models are parsed or executed.
        digest = hashlib.sha256()
        for name in MEMBERS:
            digest.update(name.encode() + b"\0" + values[name])
        model = types.SimpleNamespace(
            read_sha256=digest.hexdigest(),
            tokens=list(values["model.safetensors"]),
            calls=0,
        )
        plan._custody.loaded = model
        if self.hold_load:
            print("controlled load entered", file=sys.stderr, flush=True)
            self.actor._active.cancel.wait()
        return LoadedModel(model, object(), device, "cohere-asr")

    def cleanup(self, plan, loaded, device):
        plan._custody.loaded = None

    def release_custody(self, plan):
        assert plan._custody.loaded is None
        plan._custody.released = True


def create_channel(args):
    manager = ModelManager(Devices())
    if args.unqualified:
        from private_owned_channel import create_private_owned_channel

        channel = create_private_owned_channel(manager)
    else:
        gate = Gate(Snapshot(args.fixture_source_fd), hold_load=args.hold_load_until_cancel)
        actor = OwnedAudioActor(manager, native_gate=gate)
        gate.actor = actor

        def native(model, processor, pcm, language, cancel):
            model.calls += 1
            if args.hold_use_until_cancel:
                print("controlled use entered", file=sys.stderr, flush=True)
                cancel.wait()
            reason = _finish_reason([7, *model.tokens], [7], frozenset({0}))
            return NativeSpeechResult(
                f"controlled:{model.read_sha256};calls={model.calls};pcm={pcm.hex()}",
                reason,
            )

        channel = PrivateOwnedChannel(actor, native_gate=gate, adapter=native)
    return channel


async def run(args):
    channel = create_channel(args)
    if args.channel_fd is None:
        await channel.serve_stdio()
    else:
        await channel.serve_fd(args.channel_fd)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--channel-fd", type=int)
    parser.add_argument("--fixture-source-fd", type=int)
    parser.add_argument("--hold-use-until-cancel", action="store_true")
    parser.add_argument("--hold-load-until-cancel", action="store_true")
    parser.add_argument("--unqualified", action="store_true")
    asyncio.run(run(parser.parse_args()))
