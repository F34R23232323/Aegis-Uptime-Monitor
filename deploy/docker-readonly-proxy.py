#!/usr/bin/python3
"""Narrow local Docker API projection for Aegis container discovery."""

import http.client
import json
import os
import re
import socket
import socketserver
from urllib.parse import urlsplit

DOCKER_SOCKET = "/var/run/docker.sock"
PROXY_SOCKET = "/run/aegis-docker-proxy/docker.sock"
MAX_RESPONSE = 8_000_000
CONTAINER_ID = re.compile(r"/containers/([0-9a-fA-F]{64})/json\Z")
VERSION = re.compile(r"/v[0-9]+\.[0-9]+(?=/)")


class DockerConnection(http.client.HTTPConnection):
    def __init__(self):
        super().__init__("localhost", timeout=5)

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(5)
        self.sock.connect(DOCKER_SOCKET)


def project(path):
    parsed = urlsplit(path)
    if parsed.scheme or parsed.netloc or parsed.fragment or len(path) > 1024:
        raise ValueError("Unsupported request target")
    route = VERSION.sub("", parsed.path, count=1)
    if route == "/containers/json":
        upstream = "/containers/json?all=true"
        mode = "list"
    elif match := CONTAINER_ID.fullmatch(route):
        upstream = f"/containers/{match.group(1)}/json"
        mode = "inspect"
    else:
        raise ValueError("Unsupported Docker API route")
    connection = DockerConnection()
    try:
        connection.request("GET", upstream)
        response = connection.getresponse()
        if response.status != 200:
            raise ValueError("Docker API request failed")
        raw = response.read(MAX_RESPONSE + 1)
        if len(raw) > MAX_RESPONSE:
            raise ValueError("Docker API response exceeded limit")
    finally:
        connection.close()
    data = json.loads(raw)
    if mode == "list":
        if not isinstance(data, list):
            raise ValueError("Invalid Docker list response")
        return [
            {
                "Id": item.get("Id"),
                "Names": item.get("Names"),
                "State": item.get("State"),
                "Status": item.get("Status"),
                "Labels": {},
            }
            for item in data[:200]
            if isinstance(item, dict)
        ]
    if not isinstance(data, dict):
        raise ValueError("Invalid Docker inspect response")
    state = data.get("State") or {}
    health = state.get("Health") or {}
    return {
        "State": {"Health": {"Status": health.get("Status")} if health else None},
        "RestartCount": data.get("RestartCount"),
    }


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        first = self.rfile.readline(4097)
        if len(first) > 4096:
            return self.respond(400, {"message": "Request too large"})
        parts = first.decode("ascii", errors="replace").strip().split()
        if len(parts) != 3 or parts[0] != "GET" or not parts[2].startswith("HTTP/1."):
            return self.respond(405, {"message": "Only GET is allowed"})
        header_bytes = 0
        while True:
            line = self.rfile.readline(4097)
            header_bytes += len(line)
            if not line or header_bytes > 16384:
                return self.respond(400, {"message": "Invalid headers"})
            if line in (b"\r\n", b"\n"):
                break
        try:
            result = project(parts[1])
        except (ValueError, OSError, TimeoutError, json.JSONDecodeError):
            return self.respond(502, {"message": "Docker discovery request failed"})
        self.respond(200, result)

    def respond(self, status, value):
        body = json.dumps(value, separators=(",", ":")).encode()
        self.wfile.write(
            f"HTTP/1.1 {status} {'OK' if status == 200 else 'Error'}\r\n"
            f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
            "Connection: close\r\n\r\n".encode()
            + body
        )


class Server(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


if __name__ == "__main__":
    if os.path.exists(PROXY_SOCKET):
        os.unlink(PROXY_SOCKET)
    with Server(PROXY_SOCKET, Handler) as server:
        os.chmod(PROXY_SOCKET, 0o660)
        server.serve_forever()
