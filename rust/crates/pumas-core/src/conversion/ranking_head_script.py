"""Derive a llama.cpp ranking head into a copy of a causal-LM GGUF.

The scoring contract is the ``LogitScore`` part of the model's validated
sentence-transformers declarations, written by Pumas as JSON: the scored
token ids and their strings, the chat template that formats ``query`` and
``document`` messages, its options and the special-token strings it may use.
The source GGUF is never modified; the output holds its tensors byte for byte
plus ``cls.output.weight``, rank pooling, classifier labels and a ``rerank``
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
from pathlib import Path

CONTRACT_KEYS = {
    "true_token_id",
    "true_token",
    "false_token_id",
    "false_token",
    "chat_template",
    "chat_template_options",
    "special_tokens",
}
# Template options this renderer applies; any other option could change the
# prompt the model was trained with.
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


def is_token_id(value):
    return type(value) is int and value >= 0


def load_contract(path):
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError) as error:
        raise Refusal(
            "contract_invalid", f"The scoring contract could not be read: {error}"
        ) from None
    if not isinstance(contract, dict) or set(contract) != CONTRACT_KEYS:
        raise Refusal("contract_invalid", "The scoring contract has missing or unknown fields")
    true_id, false_id = contract["true_token_id"], contract["false_token_id"]
    if (
        not is_token_id(true_id)
        or not (false_id is None or is_token_id(false_id))
        or true_id == false_id
    ):
        raise Refusal("contract_invalid", "Scored token ids must be distinct non-negative integers")
    if not isinstance(contract["true_token"], str) or (false_id is None) != (
        contract["false_token"] is None
    ):
        raise Refusal("contract_invalid", "Scored token strings must match their ids")
    if not isinstance(contract["chat_template"], str):
        raise Refusal("contract_invalid", "The chat template must be a string")
    options, special = contract["chat_template_options"], contract["special_tokens"]
    if not isinstance(options, dict) or any(type(value) is not bool for value in options.values()):
        raise Refusal("contract_invalid", "Chat-template options must be booleans")
    if not set(options) <= CHAT_TEMPLATE_OPTIONS:
        unknown = ", ".join(sorted(set(options) - CHAT_TEMPLATE_OPTIONS))
        raise Refusal("contract_unsupported", f"Unsupported chat-template options: {unknown}")
    if not isinstance(special, dict) or not all(
        isinstance(value, str) for value in special.values()
    ):
        raise Refusal("contract_invalid", "Special tokens must be strings")
    return {
        "true_id": true_id,
        "false_id": false_id,
        "true_token": contract["true_token"],
        "false_token": contract["false_token"],
        "template": contract["chat_template"],
        "options": options,
        "special": special,
    }


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


def derive(source, contract_path, output, report_path):
    import gguf

    report("validating")
    contract = load_contract(contract_path)
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
