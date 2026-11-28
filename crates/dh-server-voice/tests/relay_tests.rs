//! Comprehensive unit tests for the hardened zero-extra-port DHV2 server voice relay state machine.

use dh_voice_proto::consts::FLAG_HAS_FEC;
use dh_voice_proto::{
    decode_message, encode_bye, encode_hello, encode_voice_frame, Dhv2Message, PeerMapEntry,
};
use dh_voice_server::consts::patch::{
    ADDR_FAMILY_INET, ADDR_FAMILY_INET6, SOCKADDR_IN6_LEN, SOCKADDR_IN_LEN,
};
use dh_voice_server::consts::relay::{
    GAME_PEER_AUTH_TTL_MS, GLOBAL_RELAY_BURST_PACKETS, HELLO_BURST_PACKETS,
    MAX_AUTHORIZED_GAME_PEERS, MAX_SERVER_UID_LEN, MAX_SERVER_VOICE_PAYLOAD, MAX_VOICE_PEERS,
    VOICE_BURST_PACKETS, VOICE_PEER_TIMEOUT_MS,
};
use dh_voice_server::{
    is_valid_server_uid, OutboundControlPacket, PeerEndpoint, RelayDispatch, RelayEngine,
};

fn v4(last_octet: u8, port: u16) -> PeerEndpoint {
    PeerEndpoint::V4 {
        addr: [127, 0, 0, last_octet],
        port,
    }
}

#[test]
fn sockaddr_v4_and_v6_round_trip() {
    let ep4: PeerEndpoint = v4(1, 51_234);
    let (raw4, len4): ([u8; 28], i32) = ep4.to_sockaddr_bytes();
    assert_eq!(len4, SOCKADDR_IN_LEN);
    assert_eq!(u16::from_ne_bytes([raw4[0], raw4[1]]), ADDR_FAMILY_INET);
    assert_eq!(PeerEndpoint::from_sockaddr_bytes(&raw4), Some(ep4));

    let ep6: PeerEndpoint = PeerEndpoint::V6 {
        addr: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 127, 0, 0, 1],
        port: 54_321,
        flowinfo: [1, 2, 3, 4],
        scope_id: [5, 6, 7, 8],
    };
    let (raw6, len6): ([u8; 28], i32) = ep6.to_sockaddr_bytes();
    assert_eq!(len6, SOCKADDR_IN6_LEN);
    assert_eq!(u16::from_ne_bytes([raw6[0], raw6[1]]), ADDR_FAMILY_INET6);
    assert_eq!(PeerEndpoint::from_sockaddr_bytes(&raw6), Some(ep6));

    // Port 0 and unknown address family must be rejected.
    let mut bad_port: [u8; 28] = raw4;
    bad_port[2] = 0;
    bad_port[3] = 0;
    assert_eq!(PeerEndpoint::from_sockaddr_bytes(&bad_port), None);

    let mut bad_family: [u8; 28] = [0u8; 28];
    bad_family[0..2].copy_from_slice(&99u16.to_ne_bytes());
    assert_eq!(PeerEndpoint::from_sockaddr_bytes(&bad_family), None);
}

#[test]
fn rejects_unauthenticated_and_expired_game_peers() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 1001;
    let peer_a: PeerEndpoint = v4(1, 40_001);

    // Before post-handshake UE4 authorization: DHV2 Hello must be dropped (0x amplification).
    let mut hello: Vec<u8> = encode_hello("76561198000000001|eosA").unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut hello, 1_000),
        RelayDispatch::Drop
    );
    assert_eq!(engine.active_voice_peer_count(sock), 0);

    // Record post-handshake UE4 packet at t=1_000.
    engine.record_authenticated_peer(sock, peer_a, 1_000);

    // Within 30s TTL: Hello succeeds.
    let dispatch: RelayDispatch =
        engine.handle_dhv2_packet(sock, peer_a, &mut hello, 1_000 + GAME_PEER_AUTH_TTL_MS);
    assert!(matches!(dispatch, RelayDispatch::SendControl(_)));
    assert_eq!(engine.active_voice_peer_count(sock), 1);

    // After 30s TTL without fresh authenticated UE4 packet: DHV2 VoiceFrame is rejected.
    let mut vf: Vec<u8> = encode_voice_frame(1, 0, 1, 960, &[1, 2, 3]).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut vf, 1_000 + GAME_PEER_AUTH_TTL_MS + 1),
        RelayDispatch::Drop
    );
}

