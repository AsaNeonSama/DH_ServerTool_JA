//! Encoding, decoding, and zero-allocation inspection utilities for DHV2 packets.

use crate::consts::{
    DHV2_HEADER_LEN, DHV2_MAGIC, DHV2_VERSION, MAX_OPUS_PAYLOAD, MAX_PACKET_SIZE, MAX_PEERS,
    MAX_PEER_SLOT, MAX_UID_LEN, MIN_PEER_SLOT, MSG_TYPE_VOICE_FRAME, OFFSET_FLAGS, OFFSET_MAGIC,
    OFFSET_MSG_TYPE, OFFSET_PAYLOAD_LEN, OFFSET_PEER_SLOT, OFFSET_RESERVED, OFFSET_SEQ,
    OFFSET_TIMESTAMP, OFFSET_VERSION, PEER_MAP_ENTRY_OVERHEAD,
};
use crate::structs::{Dhv2Header, Dhv2Message, MsgType, PeerMapEntry, ProtoError};

/// Fast O(1) prefix check returning `true` for any datagram starting with `DHV2_MAGIC`.
///
/// The `FSocketBSD::RecvFrom` hook uses this to intercept every DHV2 packet (including
/// malformed or future-version frames) so none ever reach UE4's `UNetConnection`.
#[must_use]
pub fn is_dhv2_packet(buf: &[u8]) -> bool {
    buf.len() >= DHV2_MAGIC.len() && buf[OFFSET_MAGIC..DHV2_MAGIC.len()] == DHV2_MAGIC
}

/// Validates a player UID string (`1..=128` UTF-8 bytes, no embedded NUL).
fn validate_uid_bytes(bytes: &[u8]) -> Result<&str, ProtoError> {
    if bytes.is_empty() || bytes.len() > MAX_UID_LEN || bytes.contains(&0u8) {
        return Err(ProtoError::InvalidUid);
    }
    std::str::from_utf8(bytes).map_err(|_err: std::str::Utf8Error| ProtoError::InvalidUid)
}

/// Validates that a peer slot lies in the server-assigned range `1..=64`.
const fn validate_peer_slot(slot: u16) -> Result<u16, ProtoError> {
    if slot < MIN_PEER_SLOT || slot > MAX_PEER_SLOT {
        return Err(ProtoError::InvalidPeerSlot(slot));
    }
    Ok(slot)
}

/// Reads a little-endian `u16` from `buf` at `offset` (caller guarantees bounds).
fn read_u16_le(buf: &[u8], offset: usize) -> Result<u16, ProtoError> {
    let end: usize = offset.checked_add(2).ok_or(ProtoError::Truncated)?;
    let slice: &[u8] = buf.get(offset..end).ok_or(ProtoError::Truncated)?;
    let bytes: [u8; 2] = [slice[0], slice[1]];
    Ok(u16::from_le_bytes(bytes))
}

/// Reads a little-endian `u32` from `buf` at `offset` (caller guarantees bounds).
fn read_u32_le(buf: &[u8], offset: usize) -> Result<u32, ProtoError> {
    let end: usize = offset.checked_add(4).ok_or(ProtoError::Truncated)?;
    let slice: &[u8] = buf.get(offset..end).ok_or(ProtoError::Truncated)?;
    let bytes: [u8; 4] = [slice[0], slice[1], slice[2], slice[3]];
    Ok(u32::from_le_bytes(bytes))
}

/// Writes the fixed 22-byte DHV2 header into `out[..DHV2_HEADER_LEN]`.
fn write_header(out: &mut [u8], header: Dhv2Header) -> Result<(), ProtoError> {
    if out.len() < DHV2_HEADER_LEN {
        return Err(ProtoError::BufferTooSmall);
    }
    out[OFFSET_MAGIC..DHV2_MAGIC.len()].copy_from_slice(&DHV2_MAGIC);
    out[OFFSET_VERSION] = header.version;
    out[OFFSET_MSG_TYPE] = header.msg_type.as_u8();
    out[OFFSET_FLAGS] = header.flags;
    out[OFFSET_RESERVED] = header.reserved;
    out[OFFSET_PEER_SLOT..OFFSET_PEER_SLOT + 2].copy_from_slice(&header.peer_slot.to_le_bytes());
    out[OFFSET_SEQ..OFFSET_SEQ + 2].copy_from_slice(&header.seq.to_le_bytes());
    out[OFFSET_TIMESTAMP..OFFSET_TIMESTAMP + 4].copy_from_slice(&header.timestamp.to_le_bytes());
    out[OFFSET_PAYLOAD_LEN..OFFSET_PAYLOAD_LEN + 2]
        .copy_from_slice(&header.payload_len.to_le_bytes());
    Ok(())
}

