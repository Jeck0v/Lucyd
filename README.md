# Lucy
 
**Lucy** is a set of internal development tools designed for the Flipper project, an interactive documentation and API testing interface developed in Rust.  

---
 
## Why Lucy?
 
Modern real-time backends do not use a single protocol.
The Flipper 3D backend (Rust/Axum) handles:
- **REST API endpoints** for standard CRUD operations
- **WebSocket connections** for real-time physics events and multiplayer synchronisation
- **MQTT topics** for communication with IoT devices
The current ecosystem forces developers to juggle multiple tools simultaneously: Scalar for HTTP, MQTT Explorer for broker messages, and Postman for everything else.
Lucy unifies these three elements into a single `localhost/docs` interface, automatically generated from the source code via internal tags placed within the Flipper backend.
 
As the backend is built on **Rust/Axum** (rather than FastAPI or a similar framework), there is no off-the-shelf solution covering all three protocols in a single code-generated interactive interface. Lucy fills this gap.
 
---

## Features
 
- **HTTP REST** - Interactive playground for all Axum routes, request/response schema visualization
- **WebSocket** - Live connection panel, real-time message stream visualization, send/receive testing
- **MQTT** - Subscribe to topics, publish messages, live message feed
- **Auto-generated** - Annotate your Rust handlers, Lucy generates the docs at runtime
