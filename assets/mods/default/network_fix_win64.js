'use strict';

// ============================================================================
// Network & Reconnect Fix (Server-Side Default Patch)
// Fixes verified DreadHungerServer-Win64-Shipping.exe reconnect & join issues:
//   1. Opening-roster lock at match start (anti-intrusion guard): snapshots
//      all opening participants when poker finishes and the match starts
//      (ADH_GameMode::RandomizeThralls / ADH_GameMode::HandleMatchHasStarted /
//      leaving MatchState::WaitingToStart). Captures PlayerArray, active
//      ClientConnections, and ExpectedPlayerIDs so already-loading players are
//      never dropped while slow-connecting teammates can still join during
//      WaitingToStart / poker, and blocks any outsider once the match starts.
//   2. Stale/zombie UNetConnection (ConnectionTimeout = 60s) blocking reconnect
//      with "Server full." on full servers or corrupting active PlayerState on
//      delayed timeout -> proactively cleans up old UNetConnection with the
//      same UniqueId during PreLogin/Login (strictly excluding incoming conn),
//      plus proactive zombie OldPC/OldConn teardown in FindInactivePlayer.
//   3. AGameSession::AtCapacity rejecting returning opening-roster participants
//      whose APlayerState already exists or whose roster slot is locked.
//   4. ADH_GameMode::AddInactivePlayer 10s empty-server CloseServer timer
//      (GameMode + 0x478) never being cancelled when a player reconnects, plus
//      CloseServer guard preventing premature exit while players are active.
//   5. ADH_GameMode::FindInactivePlayer fallback matching (case-insensitive
//      UniqueId string match + unambiguous single-match inactive/disconnected
//      PlayerName fallback restricted to opening-roster participants).
//   6. ADH_GameMode::HandleStartingNewPlayer_Implementation RealPawn (+0x638)
//      recovery when PawnPrivate (+0x280) is null/dead (e.g. disconnected
//      while controlling a Doppelganger).
// ============================================================================

var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

// Verified against DreadHungerServer-Win64-Shipping.pdb
var RVA = {
    GWorld: 0x46ED420,
    GUObjectArray: 0x45C40C8,
    MatchState_WaitingToStart: 0x46CD2A0,
    MatchState_PokerGame: 0x4500DB0,
    MatchState_InProgress: 0x46CD2A8,

    UWorld_NotifyControlMessage: 0x296D350,
    AGameModeBase_PreLogin: 0x25709B0,
    AGameSession_AtCapacity: 0x2556F70,
    ADH_GameMode_Login: 0xDA0700,
    AGameMode_PostLogin: 0x256FEB0,
    ADH_GameMode_FindInactivePlayer: 0xD8F900,
    ADH_GameMode_HandleStartingNewPlayer: 0xD995A0,
    ADH_GameMode_RandomizeThralls: 0xDA4620,
    ADH_GameMode_HandleMatchHasStarted: 0xD98A50,
    ADH_GameMode_CloseServer: 0xD89800,
    AGameMode_HasMatchEnded: 0x2568770,
    AGameMode_OverridePlayerState: 0x256EEB0,
    AGameModeBase_MustSpectate: 0x29EA330,

    UNetConnection_CleanUp: 0x2670DF0,
    FUniqueNetIdWrapper_ToString: 0x894480,
    FMemory_Free: 0x105DD50,
    FMemory_Realloc: 0x106C3D0,
    AActor_GetWorldTimerManager: 0x22DDC30,
    FTimerManager_FindTimer: 0x28F2D80,
    FTimerManager_InternalClearTimer: 0x28F6A00,

    APlayerState_GetPlayerName: 0x27CAED0,
    APlayerState_SetIsInactive: 0x27D68B0,
    APlayerState_SetUniqueId: 0x27D6D60,
    AActor_SetOwner: 0x22E9CE0,
    AActor_Destroy: 0x22D1D40,
    APawn_SetPlayerState: 0x2752480,
    ADH_Character_IsAlive: 0xD5C850,
    ADH_PlayerController_SetDHPlayerState: 0xE4D880,

    ADH_GameMode_GetPrivateStaticClass: 0xEB5E20,
    ADH_GameState_StaticClass: 0xEBE030,
    ADH_PlayerState_GetPrivateStaticClass: 0xEED6B0,
    APlayerController_GetPrivateStaticClass: 0x2AA84E0,
    ADH_PlayerController_GetPrivateStaticClass: 0xEE2F70,
    ADH_HumanCharacter_StaticClass: 0xEC3DA0
};

var OFF = {
    uObjectClassPrivate: 0x10,
    uObjectInternalIndex: 0x0C,
    uStructChainArray: 0x30,
    uStructChainDepth: 0x38,

    actorOwner: 0xE0,

    worldNetDriver: 0x38,
    worldAuthorityGameMode: 0x118,

    netDriverClientConnections: 0x90,
    netConnPlayerController: 0x30,
    netConnOwningActor: 0x98,
    netConnState: 0x134,
    netConnPlayerId: 0x160,
    netConnClientLoginState: 0x1C8,

    gameModeGameState: 0x280,
    gameModeMatchState: 0x2C0,
    gameModeCloseServerTimerHandle: 0x478,

    gameStatePlayerArray: 0x238,
    gameStateExpectedPlayerIds: 0x420,

    controllerPlayerState: 0x228,
    controllerStartSpotWeak: 0x230,
    playerControllerNetConnection: 0x418,
    dhControllerPlayerState: 0x658,

    playerStateFlags: 0x22A,
    playerStateSavedNetworkAddress: 0x240,
    playerStateUniqueId: 0x250,
    playerStatePawnPrivate: 0x280,
    playerStateIsDead: 0x570,
    playerStateIsDisconnected: 0x571,
    playerStateSelectedRole: 0x590,
    playerStateRealPawn: 0x638
};

var NMT_LOGIN = 5;
var NMT_JOIN_SPLIT = 10;
var USOCK_CLOSED = 1;
var CLIENT_LOGIN_STATE_CLEANED_UP = 4;
var PLAYER_STATE_INACTIVE_MASK = 0x20;
var MAX_PLAYER_ARRAY = 64;
var MAX_CLIENT_CONNECTIONS = 128;
var REJECT_MID_MATCH_MESSAGE = 'Match already in progress.';

// Global pointers
var GWorld_ptr = moduleBase.add(RVA.GWorld);
var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
var MatchState_WaitingToStart_ptr = moduleBase.add(RVA.MatchState_WaitingToStart);
var MatchState_PokerGame_ptr = moduleBase.add(RVA.MatchState_PokerGame);
var MatchState_InProgress_ptr = moduleBase.add(RVA.MatchState_InProgress);

