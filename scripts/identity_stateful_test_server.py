#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


STATEFUL_HTML_TEMPLATE = """<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>identity-stateful-loading</title>
  </head>
  <body>
    <pre id="state">booting</pre>
    <script>
      (() => {
        const params = new URLSearchParams(window.location.search);
        const slot = params.get("slot") || "default";
        const cookieName = `pp_${slot}_cookie`;
        const localKey = `pp_${slot}_local`;
        const sessionKey = `pp_${slot}_session`;
        const iterationKey = `pp_${slot}_iteration`;
        const readCookie = (name) => {
          const token = document.cookie
            .split("; ")
            .find((item) => item.startsWith(`${name}=`));
          return token ? decodeURIComponent(token.split("=").slice(1).join("=")) : "";
        };

        const previousCookie = readCookie(cookieName) || "none";
        const previousLocal = window.localStorage.getItem(localKey) || "none";
        const previousSession = window.sessionStorage.getItem(sessionKey) || "none";
        const nextIteration = Number(window.localStorage.getItem(iterationKey) || "0") + 1;

        const nextCookie = `${slot}-cookie-${nextIteration}`;
        const nextLocal = JSON.stringify({ slot, iteration: nextIteration, previousLocal });
        const nextSession = JSON.stringify({ slot, iteration: nextIteration, previousSession });

        document.cookie = `${cookieName}=${encodeURIComponent(nextCookie)}; path=/; max-age=3600; SameSite=Lax`;
        window.localStorage.setItem(localKey, nextLocal);
        window.sessionStorage.setItem(sessionKey, nextSession);
        window.localStorage.setItem(iterationKey, String(nextIteration));

        const payload = {
          slot,
          iteration: nextIteration,
          previousCookie,
          previousLocal,
          previousSession,
          currentCookie: readCookie(cookieName) || "none",
          currentLocal: window.localStorage.getItem(localKey) || "none",
          currentSession: window.sessionStorage.getItem(sessionKey) || "none"
        };

        document.title = `stateful slot=${slot} iteration=${nextIteration} cookie=${payload.currentCookie}`;
        document.getElementById("state").textContent = JSON.stringify(payload, null, 2);
      })();
    </script>
  </body>
</html>
"""


FORM_HTML_TEMPLATE = """<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>identity-form-loading</title>
    <style>
      body { font-family: sans-serif; margin: 24px; }
      form { display: grid; gap: 12px; max-width: 480px; }
      input, button, textarea { font: inherit; padding: 8px 10px; }
      pre { margin-top: 20px; background: #f4f4f4; padding: 12px; white-space: pre-wrap; }
    </style>
  </head>
  <body>
    <h1>identity form stateful</h1>
    <form id="writer">
      <label>
        Message
        <input id="message" name="message" autocomplete="off" />
      </label>
      <label>
        Notes
        <textarea id="notes" name="notes" rows="3"></textarea>
      </label>
      <button id="submit-btn" type="submit">Submit</button>
    </form>
    <pre id="state">booting</pre>
    <script>
      (() => {
        const params = new URLSearchParams(window.location.search);
        const slot = params.get("slot") || "default";
        const cookieName = `pp_${slot}_form_cookie`;
        const localKey = `pp_${slot}_form_local`;
        const sessionKey = `pp_${slot}_form_session`;
        const iterationKey = `pp_${slot}_form_iteration`;
        const form = document.getElementById("writer");
        const messageInput = document.getElementById("message");
        const notesInput = document.getElementById("notes");
        const stateNode = document.getElementById("state");

        const readCookie = (name) => {
          const token = document.cookie
            .split("; ")
            .find((item) => item.startsWith(`${name}=`));
          return token ? decodeURIComponent(token.split("=").slice(1).join("=")) : "";
        };

        const previousCookie = readCookie(cookieName) || "none";
        const previousLocal = window.localStorage.getItem(localKey) || "none";
        const previousSession = window.sessionStorage.getItem(sessionKey) || "none";
        const nextIteration = Number(window.localStorage.getItem(iterationKey) || "0") + 1;
        const defaultMessage = `${slot}-message-${nextIteration}`;
        const defaultNotes = `notes-${slot}-${nextIteration}`;

        messageInput.value = defaultMessage;
        notesInput.value = defaultNotes;

        const render = (phase, submitted) => {
          const payload = {
            slot,
            phase,
            iteration: nextIteration,
            previousCookie,
            previousLocal,
            previousSession,
            currentCookie: readCookie(cookieName) || "none",
            currentLocal: window.localStorage.getItem(localKey) || "none",
            currentSession: window.sessionStorage.getItem(sessionKey) || "none",
            submitted
          };
          document.title = `form-stateful slot=${slot} phase=${phase} iteration=${nextIteration}`;
          stateNode.textContent = JSON.stringify(payload, null, 2);
        };

        render("ready", null);

        form.addEventListener("submit", (event) => {
          event.preventDefault();
          const submitted = {
            message: messageInput.value,
            notes: notesInput.value
          };
          document.cookie = `${cookieName}=${encodeURIComponent(JSON.stringify(submitted))}; path=/; max-age=3600; SameSite=Lax`;
          window.localStorage.setItem(localKey, JSON.stringify({ slot, iteration: nextIteration, submitted }));
          window.sessionStorage.setItem(sessionKey, JSON.stringify({ slot, iteration: nextIteration, submitted }));
          window.localStorage.setItem(iterationKey, String(nextIteration));
          render("submitted", submitted);
        });
      })();
    </script>
  </body>
</html>
"""


class Handler(BaseHTTPRequestHandler):
    server_version = "PersonaPilotStatefulTest/1.0"

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/healthz":
            body = b"ok\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        if parsed.path not in {"/", "/stateful", "/form-stateful"}:
            body = json.dumps({"error": "not_found", "path": parsed.path}).encode("utf-8")
            self.send_response(404)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        query = parse_qs(parsed.query)
        slot = query.get("slot", ["default"])[0]
        template = FORM_HTML_TEMPLATE if parsed.path == "/form-stateful" else STATEFUL_HTML_TEMPLATE
        html = template.replace("default", slot, 1)
        body = html.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Serve a small page that writes cookie/localStorage/sessionStorage for identity continuity verification.",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8766)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    print(
        json.dumps(
            {
                "status": "listening",
                "host": args.host,
                "port": args.port,
                "stateful_url": f"http://{args.host}:{args.port}/stateful?slot=main",
                "form_stateful_url": f"http://{args.host}:{args.port}/form-stateful?slot=form",
            },
            ensure_ascii=False,
        ),
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
