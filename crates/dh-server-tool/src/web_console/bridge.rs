use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::consts::web_console::MAX_SAFE_INT_F64;
pub use crate::consts::web_console::{BONE_DAGGER_ITEM, DEFAULT_TCP_TIMEOUT_MS, PLAYER_NAME_LIST};
use crate::languages;
use crate::structs::languages::BroadcastEvent;
pub use crate::structs::web_console::ConsoleBridge;
use crate::structs::web_console::ConsoleBridgePayload;

#[must_use]
pub fn role_display(name: &str) -> &str {
    languages::player_role_name(name).unwrap_or(name)
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
pub fn build_console_broadcast_event<'a>(
    player_name: &'a str,
    cmd_name: &str,
    cmd_arg: &'a str,
) -> Option<BroadcastEvent<'a>> {
    let normalized_cmd: String = cmd_name.trim().to_ascii_lowercase();
    let trimmed_arg: &'a str = cmd_arg.trim();
    match normalized_cmd.as_str() {
        "summon" => Some(BroadcastEvent::Summon {
            role_key: player_name,
            target_key: trimmed_arg,
        }),
        "setisthrall" => Some(if trimmed_arg == "0" {
            BroadcastEvent::UnsetThrall {
                role_key: player_name,
            }
        } else {
            BroadcastEvent::SetThrall {
                role_key: player_name,
            }
        }),
        "setspellcharge" => Some(BroadcastEvent::SpellCharge {
            role_key: player_name,
        }),
        "setblizzardintensity" => Some(if trimmed_arg == "0" || trimmed_arg == "0.00" {
            BroadcastEvent::StopBlizzard {
                role_key: player_name,
            }
        } else {
            BroadcastEvent::StartBlizzard {
                role_key: player_name,
            }
        }),
        "slomo" => Some(if trimmed_arg == "1" {
            BroadcastEvent::ResetTimeScale {
                role_key: player_name,
            }
        } else {
            BroadcastEvent::AccelerateTimeScale {
                role_key: player_name,
            }
        }),
        "god" => Some(if trimmed_arg == "0" {
            BroadcastEvent::DisableGodMode {
                role_key: player_name,
            }
        } else {
            BroadcastEvent::EnableGodMode {
                role_key: player_name,
            }
        }),
        _ => None,
    }
}

#[must_use]
pub fn build_console_broadcast_message(
    player_name: &str,
    cmd_name: &str,
    cmd_arg: &str,
) -> Option<String> {
    build_console_broadcast_event(player_name, cmd_name, cmd_arg)
        .map(|event: BroadcastEvent<'_>| languages::render_broadcast(&event))
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
