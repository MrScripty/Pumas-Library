"""Declared sentence-transformers modules after pooling (Dense, LayerNorm)."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))


def _real_torch():
    """Torch and Safetensors themselves, not the stubs other tests install."""
    try:
        import safetensors.torch  # noqa: F401
        import torch
    except Exception:
        return False
    return hasattr(torch, "nn") and hasattr(torch.nn, "functional")


REAL_TORCH = _real_torch()

from test_validation_and_app import (  # noqa: E402
    _install_optional_dependency_stubs as _install_dependency_stubs,
)

_install_dependency_stubs()

from loaders import text_encoder_loader as loader  # noqa: E402
from validation import ModelLoadRefusal  # noqa: E402


TRANSFORMER = {
    "type": "sentence_transformers.models.Transformer",
    "role": "transformer",
    "path": "",
}
POOLING = {"type": "sentence_transformers.models.Pooling", "role": "pooling", "path": "1_Pooling"}
NORMALIZE = {
    "type": "sentence_transformers.models.Normalize",
    "role": "normalize",
    "path": "4_Normalize",
}


def dense(
    path, in_features, out_features, bias=False, activation="torch.nn.modules.linear.Identity"
):
    return {
        "type": "sentence_transformers.models.Dense",
        "role": "dense",
        "path": path,
        "dense": {
            "in_features": in_features,
            "out_features": out_features,
            "bias": bias,
            "activation": activation,
        },
    }


def gemma_declarations(hidden=4, wide=6):
    """EmbeddingGemma's shape: Transformer, Pooling, Dense, Dense, Normalize."""
    return {
        "model_kind": "SentenceTransformer",
        "pooling": "mean",
        "normalize": True,
        "module_details": [
            TRANSFORMER,
            POOLING,
            dense("2_Dense", hidden, wide),
            dense("3_Dense", wide, hidden),
            NORMALIZE,
        ],
    }


class StagePlanTests(unittest.TestCase):
    def test_dense_modules_are_planned_in_declared_order(self):
        plan = loader.plan_for(loader.TEXT_EMBEDDING, gemma_declarations())
        self.assertEqual([stage.path for stage in plan.stages], ["2_Dense", "3_Dense"])
        self.assertEqual(plan.stages[0].out_features, 6)
        self.assertEqual(plan.stages[1].activation, "identity")
        self.assertTrue(plan.normalize)

    def test_a_layer_norm_module_is_planned(self):
        declared = gemma_declarations()
        declared["module_details"].insert(
            4,
            {
                "type": "sentence_transformers.models.LayerNorm",
                "role": "layer_norm",
                "path": "4_LayerNorm",
                "layer_norm": {"dimension": 4},
            },
        )
        plan = loader.plan_for(loader.TEXT_EMBEDDING, declared)
        self.assertEqual(plan.stages[2].role, "layer_norm")

    def test_unimplemented_or_unusable_modules_are_refused_with_the_reason(self):
        cases = []
        unknown = gemma_declarations()
        unknown["module_details"].insert(
            2, {"type": "acme.modules.Projection", "role": "other", "path": "2_P"}
        )
        cases.append((loader.TEXT_EMBEDDING, unknown, "acme.modules.Projection"))
        undetailed = gemma_declarations()
        del undetailed["module_details"][2]["dense"]
        cases.append((loader.TEXT_EMBEDDING, undetailed, "resolve the model's declarations"))
        swish = gemma_declarations()
        swish["module_details"][2]["dense"]["activation"] = "torch.nn.modules.activation.SiLU"
        cases.append((loader.TEXT_EMBEDDING, swish, "SiLU"))
        mismatched = gemma_declarations()
        mismatched["module_details"][3]["dense"]["in_features"] = 5
        cases.append((loader.TEXT_EMBEDDING, mismatched, "takes 5 features"))
        escape = gemma_declarations()
        escape["module_details"][2]["path"] = "../outside"
        cases.append((loader.TEXT_EMBEDDING, escape, "package-relative"))
        after_normalize = gemma_declarations()
        after_normalize["module_details"].append(dense("5_Dense", 4, 4))
        cases.append((loader.TEXT_EMBEDDING, after_normalize, "precede Normalize"))
        pooled_reranker = {"module_details": [TRANSFORMER, POOLING]}
        cases.append((loader.TEXT_RERANK, pooled_reranker, "Pooling"))
        for model_type, declared, reason in cases:
            with self.subTest(reason=reason):
                with self.assertRaises(ModelLoadRefusal) as refused:
                    loader.plan_for(model_type, declared)
                self.assertIn(reason, str(refused.exception))

    def test_records_without_module_details_keep_their_plan(self):
        plan = loader.plan_for(loader.TEXT_EMBEDDING, {"pooling": "cls", "normalize": False})
        self.assertEqual(plan.stages, ())


