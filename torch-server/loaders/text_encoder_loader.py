"""Text embedders and rerankers served from local Transformers packages.

What a model computes comes from Pumas' validated sentence-transformers
declarations (ADR 0007), passed with the load request, not from the package's
own module files, which exported or partial copies may lack:

- a causal cross-encoder with a ``LogitScore`` declaration scores
  ``l_true - l_false`` (or ``l_true``) at the last position of the prompt its
  declared chat template renders for ``query``/``document`` messages;
- any other reranker is a sequence classifier with one relevance logit;
- an embedder pools (mean, CLS, last token, max) and normalizes as declared,
  mean-pooled and L2-normalized when nothing is declared.

A declared activation (Sigmoid or Identity) is applied to scores; otherwise
scores are raw logits. Models load from local files only, without repository
code.
"""

from __future__ import annotations

import math
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

TEXT_EMBEDDING = "text-embedding"
TEXT_RERANK = "text-rerank"
TEXT_ENCODER_TYPES = frozenset({TEXT_EMBEDDING, TEXT_RERANK})
POOLINGS = {"mean", "cls", "last_token", "max"}
ACTIVATIONS = {
    "torch.nn.modules.linear.Identity": "identity",
    "torch.nn.modules.activation.Sigmoid": "sigmoid",
}
TEMPLATE_OPTIONS = {"add_generation_prompt"}
# Pairs or texts run through the model together; bounds activation memory.
BATCH_SIZE = 16


class TextEncoderCancelled(Exception):
    """The caller stopped the request between batches."""


@dataclass(frozen=True)
class Scoring:
    """A causal cross-encoder's declared logit scoring."""

    true_token_id: int
    false_token_id: Optional[int]
    template: str
    add_generation_prompt: bool
    special_tokens: dict


@dataclass(frozen=True)
class EncoderPlan:
    """What a loaded text encoder computes, decoded from the declarations."""

    kind: str
    pooling: str = "mean"
    normalize: bool = True
    activation: str = "identity"
    scoring: Optional[Scoring] = None
    max_tokens: Optional[int] = None


def _refuse(message: str):
    from validation import ModelLoadRefusal

    return ModelLoadRefusal(message)


def _token_id(value: Any, field: str) -> int:
    if type(value) is not int or value < 0:
        raise _refuse(f"declared {field} is not a token id")
    return value


def plan_for(model_type: str, declarations: Optional[dict]) -> EncoderPlan:
    """Decode the parts of the declarations this adapter uses, strictly."""
    if model_type not in TEXT_ENCODER_TYPES:
        raise _refuse(f"unsupported text encoder type {model_type!r}")
    if declarations is not None and not isinstance(declarations, dict):
        raise _refuse("declarations must be an object")
    declared = declarations or {}
    activation = declared.get("activation")
    if activation is not None and activation not in ACTIVATIONS:
        raise _refuse(f"declared activation {activation} is not monotone or not supported")
    max_tokens = declared.get("max_seq_length")
    if max_tokens is not None and (type(max_tokens) is not int or max_tokens <= 0):
        raise _refuse("declared max_seq_length is invalid")
    if model_type == TEXT_RERANK:
        scoring = None
        logit = declared.get("logit_score")
        if logit is not None:
            if not isinstance(logit, dict):
                raise _refuse("declared logit_score must be an object")
            template = declared.get("chat_template")
            if not isinstance(template, str):
                raise _refuse("a LogitScore reranker must declare its chat template")
            options = declared.get("chat_template_options") or {}
            if not isinstance(options, dict) or not set(options) <= TEMPLATE_OPTIONS:
                raise _refuse("unsupported chat-template options")
            if any(type(value) is not bool for value in options.values()):
                raise _refuse("chat-template options must be booleans")
            special = declared.get("special_tokens") or {}
            if not isinstance(special, dict) or not all(
                isinstance(v, str) for v in special.values()
            ):
                raise _refuse("declared special tokens must be strings")
            false_id = logit.get("false_token_id")
            scoring = Scoring(
                true_token_id=_token_id(logit.get("true_token_id"), "true_token_id"),
                false_token_id=None if false_id is None else _token_id(false_id, "false_token_id"),
                template=template,
                add_generation_prompt=options.get("add_generation_prompt", False),
                special_tokens=special,
            )
        return EncoderPlan(
            kind=TEXT_RERANK,
            activation=ACTIVATIONS.get(activation, "identity"),
            scoring=scoring,
            max_tokens=max_tokens,
        )
    pooling = declared.get("pooling")
    if pooling is not None and pooling not in POOLINGS:
        raise _refuse(f"declared pooling {pooling} is not implemented by the Torch adapter")
    normalize = declared.get("normalize", True) if declarations else True
    if type(normalize) is not bool:
        raise _refuse("declared normalize must be a boolean")
    return EncoderPlan(
        kind=TEXT_EMBEDDING,
        pooling=pooling or "mean",
        normalize=normalize,
        max_tokens=max_tokens,
    )


