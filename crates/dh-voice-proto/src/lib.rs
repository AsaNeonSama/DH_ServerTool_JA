//! Compact DHV2 UDP wire protocol shared between `dh-server-voice` and `dh-client-voice`.

#![forbid(unsafe_code)]

pub mod codec;
pub mod consts;
pub mod structs;

pub use codec::{
    decode_header, decode_message, encode_bye, encode_bye_into, encode_hello, encode_hello_ack,
    encode_hello_ack_into, encode_hello_into, encode_message, encode_message_into, encode_peer_map,
    encode_peer_map_into, encode_voice_frame, encode_voice_frame_into, is_dhv2_packet,
    patch_voice_frame_peer_slot,
};
pub use consts::{
    DHV2_HEADER_LEN, DHV2_MAGIC, DHV2_VERSION, FLAG_END_OF_TALKSPURT, FLAG_HAS_FEC,
    MAX_OPUS_PAYLOAD, MAX_PACKET_SIZE, MAX_PAYLOAD_SIZE, MAX_PEERS, MAX_PEER_SLOT, MAX_UID_LEN,
    MIN_PEER_SLOT,
};
pub use structs::{Dhv2Header, Dhv2Message, MsgType, PeerMapEntry, ProtoError};
