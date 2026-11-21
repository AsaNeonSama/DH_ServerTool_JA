var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var CONSOLE_BRIDGE_HOST = '127.0.0.1';
var CONSOLE_BRIDGE_PORT = __CONSOLE_PATCH_PORT__;
var MAX_PENDING_COMMANDS = 100;

// ============================================================================
// UE4 Native Function Bindings & Hook Addresses
// ============================================================================
var FText_FromString = new NativeFunction(moduleBase.add(0x10963B0), 'pointer', ['pointer', 'pointer']);
var FName_GetPlainNameString = new NativeFunction(moduleBase.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');

var UClass_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0x11F02E0), 'pointer', [], 'win64');
var StaticLoadObject = new NativeFunction(moduleBase.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(moduleBase.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');

var UDH_InventoryManager_AddInventory = new NativeFunction(moduleBase.add(0xDBC040), 'void', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], 'win64');
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(moduleBase.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');
var AGameSession_KickPlayer = new NativeFunction(moduleBase.add(0x256B770), 'void', ['pointer', 'pointer', 'pointer']);
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(moduleBase.add(0xEE7810), 'void', ['pointer', 'pointer', 'pointer']);
var ADH_PlayerController_AddCheats = new NativeFunction(moduleBase.add(0xE2B7E0), 'void', ['pointer', 'int8'], 'win64');
var ADH_PlayerController_SetDesiresThrallVision_Impl = new NativeFunction(moduleBase.add(0xE4D980), 'void', ['pointer', 'int8'], 'win64');
var ADH_PlayerState_SetIsThrall = new NativeFunction(moduleBase.add(0xE4E880), 'void', ['pointer', 'int8'], 'win64');
var ADH_HumanCharacter_SetIsSpiritWalking = new NativeFunction(moduleBase.add(0xD70310), 'void', ['pointer', 'int8', 'float'], 'win64');
var AActor_GetWorldTimerManager = new NativeFunction(moduleBase.add(0x22DDC30), 'pointer', ['pointer'], 'win64');
var FTimerManager_FindTimer = new NativeFunction(moduleBase.add(0x28F2D80), 'pointer', ['pointer', 'pointer'], 'win64');
var FTimerManager_InternalClearTimer = new NativeFunction(moduleBase.add(0x28F6A00), 'void', ['pointer', 'uint64'], 'win64');
var UKismetSystemLibrary_ExecuteConsoleCommand = new NativeFunction(moduleBase.add(0x25EFC50), 'void', ['pointer', 'pointer', 'pointer'], 'win64');

var APlayerController_BeginPlay_addr = moduleBase.add(0xE2DBC0);
var ADH_GameState_Tick_addr = moduleBase.add(0xDAD420);
var ADH_GameState_HandleMatchHasStarted_addr = moduleBase.add(0xD99080);

// ============================================================================
// UE4 Struct Field Offsets
// ============================================================================
var UE_OFFSETS = {
    uObjectClassPrivate: 0x10,
    uObjectNamePrivate: 0x18,
    actorVtableK2DestroyActor: 0x310,
    gameStateAuthorityGameMode: 0x228,
    gameStatePlayerArray: 0x238,
    gameModeGameSession: 0x278,
    playerStateOwnerController: 0xE0,
    playerStatePlayerName: 0x300,
    playerStateSpellManager: 0x488,
    playerStateOtherThralls: 0x578,
    playerStateSelectedRole: 0x590,
    roleDataOnlineName: 0x48,
    playerControllerPlayerState: 0x228,
    playerControllerPawn: 0x250,
    playerControllerCheatManager: 0x338,
    humanInventoryManager: 0x808,
    humanSpiritWalkTimerHandle: 0xD50
};

var HUMAN_PROPERTY_OFFSETS = {
    bUpdateHunger: ['int8', 0xA40],
    bUpdateWarmth: ['int8', 0xA41],
    HealthBoostRegenRate: ['float', 0xA58],
    WarmthRegenRate: ['float', 0xA80],
    HungerRegenRate: ['float', 0xA84],
    JumpStaminaConsumption: ['float', 0xA8C],
    BodyTossStaminaConsumption: ['float', 0xA90],
    TossPawnVelocity: ['float', 0xB1C],
    SpiritWalkSpeedModifier: ['float', 0xBC0],
    MaxHealthRegenSpeed: ['float', 0xCA4],
    WarmthBoostRemaining: ['float', 0xFF4],
    HungerBoostRemaining: ['float', 0xFF8],
    HealthBoostRemaining: ['float', 0xFFC],
    MaximumStamina: ['float', 0xA48],
    MinimumStamina: ['float', 0xA4C],
    CurrentHealth: ['float', 0x94C],
    MaximumHealth: ['float', 0x7E0],
    MinHealth: ['float', 0x1000],
    MaxHunger: ['float', 0x1010],
    bSpiritWalking: ['int8', 0xC71],
    bPoisoned: ['int8', 0x1250]
};

// Preload map for `summon <ShortClassName>` so classes not placed in the current map
// (e.g. BP_Prey_Rat_C on Departure) are loaded before UCheatManager::Summon runs.
var SUMMON_BLUEPRINT_PATHS = {
    BP_Prey_Rat_C: '/Game/Blueprints/AI/Prey/BP_Prey_Rat.BP_Prey_Rat_C',
    BP_Prey_Rabbit_C: '/Game/Blueprints/AI/Prey/BP_Prey_Rabbit.BP_Prey_Rabbit_C',
    BP_Prey_Seal_C: '/Game/Blueprints/AI/Prey/BP_Prey_Seal.BP_Prey_Seal_C',
    BP_Predator_Wolf_C: '/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C',
    BP_Predator_Bear_C: '/Game/Blueprints/AI/Predators/BP_Predator_Bear.BP_Predator_Bear_C',
    BP_Cannibal_C: '/Game/Blueprints/AI/Cannibal/BP_Cannibal.BP_Cannibal_C',
    BP_Cannibal_Crawler_C: '/Game/Blueprints/AI/Cannibal/BP_Cannibal_Crawler.BP_Cannibal_Crawler_C',
    BP_Cannibal_Shambler_C: '/Game/Blueprints/AI/Cannibal/BP_Cannibal_Shambler.BP_Cannibal_Shambler_C',
    BP_HumanCharacter_C: '/Game/Blueprints/Player/BP_HumanCharacter.BP_HumanCharacter_C',
    BP_Totem_C: '/Game/Blueprints/Game/Totems/BP_Totem.BP_Totem_C',
    BP_ExplosionActor_C: '/Game/Blueprints/Game/BP_ExplosionActor.BP_ExplosionActor_C',
    BP_Explosion_Nitro_C: '/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C'
};

// ============================================================================
// Logging & UE4 String / Object Utilities
// ============================================================================
function writePatchLog(message) {
    try {
        var logFile = new File('output.log', 'a');
        logFile.write('[Patches] ' + message + '\n');
        logFile.close();
    } catch (e) {}
}

function allocateFString(capacityChars) {
    var fStringPtr = Memory.alloc(16 + capacityChars * 3);
    fStringPtr.writePointer(fStringPtr.add(16));
    fStringPtr.add(8).writeU32(0);
    fStringPtr.add(12).writeU32(capacityChars);
    return fStringPtr;
}

function writeFStringValue(fStringPtr, text) {
    fStringPtr.add(8).writeU32(text.length + 1);
    fStringPtr.readPointer().writeUtf16String(text);
}

function createFTextFromString(text) {
    var fTextPtr = Memory.alloc(24);
    var fStringPtr = allocateFString(text.length + 3);
    writeFStringValue(fStringPtr, text);
    FText_FromString(fTextPtr, fStringPtr);
    return fTextPtr;
}

function getUObjectName(uObjectPtr) {
    var namePrivatePtr = uObjectPtr.add(UE_OFFSETS.uObjectNamePrivate);
    var nameBuffer = allocateFString(100);
    FName_GetPlainNameString(namePrivatePtr, nameBuffer);
    return nameBuffer.readPointer().readUtf16String();
}

function findObjectByPath(objectPath, targetClassPtr) {
    var utf16Buffer = Memory.alloc((objectPath.length + 1) * 2);
    utf16Buffer.writeUtf16String(objectPath);
    return StaticFindObject(targetClassPtr, ptr(0xFFFFFFFFFFFFFFFF), utf16Buffer, 0);
}

function loadObjectByPath(objectPath, targetClassPtr) {
    var utf16Buffer = Memory.alloc((objectPath.length + 1) * 2);
    utf16Buffer.writeUtf16String(objectPath);
    return StaticLoadObject(targetClassPtr, ptr(0), utf16Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}

function resolveBlueprintClass(blueprintPath) {
    var uClassStatic = UClass_GetPrivateStaticClass();
    var resolvedClass = findObjectByPath(blueprintPath, uClassStatic);
    if (resolvedClass.isNull()) {
        resolvedClass = loadObjectByPath(blueprintPath, uClassStatic);
    }
    return resolvedClass;
}

function preloadSummonBlueprint(commandText) {
    if (!commandText || commandText.toLowerCase().indexOf('summon ') !== 0) {
        return;
    }
    var shortClassName = commandText.slice(7).trim();
    if (SUMMON_BLUEPRINT_PATHS.hasOwnProperty(shortClassName)) {
        resolveBlueprintClass(SUMMON_BLUEPRINT_PATHS[shortClassName]);
    }
}

// ============================================================================
// Gameplay Operations
// ============================================================================
function writeObjectProperty(propertyTable, propertyName, propertyValue, targetObjectPtr) {
    if (!propertyTable.hasOwnProperty(propertyName)) {
        return;
    }
    var valueType = propertyTable[propertyName][0];
    var fieldOffset = propertyTable[propertyName][1];
    var fieldPtr = targetObjectPtr.add(fieldOffset);
    if (valueType === 'int8') {
        fieldPtr.writeU8(propertyValue);
    } else if (valueType === 'int16') {
        fieldPtr.writeU16(propertyValue);
    } else if (valueType === 'int32') {
        fieldPtr.writeU32(propertyValue);
    } else if (valueType === 'int64') {
        fieldPtr.writeU64(propertyValue);
    } else if (valueType === 'float') {
        fieldPtr.writeFloat(propertyValue);
    }
}

function clearSpiritWalkTimer(pawnPtr) {
    var timerHandlePtr = pawnPtr.add(UE_OFFSETS.humanSpiritWalkTimerHandle);
    var rawHandle = timerHandlePtr.readU64();
    if (!rawHandle.equals(0)) {
        var timerManager = AActor_GetWorldTimerManager(pawnPtr);
        if (!timerManager.isNull()) {
            var timerData = FTimerManager_FindTimer(timerManager, timerHandlePtr);
            if (!timerData.isNull()) {
                FTimerManager_InternalClearTimer(timerManager, rawHandle);
            }
        }
        timerHandlePtr.writeU64(0);
    }
}

function setSpiritWalkingState(pawnPtr, enabled) {
    clearSpiritWalkTimer(pawnPtr);
    ADH_HumanCharacter_SetIsSpiritWalking(pawnPtr, enabled ? 1 : 0, 0.0);
}

function destroyActorSafe(actorPtr) {
    if (actorPtr.isNull()) {
        return;
    }
    var vtablePtr = actorPtr.readPointer();
    if (vtablePtr.isNull()) {
        return;
    }
    var fnPtr = vtablePtr.add(UE_OFFSETS.actorVtableK2DestroyActor).readPointer();
    if (fnPtr.isNull()) {
        return;
    }
    var k2DestroyActor = new NativeFunction(fnPtr, 'void', ['pointer'], 'win64');
    k2DestroyActor(actorPtr);
}

function removeFromOtherThrallsArray(playerStatePtr, targetToRemovePtr) {
    var otherThrallsPtr = playerStatePtr.add(UE_OFFSETS.playerStateOtherThralls);
    var dataPtr = otherThrallsPtr.readPointer();
    var count = otherThrallsPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0) {
        return;
    }
    var writeIdx = 0;
    for (var i = 0; i < count; i++) {
        var elemPtr = dataPtr.add(8 * i).readPointer();
        if (!elemPtr.equals(targetToRemovePtr)) {
            if (writeIdx !== i) {
                dataPtr.add(8 * writeIdx).writePointer(elemPtr);
            }
            writeIdx += 1;
        }
    }
    if (writeIdx !== count) {
        otherThrallsPtr.add(8).writeS32(writeIdx);
    }
}

function setTargetPlayerIsThrall(gameStatePtr, playerControllerPtr, isThrall) {
    if (gameStatePtr.isNull() || playerControllerPtr.isNull()) {
        return;
    }
    var playerStatePtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPlayerState).readPointer();
    if (playerStatePtr.isNull()) {
        return;
    }
    if (isThrall) {
        ADH_PlayerState_SetIsThrall(playerStatePtr, 1);
        return;
    }
    // Revert Thrall back to Explorer cleanly:
    // 1. Disable Thrall Vision if currently enabled
    ADH_PlayerController_SetDesiresThrallVision_Impl(playerControllerPtr, 0);

    // 2. Clear target's OtherThralls array and remove target from every other player's OtherThralls
    var targetOtherThrallsPtr = playerStatePtr.add(UE_OFFSETS.playerStateOtherThralls);
    if (targetOtherThrallsPtr.add(8).readS32() > 0) {
        targetOtherThrallsPtr.add(8).writeS32(0);
    }
    var playerArrayPtr = gameStatePtr.add(UE_OFFSETS.gameStatePlayerArray);
    var playerListDataPtr = playerArrayPtr.readPointer();
    var playerCount = playerArrayPtr.add(8).readU32();
    if (!playerListDataPtr.isNull() && playerCount > 0) {
        for (var i = 0; i < playerCount; i++) {
            var otherPlayerStatePtr = playerListDataPtr.add(8 * i).readPointer();
            if (!otherPlayerStatePtr.isNull() && !otherPlayerStatePtr.equals(playerStatePtr)) {
                removeFromOtherThrallsArray(otherPlayerStatePtr, playerStatePtr);
            }
        }
    }

    // 3. Destroy and clear SpellManager (+0x488) so spells/totems are revoked and re-promotion spawns cleanly
    var spellManagerPtr = playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).readPointer();
    if (!spellManagerPtr.isNull()) {
        destroyActorSafe(spellManagerPtr);
        playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).writePointer(ptr(0));
    }

    // 4. Set bIsThrall (+0x572) = 0, fire delegates, update Hush & CannibalLevel (0.0f)
    ADH_PlayerState_SetIsThrall(playerStatePtr, 0);
}

