use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::messages::ja::web_console::{
    actor_display_name, broadcasts, player_role_name, PLAYER_NAME_LIST,
};

pub const DEFAULT_TCP_TIMEOUT_MS: u64 = 1500;
pub const BONE_DAGGER_ITEM: &str =
    "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C";

const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

#[must_use]
pub fn role_display(name: &str) -> &str {
    player_role_name(name).unwrap_or(name)
}

#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ConsoleBridgePayload<'a> {
    ExecuteConsoleCommand {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        command: &'a str,
    },
    SetHumanProperty {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "propertyName")]
        property_name: &'a str,
        #[serde(rename = "propertyValue")]
        property_value: serde_json::Number,
    },
    AddInventoryItem {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "itemBlueprintPath")]
        item_blueprint_path: &'a str,
        #[serde(rename = "itemCount")]
        item_count: i64,
    },
    SetStorageLimit {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "storageLimit")]
        storage_limit: i64,
    },
    SendThrallMessage {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        message: &'a str,
    },
    KickPlayer {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
    },
}

fn f64_to_json_number(value: f64) -> Option<serde_json::Number> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INT_F64 {
        let int_text: String = format!("{value:.0}");
        if let Ok(int_val) = int_text.parse::<i64>() {
            return Some(serde_json::Number::from(int_val));
        }
    }
    serde_json::Number::from_f64(value)
}

