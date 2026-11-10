"""Text embedding and rerank operations: declaration decoding, routes, refusals."""

import asyncio
import sys
import unittest
from contextlib import asynccontextmanager
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from test_validation_and_app import (  # noqa: E402
    _install_optional_dependency_stubs as _install_dependency_stubs,
)

_install_dependency_stubs()

import serve  # noqa: E402
from fastapi import HTTPException  # noqa: E402
from loaders import text_encoder_loader as loader  # noqa: E402
from model_manager import SlotConflict  # noqa: E402
from pydantic import ValidationError  # noqa: E402
from text_api import EmbeddingRequest, RerankRequest, text_embeddings, text_rerank  # noqa: E402
from validation import ModelLoadRefusal  # noqa: E402

LOGIT_DECLARATIONS = {
    "model_kind": "CrossEncoder",
    "logit_score": {
        "true_token_id": 9405,
        "true_token": "yes",
        "false_token_id": 2083,
        "false_token": "no",
    },
    "activation": "torch.nn.modules.activation.Sigmoid",
    "chat_template": "{{ messages }}",
    "chat_template_options": {"add_generation_prompt": True},
    "special_tokens": {"eos_token": "<|im_end|>"},
}


class PlanTests(unittest.TestCase):
    def test_a_logit_score_reranker_plans_its_tokens_template_and_activation(self):
        plan = loader.plan_for(loader.TEXT_RERANK, LOGIT_DECLARATIONS)
        self.assertEqual(plan.activation, "sigmoid")
        self.assertEqual(
            (
                plan.scoring.true_token_id,
                plan.scoring.false_token_id,
                plan.scoring.add_generation_prompt,
            ),
            (9405, 2083, True),
        )

    def test_a_reranker_without_declarations_is_a_raw_logit_sequence_classifier(self):
        plan = loader.plan_for(loader.TEXT_RERANK, None)
        self.assertIsNone(plan.scoring)
        self.assertEqual(plan.activation, "identity")

    def test_an_embedder_pools_and_normalizes_as_declared_or_by_default(self):
        self.assertEqual(
            (
                loader.plan_for(loader.TEXT_EMBEDDING, None).pooling,
                loader.plan_for(loader.TEXT_EMBEDDING, None).normalize,
            ),
            ("mean", True),
        )
        declared = loader.plan_for(
            loader.TEXT_EMBEDDING, {"pooling": "last_token", "normalize": False}
        )
        self.assertEqual((declared.pooling, declared.normalize), ("last_token", False))

    def test_unusable_declarations_are_refused_with_the_reason(self):
        cases = [
            (loader.TEXT_EMBEDDING, {"pooling": "weighted_mean"}, "not implemented"),
            (
                loader.TEXT_RERANK,
                {**LOGIT_DECLARATIONS, "activation": "torch.nn.Tanh"},
                "not monotone",
            ),
            (loader.TEXT_RERANK, {**LOGIT_DECLARATIONS, "chat_template": None}, "chat template"),
            (
                loader.TEXT_RERANK,
                {**LOGIT_DECLARATIONS, "chat_template_options": {"enable_thinking": False}},
                "unsupported chat-template options",
            ),
            (
                loader.TEXT_RERANK,
                {**LOGIT_DECLARATIONS, "logit_score": {"true_token_id": -1}},
                "not a token id",
            ),
            ("text-generation", None, "unsupported text encoder type"),
        ]
        for model_type, declarations, reason in cases:
            with self.subTest(reason=reason), self.assertRaises(ModelLoadRefusal) as refused:
                loader.plan_for(model_type, declarations)
            self.assertIn(reason, str(refused.exception))

    def test_prompts_render_query_and_document_messages_with_the_declared_options(self):
        calls = []

        class Tokenizer:
            def apply_chat_template(self, messages, **options):
                calls.append((messages, options))
                return "prompt"

        scoring = loader.plan_for(loader.TEXT_RERANK, LOGIT_DECLARATIONS).scoring
        self.assertEqual(
            loader.rerank_prompts(scoring, Tokenizer(), "q", ["a", "b"]), ["prompt", "prompt"]
        )
        messages, options = calls[0]
        self.assertEqual(
            messages, [{"role": "query", "content": "q"}, {"role": "document", "content": "a"}]
        )
        self.assertEqual(
            options,
            {
                "chat_template": "{{ messages }}",
                "add_generation_prompt": True,
                "tokenize": False,
                "eos_token": "<|im_end|>",
            },
        )

    def test_activation_maps_logits_monotonically(self):
        self.assertEqual(loader.activate(2.0, "identity"), 2.0)
        self.assertAlmostEqual(loader.activate(0.0, "sigmoid"), 0.5)


