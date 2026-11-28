//! Protocol constants for the DHV2 UDP voice wire format.

/// 8-byte magic prefix guaranteed not to collide with UE4 `UNetConnection` packets.
pub const DHV2_MAGIC: [u8; 8] = [0xFE, 0x44, 0x48, 0x56, 0x32, 0x00, 0xA5, 0x5A];

/// Protocol version number.
pub const DHV2_VERSION: u8 = 2;

/// Fixed DHV2 header length in bytes.
pub const DHV2_HEADER_LEN: usize = 22;

/// Byte offset of the 8-byte magic prefix.
pub const OFFSET_MAGIC: usize = 0;

/// Byte offset of the protocol version byte.
pub const OFFSET_VERSION: usize = 8;

/// Byte offset of the message type byte.
pub const OFFSET_MSG_TYPE: usize = 9;

/// Byte offset of the flags bitmask byte.
pub const OFFSET_FLAGS: usize = 10;

/// Byte offset of the reserved byte.
pub const OFFSET_RESERVED: usize = 11;

/// Byte offset of the little-endian `u16` `peer_slot` field.
pub const OFFSET_PEER_SLOT: usize = 12;

/// Byte offset of the little-endian `u16` sequence number field.
pub const OFFSET_SEQ: usize = 14;

/// Byte offset of the little-endian `u32` timestamp field.
pub const OFFSET_TIMESTAMP: usize = 16;

/// Byte offset of the little-endian `u16` payload length field.
pub const OFFSET_PAYLOAD_LEN: usize = 20;

/// Flag bit 0: marks the end of a talkspurt.
pub const FLAG_END_OF_TALKSPURT: u8 = 1 << 0;

/// Flag bit 1: indicates in-band FEC data is present in the Opus payload.
pub const FLAG_HAS_FEC: u8 = 1 << 1;

/// Wire discriminant for `Hello`.
pub const MSG_TYPE_HELLO: u8 = 1;

/// Wire discriminant for `HelloAck`.
pub const MSG_TYPE_HELLO_ACK: u8 = 2;

/// Wire discriminant for `PeerMap`.
pub const MSG_TYPE_PEER_MAP: u8 = 3;

/// Wire discriminant for `VoiceFrame`.
pub const MSG_TYPE_VOICE_FRAME: u8 = 4;

/// Wire discriminant for `Bye`.
pub const MSG_TYPE_BYE: u8 = 5;

/// Maximum UTF-8 byte length of a player UID string.
pub const MAX_UID_LEN: usize = 128;

/// Maximum Opus payload byte length in a single `VoiceFrame`.
pub const MAX_OPUS_PAYLOAD: usize = 512;

/// Maximum total DHV2 datagram size in bytes (safe below standard Ethernet MTU).
pub const MAX_PACKET_SIZE: usize = 1200;

/// Maximum payload length that fits inside `MAX_PACKET_SIZE`.
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - DHV2_HEADER_LEN;

/// Maximum concurrent voice peers supported per server socket.
pub const MAX_PEERS: usize = 64;

/// Minimum valid server-assigned peer slot (inclusive).
pub const MIN_PEER_SLOT: u16 = 1;

/// Maximum valid server-assigned peer slot (inclusive).
pub const MAX_PEER_SLOT: u16 = 64;

/// Minimum payload header bytes per entry inside a `PeerMap` (`peer_slot: u16` + `uid_len: u8`).
pub const PEER_MAP_ENTRY_OVERHEAD: usize = 3;