@dataclass
class TextEncoder:
    plan: EncoderPlan
    model: Any
    tokenizer: Any
    device: Any


def load_text_encoder(path: Path, device, model_type: str, declarations: Optional[dict]):
    """Load a text encoder; returns ``(encoder, tokenizer, model_type)``."""
    import torch
    from transformers import (
        AutoModel,
        AutoModelForCausalLM,
        AutoModelForSequenceClassification,
        AutoTokenizer,
    )

    plan = plan_for(model_type, declarations)
    if not any(path.glob("*.safetensors")):
        raise _refuse("Torch text encoders must provide .safetensors weights")
    common = {"local_files_only": True, "trust_remote_code": False}
    tokenizer = AutoTokenizer.from_pretrained(path, **common)
    if plan.kind == TEXT_RERANK and plan.scoring is not None:
        # Causal scoring reads the last position; left padding keeps it real.
        tokenizer.padding_side = "left"
        model_class = AutoModelForCausalLM
    elif plan.kind == TEXT_RERANK:
        model_class = AutoModelForSequenceClassification
    else:
        tokenizer.padding_side = "right"
        model_class = AutoModel
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    model = model_class.from_pretrained(path, dtype="auto", **common)
    if plan.kind == TEXT_RERANK and plan.scoring is None:
        labels = getattr(model.config, "num_labels", None)
        if labels != 1:
            raise _refuse(f"a sequence-classifier reranker needs one relevance label, not {labels}")
    model.to(device)
    model.eval()
    if device.type == "cuda":
        torch.cuda.synchronize(device)
    return TextEncoder(plan, model, tokenizer, device), tokenizer, model_type


def _batches(items, size=BATCH_SIZE):
    for start in range(0, len(items), size):
        yield start, items[start : start + size]


def _check(cancel: Optional[threading.Event]):
    if cancel is not None and cancel.is_set():
        raise TextEncoderCancelled()


def _limit(encoder: TextEncoder) -> Optional[int]:
    limit = encoder.plan.max_tokens
    model_limit = getattr(encoder.model.config, "max_position_embeddings", None)
    if isinstance(model_limit, int) and model_limit > 0:
        limit = model_limit if limit is None else min(limit, model_limit)
    return limit


def _refuse_long(lengths, limit, offset, what):
    if limit is None:
        return
    for index, length in enumerate(lengths, start=offset):
        if length > limit:
            raise ValueError(
                f"{what} {index} has {length} tokens; this model accepts at most {limit}"
            )


def rerank_prompts(scoring: Scoring, tokenizer, query: str, documents: list[str]) -> list[str]:
    """Render each pair with the declared template and options."""
    prompts = []
    for document in documents:
        messages = [{"role": "query", "content": query}, {"role": "document", "content": document}]
        prompts.append(
            tokenizer.apply_chat_template(
                messages,
                chat_template=scoring.template,
                add_generation_prompt=scoring.add_generation_prompt,
                tokenize=False,
                **scoring.special_tokens,
            )
        )
    return prompts


