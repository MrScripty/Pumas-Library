"""Derive a llama.cpp ranking head into a copy of a causal-LM GGUF.

The scoring contract is a sentence-transformers ``LogitScore`` package:
``modules.json`` locates the ``LogitScore`` module, whose ``config.json`` names
``true_token_id`` and an optional ``false_token_id``; ``chat_template.jinja``
(or ``tokenizer_config.json``) renders ``query``/``document`` messages; and
``sentence_bert_config.json`` may supply chat-template options. The source
GGUF is never modified; the output holds its tensors byte for byte plus
``cls.output.weight``, rank pooling, classifier labels and a ``rerank``
template with ``{query}``/``{document}`` placeholders.

Progress lines use the conversion runner's stages (``validating``,
``converting``, ``complete``). Every refusal is one ``error`` line whose
``message`` starts with a stable code (``vocabulary_mismatch: ...``) and is
also given as ``code``; the script then exits with status 3.
"""

import argparse
import json
import secrets
import sys
from pathlib import Path, PurePosixPath

LOGIT_SCORE_TYPE = "sentence_transformers.cross_encoder.modules.logit_score.LogitScore"
LOGIT_SCORE_KEYS = {"true_token_id", "false_token_id"}
# Monotone activations: the served score orders documents as the reference.
MONOTONE_ACTIVATIONS = {
    "torch.nn.modules.linear.Identity",
    "torch.nn.modules.activation.Sigmoid",
}
CHAT_TEMPLATE_OPTIONS = {"add_generation_prompt"}
# llama.cpp rank-pooling behaviour per architecture. Architectures whose rank
# pooling applies softmax over the head take a two-row head; the score is then
# sigmoid(l_true - l_false), monotone in the reference score. A one-token
# contract uses a zero second row, giving sigmoid(l_true).
HEAD_LAYOUTS = {"qwen3": "softmax-pair", "qwen3vl": "softmax-pair"}
CLS_OUT = "cls.output.weight"
REFUSED_EXIT = 3