// NativeFunction bindings (WINDOWS_NATIVE_FUNCTION_PRELUDE enforces exceptions: 'propagate')
var UNetConnection_CleanUp = new NativeFunction(moduleBase.add(RVA.UNetConnection_CleanUp), 'void', ['pointer'], 'win64');
var FUniqueNetIdWrapper_ToString = new NativeFunction(moduleBase.add(RVA.FUniqueNetIdWrapper_ToString), 'pointer', ['pointer', 'pointer'], 'win64');
var FMemory_Free = new NativeFunction(moduleBase.add(RVA.FMemory_Free), 'void', ['pointer'], 'win64');
var FMemory_Realloc = new NativeFunction(moduleBase.add(RVA.FMemory_Realloc), 'pointer', ['pointer', 'uint64', 'uint32'], 'win64');
var AActor_GetWorldTimerManager = new NativeFunction(moduleBase.add(RVA.AActor_GetWorldTimerManager), 'pointer', ['pointer'], 'win64');
var FTimerManager_FindTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_FindTimer), 'pointer', ['pointer', 'pointer'], 'win64');
var FTimerManager_InternalClearTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_InternalClearTimer), 'void', ['pointer', 'uint64'], 'win64');
var AGameMode_HasMatchEnded = new NativeFunction(moduleBase.add(RVA.AGameMode_HasMatchEnded), 'uint8', ['pointer'], 'win64');
var AGameMode_OverridePlayerState = new NativeFunction(moduleBase.add(RVA.AGameMode_OverridePlayerState), 'void', ['pointer', 'pointer', 'pointer'], 'win64');
var AGameModeBase_MustSpectate = new NativeFunction(moduleBase.add(RVA.AGameModeBase_MustSpectate), 'uint8', ['pointer', 'pointer'], 'win64');
var APlayerState_GetPlayerName = new NativeFunction(moduleBase.add(RVA.APlayerState_GetPlayerName), 'pointer', ['pointer', 'pointer'], 'win64');
var APlayerState_SetIsInactive = new NativeFunction(moduleBase.add(RVA.APlayerState_SetIsInactive), 'void', ['pointer', 'uint8'], 'win64');
var APlayerState_SetUniqueId = new NativeFunction(moduleBase.add(RVA.APlayerState_SetUniqueId), 'void', ['pointer', 'pointer'], 'win64');
var AActor_SetOwner = new NativeFunction(moduleBase.add(RVA.AActor_SetOwner), 'void', ['pointer', 'pointer'], 'win64');
var AActor_Destroy = new NativeFunction(moduleBase.add(RVA.AActor_Destroy), 'uint8', ['pointer', 'uint8', 'uint8'], 'win64');
var APawn_SetPlayerState = new NativeFunction(moduleBase.add(RVA.APawn_SetPlayerState), 'void', ['pointer', 'pointer'], 'win64');
var ADH_Character_IsAlive = new NativeFunction(moduleBase.add(RVA.ADH_Character_IsAlive), 'uint8', ['pointer'], 'win64');
var ADH_PlayerController_SetDHPlayerState = new NativeFunction(moduleBase.add(RVA.ADH_PlayerController_SetDHPlayerState), 'void', ['pointer', 'pointer'], 'win64');

var ADH_GameMode_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_GameMode_GetPrivateStaticClass), 'pointer', [], 'win64');
var ADH_GameState_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_GameState_StaticClass), 'pointer', [], 'win64');
var ADH_PlayerState_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerState_GetPrivateStaticClass), 'pointer', [], 'win64');
var APlayerController_StaticClass = new NativeFunction(moduleBase.add(RVA.APlayerController_GetPrivateStaticClass), 'pointer', [], 'win64');
var ADH_PlayerController_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerController_GetPrivateStaticClass), 'pointer', [], 'win64');
var ADH_HumanCharacter_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_HumanCharacter_StaticClass), 'pointer', [], 'win64');