def route_request(lease, disconnected=False):
    async def is_disconnected():
        return disconnected

    return SimpleNamespace(
        app=SimpleNamespace(
            state=SimpleNamespace(model_manager=SimpleNamespace(text_encoder_lease=lease))
        ),
        is_disconnected=is_disconnected,
    )


class RouteTests(unittest.TestCase):
    def test_routes_are_mounted_and_text_capabilities_follow_installed_packages(self):
        paths = {route.path for route in serve.create_app().routes}
        self.assertTrue({"/api/text/embeddings", "/api/text/rerank"} <= paths)
        with patch.object(serve, "_installed", return_value=True):
            self.assertIn("rerank", serve.torch_capabilities())
        with patch.object(serve, "_installed", return_value=False):
            self.assertNotIn("text_embedding", serve.torch_capabilities())

    def test_rerank_returns_document_order_scores_and_usage(self):
        @asynccontextmanager
        async def lease(model_id, kind):
            self.assertEqual(kind, loader.TEXT_RERANK)
            yield SimpleNamespace(model="encoder")

        with patch.object(loader, "rerank", return_value=([0.1, 0.9], 12)):
            body = asyncio.run(
                text_rerank(
                    RerankRequest(model_id="m", query="q", documents=["a", "b"]),
                    route_request(lease),
                )
            )
        self.assertEqual(
            body,
            {
                "results": [
                    {"index": 0, "relevance_score": 0.1},
                    {"index": 1, "relevance_score": 0.9},
                ],
                "usage": {"prompt_tokens": 12, "total_tokens": 12},
            },
        )

    def test_embeddings_return_openai_shaped_vectors(self):
        @asynccontextmanager
        async def lease(model_id, kind):
            self.assertEqual(kind, loader.TEXT_EMBEDDING)
            yield SimpleNamespace(model="encoder")

        with patch.object(loader, "embed", return_value=([[0.6, 0.8]], 3)):
            body = asyncio.run(
                text_embeddings(
                    EmbeddingRequest(model_id="m", input=["hi"], dimensions=2), route_request(lease)
                )
            )
        self.assertEqual(
            body["data"], [{"object": "embedding", "index": 0, "embedding": [0.6, 0.8]}]
        )
        self.assertEqual(body["usage"]["prompt_tokens"], 3)

    def test_refusals_carry_stable_codes_and_reasons(self):
        def refusing(error):
            @asynccontextmanager
            async def lease(model_id, kind):
                raise error
                yield

            return lease

        payload = RerankRequest(model_id="m", query="q", documents=["a"])
        for error, status, code in [
            (KeyError("m"), 503, "model_unavailable"),
            (ValueError("Selected model does not produce rerank scores"), 400, "unsupported_model"),
            (SlotConflict("runtime_busy", "Text runtime is busy"), 409, "runtime_busy"),
        ]:
            with self.subTest(code=code), self.assertRaises(HTTPException) as refused:
                asyncio.run(text_rerank(payload, route_request(refusing(error))))
            self.assertEqual(
                (refused.exception.status_code, refused.exception.detail["code"]), (status, code)
            )

        @asynccontextmanager
        async def ready(model_id, kind):
            yield SimpleNamespace(model="encoder")

        with patch.object(
            loader, "rerank", side_effect=ValueError("the query and document 0 has 9 tokens")
        ):
            with self.assertRaises(HTTPException) as refused:
                asyncio.run(text_rerank(payload, route_request(ready)))
        self.assertEqual(
            refused.exception.detail,
            {"code": "invalid_request", "message": "the query and document 0 has 9 tokens"},
        )

    def test_requests_reject_blank_texts_unknown_fields_and_bad_dimensions(self):
        for build in [
            lambda: RerankRequest(model_id="m", query=" ", documents=["a"]),
            lambda: RerankRequest(model_id="m", query="q", documents=["a", " "]),
            lambda: RerankRequest(model_id="m", query="q", documents=[]),
            lambda: RerankRequest(model_id="m", query="q", documents=["a"], top_n=1),
            lambda: EmbeddingRequest(model_id="m", input=["a"], dimensions=0),
        ]:
            with self.assertRaises(ValidationError):
                build()


if __name__ == "__main__":
    unittest.main()
