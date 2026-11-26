'use strict';

// ============================================================================
// サーバー接続保護・開局名簿ガード (デフォルトパッチ)
//   1. 開局メンバーホワイトリスト(乱入防止):
//      - EnteringMap / WaitingToStart 中に認証成功した PreLogin/Login 参加者を
//        開局前履歴(preMatchSeen*)へ継続記録し、ポーカー前の途中切断・再接続でも
//        正規の開局メンバーを締め出さない。
//      - RandomizeThralls および HandleMatchHasStarted (並びに MatchState 遷移
//        フォールバック) で開局前履歴 + PlayerArray + ExpectedPlayerIDs +
//        有効な ClientConnections を和集合統合して確定し、試合開始後の開局外
//        プレイヤーの乱入を拒否する。
//   2. ゾンビUNetConnection/旧Controller整理: PreLogin/Login時に同一UIDまたは
//      同一IP再接続対象の古い接続を事前整理し、FindInactivePlayer入口で残存旧PC/
//      接続を破棄して60秒タイムアウト遅延による誤切断を防ぐ。
//   3. AGameSession::AtCapacity バイパス: 開局メンバー再接続時の満員拒否を回避。
//   4. 10秒空室CloseServerタイマー(+0x478)解除とアクティブ接続存在時の終了抑止。
// ============================================================================

var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

// PDB (DreadHungerServer-Win64-Shipping.pdb) で検証済みのRVA
var RVA = {
    GWorld: 0x46ED420,
    GUObjectArray: 0x45C40C8,
    MatchState_EnteringMap: 0x46CD148,
    MatchState_WaitingToStart: 0x46CD2A0,
    MatchState_PokerGame: 0x4500DB0,
    MatchState_InProgress: 0x46CD2A8,
    MatchState_WaitingPostMatch: 0x46CD2B0,
    MatchState_LeavingMap: 0x46CD2B8,
    MatchState_Aborted: 0x46CD2C0,

    UWorld_NotifyControlMessage: 0x296D350,
    AGameModeBase_PreLogin: 0x25709B0,
    AGameSession_AtCapacity: 0x2556F70,
    ADH_GameMode_Login: 0xDA0700,
    AGameMode_PostLogin: 0x256FEB0,
    ADH_GameMode_FindInactivePlayer: 0xD8F900,
    ADH_GameMode_RandomizeThralls: 0xDA4620,
    ADH_GameMode_HandleMatchHasStarted: 0xD98A50,
    ADH_GameMode_CloseServer: 0xD89800,
    AGameMode_HasMatchEnded: 0x2568770,
    AGameModeBase_MustSpectate: 0x29EA330,

    UNetConnection_CleanUp: 0x2670DF0,
    FUniqueNetIdWrapper_ToString: 0x894480,
    FMemory_Free: 0x105DD50,
    FMemory_Realloc: 0x106C3D0,
    AActor_GetWorldTimerManager: 0x22DDC30,
    FTimerManager_FindTimer: 0x28F2D80,
    FTimerManager_InternalClearTimer: 0x28F6A00,

    APlayerState_GetPlayerName: 0x27CAED0,
    AActor_Destroy: 0x22D1D40,

    ADH_GameMode_GetPrivateStaticClass: 0xEB5E20,
    ADH_GameState_StaticClass: 0xEBE030,
    ADH_PlayerState_GetPrivateStaticClass: 0xEED6B0,
    APlayerController_GetPrivateStaticClass: 0x2AA84E0,
    ADH_PlayerController_GetPrivateStaticClass: 0xEE2F70
};

var OFF = {
    uObjectClassPrivate: 0x10,
    uObjectInternalIndex: 0x0C,
    uStructChainArray: 0x30,
    uStructChainDepth: 0x38,

    actorOwner: 0xE0,

    worldNetDriver: 0x38,

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
    playerControllerNetConnection: 0x418,
    dhControllerPlayerState: 0x658,

    playerStateFlags: 0x22A,
    playerStateSavedNetworkAddress: 0x240,
    playerStateUniqueId: 0x250,
    playerStateIsDisconnected: 0x571
};

var NMT_LOGIN = 5;
var NMT_JOIN_SPLIT = 10;
var USOCK_CLOSED = 1;
var CLIENT_LOGIN_STATE_CLEANED_UP = 4;
var PLAYER_STATE_INACTIVE_MASK = 0x20;
var MAX_PLAYER_ARRAY = 64;
var MAX_CLIENT_CONNECTIONS = 128;
var MAX_PRELOGIN_ADDR_CACHE = 128;
var MAX_ADDRS_PER_NAME = 8;
var REJECT_MID_MATCH_MESSAGE = 'Match already in progress.';

// グローバルポインタ
var GWorld_ptr = moduleBase.add(RVA.GWorld);
var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
var MatchState_EnteringMap_ptr = moduleBase.add(RVA.MatchState_EnteringMap);
var MatchState_WaitingToStart_ptr = moduleBase.add(RVA.MatchState_WaitingToStart);
var MatchState_PokerGame_ptr = moduleBase.add(RVA.MatchState_PokerGame);
var MatchState_InProgress_ptr = moduleBase.add(RVA.MatchState_InProgress);
var MatchState_WaitingPostMatch_ptr = moduleBase.add(RVA.MatchState_WaitingPostMatch);
var MatchState_LeavingMap_ptr = moduleBase.add(RVA.MatchState_LeavingMap);
var MatchState_Aborted_ptr = moduleBase.add(RVA.MatchState_Aborted);