// Reusable scratch buffers (Game Thread only)
var scratchFStringBuf = Memory.alloc(16);
var emptySharedPtrBuf = Memory.alloc(16);
emptySharedPtrBuf.writeByteArray([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
var ZERO_16 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

// Cached UClass* pointers (lazily populated once UE4 classes are initialized)
var cachedDHGameModeClass = ptr(0);
var cachedDHGameStateClass = ptr(0);
var cachedDHPlayerStateClass = ptr(0);
var cachedPlayerControllerClass = ptr(0);
var cachedDHPlayerControllerClass = ptr(0);
var cachedDHHumanCharacterClass = ptr(0);

// Game-thread call-stack state
var currentLoginConnection = ptr(0);
var reconnectCapacityBypassDepth = 0;
var isCleaningUpStaleConnection = false;

// Opening-roster whitelist state (locked per ADH_GameMode instance on 30s poker start or match start)
var lockedRosterGameMode = ptr(0);
var matchRosterLocked = false;
var allowedUids = Object.create(null);
var allowedNames = Object.create(null);

function sanitizeLogValue(value) {
    if (!value || typeof value !== 'string') {
        return '-';
    }
    var cleaned = value.replace(/[\x00-\x1f\x7f]/g, ' ').trim();
    if (cleaned.length === 0) {
        return '-';
    }
    if (cleaned.length > 48) {
        return cleaned.slice(0, 45) + '...';
    }
    return cleaned;
}

function logEvent(message) {
    try {
        send(message);
    } catch (e) {}
}

function getDHGameModeClass() {
    if (cachedDHGameModeClass.isNull()) {
        cachedDHGameModeClass = ADH_GameMode_StaticClass();
    }
    return cachedDHGameModeClass;
}

function getDHGameStateClass() {
    if (cachedDHGameStateClass.isNull()) {
        cachedDHGameStateClass = ADH_GameState_StaticClass();
    }
    return cachedDHGameStateClass;
}

function getDHPlayerStateClass() {
    if (cachedDHPlayerStateClass.isNull()) {
        cachedDHPlayerStateClass = ADH_PlayerState_StaticClass();
    }
    return cachedDHPlayerStateClass;
}

function getPlayerControllerClass() {
    if (cachedPlayerControllerClass.isNull()) {
        cachedPlayerControllerClass = APlayerController_StaticClass();
    }
    return cachedPlayerControllerClass;
}

function getDHPlayerControllerClass() {
    if (cachedDHPlayerControllerClass.isNull()) {
        cachedDHPlayerControllerClass = ADH_PlayerController_StaticClass();
    }
    return cachedDHPlayerControllerClass;
}

function getDHHumanCharacterClass() {
    if (cachedDHHumanCharacterClass.isNull()) {
        cachedDHHumanCharacterClass = ADH_HumanCharacter_StaticClass();
    }
    return cachedDHHumanCharacterClass;
}

function isA(objPtr, targetClassPtr) {
    if (objPtr.isNull() || targetClassPtr.isNull()) {
        return false;
    }
    var objClass = objPtr.add(OFF.uObjectClassPrivate).readPointer();
    if (objClass.isNull()) {
        return false;
    }
    var targetDepth = targetClassPtr.add(OFF.uStructChainDepth).readS32();
    var objDepth = objClass.add(OFF.uStructChainDepth).readS32();
    if (targetDepth < 0 || targetDepth > objDepth) {
        return false;
    }
    var chainArray = objClass.add(OFF.uStructChainArray).readPointer();
    if (chainArray.isNull()) {
        return false;
    }
    return chainArray.add(targetDepth * 8).readPointer().equals(targetClassPtr.add(OFF.uStructChainArray));
}

function isPendingKill(objPtr) {
    if (objPtr.isNull()) {
        return true;
    }
    var internalIndex = objPtr.add(OFF.uObjectInternalIndex).readS32();
    var numElements = GUObjectArray_ptr.add(0x24).readS32();
    if (internalIndex < 0 || internalIndex >= numElements) {
        return true;
    }
    var objectsPtr = GUObjectArray_ptr.add(0x10).readPointer();
    if (objectsPtr.isNull()) {
        return true;
    }
    var numChunks = GUObjectArray_ptr.add(0x2C).readS32();
    var chunkIndex = internalIndex >> 16;
    if (chunkIndex < 0 || chunkIndex >= numChunks) {
        return true;
    }
    var withinChunkIndex = internalIndex & 0xFFFF;
    var chunkPtr = objectsPtr.add(chunkIndex * 8).readPointer();
    if (chunkPtr.isNull()) {
        return true;
    }
    var itemPtr = chunkPtr.add(withinChunkIndex * 24);
    if (!itemPtr.readPointer().equals(objPtr)) {
        return true;
    }
    var flags = itemPtr.add(8).readU32();
    return ((flags >>> 29) & 1) !== 0;
}

function readFStringNonOwning(fstrPtr) {
    if (fstrPtr.isNull()) {
        return '';
    }
    var dataPtr = fstrPtr.readPointer();
    var num = fstrPtr.add(8).readS32();
    if (dataPtr.isNull() || num <= 1 || num >= 8192) {
        return '';
    }
    try {
        return dataPtr.readUtf16String(num - 1);
    } catch (e) {
        return '';
    }
}

function readAndFreeFString(fstrPtr) {
    var dataPtr = fstrPtr.readPointer();
    var num = fstrPtr.add(8).readS32();
    var out = '';
    try {
        if (!dataPtr.isNull() && num > 1 && num < 4096) {
            out = dataPtr.readUtf16String(num - 1);
        }
    } catch (e) {
        out = '';
    } finally {
        if (!dataPtr.isNull()) {
            FMemory_Free(dataPtr);
        }
    }
    return out;
}

function writeUEFString(fstrPtr, text) {
    if (fstrPtr.isNull()) {
        return;
    }
    var oldData = fstrPtr.readPointer();
    var charCount = text.length + 1;
    var byteSize = charCount * 2;
    var newData = FMemory_Realloc(oldData, byteSize, 0);
    if (newData.isNull()) {
        return;
    }
    newData.writeUtf16String(text);
    fstrPtr.writePointer(newData);
    fstrPtr.add(8).writeS32(charCount);
    fstrPtr.add(12).writeS32(charCount);
}

function parseOptionName(optionsFStringPtr) {
    var raw = readFStringNonOwning(optionsFStringPtr);
    if (raw.length === 0) {
        return '';
    }
    var parts = raw.split('?');
    for (var i = 0; i < parts.length; i++) {
        var part = parts[i];
        var eq = part.indexOf('=');
        if (eq <= 0) {
            continue;
        }
        if (part.slice(0, eq).toLowerCase() === 'name') {
            var val = part.slice(eq + 1);
            try {
                val = decodeURIComponent(val);
            } catch (e) {}
            return val.trim().toLowerCase();
        }
    }
    return '';
}

function normalizeUidRaw(raw) {
    if (!raw || typeof raw !== 'string') {
        return '';
    }
    var trimmed = raw.trim();
    if (trimmed.length === 0) {
        return '';
    }
    var lower = trimmed.toLowerCase();
    if (lower === 'invalid') {
        return '';
    }
    return lower;
}

function getNormalizedUniqueIdString(uniqueIdWrapperPtr) {
    if (uniqueIdWrapperPtr.isNull()) {
        return '';
    }
    var sharedObj = uniqueIdWrapperPtr.add(8).readPointer();
    if (sharedObj.isNull()) {
        return '';
    }
    scratchFStringBuf.writeByteArray(ZERO_16);
    FUniqueNetIdWrapper_ToString(uniqueIdWrapperPtr, scratchFStringBuf);
    return normalizeUidRaw(readAndFreeFString(scratchFStringBuf));
}

function getNormalizedPlayerName(playerStatePtr) {
    if (playerStatePtr.isNull()) {
        return '';
    }
    scratchFStringBuf.writeByteArray(ZERO_16);
    APlayerState_GetPlayerName(playerStatePtr, scratchFStringBuf);
    return readAndFreeFString(scratchFStringBuf).trim().toLowerCase();
}

function isMatchWaitingToStart(gameModePtr) {
    if (gameModePtr.isNull()) {
        return true;
    }
    var currentMatchState = gameModePtr.add(OFF.gameModeMatchState).readU64();
    var waitingToStart = MatchState_WaitingToStart_ptr.readU64();
    return currentMatchState.equals(waitingToStart);
}

function isMatchEnded(gameModePtr) {
    if (gameModePtr.isNull()) {
        return false;
    }
    return (AGameMode_HasMatchEnded(gameModePtr) & 1) !== 0;
}

function getMatchStateLabel(gameModePtr) {
    if (gameModePtr.isNull()) {
        return 'Unknown';
    }
    if (isMatchEnded(gameModePtr)) {
        return 'Ended';
    }
    var current = gameModePtr.add(OFF.gameModeMatchState).readU64();
    if (current.equals(MatchState_WaitingToStart_ptr.readU64())) {
        return 'WaitingToStart';
    }
    if (current.equals(MatchState_PokerGame_ptr.readU64())) {
        return 'PokerGame';
    }
    if (current.equals(MatchState_InProgress_ptr.readU64())) {
        return 'InProgress';
    }
    return 'Other';
}

function resetRosterState(gameModePtr) {
    lockedRosterGameMode = gameModePtr;
    matchRosterLocked = false;
    allowedUids = Object.create(null);
    allowedNames = Object.create(null);
}

function countObjectKeys(obj) {
    var count = 0;
    for (var k in obj) {
        if (obj[k] === true) {
            count += 1;
        }
    }
    return count;
}

function lockMatchRoster(gameModePtr, reasonLabel) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || !isA(gameModePtr, getDHGameModeClass())) {
        return;
    }
    if (matchRosterLocked && lockedRosterGameMode.equals(gameModePtr)) {
        return;
    }
    resetRosterState(gameModePtr);
    matchRosterLocked = true;

    // 1. Snapshot all current PlayerState entries from GameState->PlayerArray
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (!gameState.isNull() && !isPendingKill(gameState)) {
        var arrPtr = gameState.add(OFF.gameStatePlayerArray);
        var dataPtr = arrPtr.readPointer();
        var count = arrPtr.add(8).readS32();
        if (!dataPtr.isNull() && count > 0 && count <= MAX_PLAYER_ARRAY) {
            for (var i = 0; i < count; i++) {
                var ps = dataPtr.add(i * 8).readPointer();
                if (ps.isNull() || isPendingKill(ps)) {
                    continue;
                }
                var psUid = getNormalizedUniqueIdString(ps.add(OFF.playerStateUniqueId));
                if (psUid.length > 0) {
                    allowedUids[psUid] = true;
                }
                var psName = getNormalizedPlayerName(ps);
                if (psName.length > 0) {
                    allowedNames[psName] = true;
                }
            }
        }

        // 2. Snapshot ExpectedPlayerIDs (+0x420) on ADH_GameState
        if (isA(gameState, getDHGameStateClass())) {
            var expArrPtr = gameState.add(OFF.gameStateExpectedPlayerIds);
            var expData = expArrPtr.readPointer();
            var expCount = expArrPtr.add(8).readS32();
            if (!expData.isNull() && expCount > 0 && expCount <= MAX_PLAYER_ARRAY) {
                for (var e = 0; e < expCount; e++) {
                    var expUid = normalizeUidRaw(readFStringNonOwning(expData.add(e * 16)));
                    if (expUid.length > 0) {
                        allowedUids[expUid] = true;
                    }
                }
            }
        }
    }

    // 3. Snapshot any open UNetConnection still loading the map when the match starts
    var world = GWorld_ptr.readPointer();
    if (!world.isNull() && !isPendingKill(world)) {
        var netDriver = world.add(OFF.worldNetDriver).readPointer();
        if (!netDriver.isNull() && !isPendingKill(netDriver)) {
            var connsPtr = netDriver.add(OFF.netDriverClientConnections);
            var connData = connsPtr.readPointer();
            var connCount = connsPtr.add(8).readS32();
            if (!connData.isNull() && connCount > 0 && connCount <= MAX_CLIENT_CONNECTIONS) {
                for (var c = 0; c < connCount; c++) {
                    var conn = connData.add(c * 8).readPointer();
                    if (conn.isNull() || isPendingKill(conn)) {
                        continue;
                    }
                    var connState = conn.add(OFF.netConnState).readS32();
                    var loginState = conn.add(OFF.netConnClientLoginState).readS32();
                    if (connState === USOCK_CLOSED || loginState === CLIENT_LOGIN_STATE_CLEANED_UP) {
                        continue;
                    }
                    var connUid = getNormalizedUniqueIdString(conn.add(OFF.netConnPlayerId));
                    if (connUid.length > 0) {
                        allowedUids[connUid] = true;
                    }
                }
            }
        }
    }

    var uidTotal = countObjectKeys(allowedUids);
    var nameTotal = countObjectKeys(allowedNames);
    logEvent(
        '[開局ロック] ' + (reasonLabel || '試合開始') +
        'により開局メンバーを確定しました (UID: ' + uidTotal + '件 / 名前: ' + nameTotal +
        '件 / State: ' + getMatchStateLabel(gameModePtr) + ')'
    );
}

