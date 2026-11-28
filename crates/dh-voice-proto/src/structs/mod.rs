//! Data types and error definitions for the DHV2 wire protocol.

use core::fmt;

use crate::consts::{
    MSG_TYPE_BYE, MSG_TYPE_HELLO, MSG_TYPE_HELLO_ACK, MSG_TYPE_PEER_MAP, MSG_TYPE_VOICE_FRAME,
};

/// Wire message type discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MsgType {
    /// Client registration / keepalive carrying the player UID string.
    Hello = MSG_TYPE_HELLO,
    /// Server acknowledgement assigning a `peer_slot` (`1..=64`).
    HelloAck = MSG_TYPE_HELLO_ACK,
    /// Server roster broadcast mapping `peer_slot -> uid`.
    PeerMap = MSG_TYPE_PEER_MAP,
    /// Encoded Opus audio frame (client -> server -> peers).
    VoiceFrame = MSG_TYPE_VOICE_FRAME,
    /// Client clean disconnect notification.
    Bye = MSG_TYPE_BYE,
}

impl MsgType {
    /// Converts a raw wire discriminant byte into a typed [`MsgType`].
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            MSG_TYPE_HELLO => Some(Self::Hello),
            MSG_TYPE_HELLO_ACK => Some(Self::HelloAck),
            MSG_TYPE_PEER_MAP => Some(Self::PeerMap),
            MSG_TYPE_VOICE_FRAME => Some(Self::VoiceFrame),
            MSG_TYPE_BYE => Some(Self::Bye),
            _ => None,
        }
    }

    /// Returns the raw wire discriminant byte for this message type.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Decoded fixed 22-byte DHV2 header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dhv2Header {
    /// Protocol version (always `2` for decoded headers).
    pub version: u8,
    /// Message type discriminant.
    pub msg_type: MsgType,
    /// Per-message flag bits (`FLAG_END_OF_TALKSPURT`, `FLAG_HAS_FEC`).
    pub flags: u8,
    /// Reserved byte (0 in v2).
    pub reserved: u8,
    /// Authoritative peer slot (`1..=64`, or `0` when unassigned).
    pub peer_slot: u16,
    /// Per-stream monotonic sequence number.
    pub seq: u16,
    /// Audio sample timestamp (e.g. 48 kHz sample counter).
    pub timestamp: u32,
    /// Exact payload length in bytes immediately following the 22-byte header.
    pub payload_len: u16,
}

/// Single roster entry in a [`Dhv2Message::PeerMap`] message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerMapEntry {
    /// Server-assigned peer slot (`1..=64`).
    pub peer_slot: u16,
    /// Player identifier string (UTF-8, `1..=128` bytes).
    pub uid: String,
}

/// Decoded DHV2 protocol message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dhv2Message {
    /// Client registration / keepalive carrying the player UID string.
    Hello {
        /// Player identifier string (`1..=128` UTF-8 bytes, no NUL).
        uid: String,
    },
    /// Server acknowledgement assigning the sender's `peer_slot`.
    HelloAck {
        /// Assigned peer slot (`1..=64`).
        peer_slot: u16,
    },
    /// Server roster snapshot mapping every active `peer_slot` to its UID.
    PeerMap {
        /// Active peers (`0..=64` entries).
        entries: Vec<PeerMapEntry>,
    },
    /// Opus-encoded voice frame.
    VoiceFrame {
        /// Authoritative sender slot (server overwrites on relay).
        peer_slot: u16,
        /// Voice frame flags (`FLAG_END_OF_TALKSPURT`, `FLAG_HAS_FEC`).
        flags: u8,
        /// Frame sequence number.
        seq: u16,
        /// Frame sample timestamp.
        timestamp: u32,
        /// Raw Opus payload bytes (`0..=512` bytes).
        payload: Vec<u8>,
    },
    /// Client clean disconnect notification.
    Bye {
        /// Sender's peer slot (or `0` if unassigned).
        peer_slot: u16,
    },
}

/// Validation and serialization errors for DHV2 packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// Input buffer is shorter than the fixed header or expected payload.
    Truncated,
    /// Total packet size exceeds `MAX_PACKET_SIZE` (1200 bytes).
    PacketTooLarge,
    /// Output buffer passed to an `*_into` encoder is too small.
    BufferTooSmall,
    /// Packet does not begin with `DHV2_MAGIC`.
    InvalidMagic,
    /// Protocol version byte is not `DHV2_VERSION`.
    UnsupportedVersion(u8),
    /// Unknown message type discriminant.
    UnknownMsgType(u8),
    /// Header `payload_len` does not match the actual remaining byte count.
    PayloadLengthMismatch {
        /// Payload length declared in the header.
        declared: usize,
        /// Actual byte count remaining after the 22-byte header.
        actual: usize,
    },
    /// Player UID is empty, exceeds `MAX_UID_LEN`, contains NUL, or is invalid UTF-8.
    InvalidUid,
    /// Peer slot is outside `1..=64` where a valid slot is required.
    InvalidPeerSlot(u16),
    /// Opus voice payload exceeds `MAX_OPUS_PAYLOAD` (512 bytes).
    OpusPayloadTooLarge(usize),
    /// `PeerMap` entry count exceeds `MAX_PEERS` (64).
    TooManyPeers(usize),
    /// Payload for a message type is malformed or contains unexpected trailing bytes.
    MalformedPayload,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Truncated => f.write_str("DHV2 packet is truncated"),
            Self::PacketTooLarge => f.write_str("DHV2 packet exceeds MAX_PACKET_SIZE"),
            Self::BufferTooSmall => f.write_str("output buffer is too small for DHV2 packet"),
            Self::InvalidMagic => f.write_str("DHV2 magic prefix mismatch"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported DHV2 version: {v}"),
            Self::UnknownMsgType(t) => write!(f, "unknown DHV2 message type: {t}"),
            Self::PayloadLengthMismatch { declared, actual } => write!(
                f,
                "DHV2 payload length mismatch: declared {declared}, actual {actual}"
            ),
            Self::InvalidUid => f.write_str("invalid DHV2 peer UID"),
            Self::InvalidPeerSlot(slot) => write!(f, "invalid DHV2 peer slot: {slot}"),
            Self::OpusPayloadTooLarge(len) => {
                write!(f, "Opus payload too large: {len} bytes")
            }
            Self::TooManyPeers(count) => write!(f, "too many peers in PeerMap: {count}"),
            Self::MalformedPayload => f.write_str("malformed DHV2 message payload"),
        }
    }
}

impl std::error::Error for ProtoError {}
