#!/usr/bin/env python3
"""Readonly dashboard preview proxy.

This proxy intentionally exposes only the public dashboard preview page,
dashboard static assets, and the readonly bootstrap payload. It does not
forward admin routes or session token routes.
"""

from __future__ import annotations

import os
import urllib.error
import urllib.parse
import urllib.request
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


UPSTREAM_BASE = os.environ.get("DASHBOARD_PREVIEW_UPSTREAM", "http://127.0.0.1:8787").rstrip("/")
LISTEN_HOST = os.environ.get("DASHBOARD_PREVIEW_HOST", "127.0.0.1")
LISTEN_PORT = int(os.environ.get("DASHBOARD_PREVIEW_PORT", "8788"))
PREVIEW_ROOT = "/dashboard-preview/"
BOOTSTRAP_PATH = "/public/dashboard/bootstrap"
ASSET_PREFIX = "/dashboard/"
ROOT_STATIC_FILES = {
    "/app.css",
    "/app.js",
    "/charts-chartjs.html",
    "/icons-feather.html",
}
ROOT_STATIC_PREFIXES = ("/js/", "/css/", "/fonts/", "/img/")


def target_path(raw_path: str) -> str | None:
    parsed = urllib.parse.urlsplit(raw_path)
    path = parsed.path or "/"
    query = f"?{parsed.query}" if parsed.query else ""
    if path in {"/", "/index.html", "/dashboard-preview", "/dashboard-preview/"}:
        return f"{PREVIEW_ROOT}{query}"
    if path == BOOTSTRAP_PATH:
        return f"{BOOTSTRAP_PATH}{query}"
    if path.startswith(ASSET_PREFIX):
        return f"{path}{query}"
    if path in ROOT_STATIC_FILES or path.startswith(ROOT_STATIC_PREFIXES):
        return f"/dashboard{path}{query}"
    return None


class PreviewProxyHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self) -> None:
        upstream_path = target_path(self.path)
        if not upstream_path:
            self.send_error(HTTPStatus.NOT_FOUND, "preview route not found")
            return
        self.proxy_upstream(f"{UPSTREAM_BASE}{upstream_path}")

    def proxy_upstream(self, url: str) -> None:
        request = urllib.request.Request(
            url,
            headers={
                "User-Agent": "dashboard-preview-proxy/1.0",
                "Accept": self.headers.get("Accept", "*/*"),
            },
            method="GET",
        )
        try:
            with urllib.request.urlopen(request, timeout=15) as response:
                body = response.read()
                self.send_response(response.status)
                for key, value in response.headers.items():
                    key_lower = key.lower()
                    if key_lower in {"content-type", "content-length", "cache-control", "etag", "last-modified"}:
                        self.send_header(key, value)
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(body)
        except urllib.error.HTTPError as error:
            body = error.read()
            self.send_response(error.code)
            self.send_header("Content-Type", error.headers.get("Content-Type", "text/plain; charset=utf-8"))
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)
        except Exception as error:  # noqa: BLE001
            message = f"preview upstream unavailable: {error}".encode("utf-8", "replace")
            self.send_response(HTTPStatus.BAD_GATEWAY)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(message)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(message)

    def log_message(self, fmt: str, *args: object) -> None:
        return


class ReusableThreadingHTTPServer(ThreadingHTTPServer):
    allow_reuse_address = True


def main() -> None:
    server = ReusableThreadingHTTPServer((LISTEN_HOST, LISTEN_PORT), PreviewProxyHandler)
    server.serve_forever()


if __name__ == "__main__":
    main()
