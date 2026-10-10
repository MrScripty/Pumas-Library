"""Held model capabilities and correlation for the private installed bootstrap.

This is byte ownership, never native/runtime qualification. Only the closed
source-owned production catalog or separate explicit local experiment may retain
a proof or create an admitted gate. Neither identifies upstream model provenance.
"""

import hashlib
import os
import stat
import sys
import threading

# Match the closed source-owned reader set without importing a package that
# imports Torch. Native imports belong after source validation in model_source.
REQUIRED = frozenset(
    {
        "config.json",
        "model.safetensors",
        "preprocessor_config.json",
        "tokenizer.json",
        "tokenizer_config.json",
    }
)
OPTIONAL = frozenset(
    {
        "added_tokens.json",
        "generation_config.json",
        "processor_config.json",
        "special_tokens_map.json",
    }
)


_EXPERIMENTAL_LOCAL_COHERE = object()


class InstalledSourceRefusal(ValueError):
    pass


def _names(fd):
    names = set()
    with os.scandir(fd) as entries:
        for entry in entries:
            names.add(entry.name)
            if len(names) > len(REQUIRED | OPTIONAL):
                raise InstalledSourceRefusal("unsupported_installed_model_read_set")
    return frozenset(names)


def _label(value):
    if type(value) is not str or not 1 <= len(value) <= 256 or "\x00" in value:
        raise InstalledSourceRefusal("invalid_installed_source_label")
    return value


def _identity(info):
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise InstalledSourceRefusal("unsupported_installed_model_member")
    return info.st_dev, info.st_ino, info.st_size


def _digest(fd, size):
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        block = os.pread(fd, min(1024 * 1024, size - offset), offset)
        if not block:
            raise InstalledSourceRefusal("installed_model_member_changed")
        digest.update(block)
        offset += len(block)
    if os.pread(fd, 1, size):
        raise InstalledSourceRefusal("installed_model_member_changed")
    return digest.hexdigest()


