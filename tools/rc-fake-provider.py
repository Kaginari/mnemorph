#!/usr/bin/env python3
"""A scripted OpenAI-compatible provider on loopback, for the release-candidate smoke test.

It answers `POST /v1/chat/completions` with a streamed reply and never reaches the network:
a request with no tool result yet asks for the `write` tool (out.txt says `fixed`); one that
carries a tool result answers in text. Every request is appended to the log as one JSON line,
so the smoke test can say what the binary sent. It binds an ephemeral port and writes it to the
port file once it listens.

    rc-fake-provider.py <port-file> <request-log>
"""

import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ANSWER = "Fixed it."


def chunk(delta, finish=None):
    return {
        "id": "chatcmpl-rc",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "rc-smoke",
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
    }


def reply(body):
    """The stream's events for one request body."""
    messages = body.get("messages", [])
    answered_tool = any(m.get("role") == "tool" for m in messages)
    if body.get("tools") and not answered_tool:
        args = json.dumps({"path": "out.txt", "content": "fixed\n"})
        call = {
            "id": "call-rc-0",
            "index": 0,
            "type": "function",
            "function": {"name": "write", "arguments": args},
        }
        events = [
            chunk({"role": "assistant", "content": "", "tool_calls": [call]}),
            chunk({}, "tool_calls"),
        ]
    else:
        events = [chunk({"role": "assistant", "content": ANSWER}), chunk({}, "stop")]
    usage = {"prompt_tokens": 100, "completion_tokens": 10, "total_tokens": 110}
    events.append({**chunk({}), "choices": [], "usage": usage})
    return events


class Handler(BaseHTTPRequestHandler):
    log_path = ""

    def do_POST(self):  # noqa: N802 (the stdlib's name)
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        try:
            body = json.loads(raw)
        except ValueError:
            self.send_error(400, "not JSON")
            return
        with open(self.log_path, "a", encoding="utf-8") as log:
            log.write(json.dumps({"path": self.path, "body": body}) + "\n")
        if not self.path.endswith("/chat/completions"):
            self.send_error(404, "only chat completions here")
            return
        payload = "".join(f"data: {json.dumps(e)}\n\n" for e in reply(body)) + "data: [DONE]\n\n"
        data = payload.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args):
        pass


def main():
    port_file, log_path = sys.argv[1], sys.argv[2]
    Handler.log_path = log_path
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    with open(port_file + ".tmp", "w", encoding="utf-8") as f:
        f.write(str(server.server_address[1]))
    # Renamed whole, so a reader never sees a half-written port.
    os.replace(port_file + ".tmp", port_file)
    server.serve_forever()


if __name__ == "__main__":
    main()