def activate(value: float, activation: str) -> float:
    return 1.0 / (1.0 + math.exp(-value)) if activation == "sigmoid" else value


def rerank(encoder: TextEncoder, query: str, documents: list[str], cancel=None):
    """Scores in document order and the prompt tokens used."""
    import torch

    plan, tokenizer = encoder.plan, encoder.tokenizer
    limit = _limit(encoder)
    scores: list[float] = []
    tokens = 0
    with torch.inference_mode():
        for start, batch_documents in _batches(documents):
            _check(cancel)
            if plan.scoring is not None:
                prompts = rerank_prompts(plan.scoring, tokenizer, query, batch_documents)
                batch = tokenizer(
                    prompts, padding=True, add_special_tokens=False, return_tensors="pt"
                )
            else:
                batch = tokenizer(
                    [query] * len(batch_documents),
                    batch_documents,
                    padding=True,
                    return_tensors="pt",
                )
            lengths = batch["attention_mask"].sum(dim=1).tolist()
            _refuse_long(lengths, limit, start, "the query and document")
            tokens += int(sum(lengths))
            batch = {name: value.to(encoder.device) for name, value in batch.items()}
            logits = encoder.model(**batch).logits.float()
            if plan.scoring is not None:
                last = logits[:, -1, :]
                values = last[:, plan.scoring.true_token_id]
                if plan.scoring.false_token_id is not None:
                    values = values - last[:, plan.scoring.false_token_id]
            else:
                values = logits[:, 0]
            for value in values.tolist():
                if not math.isfinite(value):
                    raise ValueError("the model produced a non-finite relevance score")
                scores.append(activate(value, plan.activation))
    return scores, tokens


def _pool(hidden, mask, pooling: str):
    import torch

    mask = mask.unsqueeze(-1).to(hidden.dtype)
    if pooling == "mean":
        return (hidden * mask).sum(dim=1) / mask.sum(dim=1).clamp(min=1)
    if pooling == "cls":
        return hidden[:, 0]
    if pooling == "last_token":
        # Inputs are right-padded: the last attended position per row.
        last = mask.squeeze(-1).sum(dim=1).long() - 1
        return hidden[torch.arange(hidden.shape[0], device=hidden.device), last]
    if pooling == "max":
        return hidden.masked_fill(mask == 0, float("-inf")).max(dim=1).values
    raise ValueError(f"unsupported pooling {pooling}")


def embed(encoder: TextEncoder, texts: list[str], dimensions: Optional[int], cancel=None):
    """Vectors in input order, truncated to ``dimensions`` (Matryoshka) and
    then normalized when the model declares normalization."""
    import torch

    plan, tokenizer = encoder.plan, encoder.tokenizer
    limit = _limit(encoder)
    vectors: list[list[float]] = []
    tokens = 0
    with torch.inference_mode():
        for start, batch_texts in _batches(texts):
            _check(cancel)
            batch = tokenizer(batch_texts, padding=True, return_tensors="pt")
            lengths = batch["attention_mask"].sum(dim=1).tolist()
            _refuse_long(lengths, limit, start, "input")
            tokens += int(sum(lengths))
            batch = {name: value.to(encoder.device) for name, value in batch.items()}
            hidden = encoder.model(**batch).last_hidden_state.float()
            pooled = _pool(hidden, batch["attention_mask"], plan.pooling)
            if dimensions is not None:
                if dimensions > pooled.shape[-1]:
                    raise ValueError(
                        f"requested {dimensions} dimensions; this model produces {pooled.shape[-1]}"
                    )
                pooled = pooled[:, :dimensions]
            if plan.normalize:
                pooled = torch.nn.functional.normalize(pooled, p=2, dim=-1)
            for row in pooled.tolist():
                if not all(math.isfinite(value) for value in row):
                    raise ValueError("the model produced a non-finite embedding")
                vectors.append(row)
    return vectors, tokens