#[test]
fn full_two_peer_lifecycle_and_authoritative_slot_rewrite() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 2002;
    let peer_a: PeerEndpoint = v4(1, 50_001);
    let peer_b: PeerEndpoint = v4(1, 50_002);

    engine.record_authenticated_peer(sock, peer_a, 1_000);
    engine.record_authenticated_peer(sock, peer_b, 1_000);

    // Peer A sends Hello -> receives HelloAck(1) + PeerMap([(1, A)]).
    let mut hello_a: Vec<u8> = encode_hello("76561198000000001|eosA").unwrap();
    let disp_a: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut hello_a, 1_100);
    let RelayDispatch::SendControl(pkts_a) = disp_a else {
        panic!("expected SendControl for Peer A Hello");
    };
    assert_eq!(pkts_a.len(), 2);
    assert_eq!(pkts_a[0].target, peer_a);
    assert_eq!(
        decode_message(&pkts_a[0].payload).unwrap(),
        Dhv2Message::HelloAck { peer_slot: 1 }
    );
    assert_eq!(pkts_a[1].target, peer_a);
    assert_eq!(
        decode_message(&pkts_a[1].payload).unwrap(),
        Dhv2Message::PeerMap {
            entries: vec![PeerMapEntry {
                peer_slot: 1,
                uid: "76561198000000001|eosA".to_owned(),
            }]
        }
    );

    // Peer B sends Hello -> B gets HelloAck(2), and BOTH A & B get updated PeerMap([1, 2]).
    let mut hello_b: Vec<u8> = encode_hello("76561198000000002|eosB").unwrap();
    let disp_b: RelayDispatch = engine.handle_dhv2_packet(sock, peer_b, &mut hello_b, 1_200);
    let RelayDispatch::SendControl(pkts_b) = disp_b else {
        panic!("expected SendControl for Peer B Hello");
    };
    assert_eq!(pkts_b.len(), 3);
    assert_eq!(pkts_b[0].target, peer_b);
    assert_eq!(
        decode_message(&pkts_b[0].payload).unwrap(),
        Dhv2Message::HelloAck { peer_slot: 2 }
    );
    let expected_map: Dhv2Message = Dhv2Message::PeerMap {
        entries: vec![
            PeerMapEntry {
                peer_slot: 1,
                uid: "76561198000000001|eosA".to_owned(),
            },
            PeerMapEntry {
                peer_slot: 2,
                uid: "76561198000000002|eosB".to_owned(),
            },
        ],
    };
    let map_targets: Vec<PeerEndpoint> = pkts_b[1..]
        .iter()
        .map(|p: &OutboundControlPacket| {
            assert_eq!(decode_message(&p.payload).unwrap(), expected_map);
            p.target
        })
        .collect();
    assert!(map_targets.contains(&peer_a));
    assert!(map_targets.contains(&peer_b));

    // Keepalive Hello from Peer A (unchanged UID) replies ONLY to Peer A (no broadcast spam).
    let keepalive_dispatch: RelayDispatch =
        engine.handle_dhv2_packet(sock, peer_a, &mut hello_a, 3_000);
    let RelayDispatch::SendControl(keepalive_packets) = keepalive_dispatch else {
        panic!("expected SendControl for keepalive Hello");
    };
    assert_eq!(keepalive_packets.len(), 2);
    assert!(keepalive_packets
        .iter()
        .all(|p: &OutboundControlPacket| p.target == peer_a));

    // Peer A sends VoiceFrame with SPOOFED peer_slot = 63 -> must overwrite to 1 and relay only to Peer B!
    let opus: [u8; 6] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
    let mut voice_pkt: Vec<u8> = encode_voice_frame(63, FLAG_HAS_FEC, 42, 960, &opus).unwrap();
    let disp_voice: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut voice_pkt, 3_500);
    let RelayDispatch::RelayVoice {
        recipients,
        control_packets,
    } = disp_voice
    else {
        panic!("expected RelayVoice");
    };
    assert_eq!(recipients, vec![peer_b]);
    assert_eq!(control_packets, Vec::<OutboundControlPacket>::new());
    assert_eq!(
        decode_message(&voice_pkt).unwrap(),
        Dhv2Message::VoiceFrame {
            peer_slot: 1,
            flags: FLAG_HAS_FEC,
            seq: 42,
            timestamp: 960,
            payload: opus.to_vec(),
        }
    );

    // Peer B sends Bye -> removed immediately, Peer A gets updated PeerMap([1]).
    let mut bye_b: Vec<u8> = encode_bye(2);
    let disp_bye: RelayDispatch = engine.handle_dhv2_packet(sock, peer_b, &mut bye_b, 4_000);
    let RelayDispatch::SendControl(pkts_bye) = disp_bye else {
        panic!("expected PeerMap broadcast after Bye");
    };
    assert_eq!(pkts_bye.len(), 1);
    assert_eq!(pkts_bye[0].target, peer_a);
    assert_eq!(
        decode_message(&pkts_bye[0].payload).unwrap(),
        Dhv2Message::PeerMap {
            entries: vec![PeerMapEntry {
                peer_slot: 1,
                uid: "76561198000000001|eosA".to_owned(),
            }]
        }
    );
    assert_eq!(engine.active_voice_peer_count(sock), 1);
}

