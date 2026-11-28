//! Unit tests for the DHV2 UDP wire protocol codec.

use dh_voice_proto::consts::{
    DHV2_HEADER_LEN, DHV2_MAGIC, DHV2_VERSION, FLAG_END_OF_TALKSPURT, FLAG_HAS_FEC,
    MAX_OPUS_PAYLOAD, MAX_PACKET_SIZE, MAX_PEERS, MAX_PEER_SLOT, MAX_UID_LEN, OFFSET_MSG_TYPE,
    OFFSET_PAYLOAD_LEN, OFFSET_PEER_SLOT, OFFSET_VERSION,
};
use dh_voice_proto::{
    decode_header, decode_message, encode_bye, encode_hello, encode_hello_ack, encode_message,
    encode_message_into, encode_peer_map, encode_voice_frame, is_dhv2_packet,
    patch_voice_frame_peer_slot, Dhv2Header, Dhv2Message, MsgType, PeerMapEntry, ProtoError,
};

#[test]
fn magic_detection_matches_only_dhv2_prefix() {
    assert!(!is_dhv2_packet(&[]));
    assert!(!is_dhv2_packet(&DHV2_MAGIC[..7]));
    assert!(is_dhv2_packet(&DHV2_MAGIC));

    let ue4_like: [u8; 32] = [0x01; 32];
    assert!(!is_dhv2_packet(&ue4_like));

    let mut wrong_magic: [u8; 22] = [0u8; 22];
    wrong_magic[..8].copy_from_slice(&DHV2_MAGIC);
    wrong_magic[7] ^= 0x01;
    assert!(!is_dhv2_packet(&wrong_magic));
}

#[test]
fn round_trip_hello() {
    let uid: &str = "76561198000000001|0002abcdef1234567890";
    let encoded: Vec<u8> = encode_hello(uid).unwrap();
    assert!(is_dhv2_packet(&encoded));
    assert_eq!(encoded.len(), DHV2_HEADER_LEN + uid.len());

    let header: Dhv2Header = decode_header(&encoded).unwrap();
    assert_eq!(header.version, DHV2_VERSION);
    assert_eq!(header.msg_type, MsgType::Hello);
    assert_eq!(header.payload_len, u16::try_from(uid.len()).unwrap());

    let decoded: Dhv2Message = decode_message(&encoded).unwrap();
    assert_eq!(
        decoded,
        Dhv2Message::Hello {
            uid: uid.to_owned()
        }
    );
}

#[test]
fn round_trip_hello_ack() {
    let encoded: Vec<u8> = encode_hello_ack(7).unwrap();
    assert_eq!(encoded.len(), DHV2_HEADER_LEN);
    assert_eq!(
        decode_message(&encoded).unwrap(),
        Dhv2Message::HelloAck { peer_slot: 7 }
    );

    assert_eq!(encode_hello_ack(0), Err(ProtoError::InvalidPeerSlot(0)));
    assert_eq!(
        encode_hello_ack(MAX_PEER_SLOT + 1),
        Err(ProtoError::InvalidPeerSlot(MAX_PEER_SLOT + 1))
    );
}

#[test]
fn round_trip_peer_map_empty_and_populated() {
    let empty_pkt: Vec<u8> = encode_peer_map(&[]).unwrap();
    assert_eq!(empty_pkt.len(), DHV2_HEADER_LEN + 1);
    assert_eq!(
        decode_message(&empty_pkt).unwrap(),
        Dhv2Message::PeerMap {
            entries: Vec::new()
        }
    );

    let entries: Vec<PeerMapEntry> = vec![
        PeerMapEntry {
            peer_slot: 1,
            uid: "76561198000000001|eosA".to_owned(),
        },
        PeerMapEntry {
            peer_slot: 8,
            uid: "76561198000000008|eosH".to_owned(),
        },
    ];
    let pkt: Vec<u8> = encode_peer_map(&entries).unwrap();
    assert_eq!(
        decode_message(&pkt).unwrap(),
        Dhv2Message::PeerMap { entries }
    );
}

