import java.io.*;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;

/**
 * WireCache Java Client — sem dependências externas, compatível com Java 8+.
 *
 * Frame request:  [1B op][4B key_len][4B val_len][4B ttl][key][value]
 * Frame response: [1B status][4B payload_len][payload]
 */
public class WireCacheClient implements AutoCloseable {

    private static final int OP_PING  = 0x01;
    private static final int OP_SET   = 0x02;
    private static final int OP_GET   = 0x03;
    private static final int OP_DEL   = 0x04;
    private static final int OP_FLUSH = 0x05;
    private static final int OP_STATS = 0x06;

    private static final int STATUS_OK        = 0x00;
    private static final int STATUS_NOT_FOUND = 0x01;
    private static final int STATUS_ERROR     = 0x02;
    private static final int STATUS_PONG      = 0x03;

    private final Socket socket;
    private final DataOutputStream out;
    private final DataInputStream  in;

    public WireCacheClient(String host, int port) throws IOException {
        this.socket = new Socket();
        this.socket.connect(new InetSocketAddress(host, port), 5000);
        this.socket.setTcpNoDelay(true);
        this.out = new DataOutputStream(new BufferedOutputStream(socket.getOutputStream(), 65536));
        this.in  = new DataInputStream(new BufferedInputStream(socket.getInputStream(), 65536));
    }

    public WireCacheClient() throws IOException {
        this("127.0.0.1", 6380);
    }

    // ------------------------------------------------------------------ //
    // API pública
    // ------------------------------------------------------------------ //

    public boolean ping() throws IOException {
        send(OP_PING, new byte[0], new byte[0], 0);
        Response r = recv();
        return r.status == STATUS_PONG;
    }

    public boolean set(String key, byte[] value, int ttlSecs) throws IOException {
        send(OP_SET, key.getBytes(StandardCharsets.UTF_8), value, ttlSecs);
        Response r = recv();
        if (r.status == STATUS_ERROR) throw new IOException(new String(r.payload, StandardCharsets.UTF_8));
        return r.status == STATUS_OK;
    }

    public boolean set(String key, String value, int ttlSecs) throws IOException {
        return set(key, value.getBytes(StandardCharsets.UTF_8), ttlSecs);
    }

    /** Retorna null se a chave não existir. */
    public byte[] get(String key) throws IOException {
        send(OP_GET, key.getBytes(StandardCharsets.UTF_8), new byte[0], 0);
        Response r = recv();
        if (r.status == STATUS_NOT_FOUND) return null;
        if (r.status == STATUS_ERROR) throw new IOException(new String(r.payload, StandardCharsets.UTF_8));
        return r.payload;
    }

    public String getString(String key) throws IOException {
        byte[] b = get(key);
        return b == null ? null : new String(b, StandardCharsets.UTF_8);
    }

    public boolean delete(String key) throws IOException {
        send(OP_DEL, key.getBytes(StandardCharsets.UTF_8), new byte[0], 0);
        Response r = recv();
        return r.status == STATUS_OK && r.payload.length == 1 && r.payload[0] == '1';
    }

    public boolean flush() throws IOException {
        send(OP_FLUSH, new byte[0], new byte[0], 0);
        Response r = recv();
        return r.status == STATUS_OK;
    }

    public String stats() throws IOException {
        send(OP_STATS, new byte[0], new byte[0], 0);
        Response r = recv();
        if (r.status != STATUS_OK) throw new IOException("Falha ao obter stats");
        return new String(r.payload, StandardCharsets.UTF_8);
    }

    @Override
    public void close() throws IOException {
        socket.close();
    }

    // ------------------------------------------------------------------ //
    // Internos
    // ------------------------------------------------------------------ //

    private void send(int op, byte[] key, byte[] value, int ttl) throws IOException {
        ByteBuffer buf = ByteBuffer.allocate(13 + key.length + value.length);
        buf.put((byte) op);
        buf.putInt(key.length);
        buf.putInt(value.length);
        buf.putInt(ttl);
        buf.put(key);
        buf.put(value);
        out.write(buf.array());
        out.flush();
    }

    private Response recv() throws IOException {
        int status = in.readUnsignedByte();
        int len = in.readInt();
        byte[] payload = new byte[len];
        if (len > 0) in.readFully(payload);
        return new Response(status, payload);
    }

    private static class Response {
        final int status;
        final byte[] payload;
        Response(int status, byte[] payload) {
            this.status = status;
            this.payload = payload;
        }
    }

    // ------------------------------------------------------------------ //
    // Exemplo de uso rápido
    // ------------------------------------------------------------------ //
    public static void main(String[] args) throws Exception {
        try (WireCacheClient c = new WireCacheClient()) {
            System.out.println("PING: " + c.ping());
            c.set("chave", "valor", 60);
            System.out.println("GET: " + c.getString("chave"));
            System.out.println("STATS: " + c.stats());
        }
    }
}
