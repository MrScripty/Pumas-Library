"""Private typed projection evidence; no native package/model qualification."""

import base64
import json
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from owned_model_operations import OwnedOperationError, _request


def request_value():
    return {
        "contract_version": 1,
        "request_id": "public-request-17",
        "model": "speech",
        "profile": "torch",
        "capability": "audio_transcription",
        "input": {
            "kind": "audio",
            "encoding": "pcm_f32le",
            "sample_rate_hz": 16000,
            "channels": 2,
            "sample_count": 2,
            "data_base64": base64.b64encode(struct.pack("<ffff", 1, 0, -1, 0)).decode(),
        },
        "output": "text",
        "options": {"kind": "audio", "language": "de", "max_output_tokens": 512},
    }


class TypedProjectionTests(unittest.TestCase):
    def decode(self, value):
        return _request(json.dumps(value).encode())

    def refusal(self, value, code="invalid_request"):
        with self.assertRaises(OwnedOperationError) as caught:
            self.decode(value)
        self.assertEqual(caught.exception.code, code)

    def test_correlated_request_actually_normalizes_pcm_and_language(self):
        value, audio, language = self.decode(request_value())
        self.assertEqual(value["request_id"], "public-request-17")
        self.assertEqual(language, "de")
        self.assertEqual(audio["sample_count"], 2)
        self.assertEqual(audio["channels"], 1)
        self.assertEqual(base64.b64decode(audio["data_base64"]), struct.pack("<hh", 16384, -16384))

    def test_semantic_output_and_supported_option_bounds_are_not_ignored(self):
        value = request_value()
        for capability, output, code in [
            ("audio_transcription", "labels", "invalid_request"),
            ("audio_classification", "text", "invalid_request"),
            ("audio_classification", "labels", "capability_unavailable"),
        ]:
            self.refusal({**value, "capability": capability, "output": output}, code)
        self.refusal(
            {**value, "options": {"kind": "audio", "max_output_tokens": 1}},
            "capability_unavailable",
        )
        for limit in (0, True, -1, 1 << 32):
            self.refusal({**value, "options": {"kind": "audio", "max_output_tokens": limit}})
        self.refusal({**value, "stream": True})

    def test_closed_fields_and_duplicate_keys_cannot_install_authority(self):
        value = request_value()
        for field in ("artifact_manifest", "qualified", "slot", "model_path"):
            self.refusal({**value, field: True})
        self.refusal({**value, "input": {**value["input"], "path": "/claimed/model"}})
        duplicate = json.dumps(value).replace(
            '"contract_version": 1', '"contract_version": 1, "contract_version": 1'
        )
        with self.assertRaises(OwnedOperationError):
            _request(duplicate.encode())

    def test_invalid_bytes_and_unicode_refuse_before_native_admission(self):
        value = request_value()
        self.refusal({**value, "model": "\ud800"})
        self.refusal({**value, "model": "speech\u0080"})
        self.refusal({**value, "profile": "invalid:profile"})
        for change in (
            {"sample_count": 3},
            {"channels": True},
            {"encoding": "wav"},
            {"data_base64": "??"},
        ):
            self.refusal({**value, "input": {**value["input"], **change}})
        self.refusal({**value, "options": {"kind": "audio", "language": "claimed"}})


if __name__ == "__main__":
    unittest.main()
