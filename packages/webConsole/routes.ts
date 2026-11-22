import {
  ACTOR_COMMENT_DICT,
  BONE_DAGGER_ITEM,
  C,
  CONSOLE_MESSAGES,
  HIGH_STAT_WARNING_THRESHOLD,
  STATUS_CODE,
} from "../constants";
import { sanitizeLogText } from "../core";
import { WEB_CONSOLE_MESSAGES } from "../messages";
import type { ApiResultPayload, RouteHandler } from "../types";
import type { WebConsoleAuth } from "./auth";
import { buildConsoleBroadcastMessage } from "./bridge";
import type { ConsoleBridge } from "./bridge";
import { apiResult, roleDisplay, toStringValue } from "./httpUtils";
import {
  emitWebLog,
  formatConsoleCommandPreview,
  formatResultBadge,
} from "./logger";

export interface ApiRoutesOptions {
  readonly auth: WebConsoleAuth;
  readonly bridge: ConsoleBridge;
  readonly autoBroadcast: boolean;
}

export interface ApiRouteHandlers {
  readonly handleConsoleCommand: RouteHandler;
  readonly handleSetHuman: RouteHandler;
  readonly handleAddItem: RouteHandler;
  readonly handleSetStorage: RouteHandler;
  readonly handleSendThrallMsg: RouteHandler;
  readonly handleBroadcast: RouteHandler;
  readonly handleKickPlayer: RouteHandler;
}