/// Encodes `text` as UTF-16LE followed by a `0x0000` null terminator.
#[must_use]
pub fn encode_utf16le_nul(text: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::with_capacity((text.len() + 1) * 2);
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

/// Sends a UTF-16LE null-terminated JSON string to `default/コンソール連携.js` over TCP.
pub async fn send_utf16_tcp_packet(port: u16, text: &str, timeout: Duration) -> Result<(), String> {
    let bytes: Vec<u8> = encode_utf16le_nul(text);
    let send_result: Result<Result<(), String>, tokio::time::error::Elapsed> =
        tokio::time::timeout(timeout, async {
            let mut stream: TcpStream = TcpStream::connect(("127.0.0.1", port))
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            stream
                .write_all(&bytes)
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            stream
                .flush()
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            stream
                .shutdown()
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            Ok::<(), String>(())
        })
        .await;

    match send_result {
        Ok(result) => result,
        Err(_) => Err("TCP connection timeout".to_string()),
    }
}

#[must_use]
pub fn build_console_broadcast_message(
    player_name: &str,
    cmd_name: &str,
    cmd_arg: &str,
) -> Option<String> {
    let role: &str = role_display(player_name);
    if cmd_name == "summon" {
        let target: &str = actor_display_name(cmd_arg).unwrap_or(cmd_arg);
        return Some(broadcasts::summon(role, target));
    }
    if cmd_name.eq_ignore_ascii_case("setisthrall") {
        return Some(if cmd_arg.trim() == "0" {
            broadcasts::unset_thrall(role)
        } else {
            broadcasts::set_thrall(role)
        });
    }
    if cmd_name == "setspellcharge" {
        return Some(broadcasts::spell_charge(role));
    }
    if cmd_name == "SetBlizzardIntensity" {
        return Some(if cmd_arg == "0" || cmd_arg == "0.00" {
            broadcasts::stop_blizzard(role)
        } else {
            broadcasts::start_blizzard(role)
        });
    }
    if cmd_name == "slomo" {
        return Some(if cmd_arg == "1" {
            broadcasts::reset_time_scale(role)
        } else {
            broadcasts::accelerate_time_scale(role)
        });
    }
    if cmd_name == "god" {
        return Some(if cmd_arg == "0" {
            broadcasts::disable_god_mode(role)
        } else {
            broadcasts::enable_god_mode(role)
        });
    }
    None
}

#[derive(Debug, Clone)]
pub struct ConsoleBridge {
    port: u16,
    timeout: Duration,
}

impl ConsoleBridge {
    #[must_use]
    pub fn new(console_patch_port: u16) -> Self {
        Self {
            port: console_patch_port,
            timeout: Duration::from_millis(DEFAULT_TCP_TIMEOUT_MS),
        }
    }

    #[must_use]
    pub fn with_timeout(console_patch_port: u16, timeout: Duration) -> Self {
        Self {
            port: console_patch_port,
            timeout,
        }
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    async fn send_payload(&self, payload: &ConsoleBridgePayload<'_>) -> Result<(), String> {
        let json: String =
            serde_json::to_string(payload).map_err(|e: serde_json::Error| e.to_string())?;
        send_utf16_tcp_packet(self.port, &json, self.timeout).await
    }

    pub async fn execute_console_command(
        &self,
        target_role: &str,
        command: &str,
    ) -> Result<(), String> {
        self.send_payload(&ConsoleBridgePayload::ExecuteConsoleCommand {
            target_role,
            command,
        })
        .await
    }

    pub async fn set_human_property(
        &self,
        target_role: &str,
        property_name: &str,
        property_value: f64,
    ) -> Result<(), String> {
        let number: serde_json::Number = f64_to_json_number(property_value)
            .ok_or_else(|| "Invalid property value".to_string())?;
        self.send_payload(&ConsoleBridgePayload::SetHumanProperty {
            target_role,
            property_name,
            property_value: number,
        })
        .await
    }

    pub async fn add_inventory_item(
        &self,
        target_role: &str,
        item_blueprint_path: &str,
        item_count: i64,
    ) -> Result<(), String> {
        self.send_payload(&ConsoleBridgePayload::AddInventoryItem {
            target_role,
            item_blueprint_path,
            item_count,
        })
        .await
    }

    pub async fn set_player_storage_limit(
        &self,
        target_role: &str,
        storage_limit: i64,
    ) -> Result<(), String> {
        self.send_payload(&ConsoleBridgePayload::SetStorageLimit {
            target_role,
            storage_limit,
        })
        .await
    }

    pub async fn send_player_message(
        &self,
        target_role: &str,
        message: &str,
    ) -> Result<(), String> {
        self.send_payload(&ConsoleBridgePayload::SendThrallMessage {
            target_role,
            message,
        })
        .await
    }

    pub async fn kick_target_player(&self, target_role: &str) -> Result<(), String> {
        self.send_payload(&ConsoleBridgePayload::KickPlayer { target_role })
            .await
    }

    pub async fn broadcast_message_to_all(&self, message: &str) -> Result<(), String> {
        for role in PLAYER_NAME_LIST {
            self.send_player_message(role, message).await?;
        }
        Ok(())
    }

    pub async fn broadcast_message_except(
        &self,
        excluded_role: &str,
        message: &str,
    ) -> Result<(), String> {
        for role in PLAYER_NAME_LIST {
            if role == excluded_role {
                continue;
            }
            self.send_player_message(role, message).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    #[test]
    fn encodes_utf16le_with_nul_terminator() {
        let encoded = encode_utf16le_nul("A船");
        // 'A' = 0x0041, '船' = 0x8239, '\0' = 0x0000
        assert_eq!(encoded, vec![0x41, 0x00, 0x39, 0x82, 0x00, 0x00]);
    }

    #[test]
    fn builds_console_broadcast_messages() {
        assert_eq!(
            build_console_broadcast_message("Captain", "summon", "BP_Predator_Wolf_C").as_deref(),
            Some("船長 が狼を召喚しました")
        );
        assert_eq!(
            build_console_broadcast_message("Cook", "SetIsThrall", "1").as_deref(),
            Some("料理人 が闇の傀儡に悪墜ちしました")
        );
        assert_eq!(
            build_console_broadcast_message("Cook", "setisthrall", "0").as_deref(),
            Some("料理人 が船員に戻りました")
        );
        assert_eq!(
            build_console_broadcast_message("Doctor", "SetBlizzardIntensity", "0.00").as_deref(),
            Some("医者 が暴風雪を停止しました")
        );
        assert_eq!(
            build_console_broadcast_message("Doctor", "SetBlizzardIntensity", "1").as_deref(),
            Some("医者 が暴風雪を呼びました")
        );
        assert_eq!(
            build_console_broadcast_message("Marine", "slomo", "1").as_deref(),
            Some("海兵 が世界を元に戻しました")
        );
        assert_eq!(
            build_console_broadcast_message("Marine", "slomo", "3").as_deref(),
            Some("海兵 が世界を加速しました")
        );
        assert_eq!(
            build_console_broadcast_message("Hunter", "god", "0").as_deref(),
            Some("猟師 が無敵状態を解除しました")
        );
        assert_eq!(
            build_console_broadcast_message("Hunter", "god", "1").as_deref(),
            Some("猟師 が無敵状態になりました")
        );
        assert_eq!(
            build_console_broadcast_message("Hunter", "unknowncmd", "1"),
            None
        );
    }

    #[tokio::test]
    async fn sends_utf16le_json_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            socket.read_to_end(&mut buf).await.unwrap();
            buf
        });

        let bridge = ConsoleBridge::new(port);
        bridge
            .send_player_message("Captain", "テスト通知")
            .await
            .unwrap();

        let raw = server_task.await.unwrap();
        assert!(raw.len() >= 2);
        assert_eq!(&raw[raw.len() - 2..], &[0x00, 0x00]);
        let units: Vec<u16> = raw[..raw.len() - 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        let text = String::from_utf16(&units).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["action"], "send_thrall_message");
        assert_eq!(val["targetRole"], "Captain");
        assert_eq!(val["message"], "テスト通知");
    }
}
