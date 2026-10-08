"""Synthetic boundary evidence; does not qualify installed Cohere weights."""

import contextlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import threading
import types
import unittest
import numpy as np
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
# Reuse the suite's minimal device fallback; no native runtime is installed here.
from test_model_manager import _FakeDeviceManager, _TestModelManager
from speech_binding import SpeechBindingError
from speech_fixtures import SyntheticArtifactAuthority, bind_fixture
from loaders.cohere_asr_loader import (
    COHERE_ASR,
    MAX_AUDIO_SAMPLES,
    MAX_TEXT_BYTES,
    SpeechCancelled,
    SpeechCleanupUnconfirmed,
    SpeechRuntimeUnsupported,
    load_cohere_asr,
    transcribe,
    transcribe_detailed,
    validate_installed_package,
)


class Inputs(dict):
    def __init__(self, **kwargs):
        super().__init__(kwargs)
        self.setdefault("audio_chunk_index", [(0, None)])
        self.setdefault("input_features", np.zeros((1, 2, 1), dtype=np.float32))

    def to(self, *args, **kwargs):
        return self


def fake_torch(**kwargs):
    return types.SimpleNamespace(
        inference_mode=contextlib.nullcontext,
        is_tensor=lambda value: isinstance(value, np.ndarray),
        is_floating_point=lambda value: np.issubdtype(value.dtype, np.floating),
        isfinite=np.isfinite,
        **kwargs,
    )