function setHumanCharacterProperty(playerControllerPtr, propertyName, propertyValue) {
    if (playerControllerPtr.isNull()) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull()) {
        return;
    }
    var pawnClassPtr = pawnPtr.add(UE_OFFSETS.uObjectClassPrivate).readPointer();
    if (pawnClassPtr.isNull()) {
        return;
    }
    if (getUObjectName(pawnClassPtr) === 'BP_HumanCharacter_C') {
        if (propertyName === 'bSpiritWalking') {
            setSpiritWalkingState(pawnPtr, propertyValue ? 1 : 0);
        } else {
            writeObjectProperty(HUMAN_PROPERTY_OFFSETS, propertyName, propertyValue, pawnPtr);
        }
    }
}

function initializeItemState(itemStatePtr, isUpgraded, isTainted) {
    itemStatePtr.writeU32(0xffffffff);
    itemStatePtr.add(0x04).writeU8(1);
    itemStatePtr.add(0x08).writeFloat(1.0);
    itemStatePtr.add(0x0C).writeU8(0);
    itemStatePtr.add(0x0D).writeU8(isUpgraded);
    itemStatePtr.add(0x0E).writeU8(isTainted);
    itemStatePtr.add(0x10).writePointer(ptr(0));
    itemStatePtr.add(0x18).writePointer(ptr(0));
    itemStatePtr.add(0x20).writePointer(ptr(0));
    itemStatePtr.add(0x28).writePointer(ptr(0));
    itemStatePtr.add(0x30).writePointer(ptr(0));
}