/// Decodes and validates the fixed 22-byte DHV2 header without allocating.
///
/// Verifies magic, protocol version, message type, total size cap (`<= MAX_PACKET_SIZE`),
/// and exact `payload_len == buf.len() - DHV2_HEADER_LEN`.
pub fn decode_header(buf: &[u8]) -> Result<Dhv2Header, ProtoError> {
    if buf.len() < DHV2_HEADER_LEN {
        return Err(ProtoError::Truncated);
    }
    if buf.len() > MAX_PACKET_SIZE {
        return Err(ProtoError::PacketTooLarge);
    }
    if buf[OFFSET_MAGIC..DHV2_MAGIC.len()] != DHV2_MAGIC {
        return Err(ProtoError::InvalidMagic);
    }

    let version: u8 = buf[OFFSET_VERSION];
    if version != DHV2_VERSION {
        return Err(ProtoError::UnsupportedVersion(version));
    }

    let raw_type: u8 = buf[OFFSET_MSG_TYPE];
    let msg_type: MsgType =
        MsgType::from_u8(raw_type).ok_or(ProtoError::UnknownMsgType(raw_type))?;
    let flags: u8 = buf[OFFSET_FLAGS];
    let reserved: u8 = buf[OFFSET_RESERVED];
    let peer_slot: u16 = read_u16_le(buf, OFFSET_PEER_SLOT)?;
    let seq: u16 = read_u16_le(buf, OFFSET_SEQ)?;
    let timestamp: u32 = read_u32_le(buf, OFFSET_TIMESTAMP)?;
    let payload_len: u16 = read_u16_le(buf, OFFSET_PAYLOAD_LEN)?;

    let declared: usize = usize::from(payload_len);
    let actual: usize = buf.len() - DHV2_HEADER_LEN;
    if declared != actual {
        return Err(ProtoError::PayloadLengthMismatch { declared, actual });
    }

    Ok(Dhv2Header {
        version,
        msg_type,
        flags,
        reserved,
        peer_slot,
        seq,
        timestamp,
        payload_len,
    })
}

/// Decodes a complete DHV2 packet into a strongly typed [`Dhv2Message`].
pub fn decode_message(buf: &[u8]) -> Result<Dhv2Message, ProtoError> {
    let header: Dhv2Header = decode_header(buf)?;
    let payload: &[u8] = &buf[DHV2_HEADER_LEN..];

    match header.msg_type {
        MsgType::Hello => {
            let uid_str: &str = validate_uid_bytes(payload)?;
            Ok(Dhv2Message::Hello {
                uid: uid_str.to_owned(),
            })
        }
        MsgType::HelloAck => {
            if !payload.is_empty() {
                return Err(ProtoError::MalformedPayload);
            }
            let slot: u16 = validate_peer_slot(header.peer_slot)?;
            Ok(Dhv2Message::HelloAck { peer_slot: slot })
        }
        MsgType::PeerMap => decode_peer_map_payload(payload),
        MsgType::VoiceFrame => {
            if payload.len() > MAX_OPUS_PAYLOAD {
                return Err(ProtoError::OpusPayloadTooLarge(payload.len()));
            }
            Ok(Dhv2Message::VoiceFrame {
                peer_slot: header.peer_slot,
                flags: header.flags,
                seq: header.seq,
                timestamp: header.timestamp,
                payload: payload.to_vec(),
            })
        }
        MsgType::Bye => {
            if !payload.is_empty() {
                return Err(ProtoError::MalformedPayload);
            }
            Ok(Dhv2Message::Bye {
                peer_slot: header.peer_slot,
            })
        }
    }
}

