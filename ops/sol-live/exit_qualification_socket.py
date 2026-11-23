"""Fresh worker qualification for the local administrator, authenticated by Linux."""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import pwd
import socket
import socketserver
import stat
import struct
import threading

SOCKET = Path("/run/pf-sol-exit/qualify.sock")
MAX_RESPONSE = 65536
CREDS = struct.Struct("iII")  # Linux struct ucred: pid_t, uid_t, gid_t


def peer_uid(conn):
    return CREDS.unpack(conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, CREDS.size))[1]


def qualify(path=SOCKET):
    worker_uid = pwd.getpwnam("pf-sol-exit").pw_uid
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
        conn.settimeout(600)
        conn.connect(str(path))
        if peer_uid(conn) != worker_uid:
            raise PermissionError("Qualification peer does not have the installed worker UID.")
        conn.sendall(b"Q")
        with conn.makefile("rb") as stream:
            data = stream.read(MAX_RESPONSE + 1)
        if len(data) > MAX_RESPONSE:
            raise ValueError("Oversized qualification response.")
        return json.loads(data)


@contextmanager
def serve(path, fresh_readiness):
    directory = path.parent.lstat()
    if (not stat.S_ISDIR(directory.st_mode) or directory.st_uid != os.geteuid()
            or directory.st_mode & 0o022):
        raise PermissionError("Unsafe qualification socket directory.")
    admin_uid = pwd.getpwnam("postfiat").pw_uid

    class Server(socketserver.UnixStreamServer):
        def verify_request(self, conn, address):
            return peer_uid(conn) in (0, admin_uid)

    class Handler(socketserver.BaseRequestHandler):
        def handle(self):
            self.request.settimeout(8)
            if self.request.recv(1) == b"Q":
                self.request.sendall(json.dumps(fresh_readiness()).encode())

    # The worker holds the journal lock before replacing a stale socket.
    path.unlink(missing_ok=True)
    with Server(str(path), Handler) as server:
        try:
            # The administrator can connect. verify_request checks every caller.
            path.chmod(0o666)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                yield
            finally:
                server.shutdown()
                thread.join()
        finally:
            path.unlink(missing_ok=True)
