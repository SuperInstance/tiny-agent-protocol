# tiny-agent-protocol

> **Agents on a $3 microcontroller. 256-byte messages. No JSON.**

[![crates.io](https://img.shields.io/crates/v/tiny-agent-protocol.svg)](https://crates.io/crates/tiny-agent-protocol)
[![docs.rs](https://docs.rs/tiny-agent-protocol/badge.svg)](https://docs.rs/tiny-agent-protocol)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A Rust library implementing a minimal HTTP-like protocol for running agents on ESP32, Pico, and other microcontrollers. Messages fit in 256 bytes. Plain text serialization with pipe-separated key-value pairs — no JSON parser needed on the device. Composable middleware for auth, logging, and rate limiting.

---

## Table of Contents

- [What is TAP?](#what-is-tap)
- [Why Does This Matter?](#why-does-this-matter)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [API Reference](#api-reference)
- [Technical Background](#technical-background)
- [Installation](#installation)
- [Related Crates](#related-crates)
- [License](#license)

---

## What is TAP?

**Tiny Agent Protocol (TAP)** is a minimalist HTTP-like protocol designed for microcontrollers with severe memory constraints. It trades JSON for plain text, complex headers for pipe-separated key-value pairs, and arbitrary payloads for a hard 256-byte limit.

```
Standard HTTP Request (JSON):              TAP Request (Plain Text):
────────────────────────────               ──────────────────────────
POST /act/relay HTTP/1.1                   POST /act/relay
Content-Type: application/json             relay:on|pin:5
Content-Length: 42
                                           27 bytes ✓ (fits in 256)
{"relay":true,"pin":5,"duration":60}
                                           Standard HTTP Response:
138 bytes ✗ (may not fit on ESP32 stack)   HTTP/1.1 200 OK
                                           Content-Type: text/plain
TAP Response:                              Content-Length: 28
200 OK                                     {"status":"ok","pin":"5"}
status:activated|pin:5|ts:1717700000
                                           138 bytes
27 bytes ✓
```

The design philosophy: **intelligence lives in the cortex, not the agent**. A $3 ESP32 with 520KB RAM is a perfectly capable agent when connected to an exocortex. TAP is the protocol that makes this possible.

## Why Does This Matter?

**For IoT agents**: Most IoT protocols (MQTT, CoAP) still require JSON parsers. TAP eliminates this overhead — a 256-byte plaintext parser is trivially implementable in C on any microcontroller.

**For edge AI**: The exocortex pattern separates thinking (cloud/server) from sensing/acting (device). TAP is the communication protocol that bridges them — lean enough for a Pico, expressive enough for complex agent behaviors.

**For robotics**: A robot arm controlled by an ESP32 doesn't need JSON. It needs `position:90|speed:50|accel:20` — 30 bytes, parsed in microseconds, no heap allocation.

**For education**: TAP is simple enough to implement from scratch in a weekend. The entire spec fits on one screen. Perfect for teaching agent architecture without the complexity of HTTP+JSON.

## Architecture

```
tiny-agent-protocol
│
├── TAPMethod (enum)           ← HTTP methods
│   ├── GET                        Read state
│   ├── POST                       Trigger action
│   ├── PUT                        Update configuration
│   └── DELETE                     Remove resource
│
├── TAPRequest                 ← Outbound request
│   ├── new(method, path, body)    Create request
│   ├── fits()                     Fits in MAX_FRAME (256 bytes)?
│   └── serialized_len()           Wire size
│
├── TAPResponse                ← Inbound response
│   ├── new(status, body)          Create response
│   ├── ok(body)                   200 helper
│   ├── not_found()                404 helper
│   ├── forbidden()                403 helper
│   ├── rate_limited()             429 helper
│   ├── fits()                     Within 256 bytes?
│   └── serialized_len()           Wire size
│
├── Route / TAPRouter          ← Request routing
│   ├── new()                      Empty router (max 16 routes)
│   ├── add(method, path, handler) Register route
│   ├── dispatch(&request)         Route → handler → response
│   ├── len()                      Registered routes
│   └── is_empty()                 No routes
│
├── TAPSerializer              ← Wire format
│   ├── serialize_request(req)     Request → string
│   ├── deserialize_request(data)  String → Request
│   ├── serialize_response(res)    Response → string
│   └── deserialize_response(data) String → Response
│
├── TAPMiddleware               ← Composable middleware
│   ├── new()                      Empty middleware chain
│   ├── add(fn)                    Add middleware function
│   ├── run(&request)              Execute chain
│   └── len() / is_empty()        Chain length
│
└── Built-in Middleware
    ├── auth_middleware(req)        Check for auth: key header
    ├── logging_middleware(req)     Log request
    └── rate_limit_middleware(req)  Throttle by client IP
```

## Quick Start

```rust
use tiny_agent_protocol::{
    TAPMethod, TAPRequest, TAPResponse,
    TAPRouter, TAPSerializer, TAPMiddleware,
    auth_middleware, rate_limit_middleware,
};

// Create a request
let req = TAPRequest::new(
    TAPMethod::POST,
    "/act/relay",
    "relay:on|pin:5",
);
assert!(req.fits()); // 27 bytes ≤ 256

// Serialize for wire transmission
let wire = TAPSerializer::serialize_request(&req);
println!("Wire: {}", wire);
// "POST /act/relay\nrelay:on|pin:5"

// Deserialize on the device
let parsed = TAPSerializer::deserialize_request(&wire);
assert_eq!(parsed.unwrap().body, "relay:on|pin:5");

// Build a router
let mut router = TAPRouter::new();
router.add(TAPMethod::GET, "/sense/temperature", |req| {
    TAPResponse::ok("temp:23.4|status:ok|ts:1717700000")
});
router.add(TAPMethod::POST, "/act/relay", |req| {
    TAPResponse::ok("status:activated|pin:5")
});

// Dispatch a request
let sense_req = TAPRequest::new(TAPMethod::GET, "/sense/temperature", "");
let response = router.dispatch(&sense_req);
println!("Response: {} bytes", response.serialized_len());

// Add middleware
let mut mw = TAPMiddleware::new();
mw.add(auth_middleware);        // Check auth header
mw.add(rate_limit_middleware);  // Throttle requests

// Check middleware (returns Some(Response) to short-circuit)
let result = mw.run(&req);
if result.is_some() {
    println!("Blocked by middleware");
}

// Response helpers
let ok = TAPResponse::ok("done");
let not_found = TAPResponse::not_found();
let forbidden = TAPResponse::forbidden();
let rate_limited = TAPResponse::rate_limited();
```

## API Reference

### TAPMethod

| Variant | Description |
|---------|-------------|
| `GET` | Read state / query sensor |
| `POST` | Trigger action / send command |
| `PUT` | Update configuration |
| `DELETE` | Remove resource |

### TAPRequest

| Method | Returns | Description |
|--------|---------|-------------|
| `new(method, path, body)` | `Self` | Create request |
| `fits()` | `bool` | serialized_len() ≤ 256 |
| `serialized_len()` | `usize` | Wire size in bytes |

### TAPResponse

| Method | Returns | Description |
|--------|---------|-------------|
| `new(status, body)` | `Self` | Create response |
| `ok(body)` | `Self` | 200 OK |
| `not_found()` | `Self` | 404 |
| `forbidden()` | `Self` | 403 |
| `rate_limited()` | `Self` | 429 |
| `fits()` | `bool` | Within 256 bytes |
| `serialized_len()` | `usize` | Wire size |

### TAPRouter

| Method | Returns | Description |
|--------|---------|-------------|
| `new()` | `Self` | Empty (max 16 routes) |
| `add(method, path, handler)` | `bool` | Register route (false if full) |
| `dispatch(&request)` | `TAPResponse` | Route → execute → response |
| `len()` | `usize` | Registered routes |
| `is_empty()` | `bool` | No routes |

### TAPSerializer

| Method | Returns | Description |
|--------|---------|-------------|
| `serialize_request(req)` | `String` | Request → wire format |
| `deserialize_request(data)` | `Option<TAPRequest>` | Wire → Request |
| `serialize_response(res)` | `String` | Response → wire format |
| `deserialize_response(data)` | `Option<TAPResponse>` | Wire → Response |

### TAPMiddleware

| Method | Returns | Description |
|--------|---------|-------------|
| `new()` | `Self` | Empty chain |
| `add(fn)` | `()` | Add middleware function |
| `run(&request)` | `Option<TAPResponse>` | Execute chain (None = pass) |

### Built-in Middleware

| Function | Description |
|----------|-------------|
| `auth_middleware(req)` | Block if no `auth:` key in body |
| `logging_middleware(req)` | Log request (passes through) |
| `rate_limit_middleware(req)` | Throttle by `client:` field |

## Technical Background

### Wire Format

TAP uses a minimal text format:

**Request:**
```
METHOD PATH\n
BODY
```

**Response:**
```
STATUS_CODE\n
BODY
```

Body uses pipe-separated key-value pairs:
```
key1:value1|key2:value2|key3:value3
```

No headers, no content-length, no content-type. The protocol assumes:
- Text encoding (UTF-8)
- 256-byte max frame
- Single request-response (no streaming)
- HTTP/1.1 transport layer

### Memory Budget (ESP32)

```
ESP32 SRAM: 520 KB total
├── FreeRTOS + WiFi stack: ~200 KB
├── Application code:       ~100 KB
├── Stack per task:          ~8 KB
└── Available for TAP:       ~4 KB
    ├── Request buffer:       256 bytes
    ├── Response buffer:      256 bytes
    ├── Router (16 routes):  ~1 KB
    └── Middleware:           ~512 bytes
```

The 256-byte limit ensures that request and response buffers fit comfortably on the ESP32 stack without heap allocation.

### Design Decisions

**Why plain text, not binary?** Debuggability. You can `curl` a TAP endpoint and read the response directly. No hex dumps, no protocol buffers, no decode step.

**Why pipe separators?** Commas appear in values. Spaces appear in text. Pipes almost never appear in sensor data or control commands. `|` is the safest delimiter for this domain.

**Why 256 bytes?** Fits on the stack of any microcontroller. Fits in a single UDP packet (minus headers). Fits in a single MQTT message at the lowest tier. 256 is the golden number for constrained devices.

**Why max 16 routes?** Each route entry is ~64 bytes (method + path + function pointer). 16 routes = 1 KB — leaves room for the application. Most ESP32 agents need 5-10 routes at most.

### Protocol Flow

```
ESP32 Agent                          Exocortex Server
───────────                          ────────────────
    │                                      │
    │  GET /sense/temperature              │
    │ ──────────────────────────────────►  │
    │                                      │
    │  200 OK                              │
    │  temp:23.4|status:ok                 │
    │ ◄──────────────────────────────────  │
    │                                      │
    │  POST /act/relay                     │
    │  relay:on|pin:5                      │
    │ ──────────────────────────────────►  │
    │                                      │
    │  200 OK                              │
    │  status:activated                    │
    │ ◄──────────────────────────────────  │
```

## Installation

```bash
cargo add tiny-agent-protocol
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
tiny-agent-protocol = "0.1"
```

## Related Crates

Part of the **SuperInstance Exocortex** ecosystem:

- **[cortex-bus-protocol](https://github.com/SuperInstance/cortex-bus-protocol)** — CQRS event bus for agent messaging
- **[signal-transduction](https://github.com/SuperInstance/signal-transduction)** — Signal cascading for agents
- **[categorical-coordination](https://github.com/SuperInstance/categorical-coordination)** — Category theory for coordination
- **[active-inference](https://github.com/SuperInstance/active-inference)** — Action as surprise minimization
- **[markov-blanket](https://github.com/SuperInstance/markov-blanket)** — Statistical boundary detection

## License

MIT © [SuperInstance](https://github.com/SuperInstance)

Part of the [Exocortex](https://github.com/SuperInstance/exocortex) project.