/// Decodes the payload section of a `PeerMap` message.
fn decode_peer_map_payload(payload: &[u8]) -> Result<Dhv2Message, ProtoError> {
    let count_byte: u8 = *payload.first().ok_or(ProtoError::Truncated)?;
    let count: usize = usize::from(count_byte);
    if count > MAX_PEERS {
        return Err(ProtoError::TooManyPeers(count));
    }

    let mut entries: Vec<PeerMapEntry> = Vec::with_capacity(count);
    let mut cursor: usize = 1;

    for _idx in 0..count {
        let header_end: usize = cursor
            .checked_add(PEER_MAP_ENTRY_OVERHEAD)
            .ok_or(ProtoError::Truncated)?;
        if header_end > payload.len() {
            return Err(ProtoError::Truncated);
        }

        let raw_slot: u16 = read_u16_le(payload, cursor)?;
        let peer_slot: u16 = validate_peer_slot(raw_slot)?;
        let uid_len: usize = usize::from(payload[cursor + 2]);
        cursor = header_end;

        let uid_end: usize = cursor.checked_add(uid_len).ok_or(ProtoError::Truncated)?;
        let uid_slice: &[u8] = payload.get(cursor..uid_end).ok_or(ProtoError::Truncated)?;
        let uid_str: &str = validate_uid_bytes(uid_slice)?;
        cursor = uid_end;

        entries.push(PeerMapEntry {
            peer_slot,
            uid: uid_str.to_owned(),
        });
    }

    if cursor != payload.len() {
        return Err(ProtoError::MalformedPayload);
    }

    Ok(Dhv2Message::PeerMap { entries })
}

/// Computes the exact wire byte length of a `PeerMap` payload and validates all entries.
fn validate_peer_map_entries(entries: &[PeerMapEntry]) -> Result<usize, ProtoError> {
    if entries.len() > MAX_PEERS {
        return Err(ProtoError::TooManyPeers(entries.len()));
    }
    let mut payload_len: usize = 1;
    for entry in entries {
        let _valid_slot: u16 = validate_peer_slot(entry.peer_slot)?;
        let _valid_uid: &str = validate_uid_bytes(entry.uid.as_bytes())?;
        let entry_size: usize = PEER_MAP_ENTRY_OVERHEAD
            .checked_add(entry.uid.len())
            .ok_or(ProtoError::PacketTooLarge)?;
        payload_len = payload_len
            .checked_add(entry_size)
            .ok_or(ProtoError::PacketTooLarge)?;
    }
    let total_len: usize = DHV2_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(ProtoError::PacketTooLarge)?;
    if total_len > MAX_PACKET_SIZE {
        return Err(ProtoError::PacketTooLarge);
    }
    Ok(payload_len)
}

/// Encodes a `VoiceFrame` directly into a caller-provided buffer without heap allocation.
pub fn encode_voice_frame_into(
    out: &mut [u8],
    peer_slot: u16,
    flags: u8,
    seq: u16,
    timestamp: u32,
    payload: &[u8],
) -> Result<usize, ProtoError> {
    if payload.len() > MAX_OPUS_PAYLOAD {
        return Err(ProtoError::OpusPayloadTooLarge(payload.len()));
    }
    let total_len: usize = DHV2_HEADER_LEN
        .checked_add(payload.len())
        .ok_or(ProtoError::PacketTooLarge)?;
    if total_len > MAX_PACKET_SIZE {
        return Err(ProtoError::PacketTooLarge);
    }
    if out.len() < total_len {
        return Err(ProtoError::BufferTooSmall);
    }

    let payload_len_u16: u16 = u16::try_from(payload.len())
        .map_err(|_err: core::num::TryFromIntError| ProtoError::PacketTooLarge)?;
    let header: Dhv2Header = Dhv2Header {
        version: DHV2_VERSION,
        msg_type: MsgType::VoiceFrame,
        flags,
        reserved: 0,
        peer_slot,
        seq,
        timestamp,
        payload_len: payload_len_u16,
    };
    write_header(out, header)?;
    out[DHV2_HEADER_LEN..total_len].copy_from_slice(payload);
    Ok(total_len)
}

/// Encodes a `Hello` message into a caller-provided buffer without heap allocation.
pub fn encode_hello_into(out: &mut [u8], uid: &str) -> Result<usize, ProtoError> {
    let uid_bytes: &[u8] = validate_uid_bytes(uid.as_bytes())?.as_bytes();
    let total_len: usize = DHV2_HEADER_LEN + uid_bytes.len();
    if out.len() < total_len {
        return Err(ProtoError::BufferTooSmall);
    }
    let payload_len_u16: u16 = u16::try_from(uid_bytes.len())
        .map_err(|_err: core::num::TryFromIntError| ProtoError::InvalidUid)?;
    let header: Dhv2Header = Dhv2Header {
        version: DHV2_VERSION,
        msg_type: MsgType::Hello,
        flags: 0,
        reserved: 0,
        peer_slot: 0,
        seq: 0,
        timestamp: 0,
        payload_len: payload_len_u16,
    };
    write_header(out, header)?;
    out[DHV2_HEADER_LEN..total_len].copy_from_slice(uid_bytes);
    Ok(total_len)
}

