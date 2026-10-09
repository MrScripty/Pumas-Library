"""Held selected Cohere files for the private owning loader.

This capability is created by the original source-owned policy, never by wire
metadata. It removes model-directory/cache lookup from that loader; it does not
contain interpreter imports or native dlopen/data reads. Linux sealed copies
keep the loader's selected model bytes immutable after capture.
"""

import hashlib
import json
import math
import os
import stat
import sys

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
MAX_TOKENIZER_BYTES = 32 * 1024 * 1024
TOKEN_KEYS = frozenset(
    {
        "bos_token",
        "eos_token",
        "unk_token",
        "pad_token",
        "sep_token",
        "cls_token",
        "mask_token",
        "additional_special_tokens",
        "extra_special_tokens",
    }
)
TOKENIZER_OPTIONS = TOKEN_KEYS | {
    "model_max_length",
    "padding_side",
    "truncation_side",
    "clean_up_tokenization_spaces",
    "split_special_tokens",
    "add_prefix_space",
    "added_tokens_decoder",
}
TOKENIZER_METADATA = {
    "tokenizer_class",
    "auto_map",
    "name_or_path",
    "_name_or_path",
    "transformers_version",
}
NATIVE_REDIRECTS = {
    "quantization_config",
    "transformers_weights",
    "custom_generate",
    "kernel_config",
    "use_kernels",
    "adapter_kwargs",
    "gguf_file",
    "subfolder",
    "offload_folder",
}


def _identity(fd):
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode) or info.st_size <= 0:
        raise ValueError("Owned Cohere members must be nonempty regular files")
    return info.st_dev, info.st_ino, info.st_size


def _digest(fd, size):
    digest, offset = hashlib.sha256(), 0
    while offset < size:
        chunk = os.pread(fd, min(65536, size - offset), offset)
        if not chunk:
            raise ValueError("Owned Cohere member changed during reading")
        digest.update(chunk)
        offset += len(chunk)
    if os.pread(fd, 1, size):
        raise ValueError("Owned Cohere member changed during reading")
    return digest.digest()


