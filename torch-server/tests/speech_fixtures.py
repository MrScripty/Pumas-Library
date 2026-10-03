"""Synthetic contracts only. No real artifact custody, manifests or qualification."""

from speech_binding import SpeechBindingError


class SyntheticArtifactUse:
    def __init__(self, authority, ref):
        self.authority = authority
        self.ref = ref
        self.released = False
        self.validations = 0

    def validate(self, ref):
        self.validations += 1
        if self.released or ref != self.ref or ref not in self.authority.attested:
            raise SpeechBindingError("artifact_custody_unavailable")

    def release(self):
        if self.authority.release_error is not None:
            raise self.authority.release_error
        assert not self.released
        self.released = True


class SyntheticArtifactAuthority:
    def __init__(self):
        self.attested = set()
        self.borrows = []
        self.release_error = None

    def attest_fixture(self, ref):
        # Explicit fixture assumption, never proof of actual loaded bytes.
        self.attested.add(ref)
        return ref

    def acquire(self, ref):
        if ref not in self.attested:
            raise SpeechBindingError("artifact_custody_unavailable")
        borrow = SyntheticArtifactUse(self, ref)
        self.borrows.append(borrow)
        return borrow


def bind_fixture(manager, ref):
    """Test-only convenience; production owner retains state before transfer."""
    binding = manager.prepare_speech(ref)
    manager.bind_speech(binding)
    return binding
