"""Actual held-file reads and synthetic constructors; no real Cohere execution."""

import copy
import json
import os
from pathlib import Path
import pickle
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _TestModelManager  # noqa: F401 (suite's Torch fallback)
from loaders.cohere_asr_loader import (
    RetainedSpeechAcquisition,
    SpeechRuntimeUnsupported,
    load_cohere_asr_retained,
)
from loaders.owned_cohere_source import (
    HeldCohereReadSource,
    REQUIRED,
    decode_object,
    tokenizer_options,
)


@unittest.skipUnless(sys.platform == "linux", "Held descriptor reader is Linux-only")
class HeldSourceTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.owner = object()
        self.bodies = {name: b"{}" for name in REQUIRED}
        self.bodies["config.json"] = json.dumps(
            {"model_type": "cohere_asr", "architectures": ["CohereAsrForConditionalGeneration"]}
        ).encode()
        self.bodies["model.safetensors"] = b"synthetic weights; no real tensors"
        self.originals = {}
        self.addCleanup(self.close_originals)

    def close_originals(self):
        for fd in self.originals.values():
            os.close(fd)
        self.originals.clear()

    def select(self):
        for name, body in self.bodies.items():
            (self.root / name).write_bytes(body)
            self.originals[name] = os.open(self.root / name, os.O_RDONLY | os.O_NOFOLLOW)
        reader = HeldCohereReadSource._from_members(self.owner, self.originals)
        self.addCleanup(reader.close)
        return reader

    def acquisition(self, reader):
        acquisition = RetainedSpeechAcquisition()
        acquisition.retain("model_read_source", reader)
        return acquisition

    def native(self, report=None):
        calls = {}

        def config(data):
            calls["config"] = data
            return types.SimpleNamespace(kind="config")

        def generation(data):
            calls["generation"] = data
            return types.SimpleNamespace(kind="generation")

        def default_generation(value):
            self.assertEqual(value.kind, "config")
            return generation({"derived_from_selected_config": True})

        def backend(raw):
            calls["tokenizer_json"] = raw
            return types.SimpleNamespace(
                token_to_id=lambda value: {"<eos>": 3, "<pad>": 2}.get(value)
            )

        def tokenizer(**options):
            calls["tokenizer_options"] = options
            return object()

        def weights(filename, *, device):
            self.assertEqual(device, "cpu")
            self.assertTrue(filename.startswith("/proc/self/fd/"))
            with open(filename, "rb") as file:
                calls["weights"] = file.read()
            calls["state_dict"] = {"fixture": object()}
            return calls["state_dict"]

        model = Mock()

        def load(root, **options):
            self.assertIsNone(root)
            self.assertIs(options["state_dict"], calls["state_dict"])
            self.assertEqual(options["config"].kind, "config")
            self.assertEqual(options["generation_config"].kind, "generation")
            self.assertEqual(
                {
                    key: value
                    for key, value in options.items()
                    if key not in {"state_dict", "config", "generation_config"}
                },
                {
                    "local_files_only": True,
                    "trust_remote_code": False,
                    "use_safetensors": True,
                    "device_map": "cpu",
                    "attn_implementation": "eager",
                    "dtype": "auto",
                    "output_loading_info": True,
                },
            )
            return model, report if report is not None else {
                "missing_keys": set(),
                "unexpected_keys": set(),
                "mismatched_keys": set(),
                "error_msgs": [],
            }

        transformer = types.SimpleNamespace(
            CohereAsrConfig=types.SimpleNamespace(from_dict=config),
            GenerationConfig=types.SimpleNamespace(
                from_dict=generation, from_model_config=default_generation
            ),
            CohereAsrFeatureExtractor=types.SimpleNamespace(
                from_dict=lambda data: calls.setdefault("feature", data)
            ),
            CohereAsrProcessor=lambda **kwargs: kwargs,
            TokenizersBackend=tokenizer,
            CohereAsrForConditionalGeneration=types.SimpleNamespace(from_pretrained=load),
            StoppingCriteria=object,
            StoppingCriteriaList=list,
        )
        modules = {
            "transformers": transformer,
            "tokenizers": types.SimpleNamespace(
                Tokenizer=types.SimpleNamespace(from_str=backend), AddedToken=AddedToken
            ),
            "safetensors.torch": types.SimpleNamespace(load_file=weights),
        }
        self.enterContext(patch.dict(sys.modules, modules))
        return calls, model

    def test_original_fds_survive_path_replacement_and_original_owner_close(self):
        reader = self.select()
        replacement = self.root / "replacement"
        replacement.write_bytes(b"cache/path replacement")
        replacement.replace(self.root / "model.safetensors")
        self.close_originals()
        self.assertIs(reader.source_owner, self.owner)
        with open(reader.weights_filename(), "rb") as file:
            self.assertEqual(file.read(), self.bodies["model.safetensors"])
        reader.validate()
        self.assertEqual(reader.read("config.json", 65536), self.bodies["config.json"])

    def test_changed_held_bytes_size_and_closed_reader_refuse(self):
        reader = self.select()
        (self.root / "model.safetensors").write_bytes(b"x" * len(self.bodies["model.safetensors"]))
        with self.assertRaisesRegex(ValueError, "changed"):
            reader.weights_filename()
        (self.root / "config.json").write_bytes(b"longer" * 100)
        with self.assertRaisesRegex(ValueError, "changed"):
            reader.read("config.json", 65536)
        reader.close()
        with self.assertRaisesRegex(ValueError, "closed"):
            reader.validate()
        self.assertIsNone(reader.source_owner)

    def test_read_set_requires_original_read_only_regular_descriptors(self):
        reader = self.select()
        for bad in (
            self.root,
            {**self.originals, "unselected.json": self.originals["config.json"]},
            {name: fd for name, fd in self.originals.items() if name != "model.safetensors"},
        ):
            with self.subTest(bad=type(bad)), self.assertRaises(ValueError):
                HeldCohereReadSource._from_members(self.owner, bad)
        writable = os.open(self.root / "model.safetensors", os.O_RDWR)
        directory = os.open(self.root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            for fd in (writable, directory, -1, "file:///weights"):
                with self.subTest(fd=fd), self.assertRaises(ValueError):
                    HeldCohereReadSource._from_members(
                        self.owner, {**self.originals, "model.safetensors": fd}
                    )
        finally:
            os.close(writable)
            os.close(directory)
        self.assertEqual(reader.names, REQUIRED)

    def test_bounded_reads_and_nontransferable_reader(self):
        reader = self.select()
        with self.assertRaisesRegex(ValueError, "too large"):
            reader.read("config.json", 2)
        with self.assertRaisesRegex(ValueError, "not selected"):
            reader.read("unselected.json", 65536)
        for invoke in (
            lambda: HeldCohereReadSource(),
            lambda: copy.copy(reader),
            lambda: copy.deepcopy(reader),
            lambda: pickle.dumps(reader),
        ):
            with self.assertRaises(TypeError):
                invoke()

    def test_closed_original_is_refused_before_dup_can_reuse_its_number(self):
        self.select()
        closed = self.originals.pop("model.safetensors")
        os.close(closed)
        with self.assertRaises(OSError):
            HeldCohereReadSource._from_members(
                self.owner, {**self.originals, "model.safetensors": closed}
            )

    def test_consumed_loader_uses_held_bytes_and_semantic_optional_descriptors(self):
        self.bodies.update(
            {
                "tokenizer_config.json": b'{"eos_token":"<eos>","added_tokens_decoder":{"3":{"content":"<eos>","special":true}}}',
                "special_tokens_map.json": b'{"pad_token":"<pad>"}',
                "added_tokens.json": b'{"<eos>":3}',
                "generation_config.json": b'{"eos_token_id":3,"pad_token_id":2}',
                "processor_config.json": b'{"processor_class":"CohereAsrProcessor"}',
                "preprocessor_config.json": b'{"sampling_rate":16000}',
            }
        )
        reader = self.select()
        acquisition = self.acquisition(reader)
        calls, model = self.native()
        # A package/cache path fallback must never run in the owning loader.
        with patch(
            "loaders.cohere_asr_loader.validate_installed_package",
            side_effect=AssertionError("package resolution forbidden"),
        ):
            result, processor, kind = load_cohere_asr_retained(reader, "cpu", acquisition)
        self.assertIs(result, model)
        self.assertEqual(kind, "cohere-asr")
        self.assertEqual(calls["weights"], self.bodies["model.safetensors"])
        self.assertEqual(calls["generation"], {"eos_token_id": 3, "pad_token_id": 2})
        self.assertEqual(calls["feature"], {"sampling_rate": 16000})
        self.assertEqual(calls["tokenizer_json"], "{}")
        self.assertEqual(calls["tokenizer_options"]["pad_token"], "<pad>")
        self.assertEqual(calls["tokenizer_options"]["added_tokens_decoder"][3].content, "<eos>")
        model.eval.assert_called_once()
        self.assertIs(acquisition.objects["model"][0], model)
        self.assertIs(acquisition.objects["weights"], calls["state_dict"])
        reader.validate()
        acquisition.clear_after_cleanup()
        self.assertFalse(acquisition.objects)
        with self.assertRaisesRegex(ValueError, "closed"):
            reader.validate()

    def test_path_argument_and_unretained_reader_cannot_select_model(self):
        reader = self.select()
        for source in (self.root, reader):
            with self.assertRaisesRegex(SpeechRuntimeUnsupported, "original held"):
                load_cohere_asr_retained(source, "cpu", RetainedSpeechAcquisition())

    def test_descriptor_redirects_and_unknown_options_fail_before_native_import(self):
        for name, body in (
            (
                "config.json",
                b'{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"],"encoder_config":{"model_type":"other"}}',
            ),
            ("generation_config.json", b'{"custom_generate":"other"}'),
            ("tokenizer_config.json", b'{"tokenizer_file":"/cache/tokenizer.json"}'),
            ("tokenizer_config.json", b'{"chat_template":"unconsumed semantics"}'),
            ("tokenizer_config.json", b'{"model_max_length":true}'),
            ("tokenizer_config.json", b'{"model_max_length":-1}'),
            ("tokenizer_config.json", b'{"model_max_length":"512"}'),
            ("tokenizer_config.json", b'{"padding_side":"any"}'),
            ("tokenizer_config.json", b'{"truncation_side":1}'),
            ("tokenizer_config.json", b'{"split_special_tokens":"false"}'),
            ("tokenizer_config.json", b'{"eos_token":{"content":42}}'),
            ("tokenizer_config.json", b'{"added_tokens_decoder":{"3":"<eos>"}}'),
            ("processor_config.json", b'{"unknown_option":true}'),
            ("special_tokens_map.json", b'{"vocab_file":"/cache/vocab"}'),
        ):
            with self.subTest(name=name, body=body), tempfile.TemporaryDirectory() as folder:
                original_root, original_bodies = self.root, self.bodies
                self.root, self.bodies = Path(folder), {**original_bodies, name: body}
                reader = self.select()
                acquisition = self.acquisition(reader)
                with patch(
                    "loaders.cohere_asr_loader._native_api",
                    side_effect=AssertionError("native import forbidden"),
                ) as api:
                    with self.assertRaises(ValueError):
                        load_cohere_asr_retained(reader, "cpu", acquisition)
                    api.assert_not_called()
                self.assertFalse(acquisition.unknown_allocations)
                acquisition.clear_after_cleanup()
                self.close_originals()
                self.root, self.bodies = original_root, original_bodies

    def test_incomplete_loading_report_retains_returned_model_before_refusal(self):
        reader = self.select()
        for field in (
            "missing_keys",
            "unexpected_keys",
            "mismatched_keys",
            "error_msgs",
            "conversion_errors",
            "unrecognized",
        ):
            with self.subTest(field=field):
                report = {
                    "missing_keys": [],
                    "unexpected_keys": [],
                    "mismatched_keys": [],
                    "error_msgs": [],
                }
                report[field] = ["incomplete"]
                calls, model = self.native(report)
                acquisition = self.acquisition(reader)
                with self.assertRaisesRegex(SpeechRuntimeUnsupported, "incomplete"):
                    load_cohere_asr_retained(reader, "cpu", acquisition)
                self.assertIs(acquisition.objects["model"][0], model)
                self.assertIs(acquisition.objects["model_read_source"], reader)
                self.assertIs(acquisition.objects["weights"], calls["state_dict"])
                self.assertFalse(acquisition.unknown_allocations)
                model.eval.assert_not_called()
                reader.validate()

    def test_constructor_uncertainty_keeps_reader_and_partial_native_references(self):
        reader = self.select()
        self.native()
        acquisition = self.acquisition(reader)
        with patch.object(
            sys.modules["safetensors.torch"],
            "load_file",
            side_effect=RuntimeError("native mapping uncertainty"),
        ):
            with self.assertRaisesRegex(RuntimeError, "uncertainty"):
                load_cohere_asr_retained(reader, "cpu", acquisition)
        self.assertTrue(acquisition.unknown_allocations)
        self.assertEqual(acquisition.in_flight, "weights")
        self.assertIn("tokenizer_backend", acquisition.objects)
        self.assertIs(acquisition.objects["model_read_source"], reader)
        with self.assertRaisesRegex(SpeechRuntimeUnsupported, "unconfirmed"):
            acquisition.clear_after_cleanup()
        reader.validate()


class AddedToken:
    def __init__(self, content, **options):
        self.content, self.options = content, options

    def __str__(self):
        return self.content


class TokenizerDescriptorTests(unittest.TestCase):
    def test_duplicate_and_nonfinite_json_cannot_override_selected_fields(self):
        for raw in (
            b'{"a":1,"a":2}',
            b'{"nested":{"x":1,"x":2}}',
            b'{"a":NaN}',
            b'{"a":1e999}',
            b"[]",
        ):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                decode_object(raw)

    def test_special_token_conflicts_unknown_ids_and_unsupported_properties_refuse(self):
        backend = types.SimpleNamespace(token_to_id=lambda value: {"<eos>": 3}.get(value))
        cases = (
            {"eos_token": "absent"},
            {"additional_special_tokens": [None]},
            {"added_tokens_decoder": {"03": "<eos>"}},
            {"added_tokens_decoder": {"4": "<eos>"}},
            {"eos_token": {"content": "<eos>", "lstrip": "true"}},
            {"eos_token": {"content": "<eos>", "path": "/cache"}},
            {"additional_special_tokens": ["<eos>"], "extra_special_tokens": []},
        )
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                tokenizer_options({"tokenizer_config.json": value}, backend, AddedToken)
        with self.assertRaisesRegex(ValueError, "conflict"):
            tokenizer_options(
                {
                    "tokenizer_config.json": {"eos_token": "<eos>"},
                    "special_tokens_map.json": {"eos_token": "other"},
                },
                backend,
                AddedToken,
            )
        self.assertEqual(
            tokenizer_options({"tokenizer_config.json": {"pad_token": None}}, backend, AddedToken),
            {"pad_token": None},
        )


if __name__ == "__main__":
    unittest.main()
