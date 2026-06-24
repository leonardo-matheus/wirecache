use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use wirecache::cache::store::CacheStore;
use wirecache::protocol::codec::{encode_del, encode_get, encode_ping, encode_set};

const BENCH_ADDR: &str = "127.0.0.1:16380";

fn start_server(rt: &Runtime) {
    let store = CacheStore::new(100_000);
    let addr: SocketAddr = BENCH_ADDR.parse().unwrap();
    rt.spawn(async move {
        wirecache::server::listener::run(addr, store).await.unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
}

async fn round_trip(stream: &mut TcpStream, payload: &[u8]) -> Vec<u8> {
    stream.write_all(payload).await.unwrap();
    let mut resp = vec![0u8; 256];
    let n = stream.read(&mut resp).await.unwrap();
    resp.truncate(n);
    resp
}

fn new_stream(rt: &Runtime) -> Arc<Mutex<TcpStream>> {
    let s = rt.block_on(async {
        let s = TcpStream::connect(BENCH_ADDR).await.unwrap();
        s.set_nodelay(true).unwrap();
        s
    });
    Arc::new(Mutex::new(s))
}

fn bench_ping(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    start_server(&rt);
    let stream = new_stream(&rt);
    let frame = encode_ping();

    c.bench_function("ping", |b| {
        b.to_async(&rt).iter(|| {
            let stream = Arc::clone(&stream);
            let frame = frame.clone();
            async move {
                let mut s = stream.lock().await;
                round_trip(&mut s, &frame).await
            }
        });
    });
}

fn bench_set_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    start_server(&rt);

    let sizes = [16usize, 128, 1024, 4096];
    let mut group = c.benchmark_group("set_get");

    for size in &sizes {
        let value = vec![b'x'; *size];
        let set_frame = encode_set(b"bench_key", &value, 0);
        let get_frame = encode_get(b"bench_key");

        let stream_set = new_stream(&rt);
        let stream_get = new_stream(&rt);

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("SET", size), size, |b, _| {
            b.to_async(&rt).iter(|| {
                let stream = Arc::clone(&stream_set);
                let frame = set_frame.clone();
                async move {
                    let mut s = stream.lock().await;
                    round_trip(&mut s, &frame).await
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("GET_hit", size), size, |b, _| {
            b.to_async(&rt).iter(|| {
                let stream = Arc::clone(&stream_get);
                let frame = get_frame.clone();
                async move {
                    let mut s = stream.lock().await;
                    round_trip(&mut s, &frame).await
                }
            });
        });
    }
    group.finish();
}

fn bench_del(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    start_server(&rt);
    let stream = new_stream(&rt);

    let set_frame = encode_set(b"del_key", b"value", 0);
    let del_frame = encode_del(b"del_key");

    c.bench_function("del", |b| {
        b.to_async(&rt).iter(|| {
            let stream = Arc::clone(&stream);
            let set = set_frame.clone();
            let del = del_frame.clone();
            async move {
                let mut s = stream.lock().await;
                round_trip(&mut s, &set).await;
                round_trip(&mut s, &del).await
            }
        });
    });
}

criterion_group!(benches, bench_ping, bench_set_get, bench_del);
criterion_main!(benches);