/// Encodes a `HelloAck` message into a caller-provided buffer without heap allocation.
pub fn encode_hello_ack_into(out: &mut [u8], peer_slot: u16) -> Result<usize, ProtoError> {
    let valid_slot: u16 = validate_peer_slot(peer_slot)?;
    if out.len() < DHV2_HEADER_LEN {
        return Err(ProtoError::BufferTooSmall);
    }
    let header: Dhv2Header = Dhv2Header {
        version: DHV2_VERSION,
        msg_type: MsgType::HelloAck,
        flags: 0,
        reserved: 0,
        peer_slot: valid_slot,
        seq: 0,
        timestamp: 0,
        payload_len: 0,
    };
    write_header(out, header)?;
    Ok(DHV2_HEADER_LEN)
}

/// Encodes a `PeerMap` message into a caller-provided buffer without heap allocation.
pub fn encode_peer_map_into(out: &mut [u8], entries: &[PeerMapEntry]) -> Result<usize, ProtoError> {
    let payload_len: usize = validate_peer_map_entries(entries)?;
    let total_len: usize = DHV2_HEADER_LEN + payload_len;
    if out.len() < total_len {
        return Err(ProtoError::BufferTooSmall);
    }

    let payload_len_u16: u16 = u16::try_from(payload_len)
        .map_err(|_err: core::num::TryFromIntError| ProtoError::PacketTooLarge)?;
    let header: Dhv2Header = Dhv2Header {
        version: DHV2_VERSION,
        msg_type: MsgType::PeerMap,
        flags: 0,
        reserved: 0,
        peer_slot: 0,
        seq: 0,
        timestamp: 0,
        payload_len: payload_len_u16,
    };
    write_header(out, header)?;

    let count_u8: u8 = u8::try_from(entries.len())
        .map_err(|_err: core::num::TryFromIntError| ProtoError::TooManyPeers(entries.len()))?;
    let mut cursor: usize = DHV2_HEADER_LEN;
    out[cursor] = count_u8;
    cursor += 1;

    for entry in entries {
        let uid_bytes: &[u8] = entry.uid.as_bytes();
        let uid_len_u8: u8 = u8::try_from(uid_bytes.len())
            .map_err(|_err: core::num::TryFromIntError| ProtoError::InvalidUid)?;
        out[cursor..cursor + 2].copy_from_slice(&entry.peer_slot.to_le_bytes());
        out[cursor + 2] = uid_len_u8;
        cursor += PEER_MAP_ENTRY_OVERHEAD;
        out[cursor..cursor + uid_bytes.len()].copy_from_slice(uid_bytes);
        cursor += uid_bytes.len();
    }

    Ok(total_len)
}

/// Encodes a `Bye` message into a caller-provided buffer without heap allocation.
pub fn encode_bye_into(out: &mut [u8], peer_slot: u16) -> Result<usize, ProtoError> {
    if out.len() < DHV2_HEADER_LEN {
        return Err(ProtoError::BufferTooSmall);
    }
    let header: Dhv2Header = Dhv2Header {
        version: DHV2_VERSION,
        msg_type: MsgType::Bye,
        flags: 0,
        reserved: 0,
        peer_slot,
        seq: 0,
        timestamp: 0,
        payload_len: 0,
    };
    write_header(out, header)?;
    Ok(DHV2_HEADER_LEN)
}

/// Encodes any [`Dhv2Message`] into a caller-provided buffer without heap allocation.
pub fn encode_message_into(msg: &Dhv2Message, out: &mut [u8]) -> Result<usize, ProtoError> {
    match msg {
        Dhv2Message::Hello { uid } => encode_hello_into(out, uid),
        Dhv2Message::HelloAck { peer_slot } => encode_hello_ack_into(out, *peer_slot),
        Dhv2Message::PeerMap { entries } => encode_peer_map_into(out, entries),
        Dhv2Message::VoiceFrame {
            peer_slot,
            flags,
            seq,
            timestamp,
            payload,
        } => encode_voice_frame_into(out, *peer_slot, *flags, *seq, *timestamp, payload),
        Dhv2Message::Bye { peer_slot } => encode_bye_into(out, *peer_slot),
    }
}

/// Encodes a `Hello` message into a newly allocated `Vec<u8>`.
pub fn encode_hello(uid: &str) -> Result<Vec<u8>, ProtoError> {
    let valid_uid: &str = validate_uid_bytes(uid.as_bytes())?;
    let mut out: Vec<u8> = vec![0u8; DHV2_HEADER_LEN + valid_uid.len()];
    let written: usize = encode_hello_into(&mut out, valid_uid)?;
    out.truncate(written);
    Ok(out)
}