function giveInventoryItem(playerControllerPtr, itemBlueprintPath, itemCount) {
    if (playerControllerPtr.isNull()) {
        return;
    }
    var itemClassPtr = resolveBlueprintClass(itemBlueprintPath);
    if (itemClassPtr.isNull()) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull()) {
        return;
    }
    var inventoryManagerPtr = pawnPtr.add(UE_OFFSETS.humanInventoryManager).readPointer();
    if (inventoryManagerPtr.isNull()) {
        return;
    }

    var outParamsBuffer = Memory.alloc(8);
    var itemStateArrayPtr = Memory.alloc(16 + 56);
    itemStateArrayPtr.writePointer(itemStateArrayPtr.add(16));
    itemStateArrayPtr.add(8).writeU32(1);
    itemStateArrayPtr.add(12).writeU32(1);
    initializeItemState(itemStateArrayPtr.readPointer(), 0, 0);

    for (var i = 0; i < itemCount; i++) {
        outParamsBuffer.writeU32(0);
        outParamsBuffer.add(4).writeU32(0xffffffff);
        UDH_InventoryManager_AddInventory(
            inventoryManagerPtr,
            itemClassPtr,
            itemStateArrayPtr,
            outParamsBuffer,
            outParamsBuffer.add(4),
            0,
            ptr(0)
        );
    }
}