// ネイティブ関数バインディング (単独 Frida 16 互換のため exceptions: 'propagate' 明示)
var UNetConnection_CleanUp = new NativeFunction(moduleBase.add(RVA.UNetConnection_CleanUp), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var FUniqueNetIdWrapper_ToString = new NativeFunction(moduleBase.add(RVA.FUniqueNetIdWrapper_ToString), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var FMemory_Free = new NativeFunction(moduleBase.add(RVA.FMemory_Free), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var FMemory_Realloc = new NativeFunction(moduleBase.add(RVA.FMemory_Realloc), 'pointer', ['pointer', 'uint64', 'uint32'], { abi: 'win64', exceptions: 'propagate' });
var AActor_GetWorldTimerManager = new NativeFunction(moduleBase.add(RVA.AActor_GetWorldTimerManager), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var FTimerManager_FindTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_FindTimer), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var FTimerManager_InternalClearTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_InternalClearTimer), 'void', ['pointer', 'uint64'], { abi: 'win64', exceptions: 'propagate' });
var AGameMode_HasMatchEnded = new NativeFunction(moduleBase.add(RVA.AGameMode_HasMatchEnded), 'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var AGameModeBase_MustSpectate = new NativeFunction(moduleBase.add(RVA.AGameModeBase_MustSpectate), 'uint8', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var APlayerState_GetPlayerName = new NativeFunction(moduleBase.add(RVA.APlayerState_GetPlayerName), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var AActor_Destroy = new NativeFunction(moduleBase.add(RVA.AActor_Destroy), 'uint8', ['pointer', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' });

var ADH_GameMode_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_GameMode_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var ADH_GameState_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_GameState_StaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var ADH_PlayerState_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerState_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var APlayerController_StaticClass = new NativeFunction(moduleBase.add(RVA.APlayerController_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var ADH_PlayerController_StaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerController_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });

// 再利用スクラッチバッファ (ゲームスレッド専用)
var scratchFStringBuf = Memory.alloc(16);
var ZERO_16 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

// UClass* キャッシュ (UE4クラス初期化後に遅延取得)
var cachedDHGameModeClass = ptr(0);
var cachedDHGameStateClass = ptr(0);
var cachedDHPlayerStateClass = ptr(0);
var cachedPlayerControllerClass = ptr(0);
var cachedDHPlayerControllerClass = ptr(0);

// ゲームスレッド上の呼び出し状態
var currentLoginConnection = ptr(0);
var reconnectCapacityBypassDepth = 0;
var isCleaningUpStaleConnection = false;

// 開局名簿ホワイトリスト状態 (ADH_GameModeインスタンス単位で管理)
var lockedRosterGameMode = ptr(0);
var matchRosterLocked = false;
var preMatchSeenUids = Object.create(null);
var preMatchSeenNames = Object.create(null);
var preMatchSeenAddrsByName = Object.create(null);
var allowedUids = Object.create(null);
var allowedNames = Object.create(null);
var preLoginAddrByConn = Object.create(null);
var preLoginAddrCacheCount = 0;

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

function fnv1a32Hex(str) {
    var hash = 0x811c9dc5;
    for (var i = 0; i < str.length; i++) {
        hash ^= str.charCodeAt(i);
        hash = (hash + ((hash << 1) + (hash << 4) + (hash << 7) + (hash << 8) + (hash << 24))) >>> 0;
    }
    var hex = hash.toString(16);
    while (hex.length < 8) {
        hex = '0' + hex;
    }
    return hex;
}

// 診断ログ用: 生のSteamID/EOS ID全文を出さずに一致・形式・有効性だけ判別可能にする
function formatUidTag(normalizedUid) {
    if (!normalizedUid || typeof normalizedUid !== 'string' || normalizedUid.length === 0) {
        return 'empty';
    }
    var hasPipe = normalizedUid.indexOf('|') !== -1 ? 1 : 0;
    return 'ok#' + fnv1a32Hex(normalizedUid) + '(len=' + normalizedUid.length + ',pipe=' + hasPipe + ')';
}

function normalizeAddress(rawAddr) {
    if (!rawAddr || typeof rawAddr !== 'string') {
        return '';
    }
    var s = rawAddr.trim().toLowerCase();
    if (s.length === 0) {
        return '';
    }
    if (s.charAt(0) === '[') {
        var closeBracket = s.indexOf(']');
        if (closeBracket > 1) {
            return s.slice(1, closeBracket);
        }
    }
    var firstColon = s.indexOf(':');
    if (firstColon > 0 && s.indexOf(':', firstColon + 1) === -1) {
        return s.slice(0, firstColon);
    }
    return s;
}

