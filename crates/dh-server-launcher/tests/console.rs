use std::sync::{Arc, Mutex};

use dh_server_launcher::console::bridge::{build_console_broadcast_message, encode_utf16le_nul};
use dh_server_launcher::console::dispatch::{
    format_console_command_preview, handle_add_item, handle_broadcast, handle_console_command,
    handle_kick_player, handle_send_thrall_msg, handle_set_human, handle_set_storage,
    render_console_log_record, status_code,
};
use dh_server_launcher::structs::console::{
    ConsoleBridge, ConsoleDispatchContext, ConsoleLogRecord, ConsoleLogSink,
};
use dh_server_launcher::structs::languages::{ApiCode, ConsoleLogEvent, LogCategoryCode};
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

async fn spawn_packet_collector(
    expected_count: usize,
) -> Result<(u16, tokio::task::JoinHandle<Result<Vec<Value>, String>>), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let task = tokio::spawn(async move {
        let mut packets = Vec::with_capacity(expected_count);
        for _ in 0..expected_count {
            let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut raw = Vec::new();
            stream
                .read_to_end(&mut raw)
                .await
                .map_err(|e| e.to_string())?;
            if raw.len() < 2 || !raw.len().is_multiple_of(2) {
                return Err(format!("invalid UTF-16LE byte length: {}", raw.len()));
            }
            let units: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            if units.last().copied() != Some(0) {
                return Err("missing UTF-16LE null terminator".to_string());
            }
            let text = String::from_utf16(&units[..units.len() - 1]).map_err(|e| e.to_string())?;
            let parsed: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            packets.push(parsed);
        }
        Ok(packets)
    });

    Ok((port, task))
}

#[test]
fn encodes_utf16le_with_nul_and_builds_broadcast_notices() {
    let encoded = encode_utf16le_nul("A船");
    assert_eq!(encoded, vec![0x41, 0x00, 0x39, 0x82, 0x00, 0x00]);

    let summon_msg =
        build_console_broadcast_message("Captain", "summon", "BP_Predator_Bear_C").unwrap();
    assert!(summon_msg.contains("船長"));
    assert!(summon_msg.contains("熊"));

    let unset_thrall = build_console_broadcast_message("Doctor", "setisthrall", "0").unwrap();
    assert!(unset_thrall.contains("医者"));
    assert!(unset_thrall.contains("船員に戻りました"));

    assert_eq!(
        format_console_command_preview("EndMatch 0"),
        "endmatch 傀儡勝利"
    );
    assert_eq!(
        format_console_command_preview("EndMatch 1"),
        "endmatch 探検家勝利"
    );
    assert_eq!(
        format_console_command_preview("SUMMON BP_Predator_Wolf_C"),
        "summon 狼"
    );
}

