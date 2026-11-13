#!/usr/bin/env python3
"""Minimal StakeHub agent socket client (~/.stakehub/agent.sock, uid-authenticated).
Usage: stakehub_agent.py '<json request>'   Prints the JSON response. Sends no secrets."""
import json, socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.settimeout(600)
s.connect("/home/postfiat/.stakehub/agent.sock")
s.sendall(sys.argv[1].encode() + b"\n")
buf = b""
while not buf.endswith(b"\n"):
    chunk = s.recv(65536)
    if not chunk: break
    buf += chunk
print(buf.decode().strip())