class Refusal(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code
        self.message = message


def report(stage, **values):
    print(json.dumps({"stage": stage, **values}), flush=True)


def read_json(path, what):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise Refusal("contract_missing", f"The scoring contract has no {what}") from None
    except (UnicodeError, ValueError) as error:
        raise Refusal("contract_invalid", f"{what} is not valid JSON: {error}") from None


def contained(root, relative, what):
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise Refusal("contract_invalid", f"{what} path leaves the contract package")
    resolved = (root / Path(*path.parts)).resolve()
    if root not in resolved.parents and resolved != root:
        raise Refusal("contract_invalid", f"{what} path leaves the contract package")
    return resolved


def is_token_id(value):
    return type(value) is int and value >= 0


def load_contract(root):
    root = root.resolve()
    modules = read_json(root / "modules.json", "modules.json")
    if not isinstance(modules, list) or not all(isinstance(item, dict) for item in modules):
        raise Refusal("contract_invalid", "modules.json must be a list of module records")
    scorers = [item for item in modules if item.get("type") == LOGIT_SCORE_TYPE]
    if len(scorers) != 1:
        raise Refusal(
            "contract_unsupported",
            "The package does not declare exactly one sentence-transformers LogitScore module",
        )
    if modules[-1] is not scorers[0]:
        raise Refusal("contract_unsupported", "Modules after LogitScore are not supported")
    module_path = scorers[0].get("path")
    if not isinstance(module_path, str):
        raise Refusal("contract_invalid", "The LogitScore module has no path")
    score = read_json(
        contained(root, module_path, "LogitScore") / "config.json", "LogitScore config.json"
    )
    if not isinstance(score, dict) or set(score) != LOGIT_SCORE_KEYS:
        raise Refusal(
            "contract_unsupported",
            "LogitScore config must hold exactly true_token_id and false_token_id",
        )
    true_id, false_id = score["true_token_id"], score["false_token_id"]
    if (
        not is_token_id(true_id)
        or not (false_id is None or is_token_id(false_id))
        or true_id == false_id
    ):
        raise Refusal(
            "contract_invalid", "LogitScore token ids must be distinct non-negative integers"
        )

    model_config = root / "config_sentence_transformers.json"
    if model_config.exists():
        declared = read_json(model_config, "config_sentence_transformers.json")
        if (
            not isinstance(declared, dict)
            or declared.get("model_type", "CrossEncoder") != "CrossEncoder"
        ):
            raise Refusal(
                "contract_unsupported", "The package is not a sentence-transformers CrossEncoder"
            )
        activation = declared.get("activation_fn")
        if activation is not None and activation not in MONOTONE_ACTIVATIONS:
            raise Refusal("contract_unsupported", f"Activation {activation} may reorder scores")

    options = {}
    st_config = root / "sentence_bert_config.json"
    if st_config.exists():
        declared = read_json(st_config, "sentence_bert_config.json")
        kwargs = declared.get("processing_kwargs", {}) if isinstance(declared, dict) else None
        options = kwargs.get("chat_template", {}) if isinstance(kwargs, dict) else None
        if not isinstance(options, dict) or not set(options) <= CHAT_TEMPLATE_OPTIONS:
            raise Refusal("contract_unsupported", "Unsupported chat-template processing options")
        if any(type(value) is not bool for value in options.values()):
            raise Refusal("contract_invalid", "Chat-template options must be booleans")

    tokenizer_config = {}
    if (root / "tokenizer_config.json").exists():
        tokenizer_config = read_json(root / "tokenizer_config.json", "tokenizer_config.json")
        if not isinstance(tokenizer_config, dict):
            raise Refusal("contract_invalid", "tokenizer_config.json must be an object")
    # Transformers passes the tokenizer's special-token strings to chat templates.
    special = {
        key: value
        for key, value in tokenizer_config.items()
        if key.endswith("_token") and isinstance(value, str)
    }
    template_file = root / "chat_template.jinja"
    if template_file.exists():
        try:
            template = template_file.read_text(encoding="utf-8")
        except UnicodeError:
            raise Refusal("contract_invalid", "chat_template.jinja is not UTF-8") from None
    else:
        template = tokenizer_config.get("chat_template")
        if not isinstance(template, str):
            raise Refusal("contract_missing", "The scoring contract has no chat template")

    tokenizer = read_json(root / "tokenizer.json", "tokenizer.json")
    tokens = token_strings(tokenizer, [true_id] + ([false_id] if false_id is not None else []))
    return {
        "true_id": true_id,
        "false_id": false_id,
        "true_token": tokens[true_id],
        "false_token": tokens.get(false_id) if false_id is not None else None,
        "template": template,
        "options": options,
        "special": special,
    }


def token_strings(tokenizer, ids):
    if not isinstance(tokenizer, dict):
        raise Refusal("contract_invalid", "tokenizer.json must be an object")
    found = {}
    for item in tokenizer.get("added_tokens") or []:
        if (
            isinstance(item, dict)
            and item.get("id") in ids
            and isinstance(item.get("content"), str)
        ):
            found[item["id"]] = item["content"]
    vocab = (tokenizer.get("model") or {}).get("vocab")
    if isinstance(vocab, dict):
        for text, token_id in vocab.items():
            if token_id in ids and token_id not in found:
                found[token_id] = text
    missing = [token_id for token_id in ids if token_id not in found]
    if missing:
        raise Refusal("contract_invalid", f"tokenizer.json has no token for id {missing[0]}")
    return found


def render_rerank_template(contract):
    """Render the contract's template with unique sentinels, then substitute
    llama.cpp's ``{query}``/``{document}`` placeholders for them."""
    from jinja2 import TemplateError
    from jinja2.ext import loopcontrols
    from jinja2.sandbox import ImmutableSandboxedEnvironment

    def raise_exception(message):
        raise TemplateError(message)

    env = ImmutableSandboxedEnvironment(
        trim_blocks=True, lstrip_blocks=True, extensions=[loopcontrols]
    )
    env.filters["tojson"] = lambda value, indent=None: json.dumps(
        value, ensure_ascii=False, indent=indent
    )
    env.globals["raise_exception"] = raise_exception
    nonce = secrets.token_hex(16)
    query, document = f"PUMASQUERY{nonce}", f"PUMASDOCUMENT{nonce}"
    try:
        rendered = env.from_string(contract["template"]).render(
            messages=[
                {"role": "query", "content": query},
                {"role": "document", "content": document},
            ],
            add_generation_prompt=contract["options"].get("add_generation_prompt", False),
            **contract["special"],
        )
    except TemplateError as error:
        raise Refusal(
            "template_unsupported", f"The chat template could not be rendered: {error}"
        ) from None
    if "{query}" in rendered or "{document}" in rendered:
        raise Refusal(
            "template_unsupported", "The rendered template already contains a llama.cpp placeholder"
        )
    if rendered.count(query) != 1 or rendered.count(document) != 1:
        raise Refusal(
            "template_unsupported",
            "The template must place the query and the document exactly once",
        )
    return rendered.replace(query, "{query}").replace(document, "{document}")


def field_value(reader, name):
    field = reader.get_field(name)
    return None if field is None else field.contents()


def head_rows(reader, contract):
    import numpy as np
    from gguf import quants

    arch = field_value(reader, "general.architecture")
    layout = HEAD_LAYOUTS.get(arch)
    if layout is None:
        raise Refusal(
            "architecture_unsupported",
            f"llama.cpp has no ranking-head layout for architecture {arch!r}",
        )
    names = {tensor.name: tensor for tensor in reader.tensors}
    if CLS_OUT in names:
        raise Refusal("already_ranked", "The GGUF already has a ranking head; serve it directly")
    tokens = field_value(reader, "tokenizer.ggml.tokens")
    if not isinstance(tokens, list):
        raise Refusal("gguf_invalid", "The GGUF has no tokenizer vocabulary")
    for key, expected in (("true", contract["true_token"]), ("false", contract["false_token"])):
        token_id = contract[f"{key}_id"]
        if token_id is None:
            continue
        if token_id >= len(tokens) or tokens[token_id] != expected:
            raise Refusal(
                "vocabulary_mismatch",
                f"GGUF token {token_id} is not the contract's {key} token {expected!r}",
            )
    output = names.get("output.weight") or names.get("token_embd.weight")
    if output is None:
        raise Refusal("gguf_invalid", "The GGUF has neither output.weight nor token_embd.weight")
    if len(output.shape) != 2 or int(output.shape[1]) != len(tokens):
        raise Refusal("gguf_invalid", "The output tensor does not span the vocabulary")
    width = int(output.shape[0])

    def row(token_id):
        values = quants.dequantize(output.data[token_id : token_id + 1], output.tensor_type)
        values = np.asarray(values, dtype=np.float32).reshape(-1)
        if values.shape != (width,) or not np.isfinite(values).all():
            raise Refusal("gguf_invalid", f"Output row {token_id} is not a finite {width}-vector")
        return values

    true_row = row(contract["true_id"])
    false_row = (
        row(contract["false_id"])
        if contract["false_id"] is not None
        else np.zeros(width, np.float32)
    )
    labels = [contract["true_token"], contract["false_token"] or ""]
    return arch, np.stack([true_row, false_row]).astype(np.float32), labels


def derive(source, contract_dir, output, report_path):
    import gguf

    report("validating")
    contract = load_contract(contract_dir)
    template = render_rerank_template(contract)
    reader = gguf.GGUFReader(source, "r")
    arch, head, labels = head_rows(reader, contract)

    writer = gguf.GGUFWriter(output, arch=arch, endianess=reader.endianess)
    alignment = field_value(reader, gguf.Keys.General.ALIGNMENT)
    if alignment is not None:
        writer.data_alignment = alignment
    replaced = {
        gguf.Keys.LLM.POOLING_TYPE.format(arch=arch),
        gguf.Keys.Classifier.OUTPUT_LABELS.format(arch=arch),
        gguf.Keys.Tokenizer.CHAT_TEMPLATE + ".rerank",
        gguf.Keys.Tokenizer.CHAT_TEMPLATES,
    }
    names = []
    for field in reader.fields.values():
        if (
            field.name == gguf.Keys.General.ARCHITECTURE
            or field.name.startswith("GGUF.")
            or field.name in replaced
        ):
            continue
        kind = field.types[0]
        sub = field.types[-1] if kind == gguf.GGUFValueType.ARRAY else None
        writer.add_key_value(field.name, field.contents(), kind, sub_type=sub)
    existing = field_value(reader, gguf.Keys.Tokenizer.CHAT_TEMPLATES) or []
    names = [name for name in existing if name != "rerank"] + ["rerank"]
    writer.add_key_value(
        gguf.Keys.Tokenizer.CHAT_TEMPLATES,
        names,
        gguf.GGUFValueType.ARRAY,
        sub_type=gguf.GGUFValueType.STRING,
    )
    writer.add_key_value(
        gguf.Keys.Tokenizer.CHAT_TEMPLATE + ".rerank", template, gguf.GGUFValueType.STRING
    )
    writer.add_pooling_type(gguf.PoolingType.RANK)
    writer.add_classifier_output_labels(labels)

    tensors = list(reader.tensors)
    for tensor in tensors:
        writer.add_tensor_info(
            tensor.name,
            tensor.data.shape,
            tensor.data.dtype,
            tensor.data.nbytes,
            tensor.tensor_type,
        )
    writer.add_tensor_info(
        CLS_OUT, head.shape, head.dtype, head.nbytes, gguf.GGMLQuantizationType.F32
    )
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    total = len(tensors) + 1
    written = 0
    for index, tensor in enumerate(tensors):
        report("converting", tensor_index=index, tensor_count=total, tensor_name=tensor.name)
        writer.write_tensor_data(tensor.data, tensor_endianess=reader.endianess)
        written += int(tensor.n_bytes)
    writer.write_tensor_data(head)
    written += int(head.nbytes)
    writer.close()
    summary = {
        "architecture": arch,
        "true_token_id": contract["true_id"],
        "false_token_id": contract["false_id"],
        "labels": labels,
    }
    report_path.write_text(json.dumps(summary), encoding="utf-8")
    report(
        "complete",
        output_path=str(output),
        output_size=output.stat().st_size,
        bytes_written=written,
    )


def refuse(refusal):
    report("error", code=refusal.code, message=f"{refusal.code}: {refusal.message}")
    return REFUSED_EXIT


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.output.exists():
        return refuse(Refusal("output_exists", "The output file already exists"))
    try:
        derive(args.input, args.contract, args.output, args.report)
    except Refusal as refusal:
        if args.output.exists():
            args.output.unlink()
        return refuse(refusal)
    return 0


if __name__ == "__main__":
    sys.exit(main())
