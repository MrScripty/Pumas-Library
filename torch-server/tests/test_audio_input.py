"""Real PCM conversion tests, without Torch, models, or inference."""

import base64
from pathlib import Path
import struct
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from audio_input import AudioInputError, normalize_audio


def audio(values, *, rate=16000, channels=1, encoding="pcm_s16le"):
    code = "h" if encoding == "pcm_s16le" else "f"
    data = struct.pack("<" + code * len(values), *values)
    return {
        "encoding": encoding,
        "sample_rate_hz": rate,
        "channels": channels,
        "sample_count": len(values) // channels,
        "data_base64": base64.b64encode(data).decode("ascii"),
    }


def pcm(result):
    return np.frombuffer(base64.b64decode(result["data_base64"]), dtype="<i2")


class AudioInputTests(unittest.TestCase):
    def test_already_normalized_bytes_remain_exact(self):
        value = audio([-32768, -123, 0, 123, 32767])
        self.assertEqual(normalize_audio(value), value)

    def test_stereo_is_actually_downmixed_and_float_is_quantized(self):
        result = normalize_audio(
            audio([0.5, -0.5, 1.0, 0.0, -1.0, 0.0], channels=2, encoding="pcm_f32le")
        )
        self.assertEqual(result["channels"], 1)
        self.assertEqual(result["sample_count"], 3)
        np.testing.assert_array_equal(pcm(result), [0, 16384, -16384])

    def test_48k_stereo_signal_is_resampled_to_16k_without_relabeling(self):
        rate = 48000
        source = np.sin(2 * np.pi * 1000 * np.arange(rate // 10) / rate) * 0.5
        stereo = np.repeat(source[:, None], 2, axis=1).ravel().tolist()
        result = normalize_audio(audio(stereo, rate=rate, channels=2, encoding="pcm_f32le"))
        self.assertEqual(result["sample_count"], 1600)
        self.assertEqual(len(pcm(result)), 1600)
        expected = np.sin(2 * np.pi * 1000 * np.arange(1600) / 16000) * 16384
        np.testing.assert_allclose(pcm(result)[32:-32], expected[32:-32], atol=8)

    def test_downsampling_filters_above_output_nyquist(self):
        rate = 48000
        source = np.sin(2 * np.pi * 12000 * np.arange(4800) / rate) * 0.5
        result = normalize_audio(audio(source.tolist(), rate=rate, encoding="pcm_f32le"))
        self.assertLess(float(np.max(np.abs(pcm(result)[32:-32]))), 50)

    def test_8k_input_is_actually_upsampled(self):
        result = normalize_audio(audio([1000] * 800, rate=8000))
        self.assertEqual(result["sample_count"], 1600)
        np.testing.assert_array_equal(pcm(result), [1000] * 1600)

    def test_invalid_declarations_and_nonfinite_float_refuse(self):
        valid = audio([0, 1])
        for field, value in [
            ("channels", True),
            ("channels", 3),
            ("sample_rate_hz", 0),
            ("sample_rate_hz", 16000.0),
            ("sample_count", 3),
            ("encoding", "wav"),
            ("data_base64", "??"),
        ]:
            with self.subTest(field=field, value=value):
                with self.assertRaises(AudioInputError):
                    normalize_audio({**valid, field: value})
        for value in [float("nan"), float("inf"), 1.5]:
            with self.assertRaises(AudioInputError):
                normalize_audio(audio([value], encoding="pcm_f32le"))
        with self.assertRaises(AudioInputError):
            normalize_audio({**valid, "path": "/not/authority"})

    def test_duration_is_checked_before_decode(self):
        value = {**audio([0]), "sample_count": 480001}
        with self.assertRaises(AudioInputError):
            normalize_audio(value)


if __name__ == "__main__":
    unittest.main()
