"""Ranking-head derivation against synthetic GGUFs.

Requires the conversion environment's packages (gguf, numpy, jinja2). Run with
``python -m unittest discover -s rust/crates/pumas-core/src/conversion/script_tests``.
"""

import contextlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path

import gguf
import numpy as np

SCRIPT = Path(__file__).resolve().parents[1] / "ranking_head_script.py"
SPEC = importlib.util.spec_from_file_location("ranking_head_script", SCRIPT)
ranking = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ranking)

WIDTH = 64
TOKENS = [f"t{i}" for i in range(48)] + ["yes", "no", "Yes", "No"] + [f"u{i}" for i in range(12)]
YES, NO, CAP_YES = 48, 49, 50
TEMPLATE = (
    '{%- set q = messages | selectattr("role", "eq", "query") | map(attribute="content") | first -%}\n'
    '{%- set d = messages | selectattr("role", "eq", "document") | map(attribute="content") | first -%}\n'
    "<|im_start|>system\nJudge.<|im_end|>\n<|im_start|>user\n<Query>: {{ q }}\n<Document>: {{ d }}<|im_end|>\n"
    "{% if add_generation_prompt %}<|im_start|>assistant\n{% endif %}"
)


def write_gguf(path, arch="qwen3", tokens=TOKENS, head=False, tied=False):
    rng = np.random.default_rng(7)
    writer = gguf.GGUFWriter(path, arch=arch)
    writer.add_context_length(4096)
    writer.add_tokenizer_model("gpt2")
    writer.add_token_list(tokens)
    writer.add_chat_template("{{ messages }}")
    embedding = rng.standard_normal((len(tokens), WIDTH)).astype(np.float32)
    output = rng.standard_normal((len(tokens), WIDTH)).astype(np.float32)
    writer.add_tensor("token_embd.weight", embedding)
    if not tied:
        quantized = gguf.quants.quantize(output, gguf.GGMLQuantizationType.Q8_0)
        writer.add_tensor("output.weight", quantized, raw_dtype=gguf.GGMLQuantizationType.Q8_0)
    if head:
        writer.add_tensor(ranking.CLS_OUT, output[:2])
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()


def write_contract(path, true_id=YES, false_id=NO, template=TEMPLATE, options=None, extra=None):
    contract = {
        "true_token_id": true_id,
        "true_token": TOKENS[true_id],
        "false_token_id": false_id,
        "false_token": None if false_id is None else TOKENS[false_id],
        "chat_template": template,
        "chat_template_options": options or {},
        "special_tokens": {"eos_token": "<|im_end|>"},
        **(extra or {}),
    }
    path.write_text(json.dumps(contract))


def run(source, contract, output):
    lines = io.StringIO()
    report = output.with_suffix(".report.json")
    with contextlib.redirect_stdout(lines):
        status = ranking.main(
            [
                "--input",
                str(source),
                "--contract",
                str(contract),
                "--output",
                str(output),
                "--report",
                str(report),
            ]
        )
    events = [json.loads(line) for line in lines.getvalue().splitlines()]
    return status, events


class RankingHeadTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.source = self.root / "source.gguf"
        self.contract = self.root / "contract.json"
        self.output = self.root / "ranked.gguf"

    def tearDown(self):
        self.dir.cleanup()

    def refused(self, code):
        status, events = run(self.source, self.contract, self.output)
        self.assertEqual(status, ranking.REFUSED_EXIT)
        self.assertEqual(events[-1]["stage"], "error")
        self.assertTrue(events[-1]["message"].startswith(f"{code}: "))
        self.assertEqual(events[-1]["code"], code, events[-1]["message"])
        self.assertFalse(self.output.exists())

    def derive(self):
        status, events = run(self.source, self.contract, self.output)
        self.assertEqual(status, 0, events)
        self.assertEqual(events[-1]["stage"], "complete")
        self.report = json.loads(self.output.with_suffix(".report.json").read_text())
        return gguf.GGUFReader(self.output), gguf.GGUFReader(self.source)

    def output_rows(self, reader, ids):
        tensor = next(
            t
            for t in reader.tensors
            if t.name in ("output.weight", "token_embd.weight")
            and (
                t.name == "output.weight"
                or not any(o.name == "output.weight" for o in reader.tensors)
            )
        )
        return [
            gguf.quants.dequantize(tensor.data[i : i + 1], tensor.tensor_type).reshape(-1)
            for i in ids
        ]

    def test_two_token_contract_derives_true_and_false_rows(self):
        write_gguf(self.source)
        write_contract(self.contract, options={"add_generation_prompt": True})
        derived, source = self.derive()
        head = next(t for t in derived.tensors if t.name == ranking.CLS_OUT)
        np.testing.assert_array_equal(
            np.asarray(head.data), np.stack(self.output_rows(source, [YES, NO]))
        )
        self.assertEqual(
            derived.get_field("qwen3.pooling_type").contents(), int(gguf.PoolingType.RANK)
        )
        self.assertEqual(
            derived.get_field("qwen3.classifier.output_labels").contents(), ["yes", "no"]
        )
        self.assertEqual(
            derived.get_field("tokenizer.chat_template.rerank").contents(),
            "<|im_start|>system\nJudge.<|im_end|>\n<|im_start|>user\n<Query>: {query}\n"
            "<Document>: {document}<|im_end|>\n<|im_start|>assistant\n",
        )
        self.assertEqual(derived.get_field("tokenizer.chat_templates").contents(), ["rerank"])
        self.assertEqual(derived.get_field("tokenizer.chat_template").contents(), "{{ messages }}")
        originals = {t.name: t for t in source.tensors}
        for tensor in derived.tensors:
            if tensor.name != ranking.CLS_OUT:
                self.assertEqual(
                    bytes(tensor.data), bytes(originals[tensor.name].data), tensor.name
                )
        self.assertEqual(len(derived.tensors), len(source.tensors) + 1)
        self.assertEqual(
            self.report,
            {
                "architecture": "qwen3",
                "true_token_id": YES,
                "false_token_id": NO,
                "labels": ["yes", "no"],
            },
        )

    def test_one_token_contract_uses_a_zero_false_row(self):
        write_gguf(self.source)
        write_contract(self.contract, true_id=CAP_YES, false_id=None)
        derived, source = self.derive()
        head = np.asarray(next(t for t in derived.tensors if t.name == ranking.CLS_OUT).data)
        np.testing.assert_array_equal(head[0], self.output_rows(source, [CAP_YES])[0])
        np.testing.assert_array_equal(head[1], np.zeros(WIDTH, np.float32))
        self.assertEqual(
            derived.get_field("qwen3.classifier.output_labels").contents(), ["Yes", ""]
        )

    def test_tied_embeddings_read_token_embd_rows(self):
        write_gguf(self.source, tied=True)
        write_contract(self.contract)
        derived, source = self.derive()
        head = np.asarray(next(t for t in derived.tensors if t.name == ranking.CLS_OUT).data)
        np.testing.assert_array_equal(head, np.stack(self.output_rows(source, [YES, NO])))

    def test_generation_prompt_defaults_to_off(self):
        write_gguf(self.source)
        write_contract(self.contract)
        derived, _ = self.derive()
        self.assertTrue(
            derived.get_field("tokenizer.chat_template.rerank").contents().endswith("<|im_end|>\n")
        )

    def test_vocabulary_mismatch_is_refused(self):
        write_gguf(self.source, tokens=[*TOKENS[:YES], "maybe", *TOKENS[YES + 1 :]])
        write_contract(self.contract)
        self.refused("vocabulary_mismatch")

    def test_unsupported_architecture_is_refused(self):
        write_gguf(self.source, arch="qwen35")
        write_contract(self.contract)
        self.refused("architecture_unsupported")

    def test_existing_head_is_refused(self):
        write_gguf(self.source, head=True)
        write_contract(self.contract)
        self.refused("already_ranked")

    def test_template_placing_the_query_twice_is_refused(self):
        write_gguf(self.source)
        write_contract(self.contract, template=TEMPLATE + "{{ messages[0].content }}")
        self.refused("template_unsupported")

    def test_template_containing_a_placeholder_is_refused(self):
        write_gguf(self.source)
        write_contract(self.contract, template=TEMPLATE + "{query}")
        self.refused("template_unsupported")

    def test_unknown_template_option_is_refused(self):
        write_gguf(self.source)
        write_contract(
            self.contract, options={"add_generation_prompt": True, "enable_thinking": False}
        )
        self.refused("contract_unsupported")

    def test_equal_token_ids_are_refused(self):
        write_gguf(self.source)
        write_contract(self.contract, true_id=YES, false_id=YES)
        self.refused("contract_invalid")

    def test_unknown_contract_field_is_refused(self):
        write_gguf(self.source)
        write_contract(self.contract, extra={"scale": 5})
        self.refused("contract_invalid")

    def test_vocabulary_strings_come_from_the_contract(self):
        write_gguf(self.source)
        write_contract(self.contract)
        contract = json.loads(self.contract.read_text())
        contract["true_token"] = "Yes"
        self.contract.write_text(json.dumps(contract))
        self.refused("vocabulary_mismatch")

    def test_existing_output_is_never_overwritten(self):
        write_gguf(self.source)
        write_contract(self.contract)
        self.output.write_bytes(b"retained")
        status, events = run(self.source, self.contract, self.output)
        self.assertEqual((status, events[-1]["code"]), (ranking.REFUSED_EXIT, "output_exists"))
        self.assertEqual(self.output.read_bytes(), b"retained")


if __name__ == "__main__":
    unittest.main()
