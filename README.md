# tiny-agent-protocol

> **Agents on a $3 microcontroller. 256-byte messages. No JSON.**

[![crates.io](https://img.shields.io/crates/v/tiny-agent-protocol.svg)](https://crates.io/crates/tiny-agent-protocol)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Minimal HTTP protocol for running agents on ESP32, Pico, and other microcontrollers. Messages fit in 256 bytes. Plain text serialization — no JSON parser needed on the device.

## Design Constraints

- **256 bytes max** per request/response
- **Plain text** serialization (no JSON on microcontroller)
- **Max 16 routes** (fits in ESP32 memory)
- **Composable middleware** (auth, logging, rate limit)
- **Works over HTTP/1.1** (no websocket dependency)

## Why?

The Exocortex philosophy: *intelligence lives in the cortex, not the agent*. A $3 ESP32 with 520KB RAM can be an intelligent agent when connected to an exocortex. TAP is the protocol that makes this possible.

## Example

```
GET /sense/temperature
→ 200 OK
  temp:23.4|status:ok|ts:1717700000

POST /act/relay
  relay:on|pin:5
→ 200 OK
  status:activated|pin:5
```

## Part of [Exocortex](https://github.com/SuperInstance/exocortex)

TAP was designed for the ESP32 firmware in `SuperInstance/exocortex-esp32`.

## License

MIT © [SuperInstance](https://github.com/SuperInstance)
