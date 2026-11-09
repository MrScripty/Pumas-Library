"""Cohere ASR loader and transcription with synthetic native classes.

No Transformers 5 or model weights are installed here; these tests pin the
loader's local-only loading, request validation, cancellation and finish
reasons.
"""

import contextlib
import json
import sys
import tempfile
import threading
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from loaders.cohere_asr_loader import (  # noqa: E402
    COHERE_ASR,
    MAX_AUDIO_SAMPLES,
    MAX_NEW_TOKENS,
    MAX_TEXT_BYTES,
    SpeechCancelled,
    SpeechRuntimeUnsupported,
    load_cohere_asr,
    transcribe,
    validate_installed_package,
)

EOS = 3


class Inputs(dict):
    def __init__(self, **kwargs):
        super().__init__(kwargs)
        self.setdefault("audio_chunk_index", [(0, None)])
        self.setdefault("input_features", np.zeros((1, 2, 1), dtype=np.float32))

    def to(self, *args, **kwargs):
        return self


class StoppingCriteria:
    pass


def native_api(model_class=None):
    processor_class = Mock()
    model_class = model_class or Mock()
    return Mock(return_value=(processor_class, model_class, StoppingCriteria, list))


def fake_model(sequence, device="cpu"):
    model = Mock()
    model.device = types.SimpleNamespace(type=device)
    model.dtype = "float32"
    model.generation_config = types.SimpleNamespace(eos_token_id=EOS)
    model.generate = Mock(return_value=np.array([sequence]))
    return model


def fake_processor(text="hello world"):
    processor = Mock(return_value=Inputs())
    processor.batch_decode = Mock(return_value=[f" {text} "])
    return processor


fake_torch = types.SimpleNamespace(
    inference_mode=contextlib.nullcontext,
    cuda=types.SimpleNamespace(synchronize=Mock()),
)


class LoaderTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        (self.root / "config.json").write_text(
            json.dumps(
                {"model_type": "cohere_asr", "architectures": ["CohereAsrForConditionalGeneration"]}
            )
        )
        (self.root / "model.safetensors").write_bytes(b"weights")

    def test_loads_from_local_files_without_repository_code(self):
        api = native_api()
        with patch("loaders.cohere_asr_loader._native_api", api):
            model, processor, kind = load_cohere_asr(self.root, "cpu")
        processor_class, model_class, _, _ = api.return_value
        self.assertEqual(kind, COHERE_ASR)
        processor_class.from_pretrained.assert_called_once_with(
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

    def test_wrong_model_or_missing_weights_refuse_with_a_reason(self):
        (self.root / "config.json").write_text(json.dumps({"model_type": "whisper"}))
        with self.assertRaisesRegex(ValueError, "not a Cohere ASR model"):
            validate_installed_package(self.root)
        (self.root / "model.safetensors").unlink()
        with self.assertRaisesRegex(ValueError, "model.safetensors"):
            validate_installed_package(self.root)

    def test_dispatch_routes_cohere_configs_to_this_loader(self):
        from loaders import load_model

        with patch(
            "loaders.cohere_asr_loader.load_cohere_asr", return_value=("m", "p", COHERE_ASR)
        ) as load:
            self.assertEqual(load_model(str(self.root), "cpu", None)[2], COHERE_ASR)
        load.assert_called_once()

    def test_unsupported_device_and_old_transformers_say_why(self):
        with self.assertRaisesRegex(SpeechRuntimeUnsupported, "CPU or CUDA"):
            load_cohere_asr(self.root, "mps")
        with patch.dict(sys.modules, {"transformers": types.ModuleType("transformers")}):
            with self.assertRaisesRegex(SpeechRuntimeUnsupported, "Transformers >=5.4"):
                load_cohere_asr(self.root, "cpu")


class TranscriptionTests(unittest.TestCase):
    def setUp(self):
        self.enterContext(patch("loaders.cohere_asr_loader._native_api", native_api()))
        self.enterContext(patch.dict(sys.modules, {"torch": fake_torch}))
        self.pcm = np.zeros(1600, dtype="<i2").tobytes()

    def test_returns_text_and_stop_when_generation_ends_on_eos(self):
        model, processor = fake_model([0, 7, 8, EOS]), fake_processor()
        self.assertEqual(
            transcribe(model, processor, self.pcm, "en", threading.Event()),
            ("hello world", "stop"),
        )
        kwargs = model.generate.call_args.kwargs
        self.assertNotIn("audio_chunk_index", kwargs)
        self.assertEqual(kwargs["max_new_tokens"], MAX_NEW_TOKENS)
        self.assertEqual(processor.call_args.kwargs["language"], "en")

    def test_reports_length_when_the_token_bound_is_reached(self):
        model = fake_model([0, *([7] * MAX_NEW_TOKENS)])
        self.assertEqual(
            transcribe(model, fake_processor(), self.pcm, "de", threading.Event())[1], "length"
        )

    def test_reaching_the_bound_reports_length_even_when_the_last_token_is_eos(self):
        model = fake_model([0, *([7] * (MAX_NEW_TOKENS - 1)), EOS])
        self.assertEqual(
            transcribe(model, fake_processor(), self.pcm, "en", threading.Event())[1], "length"
        )
        # The decoder prompt is not generated output.
        processor = fake_processor()
        processor.return_value = Inputs(decoder_input_ids=np.array([[0, 5, 6]]))
        model = fake_model([0, 5, 6, *([7] * (MAX_NEW_TOKENS - 2)), EOS])
        self.assertEqual(transcribe(model, processor, self.pcm, "en", threading.Event())[1], "stop")

    def test_cuda_work_is_synchronized_when_generation_fails_or_is_cancelled(self):
        for failing in (True, False):
            fake_torch.cuda.synchronize.reset_mock()
            cancel = threading.Event()
            model = fake_model([0, EOS], device="cuda")
            if failing:
                model.generate.side_effect = RuntimeError("kernel failure")
            else:
                model.generate.side_effect = lambda **_: cancel.set() or np.array([[0, EOS]])
            with self.assertRaises(RuntimeError if failing else SpeechCancelled):
                transcribe(model, fake_processor(), self.pcm, "en", cancel)
            fake_torch.cuda.synchronize.assert_called_once()
        # A synchronize failure never replaces the original error.
        fake_torch.cuda.synchronize.side_effect = RuntimeError("sync")
        try:
            model = fake_model([0, EOS], device="cuda")
            model.generate.side_effect = ValueError("original")
            with self.assertRaisesRegex(ValueError, "original"):
                transcribe(model, fake_processor(), self.pcm, "en", threading.Event())
        finally:
            fake_torch.cuda.synchronize.side_effect = None

    def test_invalid_audio_and_language_are_refused_before_the_processor(self):
        model, processor = fake_model([0, EOS]), fake_processor()
        for pcm in (b"", b"\x00", b"\x00" * (MAX_AUDIO_SAMPLES * 2 + 2), "text"):
            with self.subTest(pcm=type(pcm)), self.assertRaises(ValueError):
                transcribe(model, processor, pcm, "en", threading.Event())
        with self.assertRaisesRegex(ValueError, "language must be one of"):
            transcribe(model, processor, self.pcm, "xx", threading.Event())
        processor.assert_not_called()

    def test_cancellation_before_or_during_generation_discards_the_result(self):
        cancel = threading.Event()
        cancel.set()
        processor = fake_processor()
        with self.assertRaises(SpeechCancelled):
            transcribe(fake_model([0, EOS]), processor, self.pcm, "en", cancel)
        processor.assert_not_called()

        cancel = threading.Event()
        model = fake_model([0, EOS])
        model.generate.side_effect = lambda **_: cancel.set() or np.array([[0, EOS]])
        with self.assertRaises(SpeechCancelled):
            transcribe(model, fake_processor(), self.pcm, "en", cancel)

    def test_cuda_work_is_synchronized_and_oversized_text_is_refused(self):
        fake_torch.cuda.synchronize.reset_mock()
        model = fake_model([0, EOS], device="cuda")
        transcribe(model, fake_processor(), self.pcm, "en", threading.Event())
        fake_torch.cuda.synchronize.assert_called_once()
        with self.assertRaisesRegex(ValueError, "oversized"):
            transcribe(
                fake_model([0, EOS]),
                fake_processor("x" * (MAX_TEXT_BYTES + 1)),
                self.pcm,
                "en",
                threading.Event(),
            )


if __name__ == "__main__":
    unittest.main()