/// Encodes a `HelloAck` message into a newly allocated `Vec<u8>`.
pub fn encode_hello_ack(peer_slot: u16) -> Result<Vec<u8>, ProtoError> {
    let mut out: Vec<u8> = vec![0u8; DHV2_HEADER_LEN];
    let written: usize = encode_hello_ack_into(&mut out, peer_slot)?;
    out.truncate(written);
    Ok(out)
}

/// Encodes a `PeerMap` message into a newly allocated `Vec<u8>`.
pub fn encode_peer_map(entries: &[PeerMapEntry]) -> Result<Vec<u8>, ProtoError> {
    let payload_len: usize = validate_peer_map_entries(entries)?;
    let mut out: Vec<u8> = vec![0u8; DHV2_HEADER_LEN + payload_len];
    let written: usize = encode_peer_map_into(&mut out, entries)?;
    out.truncate(written);
    Ok(out)
}

/// Encodes a `VoiceFrame` message into a newly allocated `Vec<u8>`.
pub fn encode_voice_frame(
    peer_slot: u16,
    flags: u8,
    seq: u16,
    timestamp: u32,
    payload: &[u8],
) -> Result<Vec<u8>, ProtoError> {
    if payload.len() > MAX_OPUS_PAYLOAD {
        return Err(ProtoError::OpusPayloadTooLarge(payload.len()));
    }
    let mut out: Vec<u8> = vec![0u8; DHV2_HEADER_LEN + payload.len()];
    let written: usize =
        encode_voice_frame_into(&mut out, peer_slot, flags, seq, timestamp, payload)?;
    out.truncate(written);
    Ok(out)
}

/// Encodes a `Bye` message into a newly allocated `Vec<u8>`.
#[must_use]
pub fn encode_bye(peer_slot: u16) -> Vec<u8> {
    let mut out: Vec<u8> = vec![0u8; DHV2_HEADER_LEN];
    let _res: Result<usize, ProtoError> = encode_bye_into(&mut out, peer_slot);
    out
}

/// Encodes any [`Dhv2Message`] into a newly allocated `Vec<u8>`.
pub fn encode_message(msg: &Dhv2Message) -> Result<Vec<u8>, ProtoError> {
    match msg {
        Dhv2Message::Hello { uid } => encode_hello(uid),
        Dhv2Message::HelloAck { peer_slot } => encode_hello_ack(*peer_slot),
        Dhv2Message::PeerMap { entries } => encode_peer_map(entries),
        Dhv2Message::VoiceFrame {
            peer_slot,
            flags,
            seq,
            timestamp,
            payload,
        } => encode_voice_frame(*peer_slot, *flags, *seq, *timestamp, payload),
        Dhv2Message::Bye { peer_slot } => Ok(encode_bye(*peer_slot)),
    }
}

/// Zero-allocation in-place authoritative `peer_slot` rewrite for a `VoiceFrame` packet.
///
/// Validates that `buf` is a well-formed `DHV2` `VoiceFrame` (version 2, exact payload length,
/// payload `<= MAX_OPUS_PAYLOAD`) and that `slot` is in `1..=64`. Returns `true` after
/// overwriting `peer_slot`, or `false` without modifying `buf` if validation fails.
#[must_use]
pub fn patch_voice_frame_peer_slot(buf: &mut [u8], slot: u16) -> bool {
    if !(MIN_PEER_SLOT..=MAX_PEER_SLOT).contains(&slot) {
        return false;
    }
    if buf.len() < DHV2_HEADER_LEN || buf.len() > MAX_PACKET_SIZE {
        return false;
    }
    if buf[OFFSET_MAGIC..DHV2_MAGIC.len()] != DHV2_MAGIC {
        return false;
    }
    if buf[OFFSET_VERSION] != DHV2_VERSION || buf[OFFSET_MSG_TYPE] != MSG_TYPE_VOICE_FRAME {
        return false;
    }
    let declared_bytes: [u8; 2] = [buf[OFFSET_PAYLOAD_LEN], buf[OFFSET_PAYLOAD_LEN + 1]];
    let declared_len: usize = usize::from(u16::from_le_bytes(declared_bytes));
    let actual_len: usize = buf.len() - DHV2_HEADER_LEN;
    if declared_len != actual_len || declared_len > MAX_OPUS_PAYLOAD {
        return false;
    }

    buf[OFFSET_PEER_SLOT..OFFSET_PEER_SLOT + 2].copy_from_slice(&slot.to_le_bytes());
    true
}
