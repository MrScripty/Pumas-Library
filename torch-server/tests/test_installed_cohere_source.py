"""Held bootstrap source tests only; fixture bytes never qualify native ASR."""

import hashlib
import os
from pathlib import Path
import struct
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _TestModelManager  # noqa: F401 (suite Torch fallback)
from installed_cohere_source import InstalledSourceRefusal, _InstalledCohereSourceOwner
from loaders.owned_cohere_source import REQUIRED
from owned_audio import OwnedAudioError, UnavailableOwnedNativeGate
from private_owned_channel import _create_bootstrap_private_owned_channel


class FixedSourcePolicy:
    """Controlled source retention, with no provider/load/disposal implementation."""

    def validate_source(self, source):
        source.validate()

    def bind_retention(self, source, proof):
        source.bind_retention(proof)

    def retain_source(self, source):
        return source.retain_source(self)


@unittest.skipUnless(sys.platform == "linux", "Linux held descriptors")
class InstalledSourceTests(unittest.IsolatedAsyncioTestCase):
    def stage(self):
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        members = {name: (name + " fixture bytes").encode() for name in sorted(REQUIRED)}
        for name, data in members.items():
            (root / name).write_bytes(data)
        fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        self.addCleanup(os.close, fd)
        manager = SimpleNamespace(runtime_instance_id="controlled-runtime")
        manager._install_owned_audio_actor = lambda actor: setattr(manager, "actor", actor)
        return root, fd, members, manager

    def capture(self, fd, manager, model="library/speech", artifact="weights"):
        owner = _InstalledCohereSourceOwner._capture(manager, fd, model, artifact)
        self.addCleanup(lambda: owner.close_unclaimed() if owner._proof is None else None)
        return owner

    def test_correlation_matches_rust_field_framing_and_original_labels(self):
        _, fd, members, manager = self.stage()
        owner = self.capture(fd, manager, "library/voix-é", "weights")
        expected = hashlib.sha256(b"pumas-selected-artifact-bytes-v1\0")
        for label in ("library/voix-é", "weights"):
            raw = label.encode()
            expected.update(struct.pack(">Q", len(raw)) + raw)
        for name, raw in sorted(members.items()):
            encoded = name.encode()
            expected.update(struct.pack(">Q", len(encoded)) + encoded)
            expected.update(struct.pack(">Q", len(raw)) + hashlib.sha256(raw).hexdigest().encode())
        self.assertEqual(owner.source_id, "pumas-cohere-owned-v1:" + expected.hexdigest())
        changed = self.capture(fd, manager, "library/voix-é", "other-artifact")
        self.assertNotEqual(changed.source_id, owner.source_id)
        owner.validate()

    def test_original_runtime_and_readonly_correlation_properties_do_not_retarget(self):
        _, fd, _, manager = self.stage()
        source = self.capture(fd, manager)
        for name in ("manager", "model_id", "selected_artifact_id", "source_id"):
            with self.assertRaises(AttributeError):
                setattr(source, name, "forged")
        manager.runtime_instance_id = "successor-runtime"
        with self.assertRaises(InstalledSourceRefusal):
            source.validate()

    def test_retargeted_locator_does_not_select_successor(self):
        root, fd, _, manager = self.stage()
        owner = self.capture(fd, manager)
        moved = root.with_name(root.name + "-retained")
        root.rename(moved)
        self.addCleanup(lambda: moved.rename(root))
        owner.validate()
        self.assertEqual(os.fstat(owner._root).st_ino, os.stat(moved).st_ino)

    def test_replacement_mutation_and_added_members_refuse(self):
        for change in ("replacement", "mutation", "added"):
            root, fd, _, manager = self.stage()
            owner = self.capture(fd, manager)
            target = root / "config.json"
            if change == "replacement":
                successor = root / "successor"
                successor.write_bytes(target.read_bytes())
                successor.replace(target)
            elif change == "mutation":
                target.write_bytes(b"changed")
            else:
                (root / "unreported.py").write_bytes(b"must never import")
            with self.assertRaises(InstalledSourceRefusal):
                owner.validate()

    def test_link_and_special_member_are_never_opened(self):
        for kind in ("link", "fifo", "hardlink"):
            root, fd, _, manager = self.stage()
            target = root / "model.safetensors"
            target.unlink()
            if kind == "link":
                target.symlink_to(root / "config.json")
            elif kind == "fifo":
                os.mkfifo(target)
            else:
                os.link(root / "config.json", target)
            with self.assertRaises(InstalledSourceRefusal):
                self.capture(fd, manager)

    async def test_shipping_factory_does_not_open_source_or_accept_labels_as_authority(self):
        _, _, _, manager = self.stage()
        with patch("installed_cohere_source._InstalledCohereSourceOwner._capture") as capture:
            channel = _create_bootstrap_private_owned_channel(manager, -1, "forged", "forged")
        capture.assert_not_called()
        self.assertIsInstance(channel.actor._gate, UnavailableOwnedNativeGate)

    async def test_fixed_policy_binds_exact_source_and_refuses_replay_and_wrong_identity(self):
        _, fd, _, manager = self.stage()
        policy = FixedSourcePolicy()
        with patch("owned_audio._INSTALLED_AUDIO_POLICIES", (policy,)):
            channel = _create_bootstrap_private_owned_channel(
                manager, fd, "library/speech", "weights"
            )
            gate, proof = channel._gate, channel._gate._proof
            source = proof._source
            self.assertEqual(source.source_id, proof.source_id)
            with self.assertRaises(InstalledSourceRefusal):
                source.retain_source(policy)
            with self.assertRaises(OwnedAudioError):
                gate.prepare_from_parent(
                    {
                        "runtime_instance_id": manager.runtime_instance_id,
                        "model_id": source.model_id,
                        "source_id": "forged",
                    },
                    manager,
                )
            reader = source.model_source(proof)
            self.assertIs(reader.source_owner, source)
            with self.assertRaises(InstalledSourceRefusal):
                source.release(proof)
            with self.assertRaises(InstalledSourceRefusal):
                source.model_source(object())
            reader.close()
            with self.assertRaises(InstalledSourceRefusal):
                source.model_source(proof)
            # No native load happened. Explicit source-policy release closes the
            # original capabilities and breaks retention, without a receipt.
            source.release(proof)
            with self.assertRaises(InstalledSourceRefusal):
                source.validate()

    def test_mutate_and_restore_between_owner_validation_and_sealing_refuses(self):
        from loaders.owned_cohere_source import HeldCohereReadSource

        root, fd, members, manager = self.stage()
        source = self.capture(fd, manager)
        policy = FixedSourcePolicy()
        original = HeldCohereReadSource._from_members
        weights = root / "model.safetensors"
        baseline = members["model.safetensors"]
        with patch("owned_audio._INSTALLED_AUDIO_POLICIES", (policy,)):
            proof = source.retain_source(policy)

            def mutate(owner, files, *, expected):
                # This runs after source.model_source's own validation. Restore
                # before it returns: a post-hoc revalidation alone would miss it.
                weights.write_bytes(b"x" * len(baseline))
                try:
                    return original(owner, files, expected=expected)
                finally:
                    weights.write_bytes(baseline)

            before = set(os.listdir("/proc/self/fd"))
            with patch.object(HeldCohereReadSource, "_from_members", side_effect=mutate):
                with self.assertRaisesRegex(ValueError, "original selected bytes"):
                    source.model_source(proof)
            self.assertEqual(set(os.listdir("/proc/self/fd")), before)
            self.assertIsNone(source._reader)
            source.validate()  # Restored originals cannot rescue the failed capture.
            source.release(proof)

    def test_unregistered_policy_and_direct_constructor_cannot_mint_source_proof(self):
        _, fd, _, manager = self.stage()
        source = self.capture(fd, manager)
        with self.assertRaises(OwnedAudioError):
            source.retain_source(FixedSourcePolicy())
        with self.assertRaises(TypeError):
            _InstalledCohereSourceOwner()
        with self.assertRaises(InstalledSourceRefusal):
            _InstalledCohereSourceOwner._capture(manager, fd, None, "weights")