class LoaderTests(unittest.TestCase):
    def setUp(self):
        self.folder = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(self.folder).resolve()
        (self.root / "config.json").write_text(
            json.dumps(
                {"model_type": "cohere_asr", "architectures": ["CohereAsrForConditionalGeneration"]}
            )
        )
        (self.root / "model.safetensors").write_bytes(b"fixture-only")

    def test_native_local_loader_forbids_remote_code_and_downloads(self):
        processor, model_class = Mock(), Mock()
        with patch(
            "loaders.cohere_asr_loader._native_api",
            return_value=(processor, model_class, object, list),
        ):
            model, _, kind = load_cohere_asr(self.root, "cpu")
        self.assertEqual(kind, COHERE_ASR)
        processor.from_pretrained.assert_called_once_with(
            str(self.root), local_files_only=True, trust_remote_code=False
        )
        model_class.from_pretrained.assert_called_once_with(
            str(self.root),
            local_files_only=True,
            trust_remote_code=False,
            use_safetensors=True,
            device_map="cpu",
        )
        model.eval.assert_called_once()

    def test_wrong_architecture_and_external_weights_rejected_before_loading(self):
        (self.root / "config.json").write_text('{"model_type":"llama"}')
        with self.assertRaisesRegex(ValueError, "native Cohere"):
            validate_installed_package(self.root)
        (self.root / "model.safetensors").unlink()
        (self.root / "model.safetensors").symlink_to(Path(__file__).resolve())
        with self.assertRaisesRegex(ValueError, "local config"):
            validate_installed_package(self.root)

    def test_dispatch_detects_native_cohere_without_text_loader(self):
        from loaders import load_model

        with patch(
            "loaders.cohere_asr_loader.load_cohere_asr",
            return_value=("model", "processor", COHERE_ASR),
        ) as loader:
            self.assertEqual(load_model(str(self.root), "cpu"), ("model", "processor", COHERE_ASR))
        loader.assert_called_once_with(self.root, "cpu")

    def test_mps_rejected_before_loading(self):
        with patch("loaders.cohere_asr_loader._native_api") as native:
            with self.assertRaises(SpeechRuntimeUnsupported):
                load_cohere_asr(self.root, "mps")
            native.assert_not_called()

    def test_old_runtime_is_explicitly_unsupported(self):
        with patch.dict(sys.modules, {"transformers": types.ModuleType("transformers")}):
            with self.assertRaises(SpeechRuntimeUnsupported):
                load_cohere_asr(self.root, "cpu")

    def test_descriptor_redirects_refuse_before_native_library_loading(self):
        selectors = (
            "audio_tokenizer",
            "audio_tokenizer_name_or_path",
            "tokenizer_file",
            "vocab_file",
            "merges_file",
            "spm_file",
            "fast_tokenizer_files",
            "auto_map",
            "custom_pipelines",
            "vocab",
            "merges",
        )
        with patch("loaders.cohere_asr_loader._native_api") as native:
            for name in (
                "tokenizer_config.json",
                "processor_config.json",
                "preprocessor_config.json",
            ):
                file = self.root / name
                for key in selectors:
                    for nested in (False, True):
                        value = {key: "/unselected/secondary"}
                        if nested:
                            value = {"feature_extractor": value}
                        file.write_text(json.dumps(value))
                        with self.subTest(file=name, key=key, nested=nested):
                            with self.assertRaisesRegex(ValueError, "redirects"):
                                load_cohere_asr(self.root, "cpu")
                file.unlink()
            native.assert_not_called()

    def test_official_remote_code_descriptor_remains_explicitly_unsupported(self):
        config = json.loads((self.root / "config.json").read_text())
        config["auto_map"] = {"AutoConfig": "configuration_cohere_asr.CohereAsrConfig"}
        (self.root / "config.json").write_text(json.dumps(config))
        with patch("loaders.cohere_asr_loader._native_api") as native:
            with self.assertRaisesRegex(ValueError, "redirects"):
                load_cohere_asr(self.root, "cpu")
            native.assert_not_called()

    def test_descriptor_bounds_malformed_and_linked_files_refuse_before_loading(self):
        file = self.root / "processor_config.json"
        with patch("loaders.cohere_asr_loader._native_api") as native:
            for data in (b"{}" + b" " * 65535, b"[]", b"not-json", b"\xff"):
                file.write_bytes(data)
                with self.subTest(data=data[:8]), self.assertRaises(ValueError):
                    load_cohere_asr(self.root, "cpu")
            file.unlink()
            file.symlink_to(self.root / "config.json")
            with self.assertRaisesRegex(ValueError, "regular file"):
                load_cohere_asr(self.root, "cpu")
            native.assert_not_called()

    def test_bounded_native_descriptor_metadata_remains_local_and_reusable(self):
        file = self.root / "processor_config.json"
        body = b'{"processor_class":"CohereAsrProcessor"}'
        file.write_bytes(body + b" " * (65536 - len(body)))
        self.assertEqual(file.stat().st_size, 65536)
        self.assertEqual(validate_installed_package(self.root), self.root)

    def test_unsupported_native_class_overrides_refuse_before_loading(self):
        file = self.root / "processor_config.json"
        selectors = {
            "processor_class": "WhisperProcessor",
            "feature_extractor_class": "WhisperFeatureExtractor",
            "feature_extractor_type": "WhisperFeatureExtractor",
            "tokenizer_class": "CohereASRTokenizer",
            "image_processor_class": "UnqualifiedImageProcessor",
        }
        with patch("loaders.cohere_asr_loader._native_api") as native:
            for key, selected in selectors.items():
                file.write_text(json.dumps({key: selected}))
                with self.subTest(key=key), self.assertRaisesRegex(ValueError, "class"):
                    load_cohere_asr(self.root, "cpu")
            native.assert_not_called()
        file.write_text(
            json.dumps(
                {
                    "processor_class": "CohereAsrProcessor",
                    "feature_extractor_type": "CohereAsrFeatureExtractor",
                    "tokenizer_class": "TokenizersBackend",
                }
            )
        )
        self.assertEqual(validate_installed_package(self.root), self.root)


