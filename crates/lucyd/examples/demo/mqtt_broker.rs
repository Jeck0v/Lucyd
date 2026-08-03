//! A minimal MQTT 3.1.1 broker, spoken over the WebSocket transport, so the
//! `/docs` MQTT panel has something to connect to.
//!
//! The panel drives `mqtt.js`, which speaks real MQTT framed inside WebSocket
//! binary frames — there is nothing to "mock", a client needs a peer that
//! actually parses the protocol. Rather than require an external broker (and a
//! second port, and Docker), this serves one from the same axum router: the
//! demo stays a single `cargo run` on a single port.
//!
//! # Scope
//!
//! Deliberately the smallest thing that makes the panel work: CONNECT,
//! SUBSCRIBE, UNSUBSCRIBE, PUBLISH, PING and DISCONNECT, with `+`/`#` wildcard
//! matching. No sessions, no retained messages, no QoS 2, no authentication.
//! Every subscription is granted QoS 0, which §3.9.3 explicitly allows and
//! which is what keeps outbound delivery free of an acknowledgement round-trip.
//! This is a demo fixture, not a broker to run anything on.

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use tokio::sync::broadcast::Sender;

/// Path the broker is mounted at, i.e. `ws://localhost:3000/mqtt`.
pub const PATH: &str = "/mqtt";

/// WebSocket subprotocol MQTT-over-WebSocket clients negotiate (RFC 6455 §1.9).
const SUBPROTOCOL: &str = "mqtt";

/// How many publications may queue up before a slow subscriber misses some.
const BUS_CAPACITY: usize = 64;

// Control packet types, from the high nibble of the fixed header.
const CONNECT: u8 = 1;
const PUBLISH: u8 = 3;
const SUBSCRIBE: u8 = 8;
const UNSUBSCRIBE: u8 = 10;
const PINGREQ: u8 = 12;
const DISCONNECT: u8 = 14;

/// CONNACK: no session present, connection accepted.
const CONNACK: [u8; 4] = [0x20, 0x02, 0x00, 0x00];
/// PINGRESP: a bare fixed header with no body.
const PINGRESP: [u8; 2] = [0xD0, 0x00];

/// A message published on a topic, fanned out to every matching subscriber.
#[derive(Clone)]
pub struct Publication {
    pub topic: String,
    pub payload: Vec<u8>,
}

impl Publication {
    /// Builds a publication from a topic and a UTF-8 payload.
    pub fn new(topic: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            payload: payload.into().into_bytes(),
        }
    }
}

/// What the connection loop should do once a packet has been handled.
enum Response {
    /// Write these bytes back to the client.
    Send(Vec<u8>),
    /// The packet needs no reply.
    Nothing,
    /// The client asked to go away.
    Disconnect,
}

/// One decoded control packet: its type, its fixed-header flags, and everything
/// after the fixed header.
struct Packet {
    kind: u8,
    flags: u8,
    body: Vec<u8>,
}

/// Creates the bus every connection publishes into and subscribes from.
///
/// Handing the [`Sender`] back lets the demo publish onto the same bus as any
/// connected client, which is how the panel shows live readings with no device
/// attached.
pub fn bus() -> Sender<Publication> {
    tokio::sync::broadcast::channel(BUS_CAPACITY).0
}

/// Mounts the broker at [`PATH`].
pub fn router(bus: Sender<Publication>) -> Router {
    Router::new().route(PATH, get(upgrade)).with_state(bus)
}

/// Accepts the upgrade, negotiating the `mqtt` subprotocol the client offers.
async fn upgrade(
    ws: WebSocketUpgrade,
    State(bus): State<Sender<Publication>>,
) -> impl IntoResponse {
    ws.protocols([SUBPROTOCOL])
        .on_upgrade(move |socket| serve(socket, bus))
}

/// Serves one client until it disconnects or the socket breaks.
///
/// A single `select!` over "bytes arrived" and "someone published" keeps the
/// subscription list local to this task, so no connection state is shared and
/// nothing needs locking.
async fn serve(mut socket: WebSocket, bus: Sender<Publication>) {
    let mut subscriber = bus.subscribe();
    let mut buffer: Vec<u8> = Vec::new();
    let mut filters: Vec<String> = Vec::new();

    loop {
        tokio::select! {
            frame = socket.recv() => match frame {
                Some(Ok(Message::Binary(bytes))) => {
                    buffer.extend_from_slice(&bytes);
                    if !drain(&mut buffer, &mut filters, &bus, &mut socket).await {
                        return;
                    }
                }
                // Text/Ping/Pong carry no MQTT; a close or error ends the loop.
                Some(Ok(_)) => {}
                _ => return,
            },
            published = subscriber.recv() => {
                if !forward(published.ok(), &filters, &mut socket).await {
                    return;
                }
            }
        }
    }
}

/// Handles every complete packet sitting in `buffer`, leaving a partial tail
/// for the next frame to finish. Returns `false` when the connection is over.
async fn drain(
    buffer: &mut Vec<u8>,
    filters: &mut Vec<String>,
    bus: &Sender<Publication>,
    socket: &mut WebSocket,
) -> bool {
    while let Some(packet) = take_packet(buffer) {
        match handle(&packet, filters, bus) {
            Response::Send(reply) => {
                if socket.send(Message::Binary(reply.into())).await.is_err() {
                    return false;
                }
            }
            Response::Nothing => {}
            Response::Disconnect => return false,
        }
    }
    true
}

