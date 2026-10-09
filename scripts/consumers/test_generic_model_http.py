"""Controlled HTTP peer tests of the reference, never provider qualification."""
import asyncio
import json
from pathlib import Path
import unittest
import urllib.parse

import local_http_session as consumer


def request():
    return {"contract_version": 1, "request_id": "reference-17", "model": "selected-local-model",
            "input": {"kind": "text_batch", "texts": ["first", "second"]},
            "output": "embeddings_float32"}


class ControlledHttpPeerTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.calls = []
        self.status = 200
        self.reply = {"contract_version": 1, "request_id": "reference-17",
                      "result": {"kind": "embeddings", "vectors": [[0.5], [-0.5]]}}
        self.accepted = asyncio.Event()
        self.disconnected = asyncio.Event()
        self.block = False
        self.server = await asyncio.start_server(self.peer, "127.0.0.1", 0)
        self.session = consumer.LocalHttpSession(Path("unused-controlled-binary"), Path("."), {})
        # This intentionally bypasses CLI authentication only in a controlled
        # transport fixture. Separate native tests select and retain real owners.
        self.session._check_available = lambda: None
        port = self.server.sockets[0].getsockname()[1]
        self.session._url = urllib.parse.urlsplit(f"http://127.0.0.1:{port}")
        self.session._fence = ("controlled-instance", "controlled-service")

    async def asyncTearDown(self):
        self.server.close()
        await self.server.wait_closed()

    async def peer(self, reader, writer):
        try:
            header = (await reader.readuntil(b"\r\n\r\n")).decode("ascii").split("\r\n")
            fields = dict(line.split(": ", 1) for line in header[1:] if line)
            body = await reader.readexactly(int(fields["Content-Length"]))
            self.calls.append({"line": header[0], "headers": fields,
                               "body": json.loads(body) if body else None})
            self.accepted.set()
            if self.block:
                await reader.read()
                self.disconnected.set()
            else:
                data = json.dumps(self.reply).encode()
                writer.write(f"HTTP/1.1 {self.status} Fixture\r\nContent-Type: application/json\r\n"
                             f"Content-Length: {len(data)}\r\nConnection: close\r\n\r\n".encode() + data)
                await writer.drain()
        finally:
            writer.close()
            await writer.wait_closed()

    async def test_generic_request_and_typed_result_preserve_both_generation_fences(self):
        value = request()
        reply = await self.session.model_operation(value)
        self.assertEqual(reply, {"status": 200, "body": self.reply})
        self.assertEqual(self.calls[0]["line"], "POST /v1/model-operations HTTP/1.1")
        self.assertEqual(self.calls[0]["body"], value)
        self.assertNotIn("capability", self.calls[0]["body"])
        self.assertEqual(self.calls[0]["headers"]["Pumas-Instance-Generation"], "controlled-instance")
        self.assertEqual(self.calls[0]["headers"]["Pumas-Service-Generation"], "controlled-service")

    async def test_capabilities_use_existing_query_and_supported_version_advertisement(self):
        self.reply = {"supported_contract_versions": [2, 1], "model": "canonical-local-id",
                      "profile": "selected-profile", "max_request_bytes": consumer.MAX_OPERATION,
                      "max_response_bytes": consumer.MAX_OPERATION, "max_stream_event_bytes": 262144,
                      "capabilities": []}
        result = await self.session.capabilities("local/model+alias", profile="profile+one")
        self.assertEqual(result, {"status": 200, "body": self.reply})
        target = self.calls[0]["line"].split()[1]
        self.assertEqual(urllib.parse.parse_qs(urllib.parse.urlsplit(target).query),
                         {"model": ["local/model+alias"], "profile": ["profile+one"]})
        self.assertEqual(self.calls[0]["headers"]["Pumas-Service-Generation"], "controlled-service")

    async def test_unknown_admitted_error_is_returned_once_without_retry(self):
        self.status = 503
        self.reply = {"contract_version": 1, "request_id": "reference-17",
                      "error": {"code": "provider_failure", "outcome": "unknown"}}
        self.assertEqual(await self.session.model_operation(request()), {"status": 503, "body": self.reply})
        self.assertEqual(len(self.calls), 1)

    async def test_response_correlation_mismatch_is_not_replayed(self):
        self.reply["request_id"] = "a-different-request"
        with self.assertRaisesRegex(ValueError, "identity changed"):
            await self.session.model_operation(request())
        self.assertEqual(len(self.calls), 1)

    async def test_streaming_and_named_capability_requests_refuse_before_dispatch(self):
        for value in ({**request(), "stream": True}, {**request(), "capability": "text_embedding"}):
            with self.assertRaisesRegex(ValueError, "finite modality-first"):
                await self.session.model_operation(value)
        self.assertEqual(self.calls, [])

    async def test_cancellation_after_dispatch_closes_only_transport_without_replay(self):
        self.block = True
        task = asyncio.create_task(self.session.model_operation(request()))
        try:
            await asyncio.wait_for(self.accepted.wait(), 2)
            self.assertFalse(task.done())
            task.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await task
            await asyncio.wait_for(self.disconnected.wait(), 2)
            self.assertEqual(len(self.calls), 1)
        finally:
            if not task.done():
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
