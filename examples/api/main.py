"""
WireCache Demo API
FastAPI + Swagger que expõe o WireCache via HTTP para demonstrar performance.

Cada resposta inclui o header X-WireCache-Latency-Ms com a latência real
da operação TCP no servidor de cache.

Inicie o WireCache antes de rodar esta API:
    cargo run --release          (na raiz do projeto)

Depois rode a API:
    pip install -r requirements.txt
    uvicorn main:app --reload
"""

import asyncio
import time
from contextlib import asynccontextmanager
from typing import Annotated, Optional

from fastapi import FastAPI, HTTPException, Path, Query, Response
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

from wirecache_client import WireCacheClient, WireCacheError

# ---------------------------------------------------------------------------
# Instância global do cliente (uma conexão reutilizada por toda a API)
# ---------------------------------------------------------------------------
cache = WireCacheClient(host="127.0.0.1", port=6380)


@asynccontextmanager
async def lifespan(app: FastAPI):
    await cache.connect()
    yield
    await cache.close()


# ---------------------------------------------------------------------------
# Aplicação
# ---------------------------------------------------------------------------
app = FastAPI(
    title="WireCache Demo API",
    description="""
## WireCache — servidor de cache in-memory de alta performance

Esta API demonstra o WireCache via HTTP com Swagger interativo.
Cada endpoint reporta a **latência real da operação TCP** no header
`X-WireCache-Latency-Ms`.

### Protocolo subjacente
O WireCache usa um protocolo **binário TCP** próprio:
- Frame de 13 bytes de cabeçalho (opcode + key_len + val_len + ttl)
- Payload em bytes puros (zero-copy)
- `TCP_NODELAY` habilitado — sem buffering de Nagle

### Comandos disponíveis
| Comando | Descrição |
|---------|-----------|
| `PING`  | Health check |
| `SET`   | Armazena chave/valor com TTL opcional |
| `GET`   | Recupera valor por chave |
| `DEL`   | Remove uma chave |
| `FLUSH` | Invalida todas as entradas |
| `STATS` | Snapshot de métricas em JSON |
""",
    version="0.1.0",
    lifespan=lifespan,
    docs_url="/",
    redoc_url="/redoc",
)


# ---------------------------------------------------------------------------
# Schemas
# ---------------------------------------------------------------------------
class SetRequest(BaseModel):
    value: str = Field(
        ...,
        description="Valor a armazenar (string UTF-8)",
        examples=["hello world"],
    )
    ttl: int = Field(
        default=0,
        ge=0,
        description="Time-to-live em segundos. 0 = sem expiração.",
        examples=[60],
    )

    model_config = {
        "json_schema_extra": {
            "examples": [
                {"value": "hello world", "ttl": 0},
                {"value": "dados com expiração", "ttl": 30},
            ]
        }
    }


class SetResponse(BaseModel):
    key: str
    stored: bool
    latency_ms: float = Field(description="Latência da operação no WireCache (ms)")


class GetResponse(BaseModel):
    key: str
    value: Optional[str] = Field(description="Valor armazenado, ou null se não encontrado")
    found: bool
    latency_ms: float


class DeleteResponse(BaseModel):
    key: str
    deleted: bool
    latency_ms: float


class PingResponse(BaseModel):
    pong: bool
    latency_ms: float


class FlushResponse(BaseModel):
    flushed: bool
    latency_ms: float


class StatsResponse(BaseModel):
    entries: int = Field(description="Número de entradas ativas no cache")
    hits: int
    misses: int
    sets: int
    deletes: int
    flushes: int
    hit_rate_pct: float = Field(description="Taxa de acerto (%)")
    latency_ms: float


class BenchResult(BaseModel):
    operations: int
    total_ms: float
    avg_latency_ms: float
    ops_per_second: float
    value_size_bytes: int


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def latency_response(data: dict, latency_ms: float) -> JSONResponse:
    r = JSONResponse(content=data)
    r.headers["X-WireCache-Latency-Ms"] = f"{latency_ms:.3f}"
    return r