#[test]
fn stale_peer_cleanup_and_uid_reconnect_reuse_slot() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 3003;
    let peer_a: PeerEndpoint = v4(1, 51_001);
    let peer_b: PeerEndpoint = v4(1, 51_002);

    engine.record_authenticated_peer(sock, peer_a, 1_000);
    engine.record_authenticated_peer(sock, peer_b, 1_000);

    let mut hello_a: Vec<u8> = encode_hello("uid-alpha").unwrap();
    let mut hello_b: Vec<u8> = encode_hello("uid-beta").unwrap();
    let _ra: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut hello_a, 1_000);
    let _rb: RelayDispatch = engine.handle_dhv2_packet(sock, peer_b, &mut hello_b, 1_000);
    assert_eq!(engine.active_voice_peer_count(sock), 2);

    // Advance beyond VOICE_PEER_TIMEOUT_MS while keeping Peer A active.
    let t_late: u64 = 1_000 + VOICE_PEER_TIMEOUT_MS + 1_500;
    engine.record_authenticated_peer(sock, peer_a, t_late);
    let mut ka_a: Vec<u8> = encode_hello("uid-alpha").unwrap();
    let disp: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut ka_a, t_late);
    let RelayDispatch::SendControl(pkts) = disp else {
        panic!("expected SendControl after stale eviction");
    };
    assert_eq!(engine.active_voice_peer_count(sock), 1);
    assert_eq!(
        decode_message(&pkts[1].payload).unwrap(),
        Dhv2Message::PeerMap {
            entries: vec![PeerMapEntry {
                peer_slot: 1,
                uid: "uid-alpha".to_owned(),
            }]
        }
    );

    // Peer A reconnects from a new UDP port with the SAME UID -> reuses slot 1 cleanly.
    let peer_a_new: PeerEndpoint = v4(1, 59_999);
    engine.record_authenticated_peer(sock, peer_a_new, t_late + 100);
    let mut reconn: Vec<u8> = encode_hello("uid-alpha").unwrap();
    let disp_reconn: RelayDispatch =
        engine.handle_dhv2_packet(sock, peer_a_new, &mut reconn, t_late + 100);
    let RelayDispatch::SendControl(pkts_reconn) = disp_reconn else {
        panic!("expected SendControl on reconnect");
    };
    assert_eq!(engine.active_voice_peer_count(sock), 1);
    assert_eq!(
        decode_message(&pkts_reconn[0].payload).unwrap(),
        Dhv2Message::HelloAck { peer_slot: 1 }
    );
    assert_eq!(pkts_reconn[0].target, peer_a_new);
}

#[test]
fn authorized_game_peer_table_caps_at_64_with_lru_eviction() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 4004;

    for idx in 0u16..70u16 {
        let ep: PeerEndpoint = v4(1, 30_000 + idx);
        engine.record_authenticated_peer(sock, ep, 1_000 + u64::from(idx) * 10);
    }

    let state = engine.sockets.get(&sock).unwrap();
    assert_eq!(state.authorized_game_peers.len(), MAX_AUTHORIZED_GAME_PEERS);

    let mut hello: Vec<u8> = encode_hello("probe").unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, v4(1, 30_000), &mut hello, 2_000),
        RelayDispatch::Drop
    );
    assert!(matches!(
        engine.handle_dhv2_packet(sock, v4(1, 30_069), &mut hello, 2_000),
        RelayDispatch::SendControl(_)
    ));
}

