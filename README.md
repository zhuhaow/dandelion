# dandelion

A programmable network proxy where routing rules are defined in [Rune](https://github.com/rune-rs/rune), a dynamic scripting language for Rust. Instead of static config files, you write scripts that decide how each connection is routed — through direct TCP, TLS tunnels, SOCKS5 chains, HTTP CONNECT, QUIC, WebSocket tunnels, or blocked entirely — with full access to GeoIP lookups, IP list matching, and DNS resolution.

## Features

- **Fully scriptable routing** — Write handler functions in Rune that receive each connection and return the outbound path. Chain connectors arbitrarily (e.g., TCP → TLS → HTTP CONNECT → SOCKS5).
- **Acceptors** — HTTP proxy (CONNECT + plain) and SOCKS5 inbound listeners.
- **Connectors** — Direct TCP ([RFC 8305 Happy Eyeballs](https://datatracker.ietf.org/doc/html/rfc8305)), TLS (native platform), HTTP CONNECT tunnel, SOCKS5 outbound, QUIC, WebSocket-based "simplex" tunnel, and block (deny).
- **Speed racing** — Race multiple connection strategies with staggered delays, picking the fastest.
- **DNS** — System resolver, Hickory (trust-dns) UDP resolver with raw query support, fake DNS resolver for TUN mode.
- **GeoIP** — MaxMind MMDB support. Load from local file or URL with caching and auto-update.
- **IP lists** — CIDR network matching for routing decisions.
- **TUN** — Virtual network interface with fake DNS (LRU-based domain↔IP mapping) for transparent proxying.
- **Shared state** — Pass a cache object to all handler invocations for cross-connection state.
- **Cross-platform** — macOS, Linux, Windows. Docker and Snap packaging included.

## Installation

### From source

```sh
cd core
cargo build --release
```

The binary is at `core/target/release/dandelion`.

### Docker

```sh
docker pull ghcr.io/zhuhaow/dandelion:latest
docker run -v /path/to/config.rn:/config.rn ghcr.io/zhuhaow/dandelion:latest
```

### Snap

```sh
sudo snap install dandelion
```

Place your config at `$SNAP_COMMON/config.rn`.

## Usage

```sh
dandelion /path/to/config.rn
```

If no path is given, dandelion looks for config in:
1. `$SNAP_COMMON/config.rn`
2. `$HOME/.dandelion/config.rn`

### Logging

Set the `RUST_LOG` environment variable. Default: `warn,dandelion_core=info,dandelion_config=info`.

## Configuration

Configuration is a Rune script (`.rn`) that exports a `config()` async function returning a `Config` object, and one or more handler async functions.

### Minimal example

```rune
pub async fn config() {
    let config = Config::new();
    config.add_http_acceptor("127.0.0.1:8123", "handler")?;
    config.add_socks5_acceptor("127.0.0.1:8124", "handler")?;
    Ok(config)
}

pub async fn handler(connector, cache) {
    let resolver = create_system_resolver()?;
    connector.new_tcp(connector.endpoint(), resolver).await
}
```

### Config API

| Method | Description |
|---|---|
| `Config::new()` | Create a new config |
| `config.add_http_acceptor(addr, handler_name)` | Add an HTTP proxy listener |
| `config.add_socks5_acceptor(addr, handler_name)` | Add a SOCKS5 listener |
| `config.cache = Some(#{...})` | Set a shared cache object |

### Handler API

Each handler receives a `ConnectRequest` and an optional cache object.

**ConnectRequest methods:**

| Method | Description |
|---|---|
| `connector.endpoint()` | Target endpoint as string (`host:port`) |
| `connector.hostname()` | Target hostname |
| `connector.port()` | Target port |
| `connector.hostname_is_ip()` | Whether hostname is an IP address |

**Connector functions:**

| Function | Description |
|---|---|
| `new_tcp_async(endpoint, resolver)` | Direct TCP connection (Happy Eyeballs) |
| `new_tls_async(endpoint, io)` | Wrap connection in TLS |
| `new_http_async(endpoint, io)` | HTTP CONNECT tunnel |
| `new_socks5_async(endpoint, io)` | SOCKS5 outbound |
| `new_quic_connection_async(server, resolver, alpn)` | Create QUIC connection |
| `new_quic_async(connection)` | Open QUIC stream |
| `new_simplex_async(endpoint, config, io)` | WebSocket simplex tunnel |
| `new_block_async(endpoint)` | Block connection |

**Resolver functions:**

| Function | Description |
|---|---|
| `create_system_resolver()` | System DNS resolver |
| `create_udp_resolver(addrs, timeout_ms)` | Hickory UDP resolver |
| `resolver.lookup_async(hostname)` | Resolve to all IPs |
| `resolver.lookup_ipv4_async(hostname)` | Resolve to IPv4 only |
| `resolver.lookup_ipv6_async(hostname)` | Resolve to IPv6 only |

**GeoIP functions:**

| Function | Description |
|---|---|
| `create_geoip_from_absolute_path(path)` | Load MMDB from file |
| `create_geoip_from_url_async(url, handler, interval)` | Load MMDB from URL with caching |
| `geoip.lookup(ip)` | Look up country ISO code |

**IP list functions:**

| Function | Description |
|---|---|
| `new_iplist(cidrs)` | Create IP network set from CIDR list |
| `iplist.contains(ip)` | Check if IP is in any network |
| `iplist.contains_any(ips)` | Check if any IP in list matches |

### Advanced example

```rune
pub async fn config() {
    let config = Config::new();
    config.add_http_acceptor("127.0.0.1:8123", "handler")?;
    config.add_socks5_acceptor("127.0.0.1:8124", "handler")?;
    Ok(config)
}

pub async fn handler(connector, cache) {
    let resolver = create_system_resolver()?;
    let ips = resolver.lookup_async(connector.hostname()).await?;
    let geoip = create_geoip_from_absolute_path("/path/to/GeoLite2-Country.mmdb")?;

    for ip in ips {
        let country = geoip.lookup(ip);
        if country == "US" {
            // Route US traffic through a proxy
            let tcp = new_tcp_async("proxy.example.com:1080", resolver).await?;
            return new_socks5_async(connector.endpoint(), tcp).await;
        }
    }

    // Direct connection for everything else
    new_tcp_async(connector.endpoint(), resolver).await
}
```

## Architecture

```
dandelion
├── config/             Rune scripting engine & config loading
│   ├── engine/
│   │   ├── mod.rs      Engine struct, acceptor loop, Rune VM execution
│   │   ├── connect.rs  Rune-exposed connector functions
│   │   ├── resolver.rs Rune-exposed DNS resolver creation
│   │   ├── geoip.rs    GeoIP database loading (file or URL)
│   │   ├── iplist.rs   IP network set matching (CIDR)
│   │   └── tun.rs      Fake DNS resolver for TUN mode
│   └── rune.rs         Macro for creating Rune type wrappers
│
└── core/               Low-level network primitives
    ├── endpoint.rs     Endpoint type (domain:port or ip:port)
    ├── io.rs           Io trait (AsyncRead + AsyncWrite)
    ├── acceptor/       Inbound protocol handlers (HTTP, SOCKS5)
    ├── connector/      Outbound connectors (TCP, TLS, HTTP, SOCKS5, QUIC, simplex, block, speed)
    ├── resolver/       DNS resolution (system, Hickory UDP)
    ├── quic/           QUIC protocol (Quinn)
    ├── simplex/        WebSocket-based tunneling protocol
    └── tun/            TUN device + fake DNS resolver
```

The proxy runs on a **single-threaded Tokio runtime** (`current_thread` flavor) since Rune objects aren't `Send`. All connections are handled concurrently via `spawn_local`.

## Development

```sh
cd core

# Format
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Test
cargo test -- --include-ignored

# Build
cargo build
```

## License

[MIT](LICENCE)

