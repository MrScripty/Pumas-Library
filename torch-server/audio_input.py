"""Bounded audio conversion before private speech admission.

Declarations describe the actual input bytes. Supported PCM is decoded,
channels are averaged, and a windowed-sinc low-pass resampler produces the
private adapter's mono 16 kHz PCM16LE contract. This module grants no artifact or
runtime authority and performs no model loading or inference.
"""

import base64
import binascii

from audio_contract import MAX_AUDIO_SAMPLES, SAMPLE_RATE

_FIELDS = frozenset({"encoding", "sample_rate_hz", "channels", "sample_count", "data_base64"})
_MAX_BODY_BYTES = 32 * 1024 * 1024
_MIN_RATE = 8000
_MAX_RATE = 192000


class AudioInputError(ValueError):
    """A fixed, data-free refusal; never include PCM or caller values."""

    def __init__(self):
        super().__init__("Audio input does not satisfy the supported PCM contract")


def normalize_audio(audio: dict) -> dict:
    """Validate and actually convert one declared input, without inference.

    sample_count counts frames per channel. Accepted rates are 8–192 kHz,
    with one or two channels and at most the existing 30 second speech bound.
    Float PCM must be finite normalized samples in [-1, 1].
    """
    if type(audio) is not dict or set(audio) != _FIELDS:
        raise AudioInputError()
    encoding = audio["encoding"]
    rate = audio["sample_rate_hz"]
    channels = audio["channels"]
    count = audio["sample_count"]
    encoded = audio["data_base64"]
    if (
        encoding not in ("pcm_s16le", "pcm_f32le")
        or type(rate) is not int
        or not _MIN_RATE <= rate <= _MAX_RATE
        or type(channels) is not int
        or channels not in (1, 2)
        or type(count) is not int
        or not 0 < count <= rate * (MAX_AUDIO_SAMPLES // SAMPLE_RATE)
        or type(encoded) is not str
        or len(encoded) > _MAX_BODY_BYTES
    ):
        raise AudioInputError()
    width = 2 if encoding == "pcm_s16le" else 4
    expected = count * channels * width
    if len(encoded) != 4 * ((expected + 2) // 3):
        raise AudioInputError()
    try:
        data = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error):
        raise AudioInputError() from None
    if len(data) != expected:
        raise AudioInputError()
    if encoding == "pcm_s16le" and rate == SAMPLE_RATE and channels == 1:
        converted = data
        output_count = count
    else:
        # Lazy import keeps unrelated control/text/image startup independent.
        import numpy as np

        dtype = "<i2" if width == 2 else "<f4"
        samples = np.frombuffer(data, dtype=dtype).reshape(count, channels)
        if width == 4 and (not np.isfinite(samples).all() or (np.abs(samples) > 1).any()):
            raise AudioInputError()
        mono = samples.astype(np.float64).mean(axis=1)
        if width == 2:
            mono /= 32768.0
        output_count = count * SAMPLE_RATE // rate
        if not 0 < output_count <= MAX_AUDIO_SAMPLES:
            raise AudioInputError()
        if rate != SAMPLE_RATE:
            mono = _resample(mono, rate, output_count, np)
        converted = np.clip(np.rint(mono * 32768.0), -32768, 32767).astype("<i2").tobytes()
    return {
        "encoding": "pcm_s16le",
        "sample_rate_hz": SAMPLE_RATE,
        "channels": 1,
        "sample_count": output_count,
        "data_base64": base64.b64encode(converted).decode("ascii"),
    }


def _resample(samples, source_rate, output_count, np):
    # A 64 tap windowed-sinc kernel, evaluated in bounded 4096 frame blocks.
    # Lower the cutoff on downsampling instead of aliasing by decimation.
    cutoff = min(1.0, SAMPLE_RATE / source_rate) * 0.95
    offsets = np.arange(-31, 33)
    output = np.empty(output_count, dtype=np.float64)
    for start in range(0, output_count, 4096):
        positions = np.arange(start, min(start + 4096, output_count)) * source_rate / SAMPLE_RATE
        indices = np.floor(positions).astype(np.int64)[:, None] + offsets
        distance = positions[:, None] - indices
        valid = (indices >= 0) & (indices < len(samples))
        window = np.where(np.abs(distance) < 32, 0.5 * (1 + np.cos(np.pi * distance / 32)), 0)
        weights = cutoff * np.sinc(cutoff * distance) * window * valid
        normalization = weights.sum(axis=1)
        values = samples[np.clip(indices, 0, len(samples) - 1)]
        output[start : start + len(positions)] = (weights * values).sum(axis=1) / normalization
    return output