function isRosterEnforced(gameModePtr) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || !isA(gameModePtr, getDHGameModeClass())) {
        return false;
    }
    if (!lockedRosterGameMode.equals(gameModePtr)) {
        resetRosterState(gameModePtr);
    }
    if (matchRosterLocked) {
        return true;
    }
    if (!isMatchWaitingToStart(gameModePtr)) {
        lockMatchRoster(gameModePtr, 'MatchState遷移');
        return true;
    }
    return false;
}

function hasUnambiguousDisconnectedPlayerByName(gameModePtr, normalizedName) {
    if (gameModePtr.isNull() || normalizedName.length === 0) {
        return false;
    }
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (gameState.isNull() || isPendingKill(gameState)) {
        return false;
    }
    var arrPtr = gameState.add(OFF.gameStatePlayerArray);
    var dataPtr = arrPtr.readPointer();
    var count = arrPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0 || count > MAX_PLAYER_ARRAY) {
        return false;
    }
    var dhPsClass = getDHPlayerStateClass();
    var disconnectedMatchCount = 0;
    for (var i = 0; i < count; i++) {
        var ps = dataPtr.add(i * 8).readPointer();
        if (ps.isNull() || isPendingKill(ps) || !isA(ps, dhPsClass)) {
            continue;
        }
        var psName = getNormalizedPlayerName(ps);
        if (psName.length === 0 || psName !== normalizedName) {
            continue;
        }
        var isInactive = (ps.add(OFF.playerStateFlags).readU8() & PLAYER_STATE_INACTIVE_MASK) !== 0;
        var isDisc = ps.add(OFF.playerStateIsDisconnected).readU8() !== 0;
        if (!isInactive && !isDisc) {
            // An original player with this name is currently active; reject name spoofing
            return false;
        }
        disconnectedMatchCount += 1;
    }
    return disconnectedMatchCount === 1;
}

function checkMidMatchReconnectAllowance(gameModePtr, incomingUid, optionsPtr) {
    if (!isRosterEnforced(gameModePtr)) {
        return { allowed: true, enforced: false, reason: 'before_lock', incomingName: '' };
    }
    var incomingName = parseOptionName(optionsPtr);
    if (isMatchEnded(gameModePtr)) {
        return { allowed: false, enforced: true, reason: 'match_ended', incomingName: incomingName };
    }
    if (incomingUid.length > 0 && allowedUids[incomingUid] === true) {
        return { allowed: true, enforced: true, reason: 'uid', incomingName: incomingName };
    }
    if (incomingName.length > 0 && allowedNames[incomingName] === true) {
        if (hasUnambiguousDisconnectedPlayerByName(gameModePtr, incomingName)) {
            return { allowed: true, enforced: true, reason: 'name_disconnected', incomingName: incomingName };
        }
    }
    return { allowed: false, enforced: true, reason: 'not_in_roster', incomingName: incomingName };
}

function clearCloseServerTimer(gameModePtr) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr)) {
        return false;
    }
    if (!isA(gameModePtr, getDHGameModeClass())) {
        return false;
    }
    if (isMatchEnded(gameModePtr)) {
        return false;
    }
    var handlePtr = gameModePtr.add(OFF.gameModeCloseServerTimerHandle);
    var handleVal = handlePtr.readU64();
    if (handleVal.equals(0)) {
        return false;
    }
    var timerManager = AActor_GetWorldTimerManager(gameModePtr);
    if (timerManager.isNull()) {
        return false;
    }
    var timerData = FTimerManager_FindTimer(timerManager, handlePtr);
    if (!timerData.isNull()) {
        FTimerManager_InternalClearTimer(timerManager, handleVal);
        logEvent('[終了タイマー解除] プレイヤー再接続により10秒サーバー終了タイマーを解除しました');
    }
    handlePtr.writeU64(0);
    return !timerData.isNull();
}

function hasExistingPlayerStateByUid(gameModePtr, normalizedUid) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || normalizedUid.length === 0) {
        return false;
    }
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (gameState.isNull() || isPendingKill(gameState)) {
        return false;
    }
    var arrPtr = gameState.add(OFF.gameStatePlayerArray);
    var dataPtr = arrPtr.readPointer();
    var count = arrPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0 || count > MAX_PLAYER_ARRAY) {
        return false;
    }
    for (var i = 0; i < count; i++) {
        var ps = dataPtr.add(i * 8).readPointer();
        if (ps.isNull() || isPendingKill(ps)) {
            continue;
        }
        var psUid = getNormalizedUniqueIdString(ps.add(OFF.playerStateUniqueId));
        if (psUid.length > 0 && psUid === normalizedUid) {
            return true;
        }
    }
    return false;
}

function hasAnyActivePlayerOrConnection(gameModePtr) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr)) {
        return false;
    }
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (!gameState.isNull() && !isPendingKill(gameState)) {
        var arrPtr = gameState.add(OFF.gameStatePlayerArray);
        var dataPtr = arrPtr.readPointer();
        var count = arrPtr.add(8).readS32();
        if (!dataPtr.isNull() && count > 0 && count <= MAX_PLAYER_ARRAY) {
            for (var i = 0; i < count; i++) {
                var ps = dataPtr.add(i * 8).readPointer();
                if (ps.isNull() || isPendingKill(ps)) {
                    continue;
                }
                var flags = ps.add(OFF.playerStateFlags).readU8();
                if ((flags & PLAYER_STATE_INACTIVE_MASK) === 0) {
                    return true;
                }
            }
        }
    }
    var world = GWorld_ptr.readPointer();
    if (!world.isNull() && !isPendingKill(world)) {
        var netDriver = world.add(OFF.worldNetDriver).readPointer();
        if (!netDriver.isNull() && !isPendingKill(netDriver)) {
            var connsPtr = netDriver.add(OFF.netDriverClientConnections);
            var connData = connsPtr.readPointer();
            var connCount = connsPtr.add(8).readS32();
            if (!connData.isNull() && connCount > 0 && connCount <= MAX_CLIENT_CONNECTIONS) {
                for (var j = 0; j < connCount; j++) {
                    var conn = connData.add(j * 8).readPointer();
                    if (conn.isNull() || isPendingKill(conn)) {
                        continue;
                    }
                    var connState = conn.add(OFF.netConnState).readS32();
                    var loginState = conn.add(OFF.netConnClientLoginState).readS32();
                    if (connState !== USOCK_CLOSED && loginState !== CLIENT_LOGIN_STATE_CLEANED_UP) {
                        return true;
                    }
                }
            }
        }
    }
    return false;
}

