"""
Cliente TCP assíncrono do WireCache (asyncio puro, sem threads).

Frame request:  [1B op][4B key_len][4B val_len][4B ttl][key][value]
Frame response: [1B status][4B payload_len][payload]
"""

import asyncio
import json
import struct
import time
from contextlib import asynccontextmanager
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


class WireCacheError(Exception):
    pass


class WireCacheClient:
    """
    Conexão única e reutilizável com o servidor WireCache.
    Thread-safe para uso com asyncio (um loop de eventos).
    """

    def __init__(self, host: str = "127.0.0.1", port: int = 6380):
        self._host = host
        self._port = port
        self._reader: Optional[asyncio.StreamReader] = None
        self._writer: Optional[asyncio.StreamWriter] = None
        self._lock = asyncio.Lock()

    async def connect(self):
        self._reader, self._writer = await asyncio.open_connection(self._host, self._port)
        # TCP_NODELAY para eliminar latência do Nagle
        sock = self._writer.get_extra_info("socket")
        if sock:
            import socket
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

    async def close(self):
        if self._writer:
            self._writer.close()
            await self._writer.wait_closed()

    # ------------------------------------------------------------------
    # API pública
    # ------------------------------------------------------------------

    async def ping(self) -> tuple[bool, float]:
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_PING, b"", b"", 0)
            status, _ = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        return status == STATUS_PONG, ms

    async def set(self, key: str, value: str, ttl: int = 0) -> tuple[bool, float]:
        k, v = key.encode(), value.encode()
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_SET, k, v, ttl)
            status, payload = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        if status == STATUS_ERROR:
            raise WireCacheError(payload.decode())
        return status == STATUS_OK, ms

    async def get(self, key: str) -> tuple[Optional[str], float]:
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_GET, key.encode(), b"", 0)
            status, payload = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        if status == STATUS_NOT_FOUND:
            return None, ms
        if status == STATUS_ERROR:
            raise WireCacheError(payload.decode())
        return payload.decode(), ms

    async def delete(self, key: str) -> tuple[bool, float]:
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_DEL, key.encode(), b"", 0)
            status, payload = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        return status == STATUS_OK and payload == b"1", ms

    async def flush(self) -> tuple[bool, float]:
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_FLUSH, b"", b"", 0)
            status, _ = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        return status == STATUS_OK, ms

    async def stats(self) -> tuple[dict, float]:
        t0 = time.perf_counter()
        async with self._lock:
            await self._send(OP_STATS, b"", b"", 0)
            status, payload = await self._recv()
        ms = (time.perf_counter() - t0) * 1000
        if status != STATUS_OK:
            raise WireCacheError("Falha ao obter stats")
        return json.loads(payload.decode()), ms

    # ------------------------------------------------------------------
    # Internos
    # ------------------------------------------------------------------

    async def _send(self, op: int, key: bytes, value: bytes, ttl: int):
        header = struct.pack(">BIII", op, len(key), len(value), ttl)
        self._writer.write(header + key + value)
        await self._writer.drain()

    async def _recv(self) -> tuple[int, bytes]:
        header = await self._reader.readexactly(5)
        status = header[0]
        payload_len = struct.unpack(">I", header[1:5])[0]
        payload = await self._reader.readexactly(payload_len) if payload_len else b""
        return status, payload