def _sealed_copy(original, identity, digest):
    """Copy only held bytes, then irreversibly prohibit writes and resizing.

    No disk-file fallback: inability to obtain or verify kernel seals refuses
    construction. The writable descriptor never escapes this function.
    """
    import fcntl

    if not all(
        hasattr(fcntl, name)
        for name in (
            "F_SEAL_WRITE",
            "F_SEAL_GROW",
            "F_SEAL_SHRINK",
            "F_SEAL_SEAL",
            "F_ADD_SEALS",
            "F_GET_SEALS",
        )
    ) or not all(
        hasattr(os, name) for name in ("memfd_create", "MFD_CLOEXEC", "MFD_ALLOW_SEALING")
    ):
        raise ValueError("Owned Cohere immutable read source unavailable")
    seals = fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL
    fd = os.memfd_create("pumas-owned-cohere", os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
    readonly = None
    try:
        observed, offset = hashlib.sha256(), 0
        while offset < identity[2]:
            chunk = os.pread(original, min(65536, identity[2] - offset), offset)
            if not chunk:
                raise ValueError("Owned Cohere member changed during capture")
            observed.update(chunk)
            written = 0
            while written < len(chunk):
                count = os.pwrite(fd, chunk[written:], offset + written)
                if count <= 0:
                    raise OSError("Owned Cohere sealed copy made no progress")
                written += count
            offset += len(chunk)
        if (
            _identity(original) != identity
            or os.pread(original, 1, identity[2])
            or observed.digest() != digest
        ):
            raise ValueError("Owned Cohere member changed during capture")
        fcntl.fcntl(fd, fcntl.F_ADD_SEALS, seals)
        if fcntl.fcntl(fd, fcntl.F_GET_SEALS) & seals != seals:
            raise ValueError("Owned Cohere immutable read source unavailable")
        readonly = os.open(f"/proc/self/fd/{fd}", os.O_RDONLY | os.O_CLOEXEC)
        if _identity(readonly) != _identity(fd) or _digest(readonly, identity[2]) != digest:
            raise ValueError("Owned Cohere sealed copy differs from selected bytes")
        result = (readonly, _identity(readonly), digest)
        readonly = None
        return result
    finally:
        if readonly is not None:
            os.close(readonly)
        os.close(fd)


class HeldCohereReadSource:
    """Original source retention plus immutable selected model snapshots.

    Hashes observe held bytes, not execution qualification. The source policy
    must validate their binding to its original prepared allocation separately.
    """

    def __init__(self, *args, **kwargs):
        raise TypeError("Owned Cohere readers come from the original source policy")

    @classmethod
    def _from_members(cls, source_owner, members):
        if (
            sys.platform != "linux"
            or source_owner is None
            or type(members) is not dict
            or not REQUIRED.issubset(members)
            or not set(members).issubset(REQUIRED | OPTIONAL)
        ):
            raise ValueError("Unsupported owned Cohere selected read set")
        import fcntl

        reader = object.__new__(cls)
        reader.source_owner, reader._members, reader._closed = source_owner, {}, False
        reader._sealed = {}
        try:
            # Inspect every original before dup can reuse an already-closed FD
            # number. Descriptor numbers alone never prove member ownership.
            identities = {}
            for name, original in members.items():
                if type(original) is not int or original < 0:
                    raise ValueError("Owned Cohere requires held file descriptors")
                if fcntl.fcntl(original, fcntl.F_GETFL) & os.O_ACCMODE != os.O_RDONLY:
                    raise ValueError("Owned Cohere requires read-only file descriptors")
                identities[name] = _identity(original)
                os.pread(original, 1, 0)
            for name, original in members.items():
                fd = os.dup(original)
                try:
                    identity = _identity(fd)
                    if identity != identities[name]:
                        raise ValueError("Owned Cohere member changed during capture")
                    digest = _digest(fd, identity[2])
                    if _identity(fd) != identity:
                        raise ValueError("Owned Cohere member changed during reading")
                except BaseException:
                    os.close(fd)
                    raise
                reader._members[name] = (fd, identity, digest)
                reader._sealed[name] = _sealed_copy(fd, identity, digest)
            reader.validate()
            return reader
        except BaseException:
            reader.close()
            raise

    @property
    def names(self):
        return frozenset(self._members)

    def validate(self):
        if self._closed:
            raise ValueError("Owned Cohere read source is closed")
        for fd, identity, digest in self._members.values():
            if (
                _identity(fd) != identity
                or _digest(fd, identity[2]) != digest
                or _identity(fd) != identity
            ):
                raise ValueError("Owned Cohere retained member changed")

    def read(self, name, maximum):
        if self._closed or name not in self._members:
            raise ValueError("Owned Cohere member is not selected")
        original, original_identity, expected = self._members[name]
        if (
            _identity(original) != original_identity
            or _digest(original, original_identity[2]) != expected
            or _identity(original) != original_identity
        ):
            raise ValueError("Owned Cohere retained member changed")
        fd, identity, digest = self._sealed[name]
        if identity[2] > maximum:
            raise ValueError("Owned Cohere descriptor is too large")
        raw = os.pread(fd, maximum + 1, 0)
        if (
            _identity(fd) != identity
            or len(raw) != identity[2]
            or hashlib.sha256(raw).digest() != digest
        ):
            raise ValueError("Owned Cohere retained member changed")
        return raw

    def weights_filename(self):
        # safetensors' mmap API needs a filename. This fixed Linux magic link
        # selects the held file, never a descriptor-controlled package pathname.
        self.validate()
        fd, identity, _ = self._sealed["model.safetensors"]
        filename = f"/proc/self/fd/{fd}"
        info = os.stat(filename)
        if (info.st_dev, info.st_ino, info.st_size) != identity:
            raise ValueError("Owned Cohere weights descriptor is unavailable")
        return filename

    def close(self):
        if not getattr(self, "_closed", True):
            self._closed = True
            members = [*self._members.values(), *self._sealed.values()]
            self._members, self._sealed = {}, {}
            try:
                for fd, _, _ in members:
                    try:
                        os.close(fd)
                    except OSError:
                        pass
            finally:
                self.source_owner = None

    def __del__(self):
        self.close()

    def __reduce_ex__(self, protocol):
        raise TypeError("Owned Cohere readers are not serializable")

    def __copy__(self):
        raise TypeError("Owned Cohere readers are not copyable")

    def __deepcopy__(self, memo):
        raise TypeError("Owned Cohere readers are not copyable")


def decode_object(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("Owned Cohere descriptor has duplicate fields")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError("Owned Cohere descriptor has nonfinite values")

    value = json.loads(raw, object_pairs_hook=pairs, parse_constant=invalid_constant)
    if type(value) is not dict:
        raise ValueError("Owned Cohere descriptor must be an object")
    pending = [value]
    while pending:
        item = pending.pop()
        if type(item) is dict:
            pending.extend(item.values())
        elif type(item) is list:
            pending.extend(item)
        elif type(item) is float and not math.isfinite(item):
            invalid_constant(item)
    return value


def validate_owned_options(descriptors):
    pending = list(descriptors.values())
    while pending:
        value = pending.pop()
        if type(value) is dict:
            if NATIVE_REDIRECTS.intersection(value):
                raise ValueError("Owned Cohere secondary native loading is unsupported")
            pending.extend(value.values())
        elif type(value) is list:
            pending.extend(value)
    config = descriptors["config.json"]
    encoder = config.get("encoder_config")
    if encoder is not None and (
        type(encoder) is not dict
        or encoder.get("model_type", "parakeet_encoder") != "parakeet_encoder"
    ):
        raise ValueError("Owned Cohere encoder class is unsupported")
    if set(descriptors["tokenizer_config.json"]) - TOKENIZER_OPTIONS - TOKENIZER_METADATA:
        raise ValueError("Owned Cohere tokenizer options are unsupported")
    if set(descriptors.get("special_tokens_map.json", {})) - TOKEN_KEYS:
        raise ValueError("Owned Cohere special token options are unsupported")
    if set(descriptors.get("processor_config.json", {})) - {"processor_class", "auto_map"}:
        raise ValueError("Owned Cohere processor options are unsupported")
    options = descriptors["tokenizer_config.json"]
    if "model_max_length" in options and (
        type(options["model_max_length"]) is not int or options["model_max_length"] <= 0
    ):
        raise ValueError("Owned Cohere tokenizer maximum length is unsupported")
    for key in ("padding_side", "truncation_side"):
        if key in options and (
            type(options[key]) is not str or options[key] not in {"left", "right"}
        ):
            raise ValueError("Owned Cohere tokenizer side is unsupported")
    for key in ("clean_up_tokenization_spaces", "split_special_tokens", "add_prefix_space"):
        if key in options and type(options[key]) is not bool:
            raise ValueError("Owned Cohere tokenizer flag is unsupported")
    merged = _merged_tokenizer_options(descriptors)
    for key in TOKEN_KEYS.intersection(merged):
        value = merged[key]
        if key in {"additional_special_tokens", "extra_special_tokens"}:
            if type(value) is not list:
                raise ValueError("Owned Cohere special token list is unsupported")
            for item in value:
                if item is None:
                    raise ValueError("Owned Cohere special token list is unsupported")
                _token_declaration(item)
        else:
            _token_declaration(value)
    decoder = merged.get("added_tokens_decoder", {})
    if type(decoder) is not dict:
        raise ValueError("Owned Cohere added token decoder is unsupported")
    for key, value in decoder.items():
        if (
            type(key) is not str
            or not key.isascii()
            or not key.isdecimal()
            or str(int(key)) != key
            or int(key) > 0xFFFFFFFF
            or type(value) is not dict
        ):
            raise ValueError("Owned Cohere added token ID is unsupported")
        _token_declaration(value)
    for value, identifier in descriptors.get("added_tokens.json", {}).items():
        if not value or type(identifier) is not int or not 0 <= identifier <= 0xFFFFFFFF:
            raise ValueError("Owned Cohere added token ID is unsupported")


def _merged_tokenizer_options(descriptors):
    options = {
        key: value
        for key, value in descriptors["tokenizer_config.json"].items()
        if key in TOKENIZER_OPTIONS
    }
    for key, value in descriptors.get("special_tokens_map.json", {}).items():
        if key in options and options[key] != value:
            raise ValueError("Owned Cohere special token declarations conflict")
        options[key] = value
    if "additional_special_tokens" in options and "extra_special_tokens" in options:
        if options["additional_special_tokens"] != options["extra_special_tokens"]:
            raise ValueError("Owned Cohere special token declarations conflict")
        options.pop("additional_special_tokens")
    return options


def _token_declaration(value):
    if value is None or type(value) is str and value:
        return value
    if type(value) is not dict or type(value.get("content")) is not str or not value["content"]:
        raise ValueError("Owned Cohere special token is unsupported")
    fields = {"content", "single_word", "lstrip", "rstrip", "normalized", "special"}
    if set(value) - fields - {"__type"} or ("__type" in value and value["__type"] != "AddedToken"):
        raise ValueError("Owned Cohere special token is unsupported")
    if any(type(v) is not bool for k, v in value.items() if k in fields - {"content"}):
        raise ValueError("Owned Cohere special token is unsupported")
    return {k: v for k, v in value.items() if k in fields}


def tokenizer_options(descriptors, backend, added_token_class):
    options = _merged_tokenizer_options(descriptors)

    def token(value):
        declaration = _token_declaration(value)
        return added_token_class(**declaration) if type(declaration) is dict else declaration

    for key in TOKEN_KEYS.intersection(options):
        value = options[key]
        if key in {"additional_special_tokens", "extra_special_tokens"}:
            if type(value) is not list:
                raise ValueError("Owned Cohere special token list is unsupported")
            options[key] = [token(item) for item in value]
            if any(item is None for item in options[key]):
                raise ValueError("Owned Cohere special token list is unsupported")
        else:
            options[key] = token(value)
    decoder = options.get("added_tokens_decoder", {})
    if type(decoder) is not dict:
        raise ValueError("Owned Cohere added token decoder is unsupported")
    converted = {}
    for key, value in decoder.items():
        if type(key) is not str or not key.isascii() or not key.isdecimal() or str(int(key)) != key:
            raise ValueError("Owned Cohere added token ID is unsupported")
        item = token(value)
        if item is None or backend.token_to_id(str(item)) != int(key):
            raise ValueError("Owned Cohere added token ID differs from selected tokenizer")
        converted[int(key)] = item
    if "added_tokens_decoder" in options:
        options["added_tokens_decoder"] = converted
    for value, identifier in descriptors.get("added_tokens.json", {}).items():
        if (
            type(identifier) is not int
            or identifier < 0
            or backend.token_to_id(value) != identifier
        ):
            raise ValueError("Owned Cohere added token ID differs from selected tokenizer")
    for key in TOKEN_KEYS.intersection(options):
        values = options[key] if type(options[key]) is list else [options[key]]
        if any(item is not None and backend.token_to_id(str(item)) is None for item in values):
            raise ValueError("Owned Cohere special token is absent from selected tokenizer")
    return options
