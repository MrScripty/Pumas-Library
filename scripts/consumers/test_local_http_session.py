"""Opt-in native CLI consumer tests. No models, downloads, inference or recovery.

PUMAS_CONSUMER_TEST_BINARY must name the independently pinned local producer.
PUMAS_CONSUMER_EVIDENCE optionally retains per-test native process diagnostics.
"""
import asyncio
import copy
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest.mock import patch

import local_http_session as consumer


class JsonFixtures(unittest.TestCase):
    def test_duplicate_and_nonfinite_json_is_refused(self):
        for data in (b'{"state":"retained","state":"revoked"}', b'{"version":NaN}'):
            with self.assertRaises(ValueError):
                consumer._decode(data)


class ContractFixtures(unittest.TestCase):
    def test_named_protocol_intersection_and_additive_contracts(self):
        protocols = consumer._advertisements([
            {"name": "pumas.local-http", "versions": [2, 1]},
            {"name": "future.protocol", "versions": [3]},
        ], protocols=True)
        self.assertIn(1, protocols["pumas.local-http"])
        self.assertNotIn(1, protocols["future.protocol"])
        self.assertEqual(consumer._advertisements([
            {"name": "pumas.http-owner-retention", "version": 1},
            {"name": "future.schema", "version": 2},
        ], protocols=False)["pumas.http-owner-retention"], 1)

    def test_ambiguous_or_malformed_advertisements_are_refused(self):
        for protocols in (True, False):
            field = "versions" if protocols else "version"
            valid = {"name": "pumas.local-http", field: [1] if protocols else 1}
            cases = [None, {}, [], [valid, valid], [{**valid, "extra": 1}],
                     [{**valid, "name": ""}], [{**valid, "name": []}]]
            for number in (True, "1", 1.0, 0, -1, 2**32):
                cases.append([{**valid, field: [number] if protocols else number}])
            if protocols:
                cases.extend([[{**valid, field: []}], [{**valid, field: 1}],
                              [{**valid, field: [1, 1]}]])
            for value in cases:
                with self.subTest(protocols=protocols, value=value), self.assertRaises(ValueError):
                    consumer._advertisements(value, protocols=protocols)


@unittest.skipUnless(sys.platform == "linux", "controlled Unix helper fixture")
class ChildReaderFixtures(unittest.IsolatedAsyncioTestCase):
    async def test_post_ready_reader_failure_drains_and_surfaces_after_exit_zero(self):
        # Actual controlled Python helper processes, not Pumas owner evidence.
        first = json.dumps({"state": "retained"})
        for name, bad_line in (("malformed", "PUMAS_LOCAL_RETENTION={invalid}"),
                               ("oversized", "x" * (128 * 1024)),
                               ("repeated", "PUMAS_LOCAL_RETENTION=" + first)):
            with self.subTest(name=name):
                bad_expression = "'x' * (128 * 1024)" if name == "oversized" else repr(bad_line)
                script = ("import os, signal, sys\n"
                          "def stop(*_):\n"
                          " os.write(1, b'drain-during-shutdown\\n' * 100000)\n"
                          " sys.exit(0)\n"
                          "signal.signal(signal.SIGTERM, stop)\n"
                          "print('PUMAS_LOCAL_RETENTION=' + " + repr(first) + ", flush=True)\n"
                          "print(" + bad_expression + ", flush=True)\n"
                          "os.write(1, b'drain-after-failure\\n' * 100000)\n"
                          "signal.pause()\n")
                process = await asyncio.create_subprocess_exec(
                    sys.executable, "-u", "-c", script, stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.DEVNULL, limit=consumer.MAX_DESCRIPTION)
                child = consumer._Child(process, b"PUMAS_LOCAL_RETENTION=", lambda _: None)
                try:
                    self.assertEqual(await child.ready(), {"state": "retained"})
                    async with asyncio.timeout(5):
                        while child.failure is None:
                            await asyncio.sleep(.01)
                    with self.assertRaisesRegex(RuntimeError, "reader failed"):
                        await asyncio.wait_for(child.close(), 5)
                    self.assertEqual(process.returncode, 0, "must drain rather than force-kill helper")
                    self.assertTrue(child.reader.done())
                finally:
                    if process.returncode is None:
                        process.kill()
                        await process.wait()