class UnsupportedSourceTests(unittest.TestCase):
    def test_unsupported_platform_refuses_before_descriptor_or_label_access(self):
        for platform in ("darwin", "win32"):
            with (
                patch("installed_cohere_source.sys.platform", platform),
                patch("installed_cohere_source.os.dup") as duplicate,
            ):
                with self.assertRaisesRegex(InstalledSourceRefusal, "unsupported.*platform"):
                    _InstalledCohereSourceOwner._capture(None, None, None, None)
                duplicate.assert_not_called()

    def test_source_and_loader_read_sets_remain_identical(self):
        import installed_cohere_source as source
        from loaders import owned_cohere_source as reader

        self.assertEqual(source.REQUIRED, reader.REQUIRED)
        self.assertEqual(source.OPTIONAL, reader.OPTIONAL)


class BootstrapLabelParsingTests(unittest.TestCase):
    def test_original_labels_cannot_be_reinterpreted_as_cli_options(self):
        from owned_worker import parse_args

        args = parse_args(
            [
                "--code-root-fd",
                "3",
                "--packages-root-fd",
                "4",
                "--model-root-fd",
                "5",
                "--model-id=--example",
                "--selected-artifact-id=--weights",
            ]
        )
        self.assertEqual(args.model_id, "--example")
        self.assertEqual(args.selected_artifact_id, "--weights")
        self.assertEqual(args.model_root_fd, 5)