@unittest.skipUnless(REAL_TORCH, "needs torch and safetensors")
class StageComputationTests(unittest.TestCase):
    def setUp(self):
        import torch
        from safetensors.torch import save_file

        torch.manual_seed(0)
        self.package = Path(tempfile.mkdtemp())
        self.first = torch.randn(6, 4)
        self.second = torch.randn(4, 6)
        for path, weight in (("2_Dense", self.first), ("3_Dense", self.second)):
            (self.package / path).mkdir()
            save_file({"linear.weight": weight}, str(self.package / path / "model.safetensors"))

    def test_dense_stages_match_the_reference_layers(self):
        import torch

        plan = loader.plan_for(loader.TEXT_EMBEDDING, gemma_declarations())
        weights = loader.load_stage_weights(self.package, plan.stages, 4)
        pooled = torch.randn(3, 4)
        expected = torch.nn.functional.linear(
            torch.nn.functional.linear(pooled, self.first), self.second
        )
        actual = loader.apply_stages(pooled, plan.stages, weights)
        self.assertTrue(torch.allclose(actual, expected, atol=1e-6))

    def test_tanh_and_layer_norm_follow_sentence_transformers(self):
        import torch
        from safetensors.torch import save_file

        (self.package / "4_LayerNorm").mkdir()
        gamma, beta = torch.randn(4), torch.randn(4)
        save_file(
            {"norm.weight": gamma, "norm.bias": beta},
            str(self.package / "4_LayerNorm" / "model.safetensors"),
        )
        bias = torch.randn(6)
        save_file(
            {"linear.weight": self.first, "linear.bias": bias},
            str(self.package / "2_Dense" / "model.safetensors"),
        )
        declared = gemma_declarations()
        declared["module_details"][2] = dense(
            "2_Dense", 4, 6, bias=True, activation="torch.nn.modules.activation.Tanh"
        )
        declared["module_details"].insert(
            4,
            {
                "type": "sentence_transformers.models.LayerNorm",
                "role": "layer_norm",
                "path": "4_LayerNorm",
                "layer_norm": {"dimension": 4},
            },
        )
        plan = loader.plan_for(loader.TEXT_EMBEDDING, declared)
        weights = loader.load_stage_weights(self.package, plan.stages, 4)
        pooled = torch.randn(2, 4)
        reference = torch.nn.LayerNorm(4)
        with torch.no_grad():
            reference.weight.copy_(gamma)
            reference.bias.copy_(beta)
            expected = reference(
                torch.nn.functional.linear(
                    torch.tanh(torch.nn.functional.linear(pooled, self.first, bias)), self.second
                )
            )
        actual = loader.apply_stages(pooled, plan.stages, weights)
        self.assertTrue(torch.allclose(actual, expected, atol=1e-5))

    def test_missing_or_misshapen_weights_are_refused(self):
        plan = loader.plan_for(loader.TEXT_EMBEDDING, gemma_declarations())
        with self.assertRaises(ModelLoadRefusal) as refused:
            loader.load_stage_weights(self.package, plan.stages, 8)
        self.assertIn("the model produces 8", str(refused.exception))
        (self.package / "3_Dense" / "model.safetensors").unlink()
        with self.assertRaises(ModelLoadRefusal) as refused:
            loader.load_stage_weights(self.package, plan.stages, 4)
        self.assertIn("3_Dense/model.safetensors", str(refused.exception))
        wrong = gemma_declarations(hidden=4, wide=5)
        with self.assertRaises(ModelLoadRefusal) as refused:
            loader.load_stage_weights(
                self.package, loader.plan_for(loader.TEXT_EMBEDDING, wrong).stages, 4
            )
        self.assertIn("linear.weight of shape [5, 4]", str(refused.exception))


if __name__ == "__main__":
    unittest.main()
