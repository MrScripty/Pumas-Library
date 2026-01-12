#!/usr/bin/env python3
"""
JSON-RPC Server for Electron IPC

HTTP-based JSON-RPC 2.0 server that wraps the ComfyUISetupAPI
for communication with the Electron main process.
"""

import argparse
import asyncio
import json
import signal
import sys
from functools import partial
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from threading import Thread
from typing import Any, Callable

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from backend.api import ComfyUISetupAPI
from backend.logging_config import get_logger, setup_logging

# Initialize logging
setup_logging(log_level="INFO", console_level="INFO")
logger = get_logger(__name__)


class JSONRPCError(Exception):
    """JSON-RPC error with code and message."""

    def __init__(self, code: int, message: str, data: Any = None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.data = data


class RPCHandler(BaseHTTPRequestHandler):
    """HTTP request handler for JSON-RPC requests."""

    # Disable default logging
    def log_message(self, format: str, *args: Any) -> None:
        pass

    def _send_json_response(self, data: dict, status: int = 200) -> None:
        """Send a JSON response."""
        response_body = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response_body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(response_body)

    def _send_error(
        self,
        code: int,
        message: str,
        request_id: Any = None,
        http_status: int = 200,
    ) -> None:
        """Send a JSON-RPC error response."""
        error_response = {
            "jsonrpc": "2.0",
            "error": {"code": code, "message": message},
            "id": request_id,
        }
        self._send_json_response(error_response, http_status)

    def do_OPTIONS(self) -> None:
        """Handle CORS preflight requests."""
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self) -> None:
        """Handle GET requests (health check)."""
        if self.path == "/health":
            self._send_json_response({"status": "ok"})
        else:
            self.send_error(404, "Not Found")

    def do_POST(self) -> None:
        """Handle POST requests (JSON-RPC)."""
        if self.path != "/rpc":
            self.send_error(404, "Not Found")
            return

        # Read request body
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0:
            self._send_error(-32700, "Parse error: Empty request body")
            return

        try:
            request_body = self.rfile.read(content_length).decode("utf-8")
            request_data = json.loads(request_body)
        except json.JSONDecodeError as e:
            logger.warning("JSON parse error: %s", e)
            self._send_error(-32700, f"Parse error: {e}")
            return

        # Validate JSON-RPC request
        if not isinstance(request_data, dict):
            self._send_error(-32600, "Invalid Request: Expected object")
            return

        request_id = request_data.get("id")
        method = request_data.get("method")
        params = request_data.get("params", {})

        if not method:
            self._send_error(-32600, "Invalid Request: Missing method", request_id)
            return

        if not isinstance(params, dict):
            self._send_error(-32602, "Invalid params: Expected object", request_id)
            return

        # Get the API instance from the server
        api: ComfyUISetupAPI = self.server.api  # type: ignore

        # Handle built-in methods
        if method == "health_check":
            self._send_json_response(
                {"jsonrpc": "2.0", "result": {"status": "ok"}, "id": request_id}
            )
            return

        if method == "shutdown":
            logger.info("Shutdown requested via RPC")
            self._send_json_response(
                {"jsonrpc": "2.0", "result": {"status": "shutting_down"}, "id": request_id}
            )
            # Schedule shutdown
            Thread(target=self._shutdown_server, daemon=True).start()
            return

        # Look up method on API
        handler = getattr(api, method, None)
        if handler is None:
            self._send_error(-32601, f"Method not found: {method}", request_id)
            return

        if not callable(handler):
            self._send_error(-32601, f"Method not callable: {method}", request_id)
            return

        # Call the method
        try:
            logger.debug("RPC call: %s(%s)", method, params)
            result = handler(**params) if params else handler()
            self._send_json_response({"jsonrpc": "2.0", "result": result, "id": request_id})
        except TypeError as e:
            # Invalid parameters
            logger.warning("RPC parameter error for %s: %s", method, e)
            self._send_error(-32602, f"Invalid params: {e}", request_id)
        except Exception as e:  # noqa: generic-exception
            # Internal error - catch-all for unexpected API errors
            logger.error("RPC error for %s: %s", method, e, exc_info=True)
            self._send_error(-32603, f"Internal error: {e}", request_id)

    def _shutdown_server(self) -> None:
        """Shutdown the server after a brief delay."""
        import time

        time.sleep(0.5)
        self.server.shutdown()


class RPCServer:
    """JSON-RPC HTTP server wrapper."""

    def __init__(self, api: ComfyUISetupAPI, port: int = 0, host: str = "127.0.0.1"):
        self.api = api
        self.host = host
        self.port = port
        self.server: HTTPServer | None = None
        self._server_thread: Thread | None = None

    def start(self) -> int:
        """Start the RPC server and return the assigned port."""
        self.server = HTTPServer((self.host, self.port), RPCHandler)
        self.server.api = self.api  # type: ignore

        # Get the actual port (in case port=0 was used)
        actual_port = self.server.server_address[1]
        self.port = actual_port

        logger.info("RPC server starting on %s:%d", self.host, self.port)

        # Start server in a thread
        self._server_thread = Thread(target=self.server.serve_forever, daemon=True)
        self._server_thread.start()

        return self.port

    def stop(self) -> None:
        """Stop the RPC server."""
        if self.server:
            logger.info("Stopping RPC server")
            self.server.shutdown()
            self.server.server_close()
            self.server = None

        if self._server_thread:
            self._server_thread.join(timeout=5)
            self._server_thread = None

    def wait(self) -> None:
        """Wait for the server to stop."""
        if self._server_thread:
            self._server_thread.join()


def main() -> None:
    """Main entry point for the RPC server."""
    parser = argparse.ArgumentParser(description="Pumas Library RPC Server")
    parser.add_argument(
        "--port",
        type=int,
        default=0,
        help="Port to listen on (0 = auto-assign)",
    )
    parser.add_argument(
        "--host",
        type=str,
        default="127.0.0.1",
        help="Host to bind to",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging",
    )
    args = parser.parse_args()

    if args.debug:
        setup_logging(log_level="DEBUG", console_level="DEBUG")

    # Create API instance
    logger.info("Initializing API...")
    api = ComfyUISetupAPI()

    # Create and start server
    server = RPCServer(api, port=args.port, host=args.host)
    port = server.start()

    # Print port for Electron to read (intentional stdout for IPC)
    print(f"RPC_PORT={port}", flush=True)  # noqa: print

    # Handle shutdown signals
    def signal_handler(signum: int, frame: Any) -> None:
        logger.info("Received signal %d, shutting down", signum)
        server.stop()
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # Wait for server to stop
    logger.info("RPC server running on port %d", port)
    try:
        server.wait()
    except KeyboardInterrupt:
        logger.info("Interrupted, shutting down")
        server.stop()


if __name__ == "__main__":
    main()