#[test]
fn socket_close_clears_only_target_socket_state() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock_a: usize = 5001;
    let sock_b: usize = 5002;
    let ep: PeerEndpoint = v4(1, 55_000);

    engine.record_authenticated_peer(sock_a, ep, 1_000);
    engine.record_authenticated_peer(sock_b, ep, 1_000);
    let mut h1: Vec<u8> = encode_hello("peer-1").unwrap();
    let mut h2: Vec<u8> = encode_hello("peer-2").unwrap();
    let _r1: RelayDispatch = engine.handle_dhv2_packet(sock_a, ep, &mut h1, 1_000);
    let _r2: RelayDispatch = engine.handle_dhv2_packet(sock_b, ep, &mut h2, 1_000);

    assert_eq!(engine.active_voice_peer_count(sock_a), 1);
    assert_eq!(engine.active_voice_peer_count(sock_b), 1);

    engine.on_socket_closed(sock_a);
    assert_eq!(engine.active_voice_peer_count(sock_a), 0);
    assert_eq!(engine.active_voice_peer_count(sock_b), 1);
}

#[test]
fn voice_frame_rate_limiter_enforces_65pps_and_burst_15() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6001;
    let peer_a: PeerEndpoint = v4(1, 56_001);
    let peer_b: PeerEndpoint = v4(1, 56_002);

    engine.record_authenticated_peer(sock, peer_a, 1_000);
    engine.record_authenticated_peer(sock, peer_b, 1_000);
    let mut ha: Vec<u8> = encode_hello("talker-a").unwrap();
    let mut hb: Vec<u8> = encode_hello("listener-b").unwrap();
    let _ra: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut ha, 1_000);
    let _rb: RelayDispatch = engine.handle_dhv2_packet(sock, peer_b, &mut hb, 1_000);

    // Instant burst of 15 frames at t=1_000 must pass; 16th at same t must drop.
    for seq in 0u16..u16::try_from(VOICE_BURST_PACKETS).unwrap() {
        let mut pkt: Vec<u8> =
            encode_voice_frame(1, 0, seq, u32::from(seq) * 960, &[1, 2, 3]).unwrap();
        assert!(matches!(
            engine.handle_dhv2_packet(sock, peer_a, &mut pkt, 1_000),
            RelayDispatch::RelayVoice { .. }
        ));
    }
    let mut burst_overflow: Vec<u8> = encode_voice_frame(1, 0, 99, 0, &[1, 2, 3]).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut burst_overflow, 1_000),
        RelayDispatch::Drop
    );

    // Over the next 1000ms, sending 100 frames (100 pps flood) must relay AT MOST 65 frames.
    let mut relayed_flood: usize = 0;
    for idx in 1u64..=100u64 {
        let ts: u64 = 1_000 + idx * 10;
        let mut pkt: Vec<u8> =
            encode_voice_frame(1, 0, u16::try_from(idx).unwrap(), 0, &[9, 8, 7]).unwrap();
        if matches!(
            engine.handle_dhv2_packet(sock, peer_a, &mut pkt, ts),
            RelayDispatch::RelayVoice { .. }
        ) {
            relayed_flood += 1;
        }
    }
    assert_eq!(relayed_flood, 65);

    // Normal 50 pps (every 20ms) after bucket recovers must pass 100% of frames.
    let mut relayed_normal: usize = 0;
    for idx in 0u64..50u64 {
        let ts: u64 = 4_000 + idx * 20;
        let mut pkt: Vec<u8> =
            encode_voice_frame(1, 0, u16::try_from(idx).unwrap(), 0, &[4, 5, 6]).unwrap();
        if matches!(
            engine.handle_dhv2_packet(sock, peer_a, &mut pkt, ts),
            RelayDispatch::RelayVoice { .. }
        ) {
            relayed_normal += 1;
        }
    }
    assert_eq!(relayed_normal, 50);
}