#[test]
fn renders_all_console_log_events_without_ansi_escape_codes() {
    let events = [
        ConsoleLogEvent::InvalidParams {
            category: LogCategoryCode::Command,
        },
        ConsoleLogEvent::CommandResult {
            role_key: "Captain",
            command: "setisthrall 1",
            added_bone_dagger: true,
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::StatusInvalidValue {
            role_key: "Hunter",
            prop_name: "CurrentHealth",
            raw_value: "-5",
        },
        ConsoleLogEvent::StatusResult {
            role_key: "Hunter",
            prop_name: "CurrentHealth",
            value_text: "250",
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::ItemInvalidCount {
            role_key: "Cook",
            item_key: "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
            raw_count: "0",
        },
        ConsoleLogEvent::ItemResult {
            role_key: "Cook",
            item_key: "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
            count: 5,
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::StorageInvalidValue {
            role_key: "Engineer",
            raw_limit: "-1",
        },
        ConsoleLogEvent::StorageResult {
            role_key: "Engineer",
            limit: 20,
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::NoticeResult {
            role_key: "Navigator",
            message: "テスト通知",
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::BroadcastResult {
            message: "全体メッセージ",
            status: status_code::SUCCESS,
            err_detail: None,
        },
        ConsoleLogEvent::KickResult {
            role_key: "Chaplain",
            status: status_code::SUCCESS,
            err_detail: None,
        },
    ];

    for ev in events {
        let rec = render_console_log_record(&ev);
        assert!(!rec.category.contains('\x1b'));
        assert!(!rec.detail.contains('\x1b'));
        assert_ne!(rec.time, "");
    }
}

#[tokio::test]
async fn dispatches_setisthrall_with_bone_dagger_and_auto_broadcast_over_loopback() {
    // 1 execute_console_command + 1 bone dagger add_inventory_item + 7 broadcast_message_except
    let (port, collector) = spawn_packet_collector(9).await.expect("spawn collector");
    let bridge = ConsoleBridge::new(port);
    let logs: Arc<Mutex<Vec<ConsoleLogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let logs_clone = Arc::clone(&logs);
    let sink: ConsoleLogSink = Arc::new(move |rec| {
        logs_clone.lock().unwrap().push(rec);
    });

    let ctx = ConsoleDispatchContext {
        bridge: &bridge,
        auto_broadcast: true,
        log_sink: &sink,
    };
    let res = handle_console_command(&ctx, "Captain", "SetIsThrall 1").await;
    assert_eq!(res.status, status_code::SUCCESS);
    assert_eq!(res.code, ApiCode::Success);

    let packets = collector
        .await
        .expect("collector join")
        .expect("collect packets");
    assert_eq!(packets.len(), 9);
    assert_eq!(packets[0]["action"], "execute_console_command");
    assert_eq!(packets[0]["command"], "setisthrall 1");
    assert_eq!(packets[1]["action"], "add_inventory_item");
    assert_eq!(
        packets[1]["itemBlueprintPath"],
        "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C"
    );
    assert_eq!(packets[1]["itemCount"], 1);
    for pkt in &packets[2..] {
        assert_eq!(pkt["action"], "send_thrall_message");
        assert_ne!(pkt["targetRole"], "Captain");
    }

    let captured = logs.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].detail.contains("骨の短剣"));
}

#[tokio::test]
async fn dispatches_status_item_storage_notice_broadcast_and_kick_actions() {
    // 1 set_human + 1 add_item + 1 set_storage + 1 notice + 8 broadcast + 1 kick = 13 packets
    let (port, collector) = spawn_packet_collector(13).await.expect("spawn collector");
    let bridge = ConsoleBridge::new(port);
    let logs: Arc<Mutex<Vec<ConsoleLogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let logs_clone = Arc::clone(&logs);
    let sink: ConsoleLogSink = Arc::new(move |rec| {
        logs_clone.lock().unwrap().push(rec);
    });

    let ctx = ConsoleDispatchContext {
        bridge: &bridge,
        auto_broadcast: false,
        log_sink: &sink,
    };

    // High stat value must succeed with HighValueWarning
    let r_stat = handle_set_human(&ctx, "Doctor", "MaximumHealth", "50000").await;
    assert_eq!(r_stat.status, status_code::SUCCESS);
    assert_eq!(r_stat.code, ApiCode::HighValueWarning);

    // Invalid negative stat value is rejected locally without touching the TCP listener
    let r_bad_stat = handle_set_human(&ctx, "Doctor", "MaximumHealth", "-10").await;
    assert_eq!(r_bad_stat.status, status_code::INVALID_VALUE);
    assert_eq!(r_bad_stat.code, ApiCode::InvalidValueError);

    let r_item = handle_add_item(
        &ctx,
        "Cook",
        "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
        "3",
    )
    .await;
    assert_eq!(r_item.code, ApiCode::Success);

    let r_storage = handle_set_storage(&ctx, "Engineer", "20").await;
    assert_eq!(r_storage.code, ApiCode::Success);

    let r_msg = handle_send_thrall_msg(&ctx, "Navigator", "集合してください").await;
    assert_eq!(r_msg.code, ApiCode::Success);

    let r_bcast = handle_broadcast(&ctx, "全体連絡です").await;
    assert_eq!(r_bcast.code, ApiCode::Success);

    let r_kick = handle_kick_player(&ctx, "Marine").await;
    assert_eq!(r_kick.code, ApiCode::Success);

    let packets = collector
        .await
        .expect("collector join")
        .expect("collect all 13 packets");
    assert_eq!(packets.len(), 13);
    assert_eq!(packets[0]["action"], "set_human_property");
    assert_eq!(packets[0]["propertyValue"], 50000);
    assert_eq!(packets[1]["action"], "add_inventory_item");
    assert_eq!(packets[2]["action"], "set_storage_limit");
    assert_eq!(packets[3]["action"], "send_thrall_message");
    assert_eq!(packets[3]["targetRole"], "Navigator");
    assert_eq!(packets[12]["action"], "kick_player");
    assert_eq!(packets[12]["targetRole"], "Marine");

    // 7 actions executed -> 7 ANSI-free log entries recorded
    assert_eq!(logs.lock().unwrap().len(), 7);
}

#[tokio::test]
async fn normalizes_multiline_chat_and_strips_embedded_nul_without_breaking_framing() {
    // 1 single-target message + 8 broadcast packets = 9 packets total
    let (port, collector) = spawn_packet_collector(9).await.expect("spawn collector");
    let bridge = ConsoleBridge::new(port);
    let logs: Arc<Mutex<Vec<ConsoleLogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let logs_clone = Arc::clone(&logs);
    let sink: ConsoleLogSink = Arc::new(move |rec| {
        logs_clone.lock().unwrap().push(rec);
    });

    let ctx = ConsoleDispatchContext {
        bridge: &bridge,
        auto_broadcast: false,
        log_sink: &sink,
    };

    let r_msg = handle_send_thrall_msg(&ctx, "Captain", "1行目\r\n2行目\0\r3行目").await;
    assert_eq!(r_msg.code, ApiCode::Success);

    let r_bcast = handle_broadcast(&ctx, "告知A\r\n告知B\0末尾").await;
    assert_eq!(r_bcast.code, ApiCode::Success);

    let packets = collector
        .await
        .expect("collector join")
        .expect("collect all 9 packets");
    assert_eq!(packets.len(), 9);
    assert_eq!(packets[0]["action"], "send_thrall_message");
    assert_eq!(packets[0]["targetRole"], "Captain");
    assert_eq!(packets[0]["message"], "1行目\n2行目\n3行目");
    for pkt in &packets[1..9] {
        assert_eq!(pkt["action"], "send_thrall_message");
        assert_eq!(pkt["message"], "告知A\n告知B末尾");
    }

    let recorded = logs.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(!recorded[0].detail.contains('\n'));
    assert!(!recorded[1].detail.contains('\n'));
}
