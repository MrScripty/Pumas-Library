"""Create a deterministic, untrained ONNX graph for packaged lifecycle checks.

This proves no pretrained Nomic behavior or embedding quality. The four-operator
graph casts token IDs to float32, adds one, and tiles each value to 256 dimensions.
It uses only the stable ONNX IR 8/opset 13 protobuf fields documented in
https://github.com/onnx/onnx/blob/main/onnx/onnx.proto and requires no model download
or Python ONNX dependency. The existing harness's model_fp16.onnx input filename
is retained; the synthetic graph itself is deliberately float32.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct


def varint(value):
    if type(value) is not int or value < 0:
        raise ValueError("Synthetic protobuf integers must be nonnegative")
    result = bytearray()
    while value > 127:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def integer(field, value):
    return varint(field << 3) + varint(value)


def raw(field, value):
    if isinstance(value, str):
        value = value.encode()
    return varint((field << 3) | 2) + varint(len(value)) + value


def value_info(name, data_type, dimensions):
    shape = b"".join(
        raw(1, integer(1, dimension) if isinstance(dimension, int) else raw(2, dimension))
        for dimension in dimensions
    )
    return raw(1, name) + raw(2, raw(1, integer(1, data_type) + raw(2, shape)))


def tensor(name, data_type, values):
    data = (
        b"".join(integer(7, value) for value in values)
        if data_type == 7
        else raw(4, struct.pack("<" + "f" * len(values), *values))
    )
    return integer(1, len(values)) + integer(2, data_type) + data + raw(8, name)


def node(operator, inputs, output, attributes=b""):
    return (
        b"".join(raw(1, name) for name in inputs) + raw(2, output) + raw(4, operator) + attributes
    )


def model_bytes():
    cast = raw(5, raw(1, "to") + integer(3, 1) + integer(20, 2))
    nodes = [
        node("Cast", ["input_ids"], "float_ids", cast),
        node("Add", ["float_ids", "one"], "positive"),
        node("Unsqueeze", ["positive", "axes"], "expanded"),
        node("Tile", ["expanded", "repeats"], "last_hidden_state"),
    ]
    graph = b"".join(raw(1, item) for item in nodes) + raw(2, "synthetic-untrained-token-tiling")
    for name, data_type, values in (
        ("one", 1, [1.0]),
        ("axes", 7, [2]),
        ("repeats", 7, [1, 1, 256]),
    ):
        graph += raw(5, tensor(name, data_type, values))
    for name in ("input_ids", "attention_mask"):
        graph += raw(11, value_info(name, 7, ["batch", "tokens"]))
    graph += raw(12, value_info("last_hidden_state", 1, ["batch", "tokens", 256]))
    return (
        integer(1, 8)
        + raw(2, "pumas synthetic lifecycle fixture")
        + raw(6, "UNTRAINED: execution/lifecycle only; not pretrained Nomic or model quality.")
        + raw(7, graph)
        + raw(8, integer(2, 13))
    )


def create_fixture(directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    tokenizer = {
        "version": "1.0",
        "truncation": None,
        "padding": None,
        "added_tokens": [],
        "normalizer": None,
        "pre_tokenizer": {"type": "WhitespaceSplit"},
        "post_processor": None,
        "decoder": None,
        "model": {
            "type": "WordLevel",
            "vocab": {"[UNK]": 0, "hello": 1, "world": 2},
            "unk_token": "[UNK]",
        },
    }
    files = {"onnx/model_fp16.onnx": model_bytes()}
    for name, value in (
        ("config.json", {"hidden_size": 256, "n_embd": 256, "model_type": "nomic_bert"}),
        ("tokenizer.json", tokenizer),
        ("tokenizer_config.json", {"unk_token": "[UNK]", "model_max_length": 512}),
    ):
        files[name] = (json.dumps(value, sort_keys=True) + "\n").encode()
    record = {
        "schema_version": 1,
        "scope": "synthetic untrained graph; execution and lifecycle only",
        "pretrained_model_acceptance": False,
        "onnx_ir_version": 8,
        "opset": 13,
        "tensor_dtype": "float32; the model_fp16.onnx filename is the legacy harness input",
        "files": {},
    }
    for name, data in files.items():
        path = directory / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        record["files"][name] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    (directory / "synthetic-fixture-record.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    create_fixture(parser.parse_args().output_dir)