/// Delivers a publication when one of `filters` matches its topic.
///
/// `None` means the receiver lagged past the bus capacity — the demo drops
/// those rather than stalling the publisher.
async fn forward(
    published: Option<Publication>,
    filters: &[String],
    socket: &mut WebSocket,
) -> bool {
    let Some(publication) = published else {
        return true;
    };
    if !filters.iter().any(|f| topic_matches(f, &publication.topic)) {
        return true;
    }
    let packet = encode_publish(&publication);
    socket.send(Message::Binary(packet.into())).await.is_ok()
}

/// Dispatches one packet to its handler.
fn handle(packet: &Packet, filters: &mut Vec<String>, bus: &Sender<Publication>) -> Response {
    match packet.kind {
        CONNECT => Response::Send(CONNACK.to_vec()),
        PUBLISH => publish(&packet.body, packet.flags, bus),
        SUBSCRIBE => subscribe(&packet.body, filters),
        UNSUBSCRIBE => unsubscribe(&packet.body, filters),
        PINGREQ => Response::Send(PINGRESP.to_vec()),
        DISCONNECT => Response::Disconnect,
        _ => Response::Nothing,
    }
}

/// Publishes an inbound PUBLISH onto the bus, acknowledging it when QoS is 1.
fn publish(body: &[u8], flags: u8, bus: &Sender<Publication>) -> Response {
    let Some((topic, header)) = read_string(body) else {
        return Response::Nothing;
    };
    // QoS sits in bits 1-2 of the fixed-header flags. Only QoS > 0 carries a
    // packet identifier, and of those only QoS 1 is acknowledged.
    let qos = (flags >> 1) & 0x03;
    let identifier = (qos > 0).then(|| body.get(header..header + 2)).flatten();
    let payload_at = header + if identifier.is_some() { 2 } else { 0 };

    let _ = bus.send(Publication {
        topic,
        payload: body.get(payload_at..).unwrap_or_default().to_vec(),
    });

    match identifier {
        Some(&[msb, lsb]) if qos == 1 => Response::Send(vec![0x40, 0x02, msb, lsb]),
        _ => Response::Nothing,
    }
}

/// Registers each requested filter, granting QoS 0 for all of them.
fn subscribe(body: &[u8], filters: &mut Vec<String>) -> Response {
    let Some(identifier) = body.get(..2) else {
        return Response::Nothing;
    };
    let mut cursor = 2;
    let mut granted = Vec::new();

    while let Some((filter, used)) = body.get(cursor..).and_then(read_string) {
        filters.push(filter);
        granted.push(0x00);
        cursor += used + 1; // each filter is followed by its requested-QoS byte
    }

    let mut suback = vec![0x90, (2 + granted.len()) as u8];
    suback.extend_from_slice(identifier);
    suback.extend(granted);
    Response::Send(suback)
}

/// Drops each listed filter and acknowledges.
fn unsubscribe(body: &[u8], filters: &mut Vec<String>) -> Response {
    let Some(identifier) = body.get(..2) else {
        return Response::Nothing;
    };
    let mut cursor = 2;

    while let Some((filter, used)) = body.get(cursor..).and_then(read_string) {
        filters.retain(|existing| *existing != filter);
        cursor += used;
    }

    Response::Send(vec![0xB0, 0x02, identifier[0], identifier[1]])
}

/// Matches an MQTT topic filter against a topic: `+` covers one level, `#` the
/// whole remainder.
fn topic_matches(filter: &str, topic: &str) -> bool {
    let mut levels = topic.split('/');
    for expected in filter.split('/') {
        if expected == "#" {
            return true;
        }
        match levels.next() {
            Some(actual) if expected == "+" || expected == actual => {}
            _ => return false,
        }
    }
    levels.next().is_none()
}

/// Encodes a QoS 0 PUBLISH for delivery to a subscriber.
fn encode_publish(publication: &Publication) -> Vec<u8> {
    let topic = publication.topic.as_bytes();
    let remaining = 2 + topic.len() + publication.payload.len();

    let mut packet = vec![0x30];
    packet.extend(encode_remaining_length(remaining));
    packet.extend((topic.len() as u16).to_be_bytes());
    packet.extend_from_slice(topic);
    packet.extend_from_slice(&publication.payload);
    packet
}

/// Splits the first complete packet off the front of `buffer`.
///
/// A WebSocket frame carries no packet boundary of its own: one frame may hold
/// several packets, and one packet may span frames. Returning `None` leaves the
/// partial bytes in place for the next frame to complete.
fn take_packet(buffer: &mut Vec<u8>) -> Option<Packet> {
    let header = *buffer.first()?;
    let (remaining, length_bytes) = read_remaining_length(buffer.get(1..)?)?;
    let body_at = 1 + length_bytes;
    if buffer.len() < body_at + remaining {
        return None;
    }

    let body = buffer[body_at..body_at + remaining].to_vec();
    buffer.drain(..body_at + remaining);
    Some(Packet {
        kind: header >> 4,
        flags: header & 0x0F,
        body,
    })
}

/// Reads a two-byte big-endian length prefix and that many UTF-8 bytes,
/// returning the string and how many bytes it occupied.
fn read_string(bytes: &[u8]) -> Option<(String, usize)> {
    let length = usize::from(u16::from_be_bytes([*bytes.first()?, *bytes.get(1)?]));
    let text = bytes.get(2..2 + length)?;
    Some((String::from_utf8(text.to_vec()).ok()?, 2 + length))
}

/// Reads MQTT's variable-byte integer: up to four bytes, seven payload bits
/// each, the high bit marking "one more byte follows".
fn read_remaining_length(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (index, byte) in bytes.iter().take(4).enumerate() {
        value += usize::from(byte & 0x7F) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

/// Encodes a length as MQTT's variable-byte integer.
fn encode_remaining_length(mut value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = (value % 128) as u8;
        value /= 128;
        if value > 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            return bytes;
        }
    }
}