function cachePreLoginAddress(connPtr, normalizedAddr) {
    if (connPtr.isNull() || normalizedAddr.length === 0) {
        return;
    }
    var key = connPtr.toString();
    if (preLoginAddrByConn[key] === undefined) {
        if (preLoginAddrCacheCount >= MAX_PRELOGIN_ADDR_CACHE) {
            preLoginAddrByConn = Object.create(null);
            preLoginAddrCacheCount = 0;
        }
        preLoginAddrCacheCount += 1;
    }
    preLoginAddrByConn[key] = normalizedAddr;
}

function getCachedPreLoginAddress(connPtr) {
    if (connPtr.isNull()) {
        return '';
    }
    var val = preLoginAddrByConn[connPtr.toString()];
    return typeof val === 'string' ? val : '';
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

// バニラ AGameMode::HasMatchStarted (0x25687E0) と同一判定:
// EnteringMap と WaitingToStart の両方を開局前とみなし、EnteringMap で空名簿ロックしない
function isMatchPreStart(gameModePtr) {
    if (gameModePtr.isNull()) {
        return true;
    }
    var currentMatchState = gameModePtr.add(OFF.gameModeMatchState).readU64();
    if (currentMatchState.equals(MatchState_EnteringMap_ptr.readU64())) {
        return true;
    }
    if (currentMatchState.equals(MatchState_WaitingToStart_ptr.readU64())) {
        return true;
    }
    return false;
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
    var current = gameModePtr.add(OFF.gameModeMatchState).readU64();
    if (current.equals(MatchState_EnteringMap_ptr.readU64())) {
        return 'EnteringMap';
    }
    if (current.equals(MatchState_WaitingToStart_ptr.readU64())) {
        return 'WaitingToStart';
    }
    if (current.equals(MatchState_PokerGame_ptr.readU64())) {
        return 'PokerGame';
    }
    if (current.equals(MatchState_InProgress_ptr.readU64())) {
        return 'InProgress';
    }
    if (current.equals(MatchState_WaitingPostMatch_ptr.readU64())) {
        return 'WaitingPostMatch';
    }
    if (current.equals(MatchState_LeavingMap_ptr.readU64())) {
        return 'LeavingMap';
    }
    if (current.equals(MatchState_Aborted_ptr.readU64())) {
        return 'Aborted';
    }
    if (isMatchEnded(gameModePtr)) {
        return 'Ended';
    }
    return 'Other';
}

function resetRosterState(gameModePtr) {
    lockedRosterGameMode = gameModePtr;
    matchRosterLocked = false;
    preMatchSeenUids = Object.create(null);
    preMatchSeenNames = Object.create(null);
    preMatchSeenAddrsByName = Object.create(null);
    allowedUids = Object.create(null);
    allowedNames = Object.create(null);
    preLoginAddrByConn = Object.create(null);
    preLoginAddrCacheCount = 0;
}

function ensureRosterGameMode(gameModePtr) {
    if (!lockedRosterGameMode.equals(gameModePtr)) {
        resetRosterState(gameModePtr);
    }
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

function recordPreMatchParticipant(gameModePtr, uid, name, normalizedAddr) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || !isA(gameModePtr, getDHGameModeClass())) {
        return;
    }
    ensureRosterGameMode(gameModePtr);
    if (matchRosterLocked || !isMatchPreStart(gameModePtr)) {
        return;
    }
    if (uid && uid.length > 0) {
        preMatchSeenUids[uid] = true;
    }
    if (name && name.length > 0) {
        preMatchSeenNames[name] = true;
        if (normalizedAddr && normalizedAddr.length > 0) {
            var addrSet = preMatchSeenAddrsByName[name];
            if (!addrSet) {
                addrSet = Object.create(null);
                preMatchSeenAddrsByName[name] = addrSet;
            }
            if (addrSet[normalizedAddr] !== true && countObjectKeys(addrSet) < MAX_ADDRS_PER_NAME) {
                addrSet[normalizedAddr] = true;
            }
        }
    }
}

