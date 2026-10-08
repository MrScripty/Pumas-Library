"""Actual inherited-FD process exchanges; controlled bytes, no native models."""

import asyncio
import hashlib
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from private_owned_channel import (
    MAX_REQUEST_BYTES,
    PrivateChannelError,
    _envelope,
    _operation_status,
)

MEMBERS = (
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
)


class FramingTests(unittest.TestCase):
    def test_all_operation_replies_refuse_unqualified_native_finish(self):
        slot = SimpleNamespace(slot_id="slot", load_generation=1)
        status = SimpleNamespace(
            operation_ref=SimpleNamespace(
                runtime_instance_id="runtime", slot_ref=slot, operation_id="operation"
            ),
            cleanup_diagnostic=None,
            operation_diagnostic=None,
            state="completed",
            cleanup="confirmed",
            text="legacy transcript",
            finish_reason=None,
        )
        for reason in (None, "ambiguous"):
            status.finish_reason = reason
            result = _operation_status(status)
            self.assertEqual((result["state"], result["cleanup"]), ("failed", "confirmed"))
            self.assertEqual(result["error_code"], "finish_reason_unqualified")
            self.assertIsNone(result["text"])
            self.assertIsNone(result["finish_reason"])
        status.finish_reason = "stop"
        self.assertEqual(_operation_status(status)["text"], "legacy transcript")

    def test_closed_envelope_and_exact_uint64_correlation(self):
        base = {"version": 1, "exchange_id": 1, "operation": "hello", "payload": {}}
        self.assertEqual(_envelope(json.dumps(base).encode()), base)
        for changes in (
            {"exchange_id": True},
            {"exchange_id": 0},
            {"exchange_id": 1 << 64},
            {"version": True},
            {"operation": "manifest"},
            {"qualified": True},
        ):
            with self.assertRaises(PrivateChannelError):
                _envelope(json.dumps({**base, **changes}).encode())
        with self.assertRaises(PrivateChannelError):
            _envelope(b'{"version":1,"version":1,"exchange_id":1,"operation":"hello","payload":{}}')