#[test]
fn hello_rate_limiter_enforces_1pps_burst_2_even_across_bye_cycling() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6002;
    let peer_a: PeerEndpoint = v4(1, 56_101);
    engine.record_authenticated_peer(sock, peer_a, 1_000);

    // First 2 Hellos (burst = 2) at t=1_000 succeed.
    for _idx in 0..HELLO_BURST_PACKETS {
        let mut hello: Vec<u8> = encode_hello("uid-hello-test").unwrap();
        assert!(matches!(
            engine.handle_dhv2_packet(sock, peer_a, &mut hello, 1_000),
            RelayDispatch::SendControl(_)
        ));
    }

    // 3rd Hello at t=1_000 must be dropped.
    let mut hello_3: Vec<u8> = encode_hello("uid-hello-test").unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut hello_3, 1_000),
        RelayDispatch::Drop
    );

    // Sending Bye and immediately sending Hello at t=1_200 must STILL be rate-limited!
    let mut bye: Vec<u8> = encode_bye(1);
    let _bye_res: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut bye, 1_200);
    let mut hello_cycle: Vec<u8> = encode_hello("uid-hello-test").unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut hello_cycle, 1_200),
        RelayDispatch::Drop
    );

    // After 1 full second (t=2_000), 1 token has refilled -> Hello succeeds again.
    let mut hello_ok: Vec<u8> = encode_hello("uid-hello-test").unwrap();
    assert!(matches!(
        engine.handle_dhv2_packet(sock, peer_a, &mut hello_ok, 2_000),
        RelayDispatch::SendControl(_)
    ));
}

#[test]
fn global_voice_relay_cap_enforces_3000_outbound_pps() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6003;

    // Register 16 peers (max fanout = 15 recipients per VoiceFrame).
    let mut peers: Vec<PeerEndpoint> = Vec::with_capacity(MAX_VOICE_PEERS);
    for idx in 0..MAX_VOICE_PEERS {
        let ep: PeerEndpoint = v4(1, 57_000 + u16::try_from(idx).unwrap());
        engine.record_authenticated_peer(sock, ep, 1_000);
        let mut h: Vec<u8> = encode_hello(&format!("player-{idx}")).unwrap();
        let _res: RelayDispatch = engine.handle_dhv2_packet(sock, ep, &mut h, 1_000);
        peers.push(ep);
    }
    assert_eq!(engine.active_voice_peer_count(sock), MAX_VOICE_PEERS);

    // Global burst is 300 outbound packets => 300 / 15 = 20 frames across different peers at t=1_000 pass,
    // and the 21st frame at t=1_000 MUST be dropped by the global cap even though per-peer buckets have tokens!
    let frames_in_burst: usize = usize::try_from(GLOBAL_RELAY_BURST_PACKETS).unwrap() / 15;
    for idx in 0..frames_in_burst {
        let sender: PeerEndpoint = peers[idx % MAX_VOICE_PEERS];
        let mut pkt: Vec<u8> =
            encode_voice_frame(1, 0, u16::try_from(idx).unwrap(), 0, &[0x11; 32]).unwrap();
        let disp: RelayDispatch = engine.handle_dhv2_packet(sock, sender, &mut pkt, 1_000);
        let RelayDispatch::RelayVoice { recipients, .. } = disp else {
            panic!("expected global burst frame {idx} to pass");
        };
        assert_eq!(recipients.len(), 15);
    }

    let mut overflow_pkt: Vec<u8> = encode_voice_frame(1, 0, 999, 0, &[0x11; 32]).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peers[15], &mut overflow_pkt, 1_000),
        RelayDispatch::Drop
    );

    // After 10ms at 3000 pps, 30 outbound tokens refill -> 2 more frames (30 outbound packets) pass, 3rd drops.
    for (idx, peer_ep) in peers.iter().copied().enumerate().take(2usize) {
        let mut pkt: Vec<u8> =
            encode_voice_frame(1, 0, u16::try_from(500 + idx).unwrap(), 0, &[0x22; 16]).unwrap();
        assert!(matches!(
            engine.handle_dhv2_packet(sock, peer_ep, &mut pkt, 1_010),
            RelayDispatch::RelayVoice { .. }
        ));
    }
    let mut pkt_drop: Vec<u8> = encode_voice_frame(1, 0, 502, 0, &[0x22; 16]).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peers[2], &mut pkt_drop, 1_010),
        RelayDispatch::Drop
    );
}

