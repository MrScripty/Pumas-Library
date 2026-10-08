"""Private typed audio projection into the existing native operation owner.

No route, capability advertisement or artifact authority is installed here.
The real generic RPC endpoint still refuses audio pending native runtime and
read-set qualification. A binding is supplied by the owning load actor in the
same process; decoding a request can never create one. This bridge is exercised
with controlled workers and is not evidence of installed ASR qualification.
"""

import asyncio
from dataclasses import dataclass
import json
import unicodedata
from uuid import uuid4

from audio_contract import LANGUAGES
from audio_input import AudioInputError, normalize_audio

MAX_REQUEST_BYTES = 32 * 1024 * 1024


class OwnedOperationError(ValueError):
    """Fixed boundary errors, without submitted bytes or backend diagnostics."""

    def __init__(self, code):
        self.code = code
        super().__init__(code)


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise OwnedOperationError("invalid_request")
        result[key] = value
    return result


def _identifier(value):
    return (
        type(value) is str
        and 1 <= len(value) <= 128
        and all(c.isascii() and (c.isalnum() or c in "-_.:") for c in value)
    )


def _request(body):
    if type(body) is not bytes or len(body) > MAX_REQUEST_BYTES:
        raise OwnedOperationError("request_limit")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique)
    except (UnicodeError, ValueError, RecursionError):
        raise OwnedOperationError("invalid_request") from None
    required = {
        "contract_version",
        "request_id",
        "model",
        "capability",
        "input",
        "output",
        "options",
    }
    if (
        type(value) is not dict
        or not required <= value.keys()
        or value.keys() - required - {"profile", "stream"}
    ):
        raise OwnedOperationError("invalid_request")
    if type(value["contract_version"]) is not int or value["contract_version"] != 1:
        raise OwnedOperationError("unsupported_contract")
    model = value["model"]
    profile = value.get("profile")
    try:
        model_bytes = model.encode("utf-8") if type(model) is str else b""
    except UnicodeError:
        raise OwnedOperationError("invalid_request") from None
    if (
        not _identifier(value["request_id"])
        or type(model) is not str
        or not model
        or len(model_bytes) > 256
        or model.strip() != model
        or any(unicodedata.category(c) == "Cc" for c in model)
        or (profile is not None and (not _identifier(profile) or ":" in profile))
        or type(value.get("stream", False)) is not bool
        or value.get("stream", False)
    ):
        raise OwnedOperationError("invalid_request")
    capability = value["capability"]
    if (capability, value["output"]) not in (
        ("audio_transcription", "text"),
        ("audio_classification", "labels"),
    ):
        raise OwnedOperationError("invalid_request")
    audio, options = value["input"], value["options"]
    if type(audio) is not dict or audio.get("kind") != "audio":
        raise OwnedOperationError("invalid_request")
    if (
        type(options) is not dict
        or options.get("kind") != "audio"
        or options.keys() - {"kind", "language", "max_output_tokens"}
    ):
        raise OwnedOperationError("invalid_request")
    language = options.get("language")
    limit = options.get("max_output_tokens")
    if language is not None and (type(language) is not str or language not in LANGUAGES):
        raise OwnedOperationError("invalid_request")
    if limit is not None and (type(limit) is not int or not 0 < limit <= (1 << 32) - 1):
        raise OwnedOperationError("invalid_request")
    try:
        normalized = normalize_audio({key: item for key, item in audio.items() if key != "kind"})
    except AudioInputError:
        raise OwnedOperationError("invalid_request") from None
    # This adapter is explicitly transcription, with a fixed 512-token native
    # bound. Do not silently ignore a requested bound or treat labels as text.
    if capability != "audio_transcription" or limit not in (None, 512):
        raise OwnedOperationError("capability_unavailable")
    return value, normalized, language or "en"


@dataclass(frozen=True, eq=False)
class OwnedOperationHandle:
    """Local custody reference; not a JSON permission or replay instruction."""

    request_id: str
    _owner: object
    _native_ref: object


class OwnedModelOperations:
    """One exact loaded-slot binding; no path fallback or automatic replay.

    SpeechOperationOwner retains the native worker, PCM and artifact borrow.
    Caller cancellation requests cooperative cancellation without releasing
    those owners. Status/cancel accept only this bridge's opaque references.
    A generic public response must additionally gain a qualified finish-reason
    projection before this private bridge can become a public adapter.
    """

    def __init__(self, actor, *, model, profile, slot_ref, adapter=None):
        # Parsing typed requests must not import Torch or native loaders.
        from speech_operations import SpeechOperationOwner

        self._actor = actor
        self._model = model
        self._profile = profile
        self._slot_ref = slot_ref
        self._token = object()
        self._closed = False
        self._last_handle = None
        manager = actor.manager
        if slot_ref.runtime_instance_id != manager.runtime_instance_id:
            raise OwnedOperationError("capability_unavailable")
        try:
            status = actor.status(slot_ref)
        except (ValueError, RuntimeError):
            raise OwnedOperationError("capability_unavailable") from None
        if status.state != "ready" or status.cleanup != "retained":
            raise OwnedOperationError("capability_unavailable")
        # The manager retains one inference owner for its entire runtime. Clean
        # unload/reload changes the bound slot, never that native worker owner.
        self._native = manager._speech_owner
        if self._native is None:
            self._native = SpeechOperationOwner(
                manager, **({"adapter": adapter} if adapter is not None else {})
            )
        elif adapter is not None and self._native._adapter is not adapter:
            raise OwnedOperationError("capability_unavailable")

    def start(self, body, *, admission=None):
        from speech_operations import SpeechOperationError

        if self._closed:
            raise OwnedOperationError("admission_closed")
        value, audio, language = _request(body)
        if value["model"] != self._model or value.get("profile") not in (None, self._profile):
            raise OwnedOperationError("model_not_found")
        # Native start itself checks the actor's exact-slot retained authority;
        # a model alias or a prepared-byte receipt cannot bypass that check.
        envelope = {
            "runtime_instance_id": self._slot_ref.runtime_instance_id,
            "slot": {
                "slot_id": self._slot_ref.slot_id,
                "load_generation": self._slot_ref.load_generation,
            },
            "request_id": str(uuid4()),
            "language": language,
            "audio": audio,
        }
        try:
            status = self._native.start(json.dumps(envelope).encode("utf-8"), admission=admission)
        except SpeechOperationError as error:
            raise OwnedOperationError(error.code) from None
        handle = OwnedOperationHandle(value["request_id"], self._token, status.operation_ref)
        self._last_handle = handle
        return handle

    def _ref(self, handle):
        if type(handle) is not OwnedOperationHandle or handle._owner is not self._token:
            raise OwnedOperationError("invalid_operation_ref")
        return handle._native_ref

    def status(self, handle):
        return self._native.status(self._ref(handle))

    def cancel(self, handle):
        return self._native.cancel(self._ref(handle))

    async def wait(self, handle):
        from speech_operations import SpeechOperationError

        ref = self._ref(handle)
        try:
            return await self._native.wait(ref)
        except asyncio.CancelledError:
            try:
                self._native.cancel(ref)
            except SpeechOperationError:
                pass  # A pruned settled receipt must not replace caller cancellation.
            raise

    def close_admission(self):
        from speech_operations import SpeechOperationError

        self._closed = True
        if self._last_handle is not None:
            try:
                return self.cancel(self._last_handle)
            except SpeechOperationError:
                pass  # Receipt expiry establishes no native work to cancel.
        return None