function lockMatchRoster(gameModePtr, reasonLabel) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || !isA(gameModePtr, getDHGameModeClass())) {
        return;
    }
    ensureRosterGameMode(gameModePtr);

    var wasLocked = matchRosterLocked;
    var prevUidTotal = countObjectKeys(allowedUids);
    var prevNameTotal = countObjectKeys(allowedNames);
    matchRosterLocked = true;

    // 1. 開局前(EnteringMap/WaitingToStart)に認証通過した全参加者を和集合統合
    for (var preUid in preMatchSeenUids) {
        if (preMatchSeenUids[preUid] === true) {
            allowedUids[preUid] = true;
        }
    }
    for (var preName in preMatchSeenNames) {
        if (preMatchSeenNames[preName] === true) {
            allowedNames[preName] = true;
        }
    }

    // 2. GameState->PlayerArray の全 PlayerState を統合
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

        // 3. ADH_GameState::ExpectedPlayerIDs (+0x420) を統合
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

    // 4. マップ読込中の有効な UNetConnection も統合
    var world = GWorld_ptr.readPointer();
    if (!world.isNull() && !isPendingKill(world)) {
        var netDriver = world.add(OFF.worldNetDriver).readPointer();
        if (!netDriver.isNull() && !isPendingKill(netDriver)) {
            var connsPtr = netDriver.add(OFF.netDriverClientConnections);
            var connData = connsPtr.readPointer();
            var connCount = connsPtr.add(8).readS32();
            var pcClass = getPlayerControllerClass();
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
                    var connPC = conn.add(OFF.netConnPlayerController).readPointer();
                    if (!connPC.isNull() && !isPendingKill(connPC) && isA(connPC, pcClass)) {
                        var connPS = connPC.add(OFF.controllerPlayerState).readPointer();
                        if (!connPS.isNull() && !isPendingKill(connPS)) {
                            var connPSUid = getNormalizedUniqueIdString(connPS.add(OFF.playerStateUniqueId));
                            if (connPSUid.length > 0) {
                                allowedUids[connPSUid] = true;
                            }
                            var connPSName = getNormalizedPlayerName(connPS);
                            if (connPSName.length > 0) {
                                allowedNames[connPSName] = true;
                            }
                        }
                    }
                }
            }
        }
    }

    var uidTotal = countObjectKeys(allowedUids);
    var nameTotal = countObjectKeys(allowedNames);
    var preUidTotal = countObjectKeys(preMatchSeenUids);
    if (!wasLocked) {
        logEvent(
            '[開局ロック] ' + (reasonLabel || '試合開始') +
            'により開局メンバーを確定しました (UID: ' + uidTotal + '件 / 名前: ' + nameTotal +
            '件 / 開局前履歴UID: ' + preUidTotal + '件 / State: ' + getMatchStateLabel(gameModePtr) + ')'
        );
    } else if (uidTotal > prevUidTotal || nameTotal > prevNameTotal) {
        logEvent(
            '[開局名簿統合] ' + (reasonLabel || '試合開始') +
            'で開局メンバーを補完しました (UID: ' + prevUidTotal + '->' + uidTotal +
            '件 / 名前: ' + prevNameTotal + '->' + nameTotal +
            '件 / State: ' + getMatchStateLabel(gameModePtr) + ')'
        );
    }
}

function isRosterEnforced(gameModePtr) {
    if (gameModePtr.isNull() || isPendingKill(gameModePtr) || !isA(gameModePtr, getDHGameModeClass())) {
        return false;
    }
    ensureRosterGameMode(gameModePtr);
    if (matchRosterLocked) {
        return true;
    }
    if (!isMatchPreStart(gameModePtr)) {
        lockMatchRoster(gameModePtr, 'MatchState遷移');
        return true;
    }
    return false;
}

// 名前フォールバック時の再接続可否判定:
//   - 別IPから同名プレイヤーが接続中の場合はなりすましとして拒否
//   - 既に切断済み(Inactive/Disconnected)の1件一致 -> 許可 (name_disconnected)
//   - 60秒ゾンビタイムアウト前でまだ Active 扱いだが SavedNetworkAddress が同一IPの1件一致 ->
//     許可 (name_same_addr) + 対象 PlayerState の旧接続を事前整理
//   - WaitingToStart 中に切断して PlayerState が破棄済みで、開局前履歴の同一IP一致 ->
//     許可 (name_prematch_addr)
function findReconnectCandidateByName(gameModePtr, normalizedName, normalizedAddr) {
    if (gameModePtr.isNull() || normalizedName.length === 0) {
        return { allowed: false, matchedPS: ptr(0), reason: 'not_in_roster', needsZombieCleanup: false };
    }
    var gameState = gameModePtr.add(OFF.gameModeGameState).readPointer();
    if (gameState.isNull() || isPendingKill(gameState)) {
        return { allowed: false, matchedPS: ptr(0), reason: 'not_in_roster', needsZombieCleanup: false };
    }
    var arrPtr = gameState.add(OFF.gameStatePlayerArray);
    var dataPtr = arrPtr.readPointer();
    var count = arrPtr.add(8).readS32();
    var dhPsClass = getDHPlayerStateClass();

    var totalNameMatches = 0;
    var disconnectedMatches = [];
    var sameAddrActiveMatches = [];
    var otherActiveMatches = 0;

    if (!dataPtr.isNull() && count > 0 && count <= MAX_PLAYER_ARRAY) {
        for (var i = 0; i < count; i++) {
            var ps = dataPtr.add(i * 8).readPointer();
            if (ps.isNull() || isPendingKill(ps) || !isA(ps, dhPsClass)) {
                continue;
            }
            var psName = getNormalizedPlayerName(ps);
            if (psName.length === 0 || psName !== normalizedName) {
                continue;
            }
            totalNameMatches += 1;
            var isInactive = (ps.add(OFF.playerStateFlags).readU8() & PLAYER_STATE_INACTIVE_MASK) !== 0;
            var isDisc = ps.add(OFF.playerStateIsDisconnected).readU8() !== 0;
            if (isInactive || isDisc) {
                disconnectedMatches.push(ps);
            } else {
                var savedAddr = normalizeAddress(readFStringNonOwning(ps.add(OFF.playerStateSavedNetworkAddress)));
                if (normalizedAddr.length > 0 && savedAddr.length > 0 && savedAddr === normalizedAddr) {
                    sameAddrActiveMatches.push(ps);
                } else {
                    otherActiveMatches += 1;
                }
            }
        }
    }

    if (otherActiveMatches > 0) {
        return { allowed: false, matchedPS: ptr(0), reason: 'name_active_conflict', needsZombieCleanup: false };
    }
    if (disconnectedMatches.length === 1 && sameAddrActiveMatches.length === 0) {
        return { allowed: true, matchedPS: disconnectedMatches[0], reason: 'name_disconnected', needsZombieCleanup: false };
    }
    if (sameAddrActiveMatches.length === 1 && disconnectedMatches.length === 0) {
        return { allowed: true, matchedPS: sameAddrActiveMatches[0], reason: 'name_same_addr', needsZombieCleanup: true };
    }
    if (totalNameMatches === 0 && normalizedAddr.length > 0) {
        var seenAddrs = preMatchSeenAddrsByName[normalizedName];
        if (seenAddrs && seenAddrs[normalizedAddr] === true) {
            return { allowed: true, matchedPS: ptr(0), reason: 'name_prematch_addr', needsZombieCleanup: false };
        }
    }
    return { allowed: false, matchedPS: ptr(0), reason: 'not_in_roster', needsZombieCleanup: false };
}