function updateStorageLimit(playerControllerPtr, storageLimit) {
    if (playerControllerPtr.isNull()) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull()) {
        return;
    }
    var inventoryManagerPtr = pawnPtr.add(UE_OFFSETS.humanInventoryManager).readPointer();
    if (inventoryManagerPtr.isNull()) {
        return;
    }
    UDH_InventoryManager_SetStorageLimit(inventoryManagerPtr, storageLimit);
}

function kickTargetPlayer(gameStatePtr, playerControllerPtr) {
    if (gameStatePtr.isNull() || playerControllerPtr.isNull()) {
        return;
    }
    var gameModePtr = gameStatePtr.add(UE_OFFSETS.gameStateAuthorityGameMode).readPointer();
    if (gameModePtr.isNull()) {
        return;
    }
    var gameSessionPtr = gameModePtr.add(UE_OFFSETS.gameModeGameSession).readPointer();
    if (gameSessionPtr.isNull()) {
        return;
    }
    var kickReasonText = createFTextFromString('kick');
    AGameSession_KickPlayer(gameSessionPtr, playerControllerPtr, kickReasonText);
}

function deliverThrallMessage(playerControllerPtr, messageText) {
    if (playerControllerPtr.isNull()) {
        return;
    }
    var messageFText = createFTextFromString(messageText);
    ADH_PlayerController_ReceiveThrallMessage(playerControllerPtr, messageFText, ptr(0));
}

