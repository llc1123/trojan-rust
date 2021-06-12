# Trojan-rust

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/llc1123/trojan-rust/blob/master/LICENSE)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fllc1123%2Ftrojan-rust.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Fllc1123%2Ftrojan-rust?ref=badge_shield)
[![CI](https://img.shields.io/github/workflow/status/llc1123/trojan-rust/nightly)](https://github.com/llc1123/trojan-rust/actions)
![release version](https://img.shields.io/github/v/release/llc1123/trojan-rust)
![release downloads](https://img.shields.io/github/downloads/llc1123/trojan-rust/total)
[![docker pulls](https://img.shields.io/docker/pulls/llc1123/trojan-rust)](https://hub.docker.com/r/llc1123/trojan-rust)
[![docker image size](https://img.shields.io/docker/image-size/llc1123/trojan-rust/latest)](https://hub.docker.com/r/llc1123/trojan-rust)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/llc1123/trojan-rust/pulls)

Yet another [trojan-gfw](https://trojan-gfw.github.io/trojan/) implementation in Rust.

## Features
- Server mode only (for now).
- Supports Redis auth & flow stat.
- Uses OpenSSL as crypto backend.
- Uses tokio as async runtime.

## How trojan handles connections

- Not a TLS connection or TLS handshake failed: Connection Reset.
- SNI mismatch: Redirect to fallback
- Expected TLS but not a trojan request: Redirect to fallback.
- Trojan request but password incorrect: Redirect to fallback.
- Trojan request and password correct: Work as a proxy tunnel.

## How the fallback server (usually) works

- Not HTTP Request: 400 Bad Request
- HTTP Request: 
  - GET: 404 Not Found
  - Other: 405 Methon Not Allowed

_This is like most cdn endpoints' behavior if you don't have a correct resource path._

## Build
```
cargo build --release
```

## Usage
```
USAGE:
    trojan-rust [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>          [default: config.toml]
        --log-level <log-level>    [env: LOGLEVEL=] [default: info]
```

## Docker Image
```bash
docker run -p 443:443 llc1123/trojan-rust example.toml
# or use github container registry
docker run -p 443:443 ghcr.io/llc1123/trojan-rust example.toml
```

## Config

example.toml

```toml
# mode = "server" # optional

## uses default values if not present
# [trojan]
# password = [] # optional
## uses built-in if not present
# fallback = "baidu.com:80" # optional

[tls]
# listen = "0.0.0.0:443" # optional
# tcp_nodelay = false # optional
# sni = [] # optional
cert = "fullchain.pem" # required
key = "privkey.pem" # required

## doesn't use redis if not present
# [redis]
# server = "127.0.0.1:6379" # optional
```

## Multiple hostnames
You can use as many hostnames as they are contained in the certificate provided. Or use all hostnames in the certificate SAN if left absent or empty.

For example:
```
SAN in cert: a.example.com b.example.com
SNI: []
a.example.com ✔️
b.example.com ✔️
other.com ❌

SAN in cert: a.example.com b.example.com
SNI: ["a.example.com"]
a.example.com ✔️
b.example.com ❌
other.com ❌

SAN in cert: a.example.com b.example.com
SNI: ["a.example.com", "b.example.com"]
a.example.com ✔️
b.example.com ✔️
other.com ❌

SAN in cert: a.example.com b.example.com
SNI: ["c.example.com"]
Error on startup
```

## Wildcard SNI matching
Trojan-rust supports wildcard certificates, but with restrictions:
- A wildcard certificate must be provided.
- The `SNI` config must be `[]` or left absent.

For example:
```
SAN in cert: *.example.com
SNI: []
a.example.com ✔️
b.example.com ✔️
example.com ❌ // doesn't match wildcard
a.b.example.com ❌ // doesn't match wildcard
other.com ❌

SAN in cert: *.example-a.com, *.example-b.com
SNI: []
a.example-a.com ✔️
a.example-b.com ✔️
other.com ❌

SAN in cert: *.example.com, example.com
SNI: []
example.com ✔️
a.example.com ✔️
other.com ❌

SAN in cert: *.example.com, example.com
SNI: ["example.com"]
example.com ✔️
a.example.com ❌
other.com ❌

// Not yet supported
SAN in cert: *.example.com, example.com
SNI: ["*.example.com"] 
example.com ❌
a.example.com ❌ 
other.com ❌

// Not yet supported
SAN in cert: *.example.com
SNI: ["a.example.com"] 
Error on startup
```

## Redis Auth
Add a user:
```
HSET [sha224(password)] download 0 upload 0
```
Trojan-rust checks if the hash exists in redis on each connection. If true, the user is authenticated and the flow will be recorded.

Trojan-rust DOES NOT offer a method adding or removing a user. Please do it by yourself.

## TODO

- [ ] Client mode
- [ ] TPROXY mode
- [ ] Benchmarks
- [ ] Wildcards in SNI config

## Contributing
PRs welcome

## License
Trojan-rust is [MIT licensed](https://github.com/llc1123/trojan-rust/blob/master/LICENSE).

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fllc1123%2Ftrojan-rust.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2Fllc1123%2Ftrojan-rust?ref=badge_large)