function checkMidMatchReconnectAllowance(gameModePtr, incomingUid, optionsPtr, normalizedAddr) {
    var incomingName = parseOptionName(optionsPtr);
    if (!isRosterEnforced(gameModePtr)) {
        return {
            allowed: true,
            enforced: false,
            reason: 'before_lock',
            incomingName: incomingName,
            matchedPS: ptr(0),
            needsZombieCleanup: false
        };
    }
    if (isMatchEnded(gameModePtr)) {
        return {
            allowed: false,
            enforced: true,
            reason: 'match_ended',
            incomingName: incomingName,
            matchedPS: ptr(0),
            needsZombieCleanup: false
        };
    }
    if (incomingUid.length > 0 && allowedUids[incomingUid] === true) {
        return {
            allowed: true,
            enforced: true,
            reason: 'uid',
            incomingName: incomingName,
            matchedPS: ptr(0),
            needsZombieCleanup: false
        };
    }
    if (incomingName.length > 0 && allowedNames[incomingName] === true) {
        var nameRes = findReconnectCandidateByName(gameModePtr, incomingName, normalizedAddr || '');
        if (nameRes.allowed) {
            return {
                allowed: true,
                enforced: true,
                reason: nameRes.reason,
                incomingName: incomingName,
                matchedPS: nameRes.matchedPS,
                needsZombieCleanup: nameRes.needsZombieCleanup
            };
        }
        return {
            allowed: false,
            enforced: true,
            reason: nameRes.reason,
            incomingName: incomingName,
            matchedPS: ptr(0),
            needsZombieCleanup: false
        };
    }
    return {
        allowed: false,
        enforced: true,
        reason: 'not_in_roster',
        incomingName: incomingName,
        matchedPS: ptr(0),
        needsZombieCleanup: false
    };
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

function getConnectionEffectiveUid(connPtr, pcClass) {
    var connUid = getNormalizedUniqueIdString(connPtr.add(OFF.netConnPlayerId));
    if (connUid.length > 0) {
        return connUid;
    }
    var connPC = connPtr.add(OFF.netConnPlayerController).readPointer();
    if (!connPC.isNull() && !isPendingKill(connPC) && isA(connPC, pcClass)) {
        var connPS = connPC.add(OFF.controllerPlayerState).readPointer();
        if (!connPS.isNull() && !isPendingKill(connPS)) {
            return getNormalizedUniqueIdString(connPS.add(OFF.playerStateUniqueId));
        }
    }
    return '';
}

function cleanupStaleConnectionsForUid(gameModePtr, incomingUid, excludeConnPtr) {
    // 受信中の新規接続を誤破棄しないよう、非NULLの excludeConnPtr を必須とする
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

    var pcClass = getPlayerControllerClass();
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
        var connUid = getConnectionEffectiveUid(conn, pcClass);
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

    // 最後のゾンビ接続を破棄すると10秒CloseServerタイマーが起動するため解除
    if (cleaned > 0) {
        if (!gameModePtr.isNull()) {
            clearCloseServerTimer(gameModePtr);
        }
        logEvent(
            '[ゾンビ切断整理] 同一UIDの古い接続を ' + cleaned + '件 クリーンアップしました (UID: ' +
            formatUidTag(incomingUid) + ')'
        );
    }
    return cleaned;
}

function cleanupStaleConnectionsForPlayerState(gameModePtr, targetPS, excludeConnPtr) {
    if (isCleaningUpStaleConnection || targetPS.isNull() || isPendingKill(targetPS) || excludeConnPtr.isNull()) {
        return 0;
    }
    var pcClass = getPlayerControllerClass();
    var dhPcClass = getDHPlayerControllerClass();
    var staleConns = [];
    var seen = Object.create(null);

    function tryRecordConnFromPC(pcPtr) {
        if (pcPtr.isNull() || isPendingKill(pcPtr) || !isA(pcPtr, pcClass)) {
            return;
        }
        var pcPS = pcPtr.add(OFF.controllerPlayerState).readPointer();
        var pcDHPS = isA(pcPtr, dhPcClass) ? pcPtr.add(OFF.dhControllerPlayerState).readPointer() : ptr(0);
        if (!pcPS.equals(targetPS) && !pcDHPS.equals(targetPS)) {
            return;
        }
        var pcConn = pcPtr.add(OFF.playerControllerNetConnection).readPointer();
        if (pcConn.isNull() || isPendingKill(pcConn) || pcConn.equals(excludeConnPtr)) {
            return;
        }
        if (pcConn.add(OFF.netConnClientLoginState).readS32() === CLIENT_LOGIN_STATE_CLEANED_UP) {
            return;
        }
        var key = pcConn.toString();
        if (!seen[key]) {
            seen[key] = true;
            staleConns.push(pcConn);
        }
    }

    tryRecordConnFromPC(targetPS.add(OFF.actorOwner).readPointer());

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
                    if (conn.isNull() || isPendingKill(conn) || conn.equals(excludeConnPtr)) {
                        continue;
                    }
                    if (conn.add(OFF.netConnClientLoginState).readS32() === CLIENT_LOGIN_STATE_CLEANED_UP) {
                        continue;
                    }
                    var connPC = conn.add(OFF.netConnPlayerController).readPointer();
                    var connOwner = conn.add(OFF.netConnOwningActor).readPointer();
                    if (!connPC.isNull()) {
                        tryRecordConnFromPC(connPC);
                    }
                    if (!connOwner.isNull() && !connOwner.equals(connPC)) {
                        tryRecordConnFromPC(connOwner);
                    }
                }
            }
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

    if (cleaned > 0) {
        if (!gameModePtr.isNull()) {
            clearCloseServerTimer(gameModePtr);
        }
        logEvent(
            '[ゾンビ切断整理] 同一IP再接続対象の旧接続を ' + cleaned + '件 クリーンアップしました (Name: ' +
            sanitizeLogValue(getNormalizedPlayerName(targetPS)) + ')'
        );
    }
    return cleaned;
}

