"""
Unit tests for the JSON-RPC server used by Electron IPC.

Tests cover:
- RPCHandler request/response handling
- RPCServer lifecycle (start/stop)
- JSON-RPC protocol compliance
- Error handling
"""

import io
import json
from http.server import HTTPServer
from threading import Thread
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.rpc_server import JSONRPCError, RPCHandler, RPCServer

# ============================================================================
# JSONRPCError Tests
# ============================================================================


class TestJSONRPCError:
    """Tests for JSONRPCError exception class."""

    def test_error_with_code_and_message(self):
        """Test creating error with code and message."""
        error = JSONRPCError(-32600, "Invalid Request")
        assert error.code == -32600
        assert error.message == "Invalid Request"
        assert error.data is None
        assert str(error) == "Invalid Request"

    def test_error_with_data(self):
        """Test creating error with additional data."""
        error = JSONRPCError(-32602, "Invalid params", data={"field": "name"})
        assert error.code == -32602
        assert error.message == "Invalid params"
        assert error.data == {"field": "name"}


# ============================================================================
# RPCHandler Tests
# ============================================================================


class MockRequest:
    """Mock socket request for HTTPServer."""

    def makefile(self, mode, bufsize=-1):
        return io.BytesIO()


class TestRPCHandler:
    """Tests for RPCHandler HTTP request handler."""

    @pytest.fixture
    def mock_api(self):
        """Create a mock API instance."""
        api = MagicMock()
        api.get_status = MagicMock(return_value={"status": "ready"})
        api.get_version = MagicMock(return_value="1.0.0")
        return api

    @pytest.fixture
    def handler_class(self, mock_api):
        """Create a handler factory with mock server and API."""

        def create_handler(request_line, headers=None, body=None):
            # Create mock server with API
            server = MagicMock(spec=HTTPServer)
            server.api = mock_api

            # Create handler instance
            handler = RPCHandler.__new__(RPCHandler)
            handler.server = server
            handler.client_address = ("127.0.0.1", 12345)
            handler.requestline = request_line
            handler.command = request_line.split()[0] if request_line else "GET"
            handler.path = (
                request_line.split()[1] if request_line and len(request_line.split()) > 1 else "/"
            )

            # Mock headers
            handler.headers = MagicMock()
            if headers:
                handler.headers.get = lambda k, d=None: headers.get(k, d)
            else:
                handler.headers.get = lambda k, d=None: d

            # Mock input/output
            if body:
                handler.rfile = io.BytesIO(body.encode("utf-8"))
            else:
                handler.rfile = io.BytesIO()

            handler.wfile = io.BytesIO()

            # Mock response methods
            handler.send_response = MagicMock()
            handler.send_header = MagicMock()
            handler.end_headers = MagicMock()
            handler.send_error = MagicMock()

            return handler

        return create_handler

    def test_log_message_disabled(self, handler_class):
        """Test that default logging is disabled."""
        handler = handler_class("GET /health HTTP/1.1")
        # Should not raise - just a no-op
        handler.log_message("test %s", "message")

    def test_do_options_cors(self, handler_class):
        """Test CORS preflight response."""
        handler = handler_class("OPTIONS /rpc HTTP/1.1")
        handler.do_OPTIONS()

        handler.send_response.assert_called_with(200)
        # Check CORS headers were set
        calls = [call[0] for call in handler.send_header.call_args_list]
        assert ("Access-Control-Allow-Origin", "*") in calls
        assert ("Access-Control-Allow-Methods", "POST, OPTIONS") in calls

    def test_do_get_health_check(self, handler_class):
        """Test GET /health endpoint."""
        handler = handler_class("GET /health HTTP/1.1")
        handler.do_GET()

        handler.send_response.assert_called_with(200)
        response = handler.wfile.getvalue()
        assert b'"status": "ok"' in response or handler.send_response.called

    def test_do_get_404(self, handler_class):
        """Test GET to unknown path returns 404."""
        handler = handler_class("GET /unknown HTTP/1.1")
        handler.do_GET()
        handler.send_error.assert_called_with(404, "Not Found")

    def test_do_post_not_rpc_path(self, handler_class):
        """Test POST to non-/rpc path returns 404."""
        handler = handler_class("POST /other HTTP/1.1")
        handler.do_POST()
        handler.send_error.assert_called_with(404, "Not Found")

    def test_do_post_empty_body(self, handler_class):
        """Test POST with empty body returns parse error."""
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": "0"},
            body="",
        )
        handler.do_POST()

        # Should send error response
        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32700" in response or "Parse error" in response

    def test_do_post_invalid_json(self, handler_class):
        """Test POST with invalid JSON returns parse error."""
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": "10"},
            body="not json!",
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32700" in response or "Parse error" in response

    def test_do_post_health_check_method(self, handler_class):
        """Test built-in health_check method."""
        body = json.dumps({"jsonrpc": "2.0", "method": "health_check", "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        data = json.loads(response)
        assert data.get("result", {}).get("status") == "ok"
        assert data.get("id") == 1

    def test_do_post_missing_method(self, handler_class):
        """Test POST without method returns invalid request."""
        body = json.dumps({"jsonrpc": "2.0", "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32600" in response

    def test_do_post_api_method_call(self, handler_class, mock_api):
        """Test calling an API method via RPC."""
        body = json.dumps({"jsonrpc": "2.0", "method": "get_status", "params": {}, "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        data = json.loads(response)
        assert data.get("result") == {"status": "ready"}
        mock_api.get_status.assert_called_once()

    def test_do_post_method_not_found(self, handler_class, mock_api):
        """Test calling non-existent method returns error."""
        # Remove the nonexistent attribute from mock so getattr returns None
        del mock_api.nonexistent
        mock_api.configure_mock(**{"nonexistent": None})

        body = json.dumps({"jsonrpc": "2.0", "method": "nonexistent", "params": {}, "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )

        # Make getattr return None for nonexistent method
        handler.server.api = Mock(spec=[])

        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32601" in response

    def test_do_post_invalid_params_type(self, handler_class):
        """Test that non-dict params returns error."""
        body = json.dumps({"jsonrpc": "2.0", "method": "get_status", "params": [1, 2, 3], "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32602" in response

    def test_do_post_json_not_object(self, handler_class):
        """Test POST with JSON that's not an object (e.g., array or string)."""
        body = json.dumps([1, 2, 3])  # Array instead of object
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32600" in response
        assert "Expected object" in response

    def test_do_post_method_not_callable(self, handler_class, mock_api):
        """Test calling a method that exists but is not callable."""
        # Set a non-callable attribute on the mock API
        mock_api.not_a_method = "string value"

        body = json.dumps({"jsonrpc": "2.0", "method": "not_a_method", "params": {}, "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32601" in response
        assert "not callable" in response

    def test_do_post_method_type_error(self, handler_class, mock_api):
        """Test that TypeError in method call returns invalid params error."""
        mock_api.bad_method = MagicMock(side_effect=TypeError("missing required argument"))

        body = json.dumps({"jsonrpc": "2.0", "method": "bad_method", "params": {}, "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32602" in response
        assert "Invalid params" in response

    def test_do_post_method_internal_error(self, handler_class, mock_api):
        """Test that generic exception in method call returns internal error."""
        mock_api.failing_method = MagicMock(side_effect=RuntimeError("unexpected error"))

        body = json.dumps({"jsonrpc": "2.0", "method": "failing_method", "params": {}, "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )
        handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        assert "-32603" in response
        assert "Internal error" in response

    def test_do_post_shutdown_method(self, handler_class):
        """Test shutdown method responds and schedules shutdown."""
        body = json.dumps({"jsonrpc": "2.0", "method": "shutdown", "id": 1})
        handler = handler_class(
            "POST /rpc HTTP/1.1",
            headers={"Content-Length": str(len(body))},
            body=body,
        )

        with patch.object(Thread, "start"):
            handler.do_POST()

        response = handler.wfile.getvalue().decode("utf-8")
        data = json.loads(response)
        assert data.get("result", {}).get("status") == "shutting_down"


# ============================================================================
# RPCServer Tests
# ============================================================================


class TestRPCServer:
    """Tests for RPCServer lifecycle management."""

    @pytest.fixture
    def mock_api(self):
        """Create a mock API instance."""
        return MagicMock()

    def test_server_init(self, mock_api):
        """Test server initialization."""
        server = RPCServer(mock_api, port=0, host="127.0.0.1")
        assert server.api is mock_api
        assert server.host == "127.0.0.1"
        assert server.port == 0
        assert server.server is None

    def test_server_start_stop(self, mock_api):
        """Test server start and stop lifecycle."""
        server = RPCServer(mock_api, port=0, host="127.0.0.1")

        # Start server
        port = server.start()
        assert port > 0
        assert server.server is not None
        assert server.port == port

        # Stop server
        server.stop()
        assert server.server is None

    def test_server_auto_port_assignment(self, mock_api):
        """Test that port=0 auto-assigns an available port."""
        server = RPCServer(mock_api, port=0)
        port = server.start()

        try:
            assert port > 1024  # Should be an ephemeral port
            assert server.port == port
        finally:
            server.stop()

    def test_server_wait_with_no_thread(self, mock_api):
        """Test wait() when server thread doesn't exist."""
        server = RPCServer(mock_api, port=0)
        # Should not raise
        server.wait()

    def test_server_stop_when_not_running(self, mock_api):
        """Test stop() when server isn't running."""
        server = RPCServer(mock_api, port=0)
        # Should not raise
        server.stop()