// ============================================================================
// Player Lookup & Command Queue
// ============================================================================
var pendingCommandQueue = [];

function enqueueCommand(payload) {
    if (pendingCommandQueue.length >= MAX_PENDING_COMMANDS) {
        return false;
    }
    pendingCommandQueue.push(payload);
    return true;
}

function dequeueCommand() {
    if (pendingCommandQueue.length !== 0) {
        return pendingCommandQueue.shift();
    }
    return null;
}

function findPlayerControllerByTarget(gameStatePtr, targetName, matchPlayerName) {
    if (gameStatePtr.isNull() || !targetName) {
        return ptr(0);
    }
    var playerArrayPtr = gameStatePtr.add(UE_OFFSETS.gameStatePlayerArray);
    var playerListDataPtr = playerArrayPtr.readPointer();
    var playerCount = playerArrayPtr.add(8).readU32();
    if (playerListDataPtr.isNull() || playerCount === 0) {
        return ptr(0);
    }
    for (var i = 0; i < playerCount; i++) {
        var playerStatePtr = playerListDataPtr.add(8 * i).readPointer();
        if (playerStatePtr.isNull()) {
            continue;
        }
        var candidateName = '';
        if (matchPlayerName) {
            var playerNamePtr = playerStatePtr.add(UE_OFFSETS.playerStatePlayerName).readPointer();
            if (playerNamePtr.isNull()) {
                continue;
            }
            candidateName = playerNamePtr.readUtf16String();
        } else {
            var selectedRolePtr = playerStatePtr.add(UE_OFFSETS.playerStateSelectedRole).readPointer();
            if (selectedRolePtr.isNull()) {
                continue;
            }
            var roleNamePtr = selectedRolePtr.add(UE_OFFSETS.roleDataOnlineName).readPointer();
            if (roleNamePtr.isNull()) {
                continue;
            }
            candidateName = roleNamePtr.readUtf16String();
        }
        if (candidateName === targetName) {
            return playerStatePtr.add(UE_OFFSETS.playerStateOwnerController).readPointer();
        }
    }
    return ptr(0);
}

function resolveTargetPlayerController(gameStatePtr, payload) {
    if (payload.hasOwnProperty('targetPlayerName')) {
        return findPlayerControllerByTarget(gameStatePtr, payload.targetPlayerName, true);
    }
    if (payload.hasOwnProperty('targetRole')) {
        return findPlayerControllerByTarget(gameStatePtr, payload.targetRole, false);
    }
    return ptr(0);
}