function cleanupStaleConnectionsForUid(gameModePtr, incomingUid, excludeConnPtr) {
    // Strictly require a non-null excludeConnPtr so we can never tear down the incoming connection itself.
    if (isCleaningUpStaleConnection || incomingUid.length === 0 || excludeConnPtr.isNull()) {
        return 0;
    }
    var world = GWorld_ptr.readPointer();
    if (world.isNull() || isPendingKill(world)) {
        return 0;
    }
    var netDriver = world.add(OFF.worldNetDriver).readPointer();
    if (netDriver.isNull() || isPendingKill(netDriver)) {
        return 0;
    }
    var connsPtr = netDriver.add(OFF.netDriverClientConnections);
    var connData = connsPtr.readPointer();
    var connCount = connsPtr.add(8).readS32();
    if (connData.isNull() || connCount <= 0 || connCount > MAX_CLIENT_CONNECTIONS) {
        return 0;
    }

    var staleConns = [];
    for (var i = 0; i < connCount; i++) {
        var conn = connData.add(i * 8).readPointer();
        if (conn.isNull() || isPendingKill(conn)) {
            continue;
        }
        if (conn.equals(excludeConnPtr)) {
            continue;
        }
        var loginState = conn.add(OFF.netConnClientLoginState).readS32();
        if (loginState === CLIENT_LOGIN_STATE_CLEANED_UP) {
            continue;
        }
        var connUid = getNormalizedUniqueIdString(conn.add(OFF.netConnPlayerId));
        if (connUid.length > 0 && connUid === incomingUid) {
            staleConns.push(conn);
        }
    }

    if (staleConns.length === 0) {
        return 0;
    }

    isCleaningUpStaleConnection = true;
    var cleaned = 0;
    try {
        for (var k = 0; k < staleConns.length; k++) {
            var targetConn = staleConns[k];
            if (targetConn.isNull() || isPendingKill(targetConn)) {
                continue;
            }
            if (targetConn.add(OFF.netConnClientLoginState).readS32() === CLIENT_LOGIN_STATE_CLEANED_UP) {
                continue;
            }
            UNetConnection_CleanUp(targetConn);
            cleaned += 1;
        }
    } finally {
        isCleaningUpStaleConnection = false;
    }

    // Cleaning up the last zombie connection can arm AddInactivePlayer's 10s CloseServer timer.
    if (cleaned > 0) {
        if (!gameModePtr.isNull()) {
            clearCloseServerTimer(gameModePtr);
        }
        logEvent(
            '[ゾンビ切断整理] 同一UIDの古い接続を ' + cleaned + '件 クリーンアップしました (UID: ' +
            sanitizeLogValue(incomingUid) + ')'
        );
    }
    return cleaned;
}

function cleanupStaleControllersForCandidatePlayerState(gameModePtr, newPC) {
    if (isCleaningUpStaleConnection || gameModePtr.isNull() || newPC.isNull() || isPendingKill(newPC)) {
        return 0;
    }
    if (!isA(gameModePtr, getDHGameModeClass()) || isMatchWaitingToStart(gameModePtr) || isMatchEnded(gameModePtr)) {
        return 0;
    }
    if ((AGameModeBase_MustSpectate(gameModePtr, newPC) & 1) !== 0) {
        return 0;
    }
    var newPS = newPC.add(OFF.controllerPlayerState).readPointer();
    if (newPS.isNull() || isPendingKill(newPS)) {
        return 0;
    }
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (gameState.isNull() || isPendingKill(gameState)) {
        return 0;
    }
    var arrPtr = gameState.add(OFF.gameStatePlayerArray);
    var dataPtr = arrPtr.readPointer();
    var count = arrPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0 || count > MAX_PLAYER_ARRAY) {
        return 0;
    }

    var dhPsClass = getDHPlayerStateClass();
    var incomingUid = getNormalizedUniqueIdString(newPS.add(OFF.playerStateUniqueId));
    var incomingName = getNormalizedPlayerName(newPS);
    var incomingNetAddr = readFStringNonOwning(newPS.add(OFF.playerStateSavedNetworkAddress)).trim().toLowerCase();
    var candidatePS = ptr(0);

    // 1. Exact/case-insensitive UniqueId match
    if (incomingUid.length > 0) {
        for (var i = 0; i < count; i++) {
            var psUidCand = dataPtr.add(i * 8).readPointer();
            if (psUidCand.isNull() || psUidCand.equals(newPS) || isPendingKill(psUidCand) || !isA(psUidCand, dhPsClass)) {
                continue;
            }
            var candUid = getNormalizedUniqueIdString(psUidCand.add(OFF.playerStateUniqueId));
            if (candUid.length > 0 && candUid === incomingUid) {
                candidatePS = psUidCand;
                break;
            }
        }
    } else if (incomingNetAddr.length > 0 && incomingName.length > 0) {
        // 2. Vanilla invalid-UID SavedNetworkAddress + PlayerName match
        for (var k = 0; k < count; k++) {
            var psAddrCand = dataPtr.add(k * 8).readPointer();
            if (psAddrCand.isNull() || psAddrCand.equals(newPS) || isPendingKill(psAddrCand) || !isA(psAddrCand, dhPsClass)) {
                continue;
            }
            var candAddr = readFStringNonOwning(psAddrCand.add(OFF.playerStateSavedNetworkAddress)).trim().toLowerCase();
            if (candAddr.length === 0 || candAddr !== incomingNetAddr) {
                continue;
            }
            var candName = getNormalizedPlayerName(psAddrCand);
            if (candName.length > 0 && candName === incomingName) {
                candidatePS = psAddrCand;
                break;
            }
        }
    }

    // 3. Unambiguous single inactive/disconnected PlayerName match (roster-checked, never kicks active players)
    if (candidatePS.isNull() && incomingName.length > 0 && allowedNames[incomingName] === true) {
        var nameMatches = [];
        var activeNameConflict = false;
        for (var m = 0; m < count; m++) {
            var psNameCand = dataPtr.add(m * 8).readPointer();
            if (psNameCand.isNull() || psNameCand.equals(newPS) || isPendingKill(psNameCand) || !isA(psNameCand, dhPsClass)) {
                continue;
            }
            var psNm = getNormalizedPlayerName(psNameCand);
            if (psNm.length === 0 || psNm !== incomingName) {
                continue;
            }
            var isInact = (psNameCand.add(OFF.playerStateFlags).readU8() & PLAYER_STATE_INACTIVE_MASK) !== 0;
            var isDisc = psNameCand.add(OFF.playerStateIsDisconnected).readU8() !== 0;
            if (!isInact && !isDisc) {
                activeNameConflict = true;
                break;
            }
            nameMatches.push(psNameCand);
        }
        if (!activeNameConflict && nameMatches.length === 1) {
            candidatePS = nameMatches[0];
        }
    }

    if (candidatePS.isNull()) {
        return 0;
    }

    var pcClass = getPlayerControllerClass();
    var dhPcClass = getDHPlayerControllerClass();
    var newConn = isA(newPC, pcClass) ? newPC.add(OFF.playerControllerNetConnection).readPointer() : ptr(0);
    var activeLoginConn = currentLoginConnection;

    function isIncomingConnection(connPtr) {
        if (connPtr.isNull()) {
            return false;
        }
        if (!newConn.isNull() && connPtr.equals(newConn)) {
            return true;
        }
        if (!activeLoginConn.isNull() && connPtr.equals(activeLoginConn)) {
            return true;
        }
        return false;
    }

    var staleConns = [];
    var staleOrphanPCs = [];
    var seenConns = Object.create(null);
    var seenPCs = Object.create(null);

    function recordStalePC(pcPtr) {
        if (pcPtr.isNull() || pcPtr.equals(newPC) || isPendingKill(pcPtr) || !isA(pcPtr, pcClass)) {
            return;
        }
        var pcPS = pcPtr.add(OFF.controllerPlayerState).readPointer();
        var pcDHPS = isA(pcPtr, dhPcClass) ? pcPtr.add(OFF.dhControllerPlayerState).readPointer() : ptr(0);
        if (!pcPS.equals(candidatePS) && !pcDHPS.equals(candidatePS)) {
            return;
        }
        var pcConn = pcPtr.add(OFF.playerControllerNetConnection).readPointer();
        if (!pcConn.isNull() && !isPendingKill(pcConn)) {
            if (isIncomingConnection(pcConn)) {
                return;
            }
            if (pcConn.add(OFF.netConnClientLoginState).readS32() !== CLIENT_LOGIN_STATE_CLEANED_UP) {
                var connKey = pcConn.toString();
                if (!seenConns[connKey]) {
                    seenConns[connKey] = true;
                    staleConns.push(pcConn);
                }
                return;
            }
        }
        var pcKey = pcPtr.toString();
        if (!seenPCs[pcKey]) {
            seenPCs[pcKey] = true;
            staleOrphanPCs.push(pcPtr);
        }
    }

    // Check current Owner of candidatePS
    recordStalePC(candidatePS.add(OFF.actorOwner).readPointer());

    // Scan NetDriver->ClientConnections for any stale connection whose PC still points at candidatePS
    var world = GWorld_ptr.readPointer();
    if (!world.isNull() && !isPendingKill(world)) {
        var netDriver = world.add(OFF.worldNetDriver).readPointer();
        if (!netDriver.isNull() && !isPendingKill(netDriver)) {
            var connsPtr = netDriver.add(OFF.netDriverClientConnections);
            var connData = connsPtr.readPointer();
            var connCount = connsPtr.add(8).readS32();
            if (!connData.isNull() && connCount > 0 && connCount <= MAX_CLIENT_CONNECTIONS) {
                for (var c = 0; c < connCount; c++) {
                    var conn = connData.add(c * 8).readPointer();
                    if (conn.isNull() || isPendingKill(conn)) {
                        continue;
                    }
                    if (isIncomingConnection(conn)) {
                        continue;
                    }
                    if (conn.add(OFF.netConnClientLoginState).readS32() === CLIENT_LOGIN_STATE_CLEANED_UP) {
                        continue;
                    }
                    var connPC = conn.add(OFF.netConnPlayerController).readPointer();
                    var connOwner = conn.add(OFF.netConnOwningActor).readPointer();
                    if (!connPC.isNull()) {
                        recordStalePC(connPC);
                    }
                    if (!connOwner.isNull() && !connOwner.equals(connPC)) {
                        recordStalePC(connOwner);
                    }
                }
            }
        }
    }

    if (staleConns.length === 0 && staleOrphanPCs.length === 0) {
        return 0;
    }

    isCleaningUpStaleConnection = true;
    var cleaned = 0;
    try {
        for (var sc = 0; sc < staleConns.length; sc++) {
            var targetConn = staleConns[sc];
            if (targetConn.isNull() || isPendingKill(targetConn)) {
                continue;
            }
            if (targetConn.add(OFF.netConnClientLoginState).readS32() === CLIENT_LOGIN_STATE_CLEANED_UP) {
                continue;
            }
            UNetConnection_CleanUp(targetConn);
            cleaned += 1;
        }
        for (var sp = 0; sp < staleOrphanPCs.length; sp++) {
            var orphanPC = staleOrphanPCs[sp];
            if (orphanPC.isNull() || isPendingKill(orphanPC)) {
                continue;
            }
            AActor_Destroy(orphanPC, 0, 1);
            cleaned += 1;
        }
    } finally {
        isCleaningUpStaleConnection = false;
    }

    if (cleaned > 0) {
        clearCloseServerTimer(gameModePtr);
        logEvent(
            '[ゾンビPC整理] 再紐付け対象の旧Controller/接続を ' + cleaned + '件 事前整理しました (Name: ' +
            sanitizeLogValue(incomingName) + ')'
        );
    }
    return cleaned;
}