export function createApiRoutes(options: ApiRoutesOptions): ApiRouteHandlers {
  const { auth, bridge, autoBroadcast }: ApiRoutesOptions = options;
  const { logCategories, logDetails, broadcasts }: typeof WEB_CONSOLE_MESSAGES =
    WEB_CONSOLE_MESSAGES;

  const handleConsoleCommand: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/console_command",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const command: unknown = data.command;
      if (typeof targetRole !== "string" || typeof command !== "string") {
        emitWebLog({
          category: logCategories.command,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const cmdPreview: string = formatConsoleCommandPreview(command);

      const execErr: Error | null = await bridge.executeConsoleCommand(
        targetRole,
        command
      );
      if (execErr) {
        emitWebLog({
          category: logCategories.command,
          categoryColor: C.LIGHT_CYAN,
          detail: `${clientIp} | ${roleLabel} -> ${cmdPreview} ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }

      const spaceIdx: number = command.indexOf(" ");
      const commandName: string =
        spaceIdx === -1 ? command : command.slice(0, spaceIdx);
      const commandArg: string =
        spaceIdx === -1 ? "" : command.slice(spaceIdx + 1);
      let extraNote: string = "";

      if (
        commandName.toLowerCase() === "setisthrall" &&
        commandArg.trim() !== "0"
      ) {
        extraNote = logDetails.boneDaggerAddedSuffix;
        const daggerErr: Error | null = await bridge.addInventoryItem(
          targetRole,
          BONE_DAGGER_ITEM,
          1
        );
        if (daggerErr) {
          emitWebLog({
            category: logCategories.command,
            categoryColor: C.LIGHT_CYAN,
            detail: `${clientIp} | ${roleLabel} -> ${cmdPreview}${extraNote} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
          });
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }

      if (autoBroadcast) {
        const notice: string | null = buildConsoleBroadcastMessage(
          targetRole,
          commandName,
          commandArg
        );
        if (notice !== null) {
          const broadcastErr: Error | null =
            await bridge.broadcastMessageExcept(targetRole, notice);
          if (broadcastErr) {
            emitWebLog({
              category: logCategories.command,
              categoryColor: C.LIGHT_CYAN,
              detail: `${clientIp} | ${roleLabel} -> ${cmdPreview}${extraNote} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
            });
            return apiResult(
              STATUS_CODE.partialSuccess,
              CONSOLE_MESSAGES.broadcastPartialError
            );
          }
        }
      }
      emitWebLog({
        category: logCategories.command,
        categoryColor: C.LIGHT_CYAN,
        detail: `${clientIp} | ${roleLabel} -> ${cmdPreview}${extraNote} ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSetHuman: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/set_human",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const propertyName: unknown = data.prop_name;
      const rawValue: unknown = data.value;
      if (
        typeof targetRole !== "string" ||
        typeof propertyName !== "string" ||
        rawValue === undefined ||
        rawValue === null ||
        rawValue === ""
      ) {
        emitWebLog({
          category: logCategories.status,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const safeProp: string = sanitizeLogText(propertyName, 32);
      const propertyValue: number = Number(rawValue);
      if (!Number.isFinite(propertyValue) || propertyValue < 0) {
        emitWebLog({
          category: logCategories.status,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${roleLabel} | ${safeProp} = ${sanitizeLogText(toStringValue(rawValue), 16)} ${formatResultBadge(STATUS_CODE.invalidValue, logDetails.invalidNumberReason)}`,
        });
        return apiResult(
          STATUS_CODE.invalidValue,
          CONSOLE_MESSAGES.invalidValueError
        );
      }

      const execErr: Error | null = await bridge.setHumanProperty(
        targetRole,
        propertyName,
        propertyValue
      );
      if (execErr) {
        emitWebLog({
          category: logCategories.status,
          categoryColor: C.LIGHT_BLUE,
          detail: `${clientIp} | ${roleLabel} | ${safeProp} = ${propertyValue} ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await bridge.broadcastMessageExcept(
          targetRole,
          broadcasts.modifiedStats(roleDisplay(targetRole))
        );
        if (broadcastErr) {
          emitWebLog({
            category: logCategories.status,
            categoryColor: C.LIGHT_BLUE,
            detail: `${clientIp} | ${roleLabel} | ${safeProp} = ${propertyValue} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
          });
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      emitWebLog({
        category: logCategories.status,
        categoryColor: C.LIGHT_BLUE,
        detail: `${clientIp} | ${roleLabel} | ${safeProp} = ${propertyValue} ${formatResultBadge(STATUS_CODE.success)}`,
      });
      if (propertyValue > HIGH_STAT_WARNING_THRESHOLD) {
        return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.highValueWarning);
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleAddItem: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/add_item",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const itemBlueprintPath: unknown = data.item_name;
      const rawItemCount: unknown = data.count;
      if (
        typeof targetRole !== "string" ||
        typeof itemBlueprintPath !== "string" ||
        rawItemCount === undefined ||
        rawItemCount === null
      ) {
        emitWebLog({
          category: logCategories.item,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const itemLabel: string = sanitizeLogText(
        ACTOR_COMMENT_DICT[itemBlueprintPath] ?? itemBlueprintPath,
        40
      );
      const itemCount: number = Number.parseInt(
        toStringValue(rawItemCount),
        10
      );
      if (!Number.isInteger(itemCount) || itemCount <= 0) {
        emitWebLog({
          category: logCategories.item,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${roleLabel} | ${itemLabel} x${sanitizeLogText(toStringValue(rawItemCount), 12)} ${formatResultBadge(STATUS_CODE.invalidValue, logDetails.invalidCountReason)}`,
        });
        return apiResult(
          STATUS_CODE.invalidValue,
          CONSOLE_MESSAGES.invalidValueError
        );
      }

      const execErr: Error | null = await bridge.addInventoryItem(
        targetRole,
        itemBlueprintPath,
        itemCount
      );
      if (execErr) {
        emitWebLog({
          category: logCategories.item,
          categoryColor: C.LIGHT_GREEN,
          detail: `${clientIp} | ${roleLabel} | ${itemLabel} x${itemCount} ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }

      if (autoBroadcast) {
        const broadcastItemLabel: string =
          ACTOR_COMMENT_DICT[itemBlueprintPath] ?? itemBlueprintPath;
        const broadcastErr: Error | null = await bridge.broadcastMessageExcept(
          targetRole,
          broadcasts.acquiredItem(
            roleDisplay(targetRole),
            broadcastItemLabel,
            itemCount
          )
        );
        if (broadcastErr) {
          emitWebLog({
            category: logCategories.item,
            categoryColor: C.LIGHT_GREEN,
            detail: `${clientIp} | ${roleLabel} | ${itemLabel} x${itemCount} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
          });
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      emitWebLog({
        category: logCategories.item,
        categoryColor: C.LIGHT_GREEN,
        detail: `${clientIp} | ${roleLabel} | ${itemLabel} x${itemCount} ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSetStorage: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/set_storage",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const rawStorageLimit: unknown = data.limit;
      if (
        typeof targetRole !== "string" ||
        rawStorageLimit === undefined ||
        rawStorageLimit === null
      ) {
        emitWebLog({
          category: logCategories.storage,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const storageLimit: number = Number.parseInt(
        toStringValue(rawStorageLimit),
        10
      );
      if (!Number.isInteger(storageLimit) || storageLimit <= 0) {
        emitWebLog({
          category: logCategories.storage,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${roleLabel} | ${logDetails.storageCapacityLabel} ${sanitizeLogText(toStringValue(rawStorageLimit), 12)} ${formatResultBadge(STATUS_CODE.invalidValue, logDetails.invalidValueReason)}`,
        });
        return apiResult(
          STATUS_CODE.invalidValue,
          CONSOLE_MESSAGES.invalidValueError
        );
      }

      const execErr: Error | null = await bridge.setPlayerStorageLimit(
        targetRole,
        storageLimit
      );
      if (execErr) {
        emitWebLog({
          category: logCategories.storage,
          categoryColor: C.LIGHT_MAGENTA,
          detail: `${clientIp} | ${roleLabel} | ${logDetails.storageCapacityLabel} ${storageLimit} ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await bridge.broadcastMessageExcept(
          targetRole,
          broadcasts.storageLimitChanged(roleDisplay(targetRole), storageLimit)
        );
        if (broadcastErr) {
          emitWebLog({
            category: logCategories.storage,
            categoryColor: C.LIGHT_MAGENTA,
            detail: `${clientIp} | ${roleLabel} | ${logDetails.storageCapacityLabel} ${storageLimit} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
          });
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      emitWebLog({
        category: logCategories.storage,
        categoryColor: C.LIGHT_MAGENTA,
        detail: `${clientIp} | ${roleLabel} | ${logDetails.storageCapacityLabel} ${storageLimit} ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSendThrallMsg: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/send_thrall_msg",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const message: unknown = data.msg;
      if (typeof targetRole !== "string" || typeof message !== "string") {
        emitWebLog({
          category: logCategories.notice,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const safeMsg: string = sanitizeLogText(message, 48);
      const execErr: Error | null = await bridge.sendPlayerMessage(
        targetRole,
        message
      );
      if (execErr) {
        emitWebLog({
          category: logCategories.notice,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${roleLabel} -> 「${safeMsg}」 ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }
      emitWebLog({
        category: logCategories.notice,
        categoryColor: C.LIGHT_YELLOW,
        detail: `${clientIp} | ${roleLabel} -> 「${safeMsg}」 ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleBroadcast: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/send_thrall_msg/broadcast",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const message: unknown = data.msg;
      if (typeof message !== "string") {
        emitWebLog({
          category: logCategories.broadcast,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const safeMsg: string = sanitizeLogText(message, 48);
      const execErr: Error | null = await bridge.broadcastMessageToAll(message);
      if (execErr) {
        emitWebLog({
          category: logCategories.broadcast,
          categoryColor: C.LIGHT_YELLOW,
          detail: `${clientIp} | ${logDetails.broadcastAllTarget} -> 「${safeMsg}」 ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }
      emitWebLog({
        category: logCategories.broadcast,
        categoryColor: C.LIGHT_YELLOW,
        detail: `${clientIp} | ${logDetails.broadcastAllTarget} -> 「${safeMsg}」 ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleKickPlayer: RouteHandler = auth.withEncryptedPostAuth(
    "/dh-console/kick_player",
    async (
      data: Record<string, unknown>,
      clientIp: string
    ): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      if (typeof targetRole !== "string") {
        emitWebLog({
          category: logCategories.kick,
          categoryColor: C.LIGHT_RED,
          detail: `${clientIp} | ${logDetails.invalidParams} ${formatResultBadge(STATUS_CODE.badRequest)}`,
        });
        return apiResult(
          STATUS_CODE.badRequest,
          CONSOLE_MESSAGES.networkParamError
        );
      }

      const roleLabel: string = sanitizeLogText(roleDisplay(targetRole), 24);
      const execErr: Error | null = await bridge.kickTargetPlayer(targetRole);
      if (execErr) {
        emitWebLog({
          category: logCategories.kick,
          categoryColor: C.LIGHT_RED,
          detail: `${clientIp} | ${roleLabel} ${logDetails.kickVerb} ${formatResultBadge(STATUS_CODE.serverError, execErr.message)}`,
        });
        return apiResult(
          STATUS_CODE.serverError,
          CONSOLE_MESSAGES.serverExecutionError
        );
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await bridge.broadcastMessageExcept(
          targetRole,
          broadcasts.kicked(roleDisplay(targetRole))
        );
        if (broadcastErr) {
          emitWebLog({
            category: logCategories.kick,
            categoryColor: C.LIGHT_RED,
            detail: `${clientIp} | ${roleLabel} ${logDetails.kickVerb} ${formatResultBadge(STATUS_CODE.partialSuccess)}`,
          });
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      emitWebLog({
        category: logCategories.kick,
        categoryColor: C.LIGHT_RED,
        detail: `${clientIp} | ${roleLabel} ${logDetails.kickVerb} ${formatResultBadge(STATUS_CODE.success)}`,
      });
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  return {
    handleConsoleCommand,
    handleSetHuman,
    handleAddItem,
    handleSetStorage,
    handleSendThrallMsg,
    handleBroadcast,
    handleKickPlayer,
  };
}