class TranscriptionTests(unittest.TestCase):
    def setUp(self):
        self.cancel = threading.Event()
        self.processor = Mock(return_value=Inputs())
        self.processor.decode.return_value = "  Test transcript.  "
        self.model = Mock(device="cpu", dtype="float32")
        self.model.generate.return_value = [[1, 2]]
        self.enterContext(
            patch("loaders.cohere_asr_loader._native_api", return_value=(None, None, object, list))
        )
        self.enterContext(patch.dict(sys.modules, {"torch": fake_torch()}))
        self.audio = struct.pack("<hhh", -32768, 0, 32767)

    def call(self):
        return transcribe(self.model, self.processor, self.audio, "en", self.cancel)

    def detailed(self):
        self.model.config = types.SimpleNamespace(is_encoder_decoder=True)
        self.model.generation_config = types.SimpleNamespace(
            eos_token_id=0,
            decoder_start_token_id=7,
            forced_eos_token_id=None,
        )
        return transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)

    def test_detailed_stop_and_token_bound_preserve_legacy_text(self):
        self.model.generate.return_value = [[7, 3, 0]]
        result = self.detailed()
        self.assertEqual((result.text, result.finish_reason), ("Test transcript.", "stop"))
        self.assertIs(type(self.call()), str)
        self.model.generate.return_value = [[7, *([1] * 512)]]
        self.assertEqual(self.detailed().finish_reason, "length")

    def test_detailed_missing_terminal_wrong_prompt_and_early_eos_refuse(self):
        for tokens in ([7, 1], [8, 0], [7, 0, 1], [7, *([1] * 513)]):
            with self.subTest(tokens=tokens), self.assertRaises(SpeechRuntimeUnsupported):
                self.model.generate.return_value = [tokens]
                self.detailed()
        self.processor.decode.assert_not_called()

    def test_detailed_forced_eos_and_unknown_decoder_semantics_refuse_before_generation(self):
        self.model.config = types.SimpleNamespace(is_encoder_decoder=False)
        with self.assertRaises(SpeechRuntimeUnsupported):
            transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)
        self.model.config = types.SimpleNamespace(is_encoder_decoder=True)
        self.model.generation_config = types.SimpleNamespace(
            eos_token_id=0,
            decoder_start_token_id=7,
            forced_eos_token_id=0,
        )
        with self.assertRaises(SpeechRuntimeUnsupported):
            transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)
        self.model.generate.assert_not_called()

    def test_official_single_chunk_metadata_removed_before_generate(self):
        class NativeInputs(Inputs):
            def to(inputs, device, dtype):
                self.assertIn("audio_chunk_index", inputs)
                self.assertEqual((device, dtype), ("cpu", "float32"))
                return inputs

        for chunk in (None, 0):
            inputs = NativeInputs(
                input_features=np.zeros((1, 2, 1), dtype=np.float32),
                attention_mask="bool-mask",
                decoder_input_ids=[[7, 8]],
                audio_chunk_index=[(0, chunk)],
            )
            self.processor.return_value = inputs

            def generate(**kwargs):
                self.assertNotIn("audio_chunk_index", kwargs)
                self.assertEqual(kwargs["attention_mask"], "bool-mask")
                self.assertEqual(kwargs["decoder_input_ids"], [[7, 8]])
                return [[7, 8, 1, 0]]

            self.model.generate.side_effect = generate
            result = self.detailed()
            self.assertEqual((result.text, result.finish_reason), ("Test transcript.", "stop"))
        self.processor.decode.assert_called_with([7, 8, 1, 0], skip_special_tokens=True)

    def test_native_handoff_preserves_integer_prompt_mask_and_single_sequence_decode_shape(self):
        class NativeInputs(Inputs):
            def to(inputs, device, dtype):
                self.assertEqual((device, dtype), ("cpu", np.float16))
                for key, value in inputs.items():
                    if isinstance(value, np.ndarray) and np.issubdtype(value.dtype, np.floating):
                        inputs[key] = value.astype(dtype)
                return inputs

        self.model.dtype = np.float16
        self.processor.return_value = NativeInputs(
            input_features=np.zeros((1, 2, 1), dtype=np.float32),
            attention_mask=np.array([[True, True]], dtype=np.bool_),
            decoder_input_ids=np.array([[8, 6]], dtype=np.int64),
        )
        self.model.config = types.SimpleNamespace(is_encoder_decoder=True)
        self.model.generation_config = types.SimpleNamespace(
            eos_token_id=0,
            decoder_start_token_id=7,
            forced_eos_token_id=None,
        )

        def generate(**kwargs):
            self.assertEqual(kwargs["input_features"].dtype, np.float16)
            self.assertEqual(kwargs["attention_mask"].dtype, np.bool_)
            self.assertEqual(kwargs["decoder_input_ids"].dtype, np.int64)
            self.assertNotIn("audio_chunk_index", kwargs)
            return np.array([[7, 8, 6, 1, 0]], dtype=np.int64)

        def decode(sequence, **kwargs):
            self.assertEqual(sequence.shape, (5,))
            self.assertEqual(sequence.dtype, np.int64)
            self.assertEqual(kwargs, {"skip_special_tokens": True})
            return "native shape"

        self.model.generate.side_effect = generate
        self.processor.decode.side_effect = decode
        result = transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)
        self.assertEqual((result.text, result.finish_reason), ("native shape", "stop"))

    def test_invalid_or_nonfinite_features_refuse_before_generation(self):
        cases = (
            np.full((1, 2, 1), np.nan, dtype=np.float32),
            np.full((1, 2, 1), np.inf, dtype=np.float32),
            np.zeros((1, 2, 1), dtype=np.int64),
            np.zeros((2, 2, 1), dtype=np.float32),
            np.zeros((1, 0, 1), dtype=np.float32),
            np.zeros((2, 1), dtype=np.float32),
            None,
        )
        for value in cases:
            self.processor.return_value = Inputs(input_features=value)
            with (
                self.subTest(features=value),
                self.assertRaisesRegex(SpeechRuntimeUnsupported, "features"),
            ):
                self.call()
        self.model.generate.assert_not_called()
        self.processor.decode.assert_not_called()

    def test_dtype_transfer_overflow_refuses_before_generation(self):
        class NativeInputs(Inputs):
            def to(inputs, *args, **kwargs):
                with np.errstate(over="ignore"):
                    inputs["input_features"] = inputs["input_features"].astype(np.float16)
                return inputs

        self.model.dtype = np.float16
        self.processor.return_value = NativeInputs(
            input_features=np.full((1, 2, 1), 100000, dtype=np.float32)
        )
        with self.assertRaisesRegex(SpeechRuntimeUnsupported, "features"):
            self.call()
        self.model.generate.assert_not_called()

    def test_cancel_during_tensor_transfer_never_starts_generation(self):
        class NativeInputs(Inputs):
            def to(inputs, *args, **kwargs):
                self.cancel.set()
                return inputs

        self.processor.return_value = NativeInputs()
        with self.assertRaises(SpeechCancelled):
            self.call()
        self.model.generate.assert_not_called()

    def test_minuscule_pcm_with_nonfinite_native_normalization_refuses_and_clears(self):
        captured = []

        def process(audio, **kwargs):
            captured.append(audio)
            # The pinned extractor divides by floor(samples / hop_length) and
            # by that count minus one. This controls those nonfinite outputs;
            # it does not execute native feature extraction or model inference.
            frames = len(audio) // 160
            with np.errstate(divide="ignore", invalid="ignore"):
                mean = np.divide(np.float32(0), np.float32(frames))
                variance = np.divide(np.float32(0), np.float32(frames - 1))
                feature = np.divide(mean, np.sqrt(variance))
            return Inputs(input_features=np.full((1, 2, 1), feature, dtype=np.float32))

        self.processor.side_effect = process
        for samples in (1, 159, 160, 319):
            with (
                self.subTest(samples=samples),
                self.assertRaisesRegex(SpeechRuntimeUnsupported, "features"),
            ):
                transcribe(self.model, self.processor, b"\x01\x00" * samples, "en", self.cancel)
        self.model.generate.assert_not_called()
        self.assertTrue(all(not audio.any() for audio in captured))

    def test_missing_malformed_or_multiple_chunks_refuse_before_generate_and_clear_audio(self):
        invalid = (
            None,
            [],
            [(1, None)],
            [(False, None)],
            [(0, False)],
            [(0, 1)],
            [(0, None, 2)],
            [(0, 0), (0, 1)],
        )
        captured = []
        for chunks in invalid:

            def process(audio, **kwargs):
                captured.append(audio)
                return Inputs(audio_chunk_index=chunks)

            self.processor.side_effect = process
            with (
                self.subTest(chunks=chunks),
                self.assertRaisesRegex(SpeechRuntimeUnsupported, "chunk"),
            ):
                self.call()
        inputs = Inputs()
        del inputs["audio_chunk_index"]
        self.processor.side_effect = None
        self.processor.return_value = inputs
        with self.assertRaisesRegex(SpeechRuntimeUnsupported, "chunk"):
            self.call()
        self.model.generate.assert_not_called()
        self.processor.decode.assert_not_called()
        self.assertTrue(all(not audio.any() for audio in captured))

    def test_detailed_effective_prompt_accounts_for_start_prepend_and_bos_fallback(self):
        self.model.config = types.SimpleNamespace(is_encoder_decoder=True)
        for start, bos, raw, effective in (
            (7, 9, [8, 6], [7, 8, 6]),
            (7, 9, [7, 8], [7, 8]),
            (None, 9, [8, 6], [9, 8, 6]),
            (None, 9, None, [9]),
        ):
            self.model.generation_config = types.SimpleNamespace(
                eos_token_id=0,
                decoder_start_token_id=start,
                bos_token_id=bos,
                forced_eos_token_id=None,
            )
            kwargs = {} if raw is None else {"decoder_input_ids": [raw]}
            self.processor.return_value = Inputs(**kwargs)
            self.model.generate.return_value = [[*effective, 1, 0]]
            with self.subTest(start=start, bos=bos, raw=raw):
                result = transcribe_detailed(
                    self.model, self.processor, self.audio, "en", self.cancel
                )
                self.assertEqual(result.finish_reason, "stop")
                self.processor.return_value = Inputs(**kwargs)
                self.model.generate.return_value = [[*effective, *([1] * 512)]]
                self.assertEqual(
                    transcribe_detailed(
                        self.model, self.processor, self.audio, "en", self.cancel
                    ).finish_reason,
                    "length",
                )

    def test_detailed_invalid_start_refuses_and_raw_prefix_cannot_mask_wrong_lineage(self):
        self.model.config = types.SimpleNamespace(is_encoder_decoder=True)
        for start in (None, True, -1, [7]):
            self.model.generation_config = types.SimpleNamespace(
                eos_token_id=0,
                decoder_start_token_id=start,
                forced_eos_token_id=None,
            )
            self.processor.return_value = Inputs(decoder_input_ids=[[8, 6]])
            with self.subTest(start=start), self.assertRaises(SpeechRuntimeUnsupported):
                transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)
        self.model.generate.assert_not_called()
        self.model.generation_config.decoder_start_token_id = 7
        self.model.generate.return_value = [[8, 6, 1, 0]]
        self.processor.return_value = Inputs(decoder_input_ids=[[8, 6]])
        with self.assertRaisesRegex(SpeechRuntimeUnsupported, "lineage"):
            transcribe_detailed(self.model, self.processor, self.audio, "en", self.cancel)
        self.processor.decode.assert_not_called()

    def test_pcm_conversion_language_token_bound_and_owned_buffer_clear(self):
        captured = []

        def process(audio, **kwargs):
            self.assertEqual(
                kwargs, {"sampling_rate": 16000, "return_tensors": "pt", "language": "en"}
            )
            self.assertEqual(audio.tolist(), [-1.0, 0.0, 32767 / 32768])
            self.assertEqual(audio.dtype, np.float32)
            captured.append(audio)
            return Inputs()

        self.processor.side_effect = process
        self.assertEqual(self.call(), "Test transcript.")
        self.processor.decode.assert_called_once_with([1, 2], skip_special_tokens=True)
        self.assertEqual(self.model.generate.call_args.kwargs["max_new_tokens"], 512)
        self.assertEqual(captured[0].tolist(), [0.0, 0.0, 0.0])

    def test_audio_and_language_rejected_before_processor(self):
        for audio in (b"", b"x", b"x" * (MAX_AUDIO_SAMPLES * 2 + 2), bytearray(b"xx")):
            with self.subTest(length=len(audio)), self.assertRaises(ValueError):
                transcribe(self.model, self.processor, audio, "en", self.cancel)
        for language in ("auto", "EN", "", None):
            with self.subTest(language=language), self.assertRaises(ValueError):
                transcribe(self.model, self.processor, self.audio, language, self.cancel)
        self.processor.assert_not_called()

    def test_generation_failure_clears_conversion_and_synchronizes_cuda(self):
        captured = []

        def process(audio, **kwargs):
            captured.append(audio)
            return Inputs()

        self.processor.side_effect = process
        self.model.device = types.SimpleNamespace(type="cuda")
        self.model.generate.side_effect = RuntimeError("fixture failure")
        sync = Mock()
        with patch.dict(
            sys.modules,
            {
                "torch": fake_torch(
                    cuda=types.SimpleNamespace(synchronize=sync),
                )
            },
        ):
            with self.assertRaisesRegex(RuntimeError, "fixture failure"):
                self.call()
        self.assertEqual(captured[0].tolist(), [0.0, 0.0, 0.0])
        sync.assert_called_once_with(self.model.device)

    def test_unconfirmed_cuda_cleanup_preserves_primary_error_and_buffer(self):
        self.model.device = types.SimpleNamespace(type="cuda")
        primary = RuntimeError("generation failure")
        cleanup = RuntimeError("device completion unknown")
        self.model.generate.side_effect = primary
        with patch.dict(
            sys.modules,
            {
                "torch": fake_torch(
                    cuda=types.SimpleNamespace(synchronize=Mock(side_effect=cleanup)),
                )
            },
        ):
            with self.assertRaises(SpeechCleanupUnconfirmed) as caught:
                self.call()
        self.assertIs(caught.exception.original_error, primary)
        self.assertIs(caught.exception.cleanup_error, cleanup)
        self.assertEqual(caught.exception.retained_audio[0], -1.0)
        # Simulate the owner's eventual cleanup after process cessation.
        caught.exception.retained_audio.fill(0)

    def test_pre_cancel_never_touches_processor(self):
        self.cancel.set()
        with self.assertRaises(SpeechCancelled):
            self.call()
        self.processor.assert_not_called()

    def test_cancel_during_preprocessing_never_generates(self):
        def process(*args, **kwargs):
            self.cancel.set()
            return Inputs()

        self.processor.side_effect = process
        with self.assertRaises(SpeechCancelled):
            self.call()
        self.model.generate.assert_not_called()

    def test_cancel_during_generate_discards_output(self):
        def generate(**kwargs):
            self.cancel.set()
            self.assertTrue(kwargs["stopping_criteria"][0](None, None))
            return [[1]]

        self.model.generate.side_effect = generate
        with self.assertRaises(SpeechCancelled):
            self.call()
        self.processor.decode.assert_not_called()

    def test_cancel_after_decode_and_oversize_output_rejected(self):
        def decode(*args, **kwargs):
            self.cancel.set()
            return "late"

        self.processor.decode.side_effect = decode
        with self.assertRaises(SpeechCancelled):
            self.call()
        self.cancel.clear()
        self.processor.decode.side_effect = None
        self.processor.decode.return_value = "語" * (MAX_TEXT_BYTES // 3 + 1)
        with self.assertRaisesRegex(ValueError, "oversized"):
            self.call()


class SpeechLeaseTests(unittest.IsolatedAsyncioTestCase):
    async def test_lease_blocks_cross_model_inference_and_unload(self):
        authority = SyntheticArtifactAuthority()
        manager = _TestModelManager(_FakeDeviceManager(), _speech_artifact_authority=authority)
        slot = await manager.load("/fixture", "speech", model_type=COHERE_ASR)
        other = await manager.load("/fixture2", "speech2", model_type=COHERE_ASR)
        binding = bind_fixture(
            manager, authority.attest_fixture(manager.speech_slot_ref(slot.slot_id))
        )
        second = bind_fixture(
            manager, authority.attest_fixture(manager.speech_slot_ref(other.slot_id))
        )
        async with manager.speech_lease(binding) as loaded:
            self.assertIs(loaded, slot._loaded)
            with self.assertRaisesRegex(RuntimeError, "busy"):
                async with manager.speech_lease(second):
                    self.fail("device shared")
            with self.assertRaisesRegex(RuntimeError, "busy"):
                await manager.unload(slot.slot_id)
            with self.assertRaisesRegex(RuntimeError, "busy"):
                await manager.unload(other.slot_id)
        self.assertTrue(binding.released)
        second.release()
        await manager.unload(slot.slot_id)
        await manager.unload(other.slot_id)

    async def test_wrong_model_refused_and_duplicate_names_require_exact_identity(self):
        authority = SyntheticArtifactAuthority()
        manager = _TestModelManager(_FakeDeviceManager(), _speech_artifact_authority=authority)
        text = await manager.load("/fixture", "text", model_type="text-generation")
        with self.assertRaisesRegex(SpeechBindingError, "model_unsupported"):
            bind_fixture(manager, manager.speech_slot_ref(text.slot_id))
        first = await manager.load("/fixture", "duplicate", model_type=COHERE_ASR)
        second = await manager.load("/fixture2", "duplicate", model_type=COHERE_ASR)
        for slot in (first, second):
            ref = authority.attest_fixture(manager.speech_slot_ref(slot.slot_id))
            binding = bind_fixture(manager, ref)
            async with manager.speech_lease(binding) as loaded:
                self.assertIs(loaded, slot._loaded)
            self.assertTrue(binding.released)
        with self.assertRaisesRegex(SpeechBindingError, "invalid_slot_ref"):
            async with manager.speech_lease("duplicate"):
                self.fail("name-only model accepted")


if __name__ == "__main__":
    unittest.main()