#[test]
fn oversized_voice_payload_above_256_bytes_is_dropped() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6004;
    let peer_a: PeerEndpoint = v4(1, 58_001);
    let peer_b: PeerEndpoint = v4(1, 58_002);

    engine.record_authenticated_peer(sock, peer_a, 1_000);
    engine.record_authenticated_peer(sock, peer_b, 1_000);
    let mut ha: Vec<u8> = encode_hello("uid-a").unwrap();
    let mut hb: Vec<u8> = encode_hello("uid-b").unwrap();
    let _ra: RelayDispatch = engine.handle_dhv2_packet(sock, peer_a, &mut ha, 1_000);
    let _rb: RelayDispatch = engine.handle_dhv2_packet(sock, peer_b, &mut hb, 1_000);

    // Exact 256-byte Opus payload passes.
    let ok_payload: Vec<u8> = vec![0x5A; MAX_SERVER_VOICE_PAYLOAD];
    let mut ok_pkt: Vec<u8> = encode_voice_frame(1, 0, 1, 960, &ok_payload).unwrap();
    assert!(matches!(
        engine.handle_dhv2_packet(sock, peer_a, &mut ok_pkt, 1_100),
        RelayDispatch::RelayVoice { .. }
    ));

    // 257-byte and 512-byte Opus payloads are silently dropped by server relay.
    let too_big_257: Vec<u8> = vec![0x5A; MAX_SERVER_VOICE_PAYLOAD + 1];
    let mut pkt_257: Vec<u8> = encode_voice_frame(1, 0, 2, 1920, &too_big_257).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut pkt_257, 1_120),
        RelayDispatch::Drop
    );

    let too_big_512: Vec<u8> = vec![0x5A; 512];
    let mut pkt_512: Vec<u8> = encode_voice_frame(1, 0, 3, 2880, &too_big_512).unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, peer_a, &mut pkt_512, 1_140),
        RelayDispatch::Drop
    );
}

#[test]
fn voice_peers_capped_at_16_slots() {
    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6005;

    for idx in 1u16..=u16::try_from(MAX_VOICE_PEERS).unwrap() {
        let ep: PeerEndpoint = v4(1, 58_100 + idx);
        engine.record_authenticated_peer(sock, ep, 1_000);
        let mut h: Vec<u8> = encode_hello(&format!("slot-{idx}")).unwrap();
        let disp: RelayDispatch = engine.handle_dhv2_packet(sock, ep, &mut h, 1_000);
        let RelayDispatch::SendControl(pkts) = disp else {
            panic!("expected slot {idx} to register");
        };
        assert_eq!(
            decode_message(&pkts[0].payload).unwrap(),
            Dhv2Message::HelloAck { peer_slot: idx }
        );
    }
    assert_eq!(engine.active_voice_peer_count(sock), MAX_VOICE_PEERS);

    // 17th peer must be silently dropped while all 16 slots are active.
    let extra: PeerEndpoint = v4(1, 58_200);
    engine.record_authenticated_peer(sock, extra, 1_000);
    let mut h_extra: Vec<u8> = encode_hello("slot-17").unwrap();
    assert_eq!(
        engine.handle_dhv2_packet(sock, extra, &mut h_extra, 1_000),
        RelayDispatch::Drop
    );
    assert_eq!(engine.active_voice_peer_count(sock), MAX_VOICE_PEERS);
}

#[test]
fn strict_hello_uid_validation_allows_only_printable_ascii_up_to_96_bytes() {
    assert!(is_valid_server_uid(
        "76561198000000001|0002abcdef1234567890"
    ));
    assert!(is_valid_server_uid(&"A".repeat(MAX_SERVER_UID_LEN)));

    assert!(!is_valid_server_uid(""));
    assert!(!is_valid_server_uid(&"A".repeat(MAX_SERVER_UID_LEN + 1)));
    assert!(!is_valid_server_uid("has space"));
    assert!(!is_valid_server_uid("has\nnewline"));
    assert!(!is_valid_server_uid("has\x7fdel"));
    assert!(!is_valid_server_uid("非ASCII文字"));

    let mut engine: RelayEngine = RelayEngine::new();
    let sock: usize = 6006;
    let ep: PeerEndpoint = v4(1, 58_300);
    engine.record_authenticated_peer(sock, ep, 1_000);

    for bad_uid in ["bad uid", "bad\tuid", "日本語UID", &"Z".repeat(97)] {
        let mut pkt: Vec<u8> = encode_hello(bad_uid).unwrap();
        assert_eq!(
            engine.handle_dhv2_packet(sock, ep, &mut pkt, 1_000),
            RelayDispatch::Drop
        );
    }
    assert_eq!(engine.active_voice_peer_count(sock), 0);

    // Valid printable ASCII UID still succeeds cleanly (invalid Hellos did not burn rate tokens).
    let mut good_pkt: Vec<u8> = encode_hello("76561198000000001|0002valid").unwrap();
    assert!(matches!(
        engine.handle_dhv2_packet(sock, ep, &mut good_pkt, 1_000),
        RelayDispatch::SendControl(_)
    ));
    assert_eq!(engine.active_voice_peer_count(sock), 1);
}