def handle_error(e: WireCacheError) -> HTTPException:
    return HTTPException(status_code=502, detail=f"Erro do WireCache: {e}")


# ---------------------------------------------------------------------------
# Endpoints
# ---------------------------------------------------------------------------
@app.get(
    "/ping",
    response_model=PingResponse,
    summary="Health check",
    tags=["Operações básicas"],
    responses={
        200: {
            "description": "Servidor respondeu PONG",
            "content": {
                "application/json": {
                    "example": {"pong": True, "latency_ms": 0.21}
                }
            },
        }
    },
)
async def ping():
    """
    Envia PING ao WireCache e retorna PONG com a latência medida.

    Útil para verificar se o servidor está ativo e medir o round-trip TCP.
    """
    try:
        pong, ms = await cache.ping()
    except Exception as e:
        raise HTTPException(status_code=503, detail=f"WireCache indisponível: {e}")
    data = {"pong": pong, "latency_ms": round(ms, 3)}
    return latency_response(data, ms)


@app.put(
    "/cache/{key}",
    response_model=SetResponse,
    summary="Armazenar valor",
    tags=["Operações básicas"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {"key": "usuario:42", "stored": True, "latency_ms": 0.35}
                }
            }
        }
    },
)
async def set_key(
    key: Annotated[str, Path(description="Chave do cache", examples=["usuario:42"])],
    body: SetRequest,
):
    """
    Armazena um par chave/valor no WireCache.

    - **key**: identificador único (qualquer string UTF-8)
    - **value**: conteúdo a armazenar
    - **ttl**: expiração em segundos (0 = sem expiração)
    """
    try:
        stored, ms = await cache.set(key, body.value, body.ttl)
    except WireCacheError as e:
        raise handle_error(e)
    data = {"key": key, "stored": stored, "latency_ms": round(ms, 3)}
    return latency_response(data, ms)


@app.get(
    "/cache/{key}",
    response_model=GetResponse,
    summary="Buscar valor",
    tags=["Operações básicas"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "examples": {
                        "encontrado": {
                            "summary": "Chave encontrada (cache hit)",
                            "value": {"key": "usuario:42", "value": "Leonardo", "found": True, "latency_ms": 0.18},
                        },
                        "não encontrado": {
                            "summary": "Chave ausente (cache miss)",
                            "value": {"key": "usuario:99", "value": None, "found": False, "latency_ms": 0.15},
                        },
                    }
                }
            }
        }
    },
)
async def get_key(
    key: Annotated[str, Path(description="Chave a buscar", examples=["usuario:42"])],
):
    """
    Recupera o valor associado à chave.

    Retorna `found: false` com `value: null` se a chave não existir ou tiver expirado.
    """
    try:
        value, ms = await cache.get(key)
    except WireCacheError as e:
        raise handle_error(e)
    data = {"key": key, "value": value, "found": value is not None, "latency_ms": round(ms, 3)}
    return latency_response(data, ms)


@app.delete(
    "/cache/{key}",
    response_model=DeleteResponse,
    summary="Remover chave",
    tags=["Operações básicas"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {"key": "usuario:42", "deleted": True, "latency_ms": 0.22}
                }
            }
        }
    },
)
async def delete_key(
    key: Annotated[str, Path(description="Chave a remover", examples=["usuario:42"])],
):
    """Remove uma chave do cache. Retorna `deleted: false` se a chave não existia."""
    try:
        deleted, ms = await cache.delete(key)
    except WireCacheError as e:
        raise handle_error(e)
    data = {"key": key, "deleted": deleted, "latency_ms": round(ms, 3)}
    return latency_response(data, ms)