class _InstalledCohereSourceOwner:
    """Original held directory/files plus one proof and optional sealed reader.

    There is deliberately no destructor that closes possibly admitted native
    inputs. Explicit pre-admission disposal or the qualified policy owns release;
    unexpected owner loss leaves descriptors for exact process-tree teardown.
    """

    def __init__(self, *args, **kwargs):
        raise TypeError("Installed sources come from the inherited bootstrap")

    @classmethod
    def _capture(cls, manager, model_root_fd, model_id, selected_artifact_id):
        if sys.platform != "linux":
            raise InstalledSourceRefusal("unsupported_installed_source_platform")
        model_id, selected_artifact_id = _label(model_id), _label(selected_artifact_id)
        _label(manager.runtime_instance_id)
        if type(model_root_fd) is not int or model_root_fd < 0:
            raise InstalledSourceRefusal("invalid_installed_model_descriptor")
        owner = object.__new__(cls)
        owner._manager, owner._runtime_instance = manager, manager.runtime_instance_id
        owner._model_id, owner._artifact_id = model_id, selected_artifact_id
        owner._root, owner._members = None, {}
        owner._proof, owner._reader, owner._closed = None, None, False
        owner._experimental = None
        owner._lock = threading.Lock()
        try:
            owner._root = os.dup(model_root_fd)
            info = os.fstat(owner._root)
            if not stat.S_ISDIR(info.st_mode):
                raise InstalledSourceRefusal("invalid_installed_model_descriptor")
            owner._root_identity = info.st_dev, info.st_ino
            names = _names(owner._root)
            if not REQUIRED.issubset(names) or not names.issubset(REQUIRED | OPTIONAL):
                raise InstalledSourceRefusal("unsupported_installed_model_read_set")
            for name in sorted(names):
                before = _identity(os.stat(name, dir_fd=owner._root, follow_symlinks=False))
                fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=owner._root)
                try:
                    if _identity(os.fstat(fd)) != before:
                        raise InstalledSourceRefusal("installed_model_member_changed")
                    sha = _digest(fd, before[2])
                    if _identity(os.fstat(fd)) != before:
                        raise InstalledSourceRefusal("installed_model_member_changed")
                    owner._members[name] = fd, before, sha
                except BaseException:
                    os.close(fd)
                    raise
            digest = hashlib.sha256(b"pumas-selected-artifact-bytes-v1\0")
            for label in (model_id, selected_artifact_id):
                encoded = label.encode("utf-8")
                digest.update(len(encoded).to_bytes(8, "big"))
                digest.update(encoded)
            for name, (_, identity, sha) in sorted(owner._members.items()):
                encoded = name.encode("utf-8")
                digest.update(len(encoded).to_bytes(8, "big"))
                digest.update(encoded)
                digest.update(identity[2].to_bytes(8, "big"))
                digest.update(sha.encode("ascii"))
            owner._source_id = "pumas-cohere-owned-v1:" + digest.hexdigest()
            owner.validate()
            return owner
        except BaseException:
            owner._close_files()
            raise

    @classmethod
    def _capture_experimental(cls, manager, model_root_fd, model_id, selected_artifact_id):
        owner = cls._capture(manager, model_root_fd, model_id, selected_artifact_id)
        owner._experimental = _EXPERIMENTAL_LOCAL_COHERE
        return owner

    @property
    def manager(self):
        return self._manager

    @property
    def model_id(self):
        return self._model_id

    @property
    def selected_artifact_id(self):
        return self._artifact_id

    @property
    def source_id(self):
        return self._source_id

    def validate(self):
        if self._closed:
            raise InstalledSourceRefusal("installed_source_closed")
        if self.manager.runtime_instance_id != self._runtime_instance:
            raise InstalledSourceRefusal("installed_runtime_identity_changed")
        info = os.fstat(self._root)
        if (info.st_dev, info.st_ino) != self._root_identity:
            raise InstalledSourceRefusal("installed_source_root_changed")
        if _names(self._root) != frozenset(self._members):
            raise InstalledSourceRefusal("installed_model_read_set_changed")
        for name, (fd, identity, sha) in self._members.items():
            before = _identity(os.stat(name, dir_fd=self._root, follow_symlinks=False))
            if before != identity or _identity(os.fstat(fd)) != identity:
                raise InstalledSourceRefusal("installed_model_member_changed")
            if _digest(fd, identity[2]) != sha or _identity(os.fstat(fd)) != identity:
                raise InstalledSourceRefusal("installed_model_member_changed")

    def retain_source(self, policy):
        from owned_audio import _InstalledAudioSourceProof

        return _InstalledAudioSourceProof._from_source_policy(
            policy,
            self,
            manager=self.manager,
            model_id=self.model_id,
            source_id=self.source_id,
        )

    def bind_retention(self, proof):
        # Called only by the source-owned policy. Wire data cannot mint a proof.
        from owned_audio import _InstalledAudioSourceProof

        with self._lock:
            if (
                self._closed
                or self._proof is not None
                or type(proof) is not _InstalledAudioSourceProof
                or proof._source is not self
                or proof.manager is not self.manager
                or proof.model_id != self.model_id
                or proof.source_id != self.source_id
            ):
                raise InstalledSourceRefusal("installed_source_already_bound")
            self._proof = proof

    def model_source(self, proof):
        with self._lock:
            if self._closed or self._proof is not proof or proof is None:
                raise InstalledSourceRefusal("installed_source_not_bound")
            self.validate()
            if self._reader is not None and self._reader._closed:
                raise InstalledSourceRefusal("installed_native_reader_closed")
            if self._reader is None:
                from loaders.owned_cohere_source import HeldCohereReadSource

                self._reader = HeldCohereReadSource._from_members(
                    self,
                    {name: member[0] for name, member in self._members.items()},
                    expected={
                        name: (member[1][2], bytes.fromhex(member[2]))
                        for name, member in self._members.items()
                    },
                )
            return self._reader

    def release(self, proof):
        with self._lock:
            if self._closed or self._proof is not proof or proof is None:
                raise InstalledSourceRefusal("installed_source_not_bound")
            if self._reader is not None and not self._reader._closed:
                raise InstalledSourceRefusal("installed_native_reader_still_live")
            self._close_files()
            self._reader, self._proof = None, None

    def close_unclaimed(self):
        with self._lock:
            if self._proof is not None or self._reader is not None:
                raise InstalledSourceRefusal("installed_source_already_bound")
            self._close_files()

    def _close_files(self):
        self._closed = True
        for fd, _, _ in self._members.values():
            os.close(fd)
        self._members.clear()
        if self._root is not None:
            os.close(self._root)
            self._root = None


class _ExperimentalLocalCoherePolicy:
    """Fixed local CPU attempt; no upstream model or native disposal qualification."""

    def validate_source(self, source):
        if (
            type(source) is not _InstalledCohereSourceOwner
            or source._experimental is not _EXPERIMENTAL_LOCAL_COHERE
        ):
            raise InstalledSourceRefusal("experimental_source_required")
        source.validate()

    def retain_source(self, source):
        self.validate_source(source)
        return source.retain_source(self)

    def bind_retention(self, source, proof):
        source.bind_retention(proof)

    def model_source(self, source):
        self.validate_source(source)
        return source.model_source(source._proof)

    def dispose_native(self, disposal, device):
        from owned_audio import OwnedAudioError

        # Do not manufacture a receipt from Python reference release. The
        # experimental profile's owner closes admission and drains this child.
        raise OwnedAudioError("experimental_requires_child_drain")

    def release_source(self, source):
        source.release(source._proof)


_EXPERIMENTAL_POLICY = _ExperimentalLocalCoherePolicy()


def _experimental_policy_for(source):
    if (
        type(source) is _InstalledCohereSourceOwner
        and source._experimental is _EXPERIMENTAL_LOCAL_COHERE
    ):
        return _EXPERIMENTAL_POLICY
    return None