#[test]
fn round_trip_voice_frame_and_flags() {
    let opus: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x11, 0x22, 0x33];
    let flags: u8 = FLAG_END_OF_TALKSPURT | FLAG_HAS_FEC;
    let pkt: Vec<u8> = encode_voice_frame(3, flags, 42_001, 0x1234_5678, &opus).unwrap();
    assert_eq!(pkt.len(), DHV2_HEADER_LEN + opus.len());

    let decoded: Dhv2Message = decode_message(&pkt).unwrap();
    assert_eq!(
        decoded,
        Dhv2Message::VoiceFrame {
            peer_slot: 3,
            flags,
            seq: 42_001,
            timestamp: 0x1234_5678,
            payload: opus,
        }
    );

    // Zero-byte end-of-talkspurt voice frame must also round-trip.
    let eot: Vec<u8> =
        encode_voice_frame(3, FLAG_END_OF_TALKSPURT, 42_002, 0x1234_5A38, &[]).unwrap();
    assert_eq!(
        decode_message(&eot).unwrap(),
        Dhv2Message::VoiceFrame {
            peer_slot: 3,
            flags: FLAG_END_OF_TALKSPURT,
            seq: 42_002,
            timestamp: 0x1234_5A38,
            payload: Vec::new(),
        }
    );
}

#[test]
fn round_trip_bye_and_encode_message_into() {
    let pkt: Vec<u8> = encode_bye(12);
    assert_eq!(pkt.len(), DHV2_HEADER_LEN);
    assert_eq!(
        decode_message(&pkt).unwrap(),
        Dhv2Message::Bye { peer_slot: 12 }
    );

    let msg: Dhv2Message = Dhv2Message::Bye { peer_slot: 12 };
    assert_eq!(encode_message(&msg).unwrap(), pkt);

    let mut stack_buf: [u8; 64] = [0u8; 64];
    let written: usize = encode_message_into(&msg, &mut stack_buf).unwrap();
    assert_eq!(&stack_buf[..written], pkt.as_slice());

    let mut tiny_buf: [u8; 10] = [0u8; 10];
    assert_eq!(
        encode_message_into(&msg, &mut tiny_buf),
        Err(ProtoError::BufferTooSmall)
    );
}

#[test]
fn patch_voice_frame_peer_slot_overwrites_authoritatively() {
    let payload: [u8; 5] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
    let mut pkt: Vec<u8> = encode_voice_frame(99, FLAG_HAS_FEC, 7, 960, &payload).unwrap();

    assert!(patch_voice_frame_peer_slot(&mut pkt, 4));
    assert_eq!(
        decode_message(&pkt).unwrap(),
        Dhv2Message::VoiceFrame {
            peer_slot: 4,
            flags: FLAG_HAS_FEC,
            seq: 7,
            timestamp: 960,
            payload: payload.to_vec(),
        }
    );

    // Invalid target slots must fail without mutating buffer.
    let snapshot: Vec<u8> = pkt.clone();
    assert!(!patch_voice_frame_peer_slot(&mut pkt, 0));
    assert!(!patch_voice_frame_peer_slot(&mut pkt, MAX_PEER_SLOT + 1));
    assert_eq!(pkt, snapshot);

    // Non-VoiceFrame packet must fail without mutating buffer.
    let mut hello: Vec<u8> = encode_hello("user-1").unwrap();
    let hello_snap: Vec<u8> = hello.clone();
    assert!(!patch_voice_frame_peer_slot(&mut hello, 2));
    assert_eq!(hello, hello_snap);

    // Payload-length mismatch must fail.
    let mut truncated: Vec<u8> = pkt[..pkt.len() - 1].to_vec();
    assert!(!patch_voice_frame_peer_slot(&mut truncated, 2));
}