@unittest.skipUnless(sys.platform == "linux" and os.environ.get("PUMAS_CONSUMER_TEST_BINARY"),
                     "native Linux producer binary must be explicitly selected")
class NativeConsumerTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.binary = Path(os.environ["PUMAS_CONSUMER_TEST_BINARY"]).resolve(strict=True)
        with self.binary.open("rb") as source:
            self.binary_sha256 = hashlib.file_digest(source, "sha256").hexdigest()
        self.temp = tempfile.TemporaryDirectory(prefix="pumas-callable-cli-")
        self.fixture = Path(self.temp.name)
        self.root = self.fixture / "library"
        self.root.mkdir()
        self.sentinel = self.root / "existing-library-sentinel"
        self.sentinel.write_bytes(b"existing selected library")
        self.environment = {**os.environ, "XDG_CONFIG_HOME": str(self.fixture / "config"),
                            "PUMAS_REGISTRY_DB_PATH": str(self.fixture / "registry.db")}
        self.external = []
        self.sessions = []
        self.records = []

    async def asyncTearDown(self):
        for process, log in self.external:
            if process.returncode is None:
                process.terminate()
            await asyncio.wait_for(process.wait(), 15)
            log.seek(0)
            self.records.append({"event": "external-owner-reaped", "pid": process.pid,
                                 "exit_code": process.returncode, "log": log.read().decode(errors="replace")})
            log.close()
        for session in self.sessions:
            self.records.append({"event": "session-cleanup", "pids": session.child_pids,
                                 "cleanup_results": session.cleanup_results,
                                 "children": [{"pid": child.process.pid, "exit_code": child.process.returncode,
                                               "last_ack": child.last} for child in session._children]})
            self.assertTrue(all(child.process.returncode is not None for child in session._children))
        self.assertEqual(self.sentinel.read_bytes(), b"existing selected library")
        destination = os.environ.get("PUMAS_CONSUMER_EVIDENCE")
        if destination:
            directory = Path(destination)
            directory.mkdir(parents=True, exist_ok=True)
            (directory / (self._testMethodName + ".json")).write_text(json.dumps({
                "test": self._testMethodName, "binary_sha256": self.binary_sha256,
                "records": self.records, "sentinel_preserved": True,
                "limits": "Linux native producer processes; no real models or external consumer sources"}, indent=2) + "\n")
        self.temp.cleanup()

    def context(self, *, allow_start=False):
        return consumer.local_http_session(self.binary, self.root, binary_sha256=self.binary_sha256,
                                           allow_start=allow_start, environment=self.environment)

    def remember(self, session, event):
        self.sessions.append(session)
        self.records.append({"event": event, "ownership": session.ownership,
                             "description": session.description, "pids": session.child_pids})

    def rows(self):
        with sqlite3.connect(self.environment["PUMAS_REGISTRY_DB_PATH"]) as database:
            return database.execute("SELECT started_at FROM instances").fetchall()

    async def external_owner(self, port=0):
        log = tempfile.TemporaryFile()
        process = await asyncio.create_subprocess_exec(
            str(self.binary), "--attach-or-start-local-http", "--launcher-root", str(self.root),
            "--port", str(port), env=self.environment, stdout=log, stderr=asyncio.subprocess.STDOUT)
        self.external.append((process, log))
        async with asyncio.timeout(20):
            while True:
                log.seek(0)
                lines = [line for line in log.read().splitlines() if line.startswith(b"PUMAS_LOCAL_ACCESS=")]
                if lines:
                    ack = consumer._decode(lines[-1].split(b"=", 1)[1])
                    self.assertEqual(ack["ownership"], "owned")
                    self.records.append({"event": "external-owner-ready", "pid": process.pid,
                                         "description": ack["description"]})
                    return process, ack["description"]
                self.assertIsNone(process.returncode)
                await asyncio.sleep(.02)

    async def assert_models(self, session):
        reply = await session.rpc("get_models")
        self.assertNotIn("error", reply)
        self.assertEqual(reply["result"]["models"], {})
        self.records.append({"event": "actual-fenced-rpc-completed", "response": reply})

    async def test_owned_start_borrowed_retention_and_context_cleanup(self):
        async with self.context(allow_start=True) as owned:
            self.remember(owned, "owned-start-authority-consumed")
            self.assertEqual(owned.ownership, "owned")
            changed_observation = owned.description
            changed_observation["service_generation"] = "cannot-replace-held-fence"
            self.assertNotEqual(changed_observation, owned.description)
            await self.assert_models(owned)
            async with self.context() as borrowed:
                self.remember(borrowed, "authenticated-borrowed-retention")
                self.assertEqual(borrowed.ownership, "borrowed")
                self.assertEqual(owned.description, borrowed.description)
                await self.assert_models(borrowed)
            self.assertIsNone(owned._bootstrap.process.returncode)
            self.assertEqual(borrowed.cleanup_results[0]["exit_code"], 0)
            await self.assert_models(owned)
        self.assertEqual(self.rows(), [])

        async with self.context(allow_start=True) as restarted:
            self.remember(restarted, "clean-restart-after-owned-context")
            self.assertNotEqual(restarted.description["instance"]["generation"],
                                owned.description["instance"]["generation"])
            await self.assert_models(restarted)
        self.assertEqual(self.rows(), [])

    async def test_generic_modality_http_uses_actual_owned_and_borrowed_selection(self):
        async with self.context(allow_start=True) as owned:
            self.remember(owned, "generic-http-actual-owned-selection")
            async with self.context() as borrowed:
                self.remember(borrowed, "generic-http-actual-borrowed-selection")
                capabilities = await borrowed.capabilities("operator-missing-model")
                self.assertEqual(capabilities["status"], 404)
                self.assertEqual(capabilities["body"]["error"],
                                 {"code": "model_not_found", "outcome": "not_admitted"})
                for kind, source in (("text", {"kind": "text", "text": "hello"}),
                                     ("audio", {"kind": "audio", "encoding": "pcm_s16le",
                                                "sample_rate_hz": 16000, "channels": 1,
                                                "sample_count": 1, "data_base64": "AAA="})):
                    value = {"contract_version": 1, "request_id": f"actual-generic-{kind}",
                             "model": "operator-missing-model", "input": source, "output": "text"}
                    reply = await borrowed.model_operation(value)
                    self.assertEqual(reply["status"], 404)
                    self.assertEqual(reply["body"], {"contract_version": 1,
                        "request_id": value["request_id"],
                        "error": {"code": "model_not_found", "outcome": "not_admitted"}})
                    self.records.append({"event": "actual-generic-modality-refused", "request": value,
                                         "reply": reply, "runtime_or_model_loaded": False})
            self.assertIsNone(owned._bootstrap.process.returncode)
            await self.assert_models(owned)
        self.assertEqual(self.rows(), [])

    async def test_generic_http_stale_fence_is_refused_by_actual_native_owner(self):
        async with self.context(allow_start=True) as held:
            self.remember(held, "generic-http-actual-fenced-owner")
            original = held._fence
            try:
                held._fence = ("controlled-stale-instance", original[1])
                reply = await held.model_operation({"contract_version": 1, "request_id": "stale-generic-1",
                    "model": "operator-missing-model", "input": {"kind": "text", "text": "hello"},
                    "output": "text"})
                self.assertEqual(reply["status"], 409)
                self.assertIsNone(reply["body"], "pre-handler fencing supplies no operation envelope")
                self.records.append({"event": "actual-native-refuses-controlled-stale-fence", "reply": reply})
            finally:
                held._fence = original
            await self.assert_models(held)
        self.assertEqual(self.rows(), [])

    async def test_native_selection_contracts_and_controlled_compatibility_refusals(self):
        owner, observed = await self.external_owner()
        async with self.context() as held:
            self.remember(held, "actual-authenticated-compatible-native-selection")
            await self.assert_models(held)
            observed = held.description
        compatible = copy.deepcopy(observed)
        compatible["build_info"]["package_version"] = "unrelated-product-version"
        compatible["build_info"]["compiled_features"] = ["does-not-grant-readiness"]
        compatible["build_info"]["protocols"].append({"name": "future.protocol", "versions": [2]})
        for item in compatible["build_info"]["protocols"]:
            if item["name"] == "pumas.local-http":
                item["versions"] = [2, 1]
        compatible["build_info"]["schemas"].append({"name": "future.schema", "version": 2})
        self.assertEqual(consumer._selection(compatible, self.root),
                         consumer._selection(observed, self.root))
        refused = []
        for versions in ([], [2], [True]):
            value = copy.deepcopy(observed)
            for item in value["build_info"]["protocols"]:
                if item["name"] == "pumas.local-http":
                    item["versions"] = versions
            refused.append(value)
        for name in ("pumas.http-advertisement", "pumas.http-admission-fence", "pumas.http-owner-retention"):
            for changed in (None, 2, True, "duplicate"):
                value = copy.deepcopy(observed)
                schemas = value["build_info"]["schemas"]
                selected = next(item for item in schemas if item["name"] == name)
                if changed is None:
                    schemas.remove(selected)
                elif changed == "duplicate":
                    schemas.append(copy.deepcopy(selected))
                else:
                    selected["version"] = changed
                refused.append(value)
        for index, value in enumerate(refused):
            async def fixture_observation(_session):
                return value
            with patch.object(consumer.LocalHttpSession, "_describe", fixture_observation), \
                    patch.object(consumer.LocalHttpSession, "_spawn", side_effect=AssertionError(
                        "incompatible observation must not acquire a retention child")) as spawn:
                # Changed descriptions are controlled client-decoder fixtures,
                # not advertisements emitted by the actual native producer.
                with self.assertRaises(ValueError):
                    async with self.context():
                        self.fail("incompatible selection admitted")
                spawn.assert_not_called()
            self.records.append({"event": "controlled-advertisement-refused", "case": index})
        self.assertIsNone(owner.returncode)
        async with self.context() as held:
            self.remember(held, "actual-owner-untouched-after-controlled-refusals")
            await self.assert_models(held)

    async def test_operator_shutdown_revokes_and_same_url_restart_requires_fresh_context(self):
        operator, description = await self.external_owner()
        async with self.context() as held:
            self.remember(held, "borrower-held-during-operator-shutdown")
            await self.assert_models(held)
            operator.terminate()
            self.assertEqual(await asyncio.wait_for(operator.wait(), 15), 0)
            self.assertEqual(await asyncio.wait_for(held._retention.process.wait(), 15), 0)
            await held._retention.reader
            self.assertEqual(held._retention.last["state"], "revoked")
            self.assertEqual(self.rows(), [])
            successor, fresh = await self.external_owner(consumer._selection(description, self.root)[0].port)
            self.assertEqual(fresh["endpoint"], description["endpoint"])
            self.assertNotEqual(fresh["service_generation"], description["service_generation"])
            with self.assertRaisesRegex(RuntimeError, "availability unconfirmed"):
                await held.rpc("get_models")
            # Actual HTTP fence refusal, bypassing the reference's earlier closed-hold check.
            url, fence = consumer._selection(description, self.root)
            reader, writer = await asyncio.open_connection(url.hostname, url.port)
            writer.write((f"GET /health HTTP/1.1\r\nHost: {url.netloc}\r\n"
                          f"Pumas-Instance-Generation: {fence[0]}\r\n"
                          f"Pumas-Service-Generation: {fence[1]}\r\nConnection: close\r\n\r\n").encode())
            await writer.drain()
            status = await reader.readline()
            self.assertIn(b"412", status)
            writer.close()
            await writer.wait_closed()
            self.records.append({"event": "actual-same-url-old-fence-refused", "status": status.decode().strip()})
            async with self.context() as new:
                self.remember(new, "fresh-context-after-same-url-restart")
                self.assertEqual(new.description, fresh)
                await self.assert_models(new)
            self.assertIsNone(successor.returncode)

    async def test_rpc_cancellation_closes_transport_after_actual_response_headers(self):
        async with self.context(allow_start=True) as session:
            self.remember(session, "actual-request-cancellation")
            received_headers = asyncio.Event()
            blocked_observer = asyncio.Event()
            original = asyncio.StreamReader.readexactly

            async def gate(reader, count):
                # Controlled client-observation fixture after actual producer HTTP200.
                received_headers.set()
                await blocked_observer.wait()
                return await original(reader, count)

            with patch.object(asyncio.StreamReader, "readexactly", gate):
                request = asyncio.create_task(session.rpc("get_models"))
                await asyncio.wait_for(received_headers.wait(), 10)
                request.cancel()
                with self.assertRaises(asyncio.CancelledError):
                    await request
            self.records.append({"event": "cancelled-after-real-producer-headers",
                                 "fixture": "controlled client response-observation gate",
                                 "nonclaim": "No remote-effect cessation or inference cancellation inferred"})
            self.assertIsNone(session._bootstrap.process.returncode)
            await self.assert_models(session)
        self.assertEqual(self.rows(), [])

    async def test_repeated_owned_context_cancellation_reaps_guard_and_owner(self):
        active = asyncio.Event()
        retained = []

        async def work():
            async with self.context(allow_start=True) as session:
                retained.append(session)
                self.remember(session, "owned-context-before-cancellation")
                await self.assert_models(session)
                active.set()
                await asyncio.Future()

        worker = asyncio.create_task(work())
        await asyncio.wait_for(active.wait(), 20)
        worker.cancel()
        await asyncio.sleep(0)
        worker.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await worker
        self.assertEqual(len(retained[0].cleanup_results), 2)
        self.assertEqual([row["exit_code"] for row in retained[0].cleanup_results], [0, 0])
        self.assertEqual(self.rows(), [])

    async def test_cancelled_borrowed_context_leaves_external_owner_healthy(self):
        operator, _ = await self.external_owner()
        active = asyncio.Event()

        async def work():
            async with self.context(allow_start=True) as session:
                self.remember(session, "explicit-bootstrap-returned-borrowed")
                self.assertEqual(session.ownership, "borrowed")
                await self.assert_models(session)
                active.set()
                await asyncio.Future()

        worker = asyncio.create_task(work())
        await asyncio.wait_for(active.wait(), 20)
        worker.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await worker
        self.assertIsNone(operator.returncode)
        async with self.context() as next_session:
            self.remember(next_session, "borrow-after-context-cancellation")
            await self.assert_models(next_session)

    async def test_owner_crash_remains_transport_failure_and_refuses_new_start(self):
        operator, description = await self.external_owner()
        with self.assertRaises(ExceptionGroup):
            async with self.context() as held:
                self.remember(held, "held-before-owner-sigkill")
                operator.kill()
                self.assertEqual(await asyncio.wait_for(operator.wait(), 15), -9)
                self.assertNotEqual(await asyncio.wait_for(held._retention.process.wait(), 15), 0)
                await held._retention.reader
                self.assertEqual(held._retention.last["state"], "retained")
                with self.assertRaisesRegex(RuntimeError, "availability unconfirmed"):
                    await held.rpc("get_models")
        self.assertEqual(self.rows(), [(description["instance"]["generation"],)])
        with self.assertRaises(ExceptionGroup):
            async with self.context(allow_start=True):
                self.fail("dead row authorized automatic restart")
        self.assertEqual(self.rows(), [(description["instance"]["generation"],)])
        self.records.append({"event": "dead-owner-row-retained-reopen-refused", "generation": self.rows()[0][0]})

    async def test_cancelled_spawn_observes_actual_child_creation_before_cleanup(self):
        # Controlled creator gate; the child itself is an actual local producer.
        created = asyncio.Event()
        release = asyncio.Event()
        original = asyncio.create_subprocess_exec
        processes = []

        async def gate(*args, **kwargs):
            process = await original(*args, **kwargs)
            if "--attach-or-start-local-http" in args:
                processes.append(process)
                async with asyncio.timeout(20):
                    while b"PUMAS_LOCAL_ACCESS=" not in process.stdout._buffer:
                        await asyncio.sleep(.01)
                created.set()
                await release.wait()
            return process

        async def work():
            async with self.context(allow_start=True):
                self.fail("cancelled creation yielded a context")

        with patch.object(asyncio, "create_subprocess_exec", gate):
            worker = asyncio.create_task(work())
            await asyncio.wait_for(created.wait(), 20)
            worker.cancel()
            await asyncio.sleep(.05)
            self.assertFalse(worker.done(), "actual creation must settle before context cleanup")
            release.set()
            with self.assertRaises(asyncio.CancelledError):
                await worker
        self.assertEqual(processes[0].returncode, 0)
        self.assertEqual(self.rows(), [])
        self.records.append({"event": "cancelled-creation-child-reaped", "pid": processes[0].pid,
                             "exit_code": processes[0].returncode,
                             "fixture": "controlled creator return gate after native bootstrap readiness"})

    async def test_changed_binary_pin_refuses_before_start(self):
        with self.assertRaisesRegex(ValueError, "pinned binary bytes changed"):
            async with consumer.local_http_session(self.binary, self.root, binary_sha256="0" * 64,
                                                   allow_start=True, environment=self.environment):
                self.fail("changed binary was accepted")
        self.assertFalse(Path(self.environment["PUMAS_REGISTRY_DB_PATH"]).exists())
        self.records.append({"event": "wrong-pin-refused-before-process-start"})

    async def test_executable_entry_point_cwd_arguments_and_no_start_fallback(self):
        operator, description = await self.external_owner()
        script = Path(consumer.__file__).resolve()

        async def run(arguments):
            process = await asyncio.create_subprocess_exec(
                sys.executable, str(script), str(self.binary), str(self.root), *arguments,
                cwd=self.fixture, env=self.environment, stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE)
            stdout, stderr = await asyncio.wait_for(process.communicate(), 20)
            self.records.append({"event": "reference-executable-reaped", "pid": process.pid,
                                 "exit_code": process.returncode, "arguments": arguments,
                                 "stdout": stdout.decode(), "stderr": stderr.decode()})
            return process.returncode, stdout

        code, stdout = await run(["--binary-sha256", self.binary_sha256])
        self.assertEqual(code, 0)
        selection, reply = [consumer._decode(line) for line in stdout.splitlines()]
        self.assertEqual(selection, {"ownership": "borrowed", "description": description})
        self.assertEqual(reply["result"]["models"], {})
        self.assertIsNone(operator.returncode)
        self.assertEqual((await run([]))[0], 2)
        self.assertEqual((await run(["--binary-sha256", "0" * 64]))[0], 1)
        self.assertIsNone(operator.returncode)
        operator.terminate()
        self.assertEqual(await asyncio.wait_for(operator.wait(), 15), 0)
        self.assertEqual(self.rows(), [])
        self.assertEqual((await run(["--binary-sha256", self.binary_sha256]))[0], 1)
        self.assertEqual(self.rows(), [], "read-only executable never falls back to startup")
        self.assertEqual((await run(["--binary-sha256", self.binary_sha256, "--allow-start"]))[0], 0)
        self.assertEqual(self.rows(), [])


if __name__ == "__main__":
    unittest.main()
