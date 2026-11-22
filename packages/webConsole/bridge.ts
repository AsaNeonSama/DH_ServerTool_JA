import {
  ACTOR_COMMENT_DICT,
  DEFAULT_TCP_TIMEOUT_MS,
  PLAYER_NAME_LIST,
} from "../constants";
import { WEB_CONSOLE_MESSAGES } from "../messages";
import type {
  ConsoleBridgePayload,
  TcpPacketOptions,
  TcpSocketWriter,
} from "../types";
import { roleDisplay } from "./httpUtils";

export interface ConsoleBridge {
  readonly executeConsoleCommand: (
    targetRole: string,
    command: string
  ) => Promise<Error | null>;
  readonly setHumanProperty: (
    targetRole: string,
    propertyName: string,
    propertyValue: number
  ) => Promise<Error | null>;
  readonly addInventoryItem: (
    targetRole: string,
    itemBlueprintPath: string,
    itemCount: number
  ) => Promise<Error | null>;
  readonly setPlayerStorageLimit: (
    targetRole: string,
    storageLimit: number
  ) => Promise<Error | null>;
  readonly sendPlayerMessage: (
    targetRole: string,
    message: string
  ) => Promise<Error | null>;
  readonly kickTargetPlayer: (targetRole: string) => Promise<Error | null>;
  readonly broadcastMessageToAll: (message: string) => Promise<Error | null>;
  readonly broadcastMessageExcept: (
    excludedRole: string,
    message: string
  ) => Promise<Error | null>;
}

/**
 * Sends a UTF-16LE null-terminated JSON string to console_win64.js over TCP.
 */
export async function sendUtf16TcpPacket(
  options: TcpPacketOptions
): Promise<Error | null> {
  const {
    port,
    text,
    timeoutMs = DEFAULT_TCP_TIMEOUT_MS,
  }: TcpPacketOptions = options;
  const bytes: Buffer = Buffer.from(`${text}\0`, "utf16le");
  const result: Promise<Error | null> = new Promise(
    (resolve: (value: Error | null) => void): void => {
      let settled: boolean = false;
      let offset: number = 0;
      let activeSocket: TcpSocketWriter | null = null;

      const finish = (err: Error | null): void => {
        if (settled) return;
        settled = true;
        activeSocket = null;
        clearTimeout(timer);
        resolve(err);
      };

      const writeRemaining = (socket: TcpSocketWriter): void => {
        try {
          while (offset < bytes.byteLength) {
            const written: number = socket.write(bytes.subarray(offset));
            if (written <= 0) {
              return;
            }
            offset += written;
          }
          socket.flush();
          socket.end();
          finish(null);
        } catch (err: unknown) {
          try {
            socket.terminate();
          } catch {
            // Ignore socket terminate error
          }
          finish(err instanceof Error ? err : new Error(String(err)));
        }
      };

      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        try {
          activeSocket?.terminate();
        } catch {
          // Ignore timeout terminate error
        }
        finish(new Error("TCP connection timeout"));
      }, timeoutMs);

      Bun.connect({
        hostname: "127.0.0.1",
        port,
        socket: {
          open(socket: TcpSocketWriter): void {
            if (settled) {
              try {
                socket.terminate();
              } catch {
                // Ignore late open terminate error
              }
              return;
            }
            activeSocket = socket;
            writeRemaining(socket);
          },
          drain(socket: TcpSocketWriter): void {
            if (settled) {
              try {
                socket.terminate();
              } catch {
                // Ignore late drain terminate error
              }
              return;
            }
            writeRemaining(socket);
          },
          data(): void {
            // console_win64.js does not send response data
          },
          close(): void {
            finish(
              offset >= bytes.byteLength
                ? null
                : new Error("Connection closed early")
            );
          },
          error(_socket: unknown, err: Error): void {
            finish(err);
          },
          connectError(_socket: unknown, err: Error): void {
            finish(err);
          },
        },
      }).catch((err: unknown): void => {
        finish(err instanceof Error ? err : new Error(String(err)));
      });
    }
  );
  return result;
}