// ============================================================================
// Hook 0: Lock opening match roster when poker completes & match starts
//   - Primary triggers: RandomizeThralls & HandleMatchHasStarted (plus
//     !WaitingToStart MatchState fallback inside isRosterEnforced)
//   - Allows slow-connecting opening teammates to finish joining during the
//     WaitingToStart / poker phase, then blocks all outsiders once match starts.
//   Uses onEnter only (0 onLeave hooks added to these RVAs).
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_RandomizeThralls), {
    onEnter: function (args) {
        try {
            lockMatchRoster(args[0], '役職確定(RandomizeThralls)');
        } catch (e) {}
    }
});

Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_HandleMatchHasStarted), {
    onEnter: function (args) {
        try {
            lockMatchRoster(args[0], '試合開始(HandleMatchHasStarted)');
        } catch (e) {}
    }
});

// ============================================================================
// Hook 1: UWorld::NotifyControlMessage (capture incoming UNetConnection on NMT_Login / NMT_JoinSplit)
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.UWorld_NotifyControlMessage), {
    onEnter: function (args) {
        this.savedLoginConn = false;
        try {
            var msgType = args[2].toInt32() & 0xFF;
            if (msgType === NMT_LOGIN || msgType === NMT_JOIN_SPLIT) {
                this.prevLoginConn = currentLoginConnection;
                currentLoginConnection = args[1];
                this.savedLoginConn = true;
            }
        } catch (e) {}
    },
    onLeave: function () {
        if (this.savedLoginConn) {
            currentLoginConnection = this.prevLoginConn || ptr(0);
        }
    }
});

