"""Text loaders must not authorize code supplied by a downloaded model."""

import importlib.util
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


LOADERS = (
    ("safetensors_loader", "load_safetensors", "text-generation"),
    ("sherry_loader", "load_sherry", "sherry"),
    ("dllm_loader", "load_dllm", "dllm"),
)


class TextLoaderTrustTests(unittest.TestCase):
    def load_fixture(self, module_name):
        tokenizer_factory = Mock()
        model_factory = Mock()
        transformers = types.ModuleType("transformers")
        transformers.AutoTokenizer = types.SimpleNamespace(from_pretrained=tokenizer_factory)
        transformers.AutoModelForCausalLM = types.SimpleNamespace(from_pretrained=model_factory)
        torch = types.ModuleType("torch")
        torch.device = object
        path = Path(__file__).resolve().parents[1] / "loaders" / f"{module_name}.py"
        spec = importlib.util.spec_from_file_location(f"trust_test_{module_name}", path)
        module = importlib.util.module_from_spec(spec)
        # Isolate lightweight loader contract tests from installed optional ML
        # dependencies and from the other tests' fake torch modules.
        with patch.dict(sys.modules, {"torch": torch, "transformers": transformers}):
            spec.loader.exec_module(module)
        return module, tokenizer_factory, model_factory

    def test_tokenizer_and_model_loading_never_authorize_model_repository_code(self):
        for module_name, function_name, model_type in LOADERS:
            with self.subTest(loader=module_name):
                module, tokenizer_factory, model_factory = self.load_fixture(module_name)
                model, tokenizer, actual_type = getattr(module, function_name)(Path("model"), "cpu")
                self.assertIs(model, model_factory.return_value)
                self.assertIs(tokenizer, tokenizer_factory.return_value)
                self.assertEqual(actual_type, model_type)
                tokenizer_factory.assert_called_once_with("model", trust_remote_code=False)
                self.assertIs(model_factory.call_args.kwargs["trust_remote_code"], False)
                self.assertEqual(model_factory.call_count, 1)
                model.eval.assert_called_once_with()

    def test_custom_code_refusal_is_preserved_without_retry_or_trust_escalation(self):
        for module_name, function_name, _ in LOADERS:
            for rejected_stage in ("tokenizer", "model"):
                with self.subTest(loader=module_name, stage=rejected_stage):
                    module, tokenizer_factory, model_factory = self.load_fixture(module_name)
                    refusal = ValueError("model requires unapproved repository code")
                    rejected = tokenizer_factory if rejected_stage == "tokenizer" else model_factory
                    rejected.side_effect = refusal
                    with self.assertRaises(ValueError) as raised:
                        getattr(module, function_name)(Path("model"), "cpu")
                    self.assertIs(raised.exception, refusal)
                    self.assertEqual(rejected.call_count, 1)
                    self.assertIs(rejected.call_args.kwargs["trust_remote_code"], False)
                    if rejected_stage == "tokenizer":
                        model_factory.assert_not_called()


if __name__ == "__main__":
    unittest.main()