export function buildConsoleBroadcastMessage(
  playerName: string,
  cmdName: string,
  cmdArg: string
): string | null {
  const role: string = roleDisplay(playerName);
  if (cmdName === "summon") {
    const target: string = ACTOR_COMMENT_DICT[cmdArg] ?? cmdArg;
    return WEB_CONSOLE_MESSAGES.broadcasts.summon(role, target);
  }
  if (cmdName.toLowerCase() === "setisthrall") {
    return cmdArg.trim() === "0"
      ? WEB_CONSOLE_MESSAGES.broadcasts.unsetThrall(role)
      : WEB_CONSOLE_MESSAGES.broadcasts.setThrall(role);
  }
  if (cmdName === "setspellcharge") {
    return WEB_CONSOLE_MESSAGES.broadcasts.spellCharge(role);
  }
  if (cmdName === "SetBlizzardIntensity") {
    return cmdArg === "0" || cmdArg === "0.00"
      ? WEB_CONSOLE_MESSAGES.broadcasts.stopBlizzard(role)
      : WEB_CONSOLE_MESSAGES.broadcasts.startBlizzard(role);
  }
  if (cmdName === "slomo") {
    return cmdArg === "1"
      ? WEB_CONSOLE_MESSAGES.broadcasts.resetTimeScale(role)
      : WEB_CONSOLE_MESSAGES.broadcasts.accelerateTimeScale(role);
  }
  if (cmdName === "god") {
    return cmdArg === "0"
      ? WEB_CONSOLE_MESSAGES.broadcasts.disableGodMode(role)
      : WEB_CONSOLE_MESSAGES.broadcasts.enableGodMode(role);
  }
  return null;
}

export function createConsoleBridge(consolePatchPort: number): ConsoleBridge {
  const sendBridgePayload = (
    payload: ConsoleBridgePayload
  ): Promise<Error | null> =>
    sendUtf16TcpPacket({
      port: consolePatchPort,
      text: JSON.stringify(payload),
    });

  const sendPlayerMessage = (
    targetRole: string,
    message: string
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "send_thrall_message",
      targetRole,
      message,
    });

  return {
    executeConsoleCommand: (
      targetRole: string,
      command: string
    ): Promise<Error | null> =>
      sendBridgePayload({
        action: "execute_console_command",
        targetRole,
        command,
      }),
    setHumanProperty: (
      targetRole: string,
      propertyName: string,
      propertyValue: number
    ): Promise<Error | null> =>
      sendBridgePayload({
        action: "set_human_property",
        targetRole,
        propertyName,
        propertyValue,
      }),
    addInventoryItem: (
      targetRole: string,
      itemBlueprintPath: string,
      itemCount: number
    ): Promise<Error | null> =>
      sendBridgePayload({
        action: "add_inventory_item",
        targetRole,
        itemBlueprintPath,
        itemCount,
      }),
    setPlayerStorageLimit: (
      targetRole: string,
      storageLimit: number
    ): Promise<Error | null> =>
      sendBridgePayload({
        action: "set_storage_limit",
        targetRole,
        storageLimit,
      }),
    sendPlayerMessage,
    kickTargetPlayer: (targetRole: string): Promise<Error | null> =>
      sendBridgePayload({
        action: "kick_player",
        targetRole,
      }),
    broadcastMessageToAll: async (message: string): Promise<Error | null> => {
      for (const role of PLAYER_NAME_LIST) {
        const err: Error | null = await sendPlayerMessage(role, message);
        if (err) return err;
      }
      return null;
    },
    broadcastMessageExcept: async (
      excludedRole: string,
      message: string
    ): Promise<Error | null> => {
      for (const role of PLAYER_NAME_LIST) {
        if (role === excludedRole) continue;
        const err: Error | null = await sendPlayerMessage(role, message);
        if (err) return err;
      }
      return null;
    },
  };
}