@app.post(
    "/cache/flush",
    response_model=FlushResponse,
    summary="Limpar todo o cache",
    tags=["Administração"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {"flushed": True, "latency_ms": 0.45}
                }
            }
        }
    },
)
async def flush():
    """Invalida todas as entradas do cache de uma vez."""
    try:
        flushed, ms = await cache.flush()
    except WireCacheError as e:
        raise handle_error(e)
    data = {"flushed": flushed, "latency_ms": round(ms, 3)}
    return latency_response(data, ms)


@app.get(
    "/stats",
    response_model=StatsResponse,
    summary="Métricas do servidor",
    tags=["Administração"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {
                        "entries": 1024,
                        "hits": 8500,
                        "misses": 340,
                        "sets": 1024,
                        "deletes": 12,
                        "flushes": 1,
                        "hit_rate_pct": 96.14,
                        "latency_ms": 0.28,
                    }
                }
            }
        }
    },
)
async def stats():
    """
    Retorna um snapshot das métricas internas do WireCache:
    entradas ativas, hits/misses, taxa de acerto, e contadores de operações.
    """
    try:
        data, ms = await cache.stats()
    except WireCacheError as e:
        raise handle_error(e)
    data["latency_ms"] = round(ms, 3)
    return latency_response(data, ms)


# ---------------------------------------------------------------------------
# Benchmark embutido
# ---------------------------------------------------------------------------
@app.post(
    "/bench/set",
    response_model=BenchResult,
    summary="Benchmark de escrita (SET)",
    tags=["Benchmark"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {
                        "operations": 1000,
                        "total_ms": 312.5,
                        "avg_latency_ms": 0.31,
                        "ops_per_second": 3200,
                        "value_size_bytes": 64,
                    }
                }
            }
        }
    },
)
async def bench_set(
    n: Annotated[int, Query(ge=1, le=10000, description="Número de operações SET", examples=[1000])] = 1000,
    size: Annotated[int, Query(ge=1, le=65536, description="Tamanho do valor em bytes", examples=[64])] = 64,
):
    """
    Executa `n` operações SET sequenciais e calcula:
    - Latência média por operação
    - Throughput total (ops/segundo)

    Use para comparar o WireCache com Redis na mesma máquina.
    """
    value = "x" * size
    t0 = time.perf_counter()
    for i in range(n):
        await cache.set(f"bench:{i}", value, 0)
    total_ms = (time.perf_counter() - t0) * 1000
    avg = total_ms / n
    return {
        "operations": n,
        "total_ms": round(total_ms, 2),
        "avg_latency_ms": round(avg, 3),
        "ops_per_second": round(n / (total_ms / 1000)),
        "value_size_bytes": size,
    }


@app.post(
    "/bench/get",
    response_model=BenchResult,
    summary="Benchmark de leitura (GET)",
    tags=["Benchmark"],
    responses={
        200: {
            "content": {
                "application/json": {
                    "example": {
                        "operations": 1000,
                        "total_ms": 280.1,
                        "avg_latency_ms": 0.28,
                        "ops_per_second": 3570,
                        "value_size_bytes": 64,
                    }
                }
            }
        }
    },
)
async def bench_get(
    n: Annotated[int, Query(ge=1, le=10000, description="Número de operações GET", examples=[1000])] = 1000,
    size: Annotated[int, Query(ge=1, le=65536, description="Tamanho do valor previamente inserido", examples=[64])] = 64,
):
    """
    Pré-popula uma chave e executa `n` GETs sequenciais (100% cache hit).

    Mede a latência pura de leitura, sem overhead de serialização complexa.
    """
    value = "x" * size
    await cache.set("bench:get_target", value, 0)

    t0 = time.perf_counter()
    for _ in range(n):
        await cache.get("bench:get_target")
    total_ms = (time.perf_counter() - t0) * 1000
    avg = total_ms / n
    return {
        "operations": n,
        "total_ms": round(total_ms, 2),
        "avg_latency_ms": round(avg, 3),
        "ops_per_second": round(n / (total_ms / 1000)),
        "value_size_bytes": size,
    }
