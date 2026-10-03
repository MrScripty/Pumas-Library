"""Private slot identities and the still-unimplemented artifact-custody seam.

These values are not content or custody attestation. A future authority must own
canonical selected-artifact byte manifests, recipe/code identity and the exact
profile/process generation from load admission through slot unload or confirmed
process cessation. Acquiring an operation borrow cannot attest a past load.
There is deliberately no production authority implementation in this milestone.
"""

from dataclasses import dataclass, field
from typing import Any, Protocol


BINDING_ERROR_CODES = frozenset(
    {
        "invalid_slot_ref",
        "runtime_replaced",
        "slot_replaced",
        "model_unavailable",
        "model_unsupported",
        "artifact_authority_unavailable",
        "artifact_custody_unavailable",
    }
)


class SpeechBindingError(ValueError):
    """A data-free private binding refusal."""

    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


class ArtifactUseReleaseUnconfirmed(RuntimeError):
    """The borrow remains owned; the original cause is retained, never projected."""


@dataclass(frozen=True)
class SpeechSlotRef:
    runtime_instance_id: str
    slot_id: str
    load_generation: str


class ArtifactUseLease(Protocol):
    """An operation borrow of already-attested, slot-lifetime artifact custody.

    Both methods are synchronous, nonblocking operations on retained evidence.
    validate raises unless this receipt still covers the exact reference and its
    attested content/recipe/profile/process. release returns only on confirmed
    borrow release; an exception requires quarantine. Neither method performs
    filesystem/DB work. Releasing this borrow does not release slot custody and
    makes no claim about artifact deletion or process cessation.
    """

    def validate(self, ref: SpeechSlotRef) -> None: ...

    def release(self) -> None: ...


class ArtifactUseAuthority(Protocol):
    """Trusted internal dependency, never assembled from caller identity values.

    acquire is a synchronous, nonblocking admission over previously retained
    load-time evidence, without filesystem/DB work or an async admission gap.
    It either returns a valid retained borrow, or refuses without transferring
    any custody to its caller. Uncertain internal acquisition remains owned by
    the authority. Values alone, a READY model or a model path do not suffice.
    """

    def acquire(self, ref: SpeechSlotRef) -> ArtifactUseLease: ...


class UnavailableArtifactUseAuthority:
    """Production default until a real load-to-unload authority is implemented."""

    def acquire(self, ref: SpeechSlotRef) -> ArtifactUseLease:
        raise SpeechBindingError("artifact_authority_unavailable")


@dataclass
class _BoundSpeechSlot:
    ref: SpeechSlotRef
    manager: Any = field(repr=False)
    slot: Any = field(repr=False)
    loaded: Any = field(repr=False)
    device: str
    artifact_use: ArtifactUseLease = field(repr=False)
    released: bool = False

    def release(self) -> None:
        if not self.released:
            try:
                self.artifact_use.release()
            except BaseException as error:
                raise ArtifactUseReleaseUnconfirmed(
                    "Artifact borrow release is unconfirmed"
                ) from error
            self.released = True
