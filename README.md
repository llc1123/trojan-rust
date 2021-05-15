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