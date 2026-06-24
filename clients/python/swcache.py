"""
SWCache Python Client
Protocolo binário TCP do SWCache.

Frame request:  [1B op][4B key_len][4B val_len][4B ttl][key][value]
Frame response: [1B status][4B payload_len][payload]

Instale apenas com a stdlib — sem dependências externas.
"""

import socket
import struct
from typing import Optional

OP_PING  = 0x01
OP_SET   = 0x02
OP_GET   = 0x03
OP_DEL   = 0x04
OP_FLUSH = 0x05
OP_STATS = 0x06

STATUS_OK        = 0x00
STATUS_NOT_FOUND = 0x01
STATUS_ERROR     = 0x02
STATUS_PONG      = 0x03

HEADER = struct.Struct(">BII I")  # op, key_len, val_len, ttl


class SWCacheError(Exception):
    pass


class SWCacheClient:
    def __init__(self, host: str = "127.0.0.1", port: int = 6380, timeout: float = 5.0):
        self._sock = socket.create_connection((host, port), timeout=timeout)
        self._sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

    # ------------------------------------------------------------------ #
    # API pública
    # ------------------------------------------------------------------ #

    def ping(self) -> bool:
        self._send(OP_PING, b"", b"", 0)
        status, _ = self._recv()
        return status == STATUS_PONG

    def set(self, key: str | bytes, value: str | bytes, ttl: int = 0) -> bool:
        k, v = self._enc(key), self._enc(value)
        self._send(OP_SET, k, v, ttl)
        status, payload = self._recv()
        if status == STATUS_ERROR:
            raise SWCacheError(payload.decode())
        return status == STATUS_OK

    def get(self, key: str | bytes) -> Optional[bytes]:
        self._send(OP_GET, self._enc(key), b"", 0)
        status, payload = self._recv()
        if status == STATUS_NOT_FOUND:
            return None
        if status == STATUS_ERROR:
            raise SWCacheError(payload.decode())
        return payload

    def delete(self, key: str | bytes) -> bool:
        self._send(OP_DEL, self._enc(key), b"", 0)
        status, payload = self._recv()
        return status == STATUS_OK and payload == b"1"

    def flush(self) -> bool:
        self._send(OP_FLUSH, b"", b"", 0)
        status, _ = self._recv()
        return status == STATUS_OK

    def stats(self) -> dict:
        self._send(OP_STATS, b"", b"", 0)
        status, payload = self._recv()
        if status != STATUS_OK:
            raise SWCacheError("Falha ao obter stats")
        import json
        return json.loads(payload.decode())

    def close(self):
        self._sock.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    # ------------------------------------------------------------------ #
    # Internos
    # ------------------------------------------------------------------ #

    @staticmethod
    def _enc(v: str | bytes) -> bytes:
        return v.encode() if isinstance(v, str) else v

    def _send(self, op: int, key: bytes, value: bytes, ttl: int):
        header = struct.pack(">BIII", op, len(key), len(value), ttl)
        self._sock.sendall(header + key + value)

    def _recv(self) -> tuple[int, bytes]:
        header = self._recv_exactly(5)
        status = header[0]
        payload_len = struct.unpack(">I", header[1:5])[0]
        payload = self._recv_exactly(payload_len) if payload_len else b""
        return status, payload

    def _recv_exactly(self, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = self._sock.recv(n - len(buf))
            if not chunk:
                raise SWCacheError("Conexão encerrada pelo servidor")
            buf += chunk
        return buf


# ------------------------------------------------------------------ #
# Exemplo de uso rápido
# ------------------------------------------------------------------ #
if __name__ == "__main__":
    with SWCacheClient() as c:
        print("PING:", c.ping())
        c.set("chave", "valor", ttl=60)
        print("GET:", c.get("chave"))
        print("STATS:", c.stats())