function cleanupStaleControllersForCandidatePlayerState(gameModePtr, newPC) {
    if (isCleaningUpStaleConnection || gameModePtr.isNull() || newPC.isNull() || isPendingKill(newPC)) {
        return 0;
    }
    if (!isA(gameModePtr, getDHGameModeClass()) || isMatchPreStart(gameModePtr) || isMatchEnded(gameModePtr)) {
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
    var incomingNetAddr = normalizeAddress(readFStringNonOwning(newPS.add(OFF.playerStateSavedNetworkAddress)));
    var candidatePS = ptr(0);

    // 1. UniqueId の完全一致(大小文字無視)
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
        // 2. UID無効時のフォールバック: SavedNetworkAddress + PlayerName 一致
        for (var k = 0; k < count; k++) {
            var psAddrCand = dataPtr.add(k * 8).readPointer();
            if (psAddrCand.isNull() || psAddrCand.equals(newPS) || isPendingKill(psAddrCand) || !isA(psAddrCand, dhPsClass)) {
                continue;
            }
            var candAddr = normalizeAddress(readFStringNonOwning(psAddrCand.add(OFF.playerStateSavedNetworkAddress)));
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

    // 対象 PlayerState の現在の Owner を確認
    recordStalePC(candidatePS.add(OFF.actorOwner).readPointer());

    // NetDriver->ClientConnections から対象 PlayerState を参照する旧接続を走査
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
// Hook 0: ポーカー開始・試合開始時に開局名簿を確定/和集合補完 (onEnter のみ使用)
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
// Hook 1: UWorld::NotifyControlMessage (NMT_Login / NMT_JoinSplit の受信接続を取得)
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
//   開局前参加者記録・開局外プレイヤー拒否・旧接続整理・AtCapacityバイパス・
//   10秒終了タイマー解除・診断ログ出力
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.AGameModeBase_PreLogin), {
    onEnter: function (args) {
        this.isDHMode = false;
        this.gameModePtr = ptr(0);
        this.pushedBypass = false;
        this.rejectOutsider = false;
        this.enforced = false;
        this.incomingUid = '';
        this.incomingName = '';
        this.incomingAddr = '';
        this.errorMsgPtr = ptr(0);
        try {
            var gameMode = args[0];
            var optionsPtr = args[1];
            var addressPtr = args[2];
            var uniqueIdPtr = args[3];
            this.errorMsgPtr = args[4];
            if (gameMode.isNull() || !isA(gameMode, getDHGameModeClass())) {
                return;
            }
            this.isDHMode = true;
            this.gameModePtr = gameMode;
            var incomingUid = getNormalizedUniqueIdString(uniqueIdPtr);
            var incomingAddr = normalizeAddress(readFStringNonOwning(addressPtr));
            this.incomingUid = incomingUid;
            this.incomingAddr = incomingAddr;
            if (!currentLoginConnection.isNull() && incomingAddr.length > 0) {
                cachePreLoginAddress(currentLoginConnection, incomingAddr);
            }

            var check = checkMidMatchReconnectAllowance(gameMode, incomingUid, optionsPtr, incomingAddr);
            this.enforced = check.enforced;
            this.incomingName = check.incomingName;

            if (check.enforced) {
                if (!check.allowed) {
                    this.rejectOutsider = true;
                    logEvent(
                        '[接続拒否] 開局外プレイヤーを拒否しました (PreLogin / 理由: ' + check.reason +
                        ' / UID: ' + formatUidTag(incomingUid) +
                        ' / Name: ' + sanitizeLogValue(check.incomingName) +
                        ' / State: ' + getMatchStateLabel(gameMode) + ')'
                    );
                    return;
                }
                logEvent(
                    '[再接続許可] 開局メンバーのPreLoginを受け入れました (一致: ' + check.reason +
                    ' / UID: ' + formatUidTag(incomingUid) +
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
            if (check.needsZombieCleanup && !check.matchedPS.isNull()) {
                cleanupStaleConnectionsForPlayerState(gameMode, check.matchedPS, currentLoginConnection);
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
            if (!this.isDHMode) {
                return;
            }
            if (this.rejectOutsider && !this.errorMsgPtr.isNull()) {
                var existingLen = this.errorMsgPtr.add(8).readS32();
                if (existingLen <= 1) {
                    writeUEFString(this.errorMsgPtr, REJECT_MID_MATCH_MESSAGE);
                }
            } else if (!this.errorMsgPtr.isNull()) {
                var ueErr = readFStringNonOwning(this.errorMsgPtr);
                if (ueErr.length > 0) {
                    logEvent(
                        '[PreLoginエラー] UE4側で接続が拒否されました (' +
                        (this.enforced ? '開局後' : '開局前') +
                        ' / 理由: ' + sanitizeLogValue(ueErr) +
                        ' / UID: ' + formatUidTag(this.incomingUid) +
                        ' / Name: ' + sanitizeLogValue(this.incomingName) +
                        ' / State: ' + getMatchStateLabel(this.gameModePtr) + ')'
                    );
                } else if (!this.enforced) {
                    recordPreMatchParticipant(
                        this.gameModePtr,
                        this.incomingUid,
                        this.incomingName,
                        this.incomingAddr
                    );
                    logEvent(
                        '[開局前PreLogin] 参加者を受け入れました (UID: ' + formatUidTag(this.incomingUid) +
                        ' / Name: ' + sanitizeLogValue(this.incomingName) +
                        ' / State: ' + getMatchStateLabel(this.gameModePtr) + ')'
                    );
                }
            }
        } catch (e) {}
    }
});

// ============================================================================
// Hook 3: ADH_GameMode::Login (replaceFast でトップレベル固定)
//   乱入時は元関数を実行せず即拒否(ExpectedPlayerIDs汚染/余分なSpawn防止)、
//   許可時は旧接続整理・AtCapacityバイパス・開局前参加者記録を実施
// ============================================================================
var dhGameModeLoginAddr = moduleBase.add(RVA.ADH_GameMode_Login);
var dhGameModeLoginTrampoline = ptr(0);
var dhGameModeLoginOrig = null;

var dhGameModeLoginWrapper = new NativeCallback(function (
    gameMode,
    newPlayerConn,
    remoteRole,
    portalPtr,
    optionsPtr,
    uniqueIdPtr,
    errorMsgPtr
) {
    var savedLoginConn = false;
    var prevLoginConn = ptr(0);
    var pushedBypass = false;
    var rejectOutsider = false;
    var enforced = false;
    var incomingUid = '';
    var incomingName = '';
    var incomingAddr = '';

    try {
        try {
            if (!newPlayerConn.isNull()) {
                prevLoginConn = currentLoginConnection;
                currentLoginConnection = newPlayerConn;
                savedLoginConn = true;
                incomingAddr = getCachedPreLoginAddress(newPlayerConn);
            }

            if (!gameMode.isNull() && !isPendingKill(gameMode) && isA(gameMode, getDHGameModeClass())) {
                incomingUid = getNormalizedUniqueIdString(uniqueIdPtr);
                var check = checkMidMatchReconnectAllowance(gameMode, incomingUid, optionsPtr, incomingAddr);
                enforced = check.enforced;
                incomingName = check.incomingName;

                if (check.enforced && !check.allowed) {
                    rejectOutsider = true;
                    logEvent(
                        '[接続拒否] 開局外プレイヤーを拒否しました (Login / 理由: ' + check.reason +
                        ' / UID: ' + formatUidTag(incomingUid) +
                        ' / Name: ' + sanitizeLogValue(incomingName) +
                        ' / State: ' + getMatchStateLabel(gameMode) + ')'
                    );
                    if (!errorMsgPtr.isNull() && errorMsgPtr.add(8).readS32() <= 1) {
                        writeUEFString(errorMsgPtr, REJECT_MID_MATCH_MESSAGE);
                    }
                } else {
                    if (!isMatchEnded(gameMode)) {
                        clearCloseServerTimer(gameMode);
                    }
                    if (incomingUid.length > 0) {
                        cleanupStaleConnectionsForUid(gameMode, incomingUid, newPlayerConn);
                    }
                    if (check.needsZombieCleanup && !check.matchedPS.isNull()) {
                        cleanupStaleConnectionsForPlayerState(gameMode, check.matchedPS, newPlayerConn);
                    }
                    if (!isMatchEnded(gameMode)) {
                        if (check.enforced || hasExistingPlayerStateByUid(gameMode, incomingUid)) {
                            reconnectCapacityBypassDepth += 1;
                            pushedBypass = true;
                        }
                    }
                }
            }
        } catch (e) {}

        if (rejectOutsider) {
            return ptr(0);
        }

        var resultPC = dhGameModeLoginOrig !== null
            ? dhGameModeLoginOrig(gameMode, newPlayerConn, remoteRole, portalPtr, optionsPtr, uniqueIdPtr, errorMsgPtr)
            : ptr(0);

        try {
            if (!resultPC.isNull() && !isPendingKill(resultPC)) {
                if (!enforced && !gameMode.isNull()) {
                    var spawnedName = incomingName;
                    var spawnedUid = incomingUid;
                    var spawnedPS = resultPC.add(OFF.controllerPlayerState).readPointer();
                    if (!spawnedPS.isNull() && !isPendingKill(spawnedPS)) {
                        var psUid = getNormalizedUniqueIdString(spawnedPS.add(OFF.playerStateUniqueId));
                        if (psUid.length > 0) {
                            spawnedUid = psUid;
                        }
                        var psName = getNormalizedPlayerName(spawnedPS);
                        if (psName.length > 0) {
                            spawnedName = psName;
                        }
                    }
                    recordPreMatchParticipant(gameMode, spawnedUid, spawnedName, incomingAddr);
                }
            } else if (enforced) {
                var loginErr = !errorMsgPtr.isNull() ? readFStringNonOwning(errorMsgPtr) : '';
                logEvent(
                    '[Loginエラー] プレイヤー生成に失敗しました (理由: ' + sanitizeLogValue(loginErr) +
                    ' / UID: ' + formatUidTag(incomingUid) +
                    ' / Name: ' + sanitizeLogValue(incomingName) + ')'
                );
            }
        } catch (e) {}

        return resultPC;
    } finally {
        if (pushedBypass && reconnectCapacityBypassDepth > 0) {
            reconnectCapacityBypassDepth -= 1;
        }
        if (savedLoginConn) {
            currentLoginConnection = prevLoginConn || ptr(0);
        }
    }
}, 'pointer', ['pointer', 'pointer', 'int32', 'pointer', 'pointer', 'pointer', 'pointer'], 'win64');

dhGameModeLoginTrampoline = Interceptor.replaceFast(dhGameModeLoginAddr, dhGameModeLoginWrapper);
dhGameModeLoginOrig = new NativeFunction(
    dhGameModeLoginTrampoline,
    'pointer',
    ['pointer', 'pointer', 'int32', 'pointer', 'pointer', 'pointer', 'pointer'],
    { abi: 'win64', exceptions: 'propagate' }
);

// ============================================================================
// Hook 4: AGameSession::AtCapacity (開局メンバー再接続時の満員拒否を回避)
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
// Hook 5: AGameMode::PostLogin (ログイン後の10秒空室終了タイマーを解除)
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
// Hook 6: ADH_GameMode::FindInactivePlayer (onEnter + 結果ログ用 onLeave: 計1件)
//   再紐付け前に対象 PlayerState を参照する残存旧Controller/接続を事前破棄し、
//   試合開始後の再紐付け成否を診断ログへ記録する
// ============================================================================
Interceptor.attach(moduleBase.add(RVA.ADH_GameMode_FindInactivePlayer), {
    onEnter: function (args) {
        this.shouldLogOutcome = false;
        this.uidTag = 'empty';
        this.playerName = '-';
        try {
            var gameMode = args[0];
            var newPC = args[1];
            cleanupStaleControllersForCandidatePlayerState(gameMode, newPC);
            if (!gameMode.isNull() && !isPendingKill(gameMode) && isA(gameMode, getDHGameModeClass())) {
                if (!isMatchPreStart(gameMode) && !isMatchEnded(gameMode)) {
                    this.shouldLogOutcome = true;
                    if (!newPC.isNull() && !isPendingKill(newPC)) {
                        var newPS = newPC.add(OFF.controllerPlayerState).readPointer();
                        if (!newPS.isNull() && !isPendingKill(newPS)) {
                            this.uidTag = formatUidTag(getNormalizedUniqueIdString(newPS.add(OFF.playerStateUniqueId)));
                            this.playerName = sanitizeLogValue(getNormalizedPlayerName(newPS));
                        }
                    }
                }
            }
        } catch (e) {}
    },
    onLeave: function (retval) {
        try {
            if (!this.shouldLogOutcome) {
                return;
            }
            var matched = (retval.toInt32() & 1) !== 0;
            logEvent(
                '[再紐付け結果] FindInactivePlayer: ' + (matched ? '成功(既存PlayerState復元)' : '未一致(新規扱い)') +
                ' (UID: ' + this.uidTag + ' / Name: ' + this.playerName + ')'
            );
        } catch (e) {}
    }
});

// ============================================================================
// Hook 7: ADH_GameMode::CloseServer ガード (トップレベルで NativeCallback を固定)
//   試合継続中にアクティブなプレイヤー/接続が残っている場合の誤終了を抑止
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
closeServerOrig = new NativeFunction(closeServerTrampoline, 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