#[test]
fn rejects_corrupted_truncated_and_oversized_packets() {
    // Truncated header
    assert_eq!(decode_header(&DHV2_MAGIC), Err(ProtoError::Truncated));

    // Oversized packet (> MAX_PACKET_SIZE)
    let mut huge: Vec<u8> = vec![0u8; MAX_PACKET_SIZE + 1];
    huge[..8].copy_from_slice(&DHV2_MAGIC);
    assert_eq!(decode_header(&huge), Err(ProtoError::PacketTooLarge));

    // Wrong version
    let mut bad_ver: Vec<u8> = encode_bye(1);
    bad_ver[OFFSET_VERSION] = 99;
    assert_eq!(
        decode_header(&bad_ver),
        Err(ProtoError::UnsupportedVersion(99))
    );

    // Unknown msg_type
    let mut bad_type: Vec<u8> = encode_bye(1);
    bad_type[OFFSET_MSG_TYPE] = 250;
    assert_eq!(
        decode_header(&bad_type),
        Err(ProtoError::UnknownMsgType(250))
    );

    // Payload length mismatch
    let mut bad_len: Vec<u8> = encode_hello("abc").unwrap();
    bad_len[OFFSET_PAYLOAD_LEN..OFFSET_PAYLOAD_LEN + 2].copy_from_slice(&10u16.to_le_bytes());
    assert_eq!(
        decode_header(&bad_len),
        Err(ProtoError::PayloadLengthMismatch {
            declared: 10,
            actual: 3
        })
    );

    // Empty / NUL / oversized / non-UTF-8 UID
    assert_eq!(encode_hello(""), Err(ProtoError::InvalidUid));
    assert_eq!(encode_hello("abc\0def"), Err(ProtoError::InvalidUid));
    let long_uid: String = "x".repeat(MAX_UID_LEN + 1);
    assert_eq!(encode_hello(&long_uid), Err(ProtoError::InvalidUid));

    let mut non_utf8_hello: Vec<u8> = encode_hello("abcd").unwrap();
    non_utf8_hello[DHV2_HEADER_LEN] = 0xFF;
    assert_eq!(decode_message(&non_utf8_hello), Err(ProtoError::InvalidUid));

    // Oversized Opus payload
    let big_opus: Vec<u8> = vec![0x55; MAX_OPUS_PAYLOAD + 1];
    assert_eq!(
        encode_voice_frame(1, 0, 1, 0, &big_opus),
        Err(ProtoError::OpusPayloadTooLarge(MAX_OPUS_PAYLOAD + 1))
    );

    // HelloAck with invalid slot or non-empty payload
    let mut ack_bad_slot: Vec<u8> = encode_hello_ack(1).unwrap();
    ack_bad_slot[OFFSET_PEER_SLOT..OFFSET_PEER_SLOT + 2].copy_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        decode_message(&ack_bad_slot),
        Err(ProtoError::InvalidPeerSlot(0))
    );

    // PeerMap exceeding MAX_PEERS
    let too_many: Vec<PeerMapEntry> = (0..=MAX_PEERS)
        .map(|i: usize| PeerMapEntry {
            peer_slot: 1,
            uid: format!("u{i}"),
        })
        .collect();
    assert_eq!(
        encode_peer_map(&too_many),
        Err(ProtoError::TooManyPeers(MAX_PEERS + 1))
    );

    // PeerMap exceeding MAX_PACKET_SIZE even with <= MAX_PEERS entries
    let big_map: Vec<PeerMapEntry> = (1u16..=20u16)
        .map(|slot: u16| PeerMapEntry {
            peer_slot: slot,
            uid: "a".repeat(100),
        })
        .collect();
    assert_eq!(encode_peer_map(&big_map), Err(ProtoError::PacketTooLarge));

    // PeerMap with trailing garbage byte
    let mut trailing_map: Vec<u8> = encode_peer_map(&[]).unwrap();
    trailing_map.push(0x99);
    let new_len: u16 = u16::try_from(trailing_map.len() - DHV2_HEADER_LEN).unwrap();
    trailing_map[OFFSET_PAYLOAD_LEN..OFFSET_PAYLOAD_LEN + 2]
        .copy_from_slice(&new_len.to_le_bytes());
    assert_eq!(
        decode_message(&trailing_map),
        Err(ProtoError::MalformedPayload)
    );
}