@unittest.skipUnless(os.name == "posix", "Inherited Unix descriptor qualification")
class ProcessChannelTests(unittest.IsolatedAsyncioTestCase):
    async def launch(
        self, *, hold=False, hold_load=False, unqualified=False, tokens=b"\x02\x03\x00"
    ):
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.assets = {name: b"controlled:" + name.encode() for name in MEMBERS}
        self.assets["model.safetensors"] = tokens
        for name, data in self.assets.items():
            (root / name).write_bytes(data)
        source = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        parent, child = socket.socketpair()
        bootstrap = Path(__file__).with_name("owned_channel_fixture.py")
        args = [
            sys.executable,
            "-I",
            "-B",
            "-S",
            str(bootstrap),
            "--channel-fd",
            str(child.fileno()),
            "--fixture-source-fd",
            str(source),
        ]
        if hold:
            args.append("--hold-use-until-cancel")
        if hold_load:
            args.append("--hold-load-until-cancel")
        if unqualified:
            args.append("--unqualified")
        process = subprocess.Popen(
            args, pass_fds=(child.fileno(), source), stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        child.close()
        os.close(source)
        reader, writer = await asyncio.open_connection(sock=parent)
        self.addAsyncCleanup(self.stop, process, writer)
        self.reader, self.writer, self.process, self.root = reader, writer, process, root
        self.exchange = 0
        hello = await self.call("hello", {})
        self.assertFalse(hello["result"]["production_available"])
        self.runtime = hello["result"]["runtime_instance_id"]
        return hello

    async def stop(self, process, writer):
        writer.close()
        try:
            await writer.wait_closed()
        except (ConnectionError, OSError):
            pass
        try:
            output, error = await asyncio.to_thread(process.communicate, timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            output, error = await asyncio.to_thread(process.communicate, timeout=3)
        self.assertEqual(output, b"")
        self.assertNotIn(b"Traceback", error)

    async def send(self, operation, payload):
        self.exchange += 1
        value = {
            "version": 1,
            "exchange_id": self.exchange,
            "operation": operation,
            "payload": payload,
        }
        raw = json.dumps(value).encode()
        self.writer.write(struct.pack("!I", len(raw)) + raw)
        await self.writer.drain()
        return self.exchange

    async def receive(self):
        async with asyncio.timeout(3):
            size = struct.unpack("!I", await self.reader.readexactly(4))[0]
            self.assertLessEqual(size, 64 * 1024)
            result = json.loads(await self.reader.readexactly(size))
        self.assertEqual(result["version"], 1)
        self.assertEqual(
            set(result) & {"result", "error"}, {"result"} if "result" in result else {"error"}
        )
        return result

    async def call(self, operation, payload):
        ident = await self.send(operation, payload)
        reply = await self.receive()
        self.assertEqual((reply["exchange_id"], reply["operation"]), (ident, operation))
        return reply

    async def load(self):
        reply = await self.call(
            "load",
            {
                "runtime_instance_id": self.runtime,
                "model_id": "library/speech",
                "source_id": "fixture-selected",
            },
        )
        self.assertEqual(
            (reply["result"]["state"], reply["result"]["cleanup"]), ("ready", "retained")
        )
        self.slot = reply["result"]["slot"]
        return reply

    def typed_request(self):
        return {
            "contract_version": 1,
            "request_id": "parent-correlated-7",
            "model": "library/speech",
            "capability": "audio_transcription",
            "input": {
                "kind": "audio",
                "encoding": "pcm_s16le",
                "sample_rate_hz": 16000,
                "channels": 1,
                "sample_count": 1,
                "data_base64": "AAA=",
            },
            "output": "text",
            "options": {"kind": "audio", "language": "en"},
        }

    def identity(self, operation_id=None):
        result = {"runtime_instance_id": self.runtime, "slot": self.slot}
        if operation_id is not None:
            result["operation_id"] = operation_id
        return result

    async def use(self):
        result = await self.call("use", {**self.identity(), "request": self.typed_request()})
        self.operation_id = result["result"]["operation_id"]
        self.assertEqual(result["result"]["slot"], self.slot)
        return result

    async def test_selected_five_member_load_use_finish_then_exact_unload(self):
        await self.launch()
        await self.load()
        await self.use()
        status = await self.call(
            "status", {**self.identity(self.operation_id), "wait_for_settlement": True}
        )
        digest = hashlib.sha256()
        for name in MEMBERS:
            digest.update(name.encode() + b"\0" + self.assets[name])
        expected = f"controlled:{digest.hexdigest()};calls=1;pcm=0000"
        self.assertEqual(status["result"]["text"], expected)
        self.assertEqual(
            (
                status["result"]["state"],
                status["result"]["cleanup"],
                status["result"]["finish_reason"],
            ),
            ("completed", "confirmed", "stop"),
        )
        again = await self.call(
            "status", {**self.identity(self.operation_id), "wait_for_settlement": False}
        )
        self.assertEqual(again["result"]["text"], expected)
        unloaded = await self.call("unload", self.identity())
        self.assertEqual(
            (unloaded["result"]["state"], unloaded["result"]["cleanup"]), ("retired", "confirmed")
        )
        refused = await self.call("use", {**self.identity(), "request": self.typed_request()})
        self.assertIn("error", refused)
        closed = await self.call("close", {"runtime_instance_id": self.runtime})
        self.assertTrue(closed["result"]["custody_complete"])

    async def test_pending_status_can_receive_finite_cancel_out_of_order(self):
        await self.launch(hold=True)
        await self.load()
        await self.use()
        wait = await self.send(
            "status", {**self.identity(self.operation_id), "wait_for_settlement": True}
        )
        cancel = await self.send("cancel", self.identity(self.operation_id))
        replies = {
            reply["exchange_id"]: reply for reply in [await self.receive(), await self.receive()]
        }
        self.assertEqual(set(replies), {wait, cancel})
        self.assertEqual(replies[cancel]["operation"], "cancel")
        self.assertEqual(replies[wait]["operation"], "status")
        final = replies[wait]["result"]
        self.assertEqual((final["state"], final["cleanup"]), ("cancelled", "confirmed"))
        self.assertIsNone(final["text"])
        self.assertIsNone(final["finish_reason"])
        self.assertEqual(final["operation_id"], self.operation_id)
        await self.call("unload", self.identity())

    async def test_admitted_load_cancel_exchange_drains_original_clean_reply(self):
        await self.launch(hold_load=True)
        load = await self.send(
            "load",
            {
                "runtime_instance_id": self.runtime,
                "model_id": "library/speech",
                "source_id": "fixture-selected",
            },
        )
        async with asyncio.timeout(3):
            marker = await asyncio.to_thread(self.process.stderr.readline)
        self.assertEqual(marker.strip(), b"controlled load entered")
        cancel = await self.send("cancel_exchange", {"target_exchange_id": load})
        replies = {
            reply["exchange_id"]: reply for reply in [await self.receive(), await self.receive()]
        }
        self.assertEqual(
            replies[cancel]["result"], {"target_exchange_id": load, "cancellation_requested": True}
        )
        self.assertEqual(replies[load]["operation"], "load")
        self.assertEqual(
            (
                replies[load]["result"]["state"],
                replies[load]["result"]["cleanup"],
                replies[load]["result"]["error_code"],
            ),
            ("failed", "confirmed", "cancelled"),
        )
        finished = await self.call("cancel_exchange", {"target_exchange_id": load})
        self.assertEqual(
            finished["error"], {"code": "exchange_not_cancellable", "effect": "not_admitted"}
        )

    async def test_length_evidence_and_missing_terminal_refusal(self):
        await self.launch(tokens=b"\x01" * 512)
        await self.load()
        await self.use()
        status = await self.call(
            "status", {**self.identity(self.operation_id), "wait_for_settlement": True}
        )
        self.assertEqual(status["result"]["finish_reason"], "length")
        await self.call("unload", self.identity())

    async def test_wrong_identity_and_semantic_projection_have_no_native_retry(self):
        await self.launch()
        await self.load()
        wrong = {**self.identity(), "runtime_instance_id": "wrong", "request": self.typed_request()}
        refused = await self.call("use", wrong)
        self.assertEqual(refused["error"], {"code": "runtime_replaced", "effect": "not_admitted"})
        malformed = self.typed_request()
        malformed["output"] = "labels"
        refused = await self.call("use", {**self.identity(), "request": malformed})
        self.assertEqual(refused["error"]["effect"], "not_admitted")
        await self.use()
        status = await self.call(
            "status", {**self.identity(self.operation_id), "wait_for_settlement": True}
        )
        self.assertIn("calls=1", status["result"]["text"])
        await self.call("unload", self.identity())

    async def test_shipping_gate_stays_closed_and_wire_receipts_cannot_open_it(self):
        await self.launch(unqualified=True)
        payload = {
            "runtime_instance_id": self.runtime,
            "model_id": "library/speech",
            "source_id": "fixture-selected",
        }
        refusal = await self.call("load", payload)
        self.assertEqual(
            refusal["error"], {"code": "native_runtime_unqualified", "effect": "not_admitted"}
        )
        refusal = await self.call("load", {**payload, "qualified": True})
        self.assertEqual(refusal["error"], {"code": "invalid_request", "effect": "not_admitted"})

    async def test_changed_selected_member_refuses_cleanly_and_no_ready_slot(self):
        await self.launch()
        member = self.root / "tokenizer_config.json"
        member.write_bytes(b"changed" + member.read_bytes()[7:])
        refusal = await self.call(
            "load",
            {
                "runtime_instance_id": self.runtime,
                "model_id": "library/speech",
                "source_id": "fixture-selected",
            },
        )
        self.assertEqual(
            (refusal["result"]["state"], refusal["result"]["cleanup"]), ("failed", "confirmed")
        )

    async def test_partial_and_oversize_frames_fence_connection(self):
        await self.launch()
        self.writer.write(struct.pack("!I", MAX_REQUEST_BYTES + 1))
        await self.writer.drain()
        async with asyncio.timeout(3):
            self.assertEqual(await self.reader.read(), b"")

    async def test_partial_body_eof_fences_connection_without_success(self):
        await self.launch()
        self.writer.write(struct.pack("!I", 100) + b'{"version":1')
        await self.writer.drain()
        self.writer.write_eof()
        async with asyncio.timeout(3):
            self.assertEqual(await self.reader.read(), b"")


@unittest.skipUnless(os.name == "posix", "Inherited stdio pipe qualification")
class StdioChannelTests(unittest.IsolatedAsyncioTestCase):
    async def test_actual_inherited_stdio_pair_load_use_and_exact_unload(self):
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        for name in MEMBERS:
            (root / name).write_bytes(
                b"\x02\x00" if name == "model.safetensors" else b"controlled member"
            )
        source = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        bootstrap = Path(__file__).with_name("owned_channel_fixture.py")
        try:
            child = await asyncio.create_subprocess_exec(
                sys.executable,
                "-I",
                "-B",
                "-S",
                str(bootstrap),
                "--fixture-source-fd",
                str(source),
                pass_fds=(source,),
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        finally:
            os.close(source)
        exchange = 0

        async def call(operation, payload):
            nonlocal exchange
            exchange += 1
            raw = json.dumps(
                {"version": 1, "exchange_id": exchange, "operation": operation, "payload": payload}
            ).encode()
            child.stdin.write(struct.pack("!I", len(raw)) + raw)
            await child.stdin.drain()
            async with asyncio.timeout(3):
                size = struct.unpack("!I", await child.stdout.readexactly(4))[0]
                reply = json.loads(await child.stdout.readexactly(size))
            self.assertEqual((reply["exchange_id"], reply["operation"]), (exchange, operation))
            return reply["result"]

        try:
            hello = await call("hello", {})
            runtime = hello["runtime_instance_id"]
            self.assertFalse(hello["production_available"])
            loaded = await call(
                "load",
                {
                    "runtime_instance_id": runtime,
                    "model_id": "library/speech",
                    "source_id": "fixture-selected",
                },
            )
            identity = {"runtime_instance_id": runtime, "slot": loaded["slot"]}
            request = {
                "contract_version": 1,
                "request_id": "stdio-1",
                "model": "library/speech",
                "capability": "audio_transcription",
                "input": {
                    "kind": "audio",
                    "encoding": "pcm_s16le",
                    "sample_rate_hz": 16000,
                    "channels": 1,
                    "sample_count": 1,
                    "data_base64": "AAA=",
                },
                "output": "text",
                "options": {"kind": "audio"},
            }
            started = await call("use", {**identity, "request": request})
            settled = await call(
                "status",
                {**identity, "operation_id": started["operation_id"], "wait_for_settlement": True},
            )
            self.assertEqual(
                (settled["state"], settled["cleanup"], settled["finish_reason"]),
                ("completed", "confirmed", "stop"),
            )
            self.assertIn("calls=1", settled["text"])
            unloaded = await call("unload", identity)
            self.assertEqual((unloaded["state"], unloaded["cleanup"]), ("retired", "confirmed"))
        finally:
            child.stdin.close()
            try:
                async with asyncio.timeout(3):
                    output, error = await child.communicate()
            except TimeoutError:
                child.kill()
                output, error = await child.communicate()
            self.assertEqual(output, b"")
            self.assertNotIn(b"Traceback", error)
            self.assertEqual(child.returncode, 0)


if __name__ == "__main__":
    unittest.main()