// ============================================================================
// Hook 2: AGameModeBase::PreLogin
//   - Reject non-opening-roster outsiders once the 30s poker timer or match has started
//   - Proactively clean up any stale UNetConnection carrying the same UniqueId
//   - Enable AtCapacity bypass for returning opening-roster participants
//   - Cancel empty-server 10s CloseServer timer
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.AGameModeBase_PreLogin), {
    onEnter: function (args) {
        this.pushedBypass = false;
        this.rejectOutsider = false;
        this.enforced = false;
        this.incomingUid = '';
        this.incomingName = '';
        this.errorMsgPtr = ptr(0);
        try {
            var gameMode = args[0];
            var optionsPtr = args[1];
            var uniqueIdPtr = args[3];
            this.errorMsgPtr = args[4];
            if (gameMode.isNull() || !isA(gameMode, getDHGameModeClass())) {
                return;
            }
            var incomingUid = getNormalizedUniqueIdString(uniqueIdPtr);
            this.incomingUid = incomingUid;
            var check = checkMidMatchReconnectAllowance(gameMode, incomingUid, optionsPtr);
            this.enforced = check.enforced;
            this.incomingName = check.incomingName;

            if (check.enforced) {
                if (!check.allowed) {
                    this.rejectOutsider = true;
                    logEvent(
                        '[接続拒否] 開局外プレイヤーを拒否しました (PreLogin / 理由: ' + check.reason +
                        ' / UID: ' + sanitizeLogValue(incomingUid) +
                        ' / Name: ' + sanitizeLogValue(check.incomingName) +
                        ' / State: ' + getMatchStateLabel(gameMode) + ')'
                    );
                    return;
                }
                logEvent(
                    '[再接続許可] 開局メンバーのPreLoginを受け入れました (一致: ' + check.reason +
                    ' / UID: ' + sanitizeLogValue(incomingUid) +
                    ' / Name: ' + sanitizeLogValue(check.incomingName) +
                    ' / State: ' + getMatchStateLabel(gameMode) + ')'
                );
            }

            if (!isMatchEnded(gameMode)) {
                clearCloseServerTimer(gameMode);
            }
            if (incomingUid.length > 0) {
                cleanupStaleConnectionsForUid(gameMode, incomingUid, currentLoginConnection);
            }
            if (!isMatchEnded(gameMode)) {
                if (check.enforced || hasExistingPlayerStateByUid(gameMode, incomingUid)) {
                    reconnectCapacityBypassDepth += 1;
                    this.pushedBypass = true;
                }
            }
        } catch (e) {}
    },
    onLeave: function () {
        try {
            if (this.pushedBypass && reconnectCapacityBypassDepth > 0) {
                reconnectCapacityBypassDepth -= 1;
            }
            if (this.rejectOutsider && !this.errorMsgPtr.isNull()) {
                var existingLen = this.errorMsgPtr.add(8).readS32();
                if (existingLen <= 1) {
                    writeUEFString(this.errorMsgPtr, REJECT_MID_MATCH_MESSAGE);
                }
            } else if (this.enforced && !this.errorMsgPtr.isNull()) {
                var ueErr = readFStringNonOwning(this.errorMsgPtr);
                if (ueErr.length > 0) {
                    logEvent(
                        '[PreLoginエラー] UE4側で接続が拒否されました (理由: ' + sanitizeLogValue(ueErr) +
                        ' / UID: ' + sanitizeLogValue(this.incomingUid) + ')'
                    );
                }
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 3: ADH_GameMode::Login
//   - Belt-and-suspenders mid-match outsider guard
//   - Re-check stale connection & AtCapacity bypass for the second ApproveLogin
//     call inside AGameModeBase::Login
//   - Cancel empty-server 10s CloseServer timer
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_Login), {
    onEnter: function (args) {
        this.pushedBypass = false;
        this.savedLoginConn = false;
        this.rejectOutsider = false;
        this.enforced = false;
        this.incomingUid = '';
        this.errorMsgPtr = ptr(0);
        try {
            var gameMode = args[0];
            var newPlayerConn = args[1];
            var optionsPtr = args[4];
            var uniqueIdPtr = args[5];
            this.errorMsgPtr = args[6];
            if (!newPlayerConn.isNull()) {
                this.prevLoginConn = currentLoginConnection;
                currentLoginConnection = newPlayerConn;
                this.savedLoginConn = true;
            }
            if (gameMode.isNull()) {
                return;
            }
            var incomingUid = getNormalizedUniqueIdString(uniqueIdPtr);
            this.incomingUid = incomingUid;
            var check = checkMidMatchReconnectAllowance(gameMode, incomingUid, optionsPtr);
            this.enforced = check.enforced;

            if (check.enforced && !check.allowed) {
                this.rejectOutsider = true;
                logEvent(
                    '[接続拒否] 開局外プレイヤーを拒否しました (Login / 理由: ' + check.reason +
                    ' / UID: ' + sanitizeLogValue(incomingUid) +
                    ' / Name: ' + sanitizeLogValue(check.incomingName) + ')'
                );
                return;
            }

            if (!isMatchEnded(gameMode)) {
                clearCloseServerTimer(gameMode);
            }
            if (incomingUid.length > 0) {
                cleanupStaleConnectionsForUid(gameMode, incomingUid, newPlayerConn);
            }
            if (!isMatchEnded(gameMode)) {
                if (check.enforced || hasExistingPlayerStateByUid(gameMode, incomingUid)) {
                    reconnectCapacityBypassDepth += 1;
                    this.pushedBypass = true;
                }
            }
        } catch (e) {}
    },
    onLeave: function (retval) {
        try {
            if (this.savedLoginConn) {
                currentLoginConnection = this.prevLoginConn || ptr(0);
            }
            if (this.pushedBypass && reconnectCapacityBypassDepth > 0) {
                reconnectCapacityBypassDepth -= 1;
            }
            if (this.rejectOutsider) {
                if (!retval.isNull() && !isPendingKill(retval)) {
                    AActor_Destroy(retval, 0, 1);
                }
                if (!this.errorMsgPtr.isNull() && this.errorMsgPtr.add(8).readS32() <= 1) {
                    writeUEFString(this.errorMsgPtr, REJECT_MID_MATCH_MESSAGE);
                }
                retval.replace(ptr(0));
            } else if (this.enforced && retval.isNull()) {
                var loginErr = !this.errorMsgPtr.isNull() ? readFStringNonOwning(this.errorMsgPtr) : '';
                logEvent(
                    '[Loginエラー] プレイヤー生成に失敗しました (理由: ' + sanitizeLogValue(loginErr) +
                    ' / UID: ' + sanitizeLogValue(this.incomingUid) + ')'
                );
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 4: AGameSession::AtCapacity
//   - Never reject a reconnecting opening-roster participant with "Server full."
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.AGameSession_AtCapacity), {
    onEnter: function (args) {
        this.bSpectator = (args[1].toInt32() & 1) !== 0;
    },
    onLeave: function (retval) {
        try {
            if (!this.bSpectator && reconnectCapacityBypassDepth > 0) {
                retval.replace(ptr(0));
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 5: AGameMode::PostLogin
//   - Always cancel empty-server 10s CloseServer timer after player login
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.AGameMode_PostLogin), {
    onEnter: function (args) {
        try {
            var gameMode = args[0];
            if (!gameMode.isNull() && isA(gameMode, getDHGameModeClass()) && !isMatchEnded(gameMode)) {
                clearCloseServerTimer(gameMode);
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 6: ADH_GameMode::FindInactivePlayer
//   - onEnter: proactively tear down any lingering zombie OldPC/OldConn still
//     referencing the target PlayerState (prevents delayed 60s ConnectionTimeout
//     from calling SetIsDisconnected(1) -> UnPossess/Destroy(NewPC) after
//     reconnect via IP+Name or UID mismatch!)
//   - onLeave: fallback recovery if vanilla exact UniqueId/IP match returned false:
//     (a) Case-insensitive UniqueId string match against PlayerArray
//     (b) Unambiguous single inactive/disconnected PlayerName match restricted
//         to locked opening-roster participants (handles IP / subsystem change)
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_FindInactivePlayer), {
    onEnter: function (args) {
        this.gameMode = args[0];
        this.newPC = args[1];
        try {
            cleanupStaleControllersForCandidatePlayerState(this.gameMode, this.newPC);
        } catch (e) {}
    },
    onLeave: function (retval) {
        try {
            var gameMode = this.gameMode;
            var newPC = this.newPC;
            if (gameMode.isNull() || isPendingKill(gameMode) || newPC.isNull() || isPendingKill(newPC)) {
                return;
            }
            if (!isA(gameMode, getDHGameModeClass()) || isMatchWaitingToStart(gameMode) || isMatchEnded(gameMode)) {
                return;
            }
            var vanillaMatched = (retval.toInt32() & 1) !== 0;
            var curPS = newPC.add(OFF.controllerPlayerState).readPointer();
            if (vanillaMatched) {
                var vUid = !curPS.isNull() ? getNormalizedUniqueIdString(curPS.add(OFF.playerStateUniqueId)) : '';
                var vName = !curPS.isNull() ? getNormalizedPlayerName(curPS) : '';
                logEvent(
                    '[状態復元] 既存PlayerStateを再紐付けしました (標準一致 / UID: ' +
                    sanitizeLogValue(vUid) + ' / Name: ' + sanitizeLogValue(vName) + ')'
                );
                return;
            }

            if ((AGameModeBase_MustSpectate(gameMode, newPC) & 1) !== 0) {
                return;
            }
            var newPS = curPS;
            if (newPS.isNull() || isPendingKill(newPS)) {
                return;
            }
            var gameState = gameMode.add(OFF.gameModeGameState).readPointer();
            if (gameState.isNull() || isPendingKill(gameState)) {
                return;
            }
            var arrPtr = gameState.add(OFF.gameStatePlayerArray);
            var dataPtr = arrPtr.readPointer();
            var count = arrPtr.add(8).readS32();
            if (dataPtr.isNull() || count <= 0 || count > MAX_PLAYER_ARRAY) {
                return;
            }

            isRosterEnforced(gameMode);

            var incomingUid = getNormalizedUniqueIdString(newPS.add(OFF.playerStateUniqueId));
            var incomingName = getNormalizedPlayerName(newPS);
            var dhPsClass = getDHPlayerStateClass();
            var matchedPS = ptr(0);
            var matchMode = '';

            // Pass 1: case-insensitive UniqueId string match (must belong to opening roster)
            if (incomingUid.length > 0 && allowedUids[incomingUid] === true) {
                for (var i = 0; i < count; i++) {
                    var cand = dataPtr.add(i * 8).readPointer();
                    if (cand.isNull() || cand.equals(newPS) || isPendingKill(cand) || !isA(cand, dhPsClass)) {
                        continue;
                    }
                    var candUid = getNormalizedUniqueIdString(cand.add(OFF.playerStateUniqueId));
                    if (candUid.length > 0 && candUid === incomingUid) {
                        matchedPS = cand;
                        matchMode = 'UID大文字小文字';
                        break;
                    }
                }
            }

            // Pass 2: unambiguous single inactive/disconnected exact PlayerName match (must belong to opening roster)
            if (matchedPS.isNull() && incomingName.length > 0 && allowedNames[incomingName] === true) {
                var nameMatches = [];
                var activeNameConflict = false;
                for (var j = 0; j < count; j++) {
                    var ps = dataPtr.add(j * 8).readPointer();
                    if (ps.isNull() || ps.equals(newPS) || isPendingKill(ps) || !isA(ps, dhPsClass)) {
                        continue;
                    }
                    var psName = getNormalizedPlayerName(ps);
                    if (psName.length === 0 || psName !== incomingName) {
                        continue;
                    }
                    var isInactive = (ps.add(OFF.playerStateFlags).readU8() & PLAYER_STATE_INACTIVE_MASK) !== 0;
                    var isDisc = ps.add(OFF.playerStateIsDisconnected).readU8() !== 0;
                    if (!isInactive && !isDisc) {
                        activeNameConflict = true;
                        break;
                    }
                    nameMatches.push(ps);
                }
                if (!activeNameConflict && nameMatches.length === 1) {
                    matchedPS = nameMatches[0];
                    matchMode = '名前一致';
                }
            }

            if (matchedPS.isNull()) {
                logEvent(
                    '[状態復元なし] 既存PlayerStateが見つかりませんでした (UID: ' +
                    sanitizeLogValue(incomingUid) + ' / Name: ' + sanitizeLogValue(incomingName) +
                    ' / PlayerArray: ' + count + '件)'
                );
                return;
            }

            // If a player legitimately recovered via Name fallback after UID change, register new UID too.
            if (incomingUid.length > 0) {
                allowedUids[incomingUid] = true;
            }

            // Mirror ADH_GameMode::FindInactivePlayer+0x21a..+0x330 swap sequence
            newPC.add(OFF.controllerPlayerState).writePointer(matchedPS);
            if (isA(newPC, getDHPlayerControllerClass())) {
                ADH_PlayerController_SetDHPlayerState(newPC, matchedPS);
            }
            AActor_SetOwner(matchedPS, newPC);
            APlayerState_SetIsInactive(matchedPS, 0);
            AGameMode_OverridePlayerState(gameMode, newPC, newPS);
            APlayerState_SetIsInactive(newPS, 1);
            APlayerState_SetUniqueId(newPS, emptySharedPtrBuf);
            AActor_Destroy(newPS, 0, 1);
            // Reset FWeakObjectPtr StartSpot to INDEX_NONE (-1, 0)
            newPC.add(OFF.controllerStartSpotWeak).writeS32(-1);
            newPC.add(OFF.controllerStartSpotWeak + 4).writeS32(0);

            logEvent(
                '[状態復元] フォールバック一致でPlayerStateを再紐付けしました (一致: ' + matchMode +
                ' / UID: ' + sanitizeLogValue(incomingUid) +
                ' / Name: ' + sanitizeLogValue(incomingName) + ')'
            );

            retval.replace(ptr(1));
        } catch (e) {}
    }
});

// ============================================================================
// Hook 7: ADH_GameMode::HandleStartingNewPlayer_Implementation
//   - If PawnPrivate (+0x280) is null/dead while RealPawn (+0x638) is still a
//     living pawn (e.g. Thrall disconnected while controlling a Doppelganger),
//     restore PawnPrivate = RealPawn so vanilla re-possesses their real body.
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_HandleStartingNewPlayer), {
    onEnter: function (args) {
        try {
            var gameMode = args[0];
            var newPC = args[1];
            if (newPC.isNull() || isPendingKill(newPC)) {
                return;
            }
            var ps = newPC.add(OFF.controllerPlayerState).readPointer();
            if (ps.isNull() || isPendingKill(ps) || !isA(ps, getDHPlayerStateClass())) {
                return;
            }
            if (ps.add(OFF.playerStateSelectedRole).readPointer().isNull()) {
                return;
            }
            if (ps.add(OFF.playerStateIsDead).readU8() !== 0) {
                return;
            }

            var humanClass = getDHHumanCharacterClass();
            var pawn = ps.add(OFF.playerStatePawnPrivate).readPointer();
            var pawnUsable = false;
            if (!pawn.isNull() && !isPendingKill(pawn)) {
                if (isA(pawn, humanClass)) {
                    pawnUsable = (ADH_Character_IsAlive(pawn) & 1) !== 0;
                } else {
                    pawnUsable = true;
                }
            }

            var restoredRealPawn = false;
            if (!pawnUsable) {
                var realPawn = ps.add(OFF.playerStateRealPawn).readPointer();
                if (!realPawn.isNull() && !isPendingKill(realPawn)) {
                    var realUsable = !isA(realPawn, humanClass) || (ADH_Character_IsAlive(realPawn) & 1) !== 0;
                    if (realUsable) {
                        APawn_SetPlayerState(realPawn, ps);
                        pawn = realPawn;
                        pawnUsable = true;
                        restoredRealPawn = true;
                    }
                }
            }

            if (!gameMode.isNull() && !isMatchWaitingToStart(gameMode)) {
                var psName = getNormalizedPlayerName(ps);
                if (pawnUsable) {
                    logEvent(
                        '[操作復帰] 既存キャラを再憑依します (Name: ' + sanitizeLogValue(psName) +
                        ' / RealPawn復元: ' + (restoredRealPawn ? 'あり' : 'なし') + ')'
                    );
                } else {
                    logEvent(
                        '[操作復帰] 生存Pawnなしのため標準リスポーン経路へ進みます (Name: ' +
                        sanitizeLogValue(psName) + ')'
                    );
                }
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 8: ADH_GameMode::CloseServer guard (pinned NativeCallback at top level)
//   - Prevents premature empty-server exit if any active player or client
//     connection exists while the match has not ended.
// ============================================================================
var closeServerAddr = moduleBase.add(RVA.ADH_GameMode_CloseServer);
var closeServerTrampoline = ptr(0);
var closeServerOrig = null;

var closeServerWrapper = new NativeCallback(function (gameModePtr) {
    try {
        if (!gameModePtr.isNull() && !isPendingKill(gameModePtr) && isA(gameModePtr, getDHGameModeClass())) {
            if (!isMatchEnded(gameModePtr) && hasAnyActivePlayerOrConnection(gameModePtr)) {
                logEvent('[終了抑止] アクティブ接続が存在するためCloseServerを抑止しました');
                return;
            }
        }
    } catch (e) {}
    if (closeServerOrig !== null) {
        closeServerOrig(gameModePtr);
    }
}, 'void', ['pointer'], 'win64');

closeServerTrampoline = Interceptor.replaceFast(closeServerAddr, closeServerWrapper);
closeServerOrig = new NativeFunction(closeServerTrampoline, 'void', ['pointer'], 'win64');