function executeQueuedPayload(gameStatePtr, payload) {
    var action = payload.action;
    var targetController = resolveTargetPlayerController(gameStatePtr, payload);

    if (action === 'execute_console_command') {
        var commandText = payload.command;
        var trimmedCommand = commandText ? commandText.trim() : '';
        var firstSpaceIdx = trimmedCommand.indexOf(' ');
        var cmdName = (firstSpaceIdx === -1 ? trimmedCommand : trimmedCommand.slice(0, firstSpaceIdx)).toLowerCase();
        if (cmdName === 'setisthrall') {
            var thrallArg = firstSpaceIdx === -1 ? '' : trimmedCommand.slice(firstSpaceIdx + 1).trim().toLowerCase();
            var enableThrall = (thrallArg !== '0' && thrallArg !== 'false');
            setTargetPlayerIsThrall(gameStatePtr, targetController, enableThrall);
            return;
        }
        preloadSummonBlueprint(commandText);
        if (!targetController.isNull() && targetController.add(UE_OFFSETS.playerControllerCheatManager).readPointer().isNull()) {
            ADH_PlayerController_AddCheats(targetController, 1);
        }
        var commandFString = allocateFString(commandText.length + 10);
        writeFStringValue(commandFString, commandText);
        UKismetSystemLibrary_ExecuteConsoleCommand(gameStatePtr, commandFString, targetController);
    } else if (action === 'add_inventory_item') {
        giveInventoryItem(targetController, payload.itemBlueprintPath, payload.itemCount);
    } else if (action === 'set_storage_limit') {
        updateStorageLimit(targetController, payload.storageLimit);
    } else if (action === 'kick_player') {
        kickTargetPlayer(gameStatePtr, targetController);
    } else if (action === 'send_thrall_message') {
        deliverThrallMessage(targetController, payload.message);
    } else if (action === 'set_human_property') {
        setHumanCharacterProperty(targetController, payload.propertyName, payload.propertyValue);
    }
}

// ============================================================================
// Game Hooks
// ============================================================================
Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        this.gameStatePtr = args[0];
    },
    onLeave: function () {
        var gameStatePtr = this.gameStatePtr;
        if (gameStatePtr.isNull()) {
            return;
        }
        if (!isBridgeServerRunning) {
            startBridgeServer();
        }
        var payload = dequeueCommand();
        if (payload) {
            try {
                executeQueuedPayload(gameStatePtr, payload);
            } catch (e) {
                writePatchLog('Command exec error (' + payload.action + '): ' + e.message);
            }
        }
    }
});

Interceptor.attach(APlayerController_BeginPlay_addr, {
    onEnter: function (args) {
        this.playerControllerPtr = args[0];
    },
    onLeave: function () {
        if (!this.playerControllerPtr.isNull()) {
            ADH_PlayerController_AddCheats(this.playerControllerPtr, 1);
        }
    }
});

// ============================================================================
// Local Loopback TCP Bridge Server (receives UTF-16LE JSON from webConsole.ts)
// ============================================================================
var tcpListener = null;
var isBridgeServerRunning = false;

function logBridgeError(err) {
    if (err && err.message) {
        writePatchLog(err.message);
    }
}

function safeCloseConnection(connection) {
    try {
        var closePromise = connection.close();
        if (closePromise && typeof closePromise.catch === 'function') {
            closePromise.catch(function () {});
        }
    } catch (e) {}
}

function handleBridgeConnection(connection) {
    connection.input.read(4096).then(function (buffer) {
        safeCloseConnection(connection);
        if (!buffer || buffer.byteLength < 2) {
            return;
        }
        var rawJson = buffer.unwrap().readUtf16String();
        if (!rawJson || rawJson.length === 0) {
            return;
        }
        try {
            var payload = JSON.parse(rawJson);
            if (payload && typeof payload === 'object') {
                enqueueCommand(payload);
            }
        } catch (parseErr) {
            logBridgeError(parseErr);
        }
    }).catch(function (err) {
        safeCloseConnection(connection);
        logBridgeError(err);
    });
}

function acceptBridgeLoop(listener) {
    tcpListener = listener;
    listener.accept().then(function (connection) {
        handleBridgeConnection(connection);
        setImmediate(function () { acceptBridgeLoop(listener); });
    }).catch(function (err) {
        var errMsg = err && err.message ? String(err.message) : '';
        if (errMsg.indexOf('cancelled') !== -1 || errMsg.indexOf('closed') !== -1) {
            isBridgeServerRunning = false;
            return;
        }
        logBridgeError(err);
        setImmediate(function () { acceptBridgeLoop(listener); });
    });
}

function startBridgeServer() {
    if (isBridgeServerRunning) {
        return;
    }
    isBridgeServerRunning = true;
    Socket.listen({ family: 'ipv4', host: CONSOLE_BRIDGE_HOST, port: CONSOLE_BRIDGE_PORT })
        .then(acceptBridgeLoop)
        .catch(function (err) {
            isBridgeServerRunning = false;
            logBridgeError(err);
        });
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function () {
        startBridgeServer();
    }
});
