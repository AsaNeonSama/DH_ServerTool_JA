var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var CONSOLE_BRIDGE_HOST = '127.0.0.1';
var CONSOLE_BRIDGE_PORT = __CONSOLE_PATCH_PORT__;
var MAX_PENDING_COMMANDS = 100;

// ============================================================================
// UE4 ネイティブ関数バインディング・フックアドレス
// ============================================================================
// FString&&ムーブ版(0x10963B0)によるRPC UAF文字化け防止のため、const FString&版(0x10964E0)でFMemoryへコピー
var FText_FromString = new NativeFunction(moduleBase.add(0x10964E0), 'pointer', ['pointer', 'pointer'], 'win64');
var FText_Dtor = new NativeFunction(moduleBase.add(0x75CB30), 'void', ['pointer'], 'win64');
var FName_GetPlainNameString = new NativeFunction(moduleBase.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');
var FMemory_Free = new NativeFunction(moduleBase.add(0x105DD50), 'void', ['pointer'], 'win64');

var UClass_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0x11F02E0), 'pointer', [], 'win64');
var ADH_Character_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0xEA0F90), 'pointer', [], 'win64');
var ADH_PlayerState_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0xEED6B0), 'pointer', [], 'win64');
var APlayerController_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0x2AA84E0), 'pointer', [], 'win64');
var GWorld_ptr = moduleBase.add(0x46ED420);
var GUObjectArray_ptr = moduleBase.add(0x45C40C8);
var cachedDHCharacterClass = ptr(0);
var cachedPlayerControllerClass = ptr(0);
var StaticLoadObject = new NativeFunction(moduleBase.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(moduleBase.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var UWorld_SpawnActor = new NativeFunction(moduleBase.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var FActorSpawnParameters_Ctor = new NativeFunction(moduleBase.add(0x29584A0), 'void', ['pointer'], 'win64');
var UGameplayStatics_BeginDeferredActorSpawnFromClass = new NativeFunction(moduleBase.add(0x25570C0), 'pointer', ['pointer', 'pointer', 'pointer', 'uint8', 'pointer'], 'win64');
var UGameplayStatics_FinishSpawningActor = new NativeFunction(moduleBase.add(0x255FCF0), 'pointer', ['pointer', 'pointer'], 'win64');
var APawn_SetPlayerState = new NativeFunction(moduleBase.add(0x2752480), 'void', ['pointer', 'pointer'], 'win64');
var ADH_HumanCharacter_InitFromPlayerState = new NativeFunction(moduleBase.add(0xD5AA60), 'void', ['pointer', 'pointer', 'int8'], 'win64');
var ADH_GameState_SetMatchResult = new NativeFunction(moduleBase.add(0xDA9C30), 'void', ['pointer', 'uint8', 'uint8'], 'win64');
var TArray_Ptr_AddUnique = new NativeFunction(moduleBase.add(0x769CC0), 'int32', ['pointer', 'pointer'], 'win64');

var UDH_InventoryManager_AddInventory = new NativeFunction(moduleBase.add(0xDBC040), 'void', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], 'win64');
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(moduleBase.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');
var AGameSession_KickPlayer = new NativeFunction(moduleBase.add(0x256B770), 'void', ['pointer', 'pointer', 'pointer']);
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(moduleBase.add(0xEE7810), 'void', ['pointer', 'pointer', 'pointer']);
var ADH_PlayerController_AddCheats = new NativeFunction(moduleBase.add(0xE2B7E0), 'void', ['pointer', 'int8'], 'win64');
var ADH_PlayerController_SetDesiresThrallVision_Impl = new NativeFunction(moduleBase.add(0xE4D980), 'void', ['pointer', 'int8'], 'win64');
var ADH_PlayerState_SetIsThrall = new NativeFunction(moduleBase.add(0xE4E880), 'void', ['pointer', 'int8'], 'win64');
var ADH_SpellManager_SetEquippedSpells = new NativeFunction(moduleBase.add(0xE81F00), 'void', ['pointer', 'pointer'], 'win64');
var ADH_SpellManager_SetSpellChargeLevel = new NativeFunction(moduleBase.add(0xE82CC0), 'void', ['pointer', 'float'], 'win64');
var ADH_HumanCharacter_SetIsSpiritWalking = new NativeFunction(moduleBase.add(0xD70310), 'void', ['pointer', 'int8', 'float'], 'win64');
var AActor_GetWorldTimerManager = new NativeFunction(moduleBase.add(0x22DDC30), 'pointer', ['pointer'], 'win64');
var FTimerManager_FindTimer = new NativeFunction(moduleBase.add(0x28F2D80), 'pointer', ['pointer', 'pointer'], 'win64');
var FTimerManager_InternalClearTimer = new NativeFunction(moduleBase.add(0x28F6A00), 'void', ['pointer', 'uint64'], 'win64');
var UKismetSystemLibrary_ExecuteConsoleCommand = new NativeFunction(moduleBase.add(0x25EFC50), 'void', ['pointer', 'pointer', 'pointer'], 'win64');

var APlayerController_BeginPlay_addr = moduleBase.add(0xE2DBC0);
var ADH_GameState_Tick_addr = moduleBase.add(0xDAD420);
var ADH_GameState_HandleMatchHasStarted_addr = moduleBase.add(0xD99080);

// ============================================================================
// UE4 構造体オフセット
// ============================================================================
var UE_OFFSETS = {
    uObjectInternalIndex: 0x0C,
    uObjectClassPrivate: 0x10,
    uObjectNamePrivate: 0x18,
    uStructChainArray: 0x30,
    uStructChainDepth: 0x38,
    actorRootComponent: 0x130,
    actorVtableK2DestroyActor: 0x310,
    sceneComponentToWorldRot: 0x1C0,
    sceneComponentToWorldPos: 0x1D0,
    worldGameInstance: 0x180,
    gameInstancePlayerRoleList: 0x3B8,
    gameStateAuthorityGameMode: 0x228,
    gameStatePlayerArray: 0x238,
    gameModePlayerStateClass: 0x248,
    gameModeGameSession: 0x278,
    gameModeVtableEndMatch: 0x838,
    playerStateOwnerController: 0xE0,
    playerStateFlags: 0x22A,
    playerStatePlayerName: 0x300,
    playerStateSpellManager: 0x488,
    playerStateOtherThralls: 0x578,
    playerStateSelectedRole: 0x590,
    playerStateRealPawn: 0x638,
    spellManagerEquippedSpells: 0x288,
    roleDataOnlineName: 0x48,
    playerControllerPlayerState: 0x228,
    playerControllerPawn: 0x250,
    playerControllerCheatManager: 0x338,
    humanInventoryManager: 0x808,
    humanLastDHPlayerState: 0xC40,
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

// マップ未配置クラス(例: 山頂のネズミ)を summon 実行前にプリロードする対応表
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
    BP_HumanCharacter_Thrall_C: '/Game/Blueprints/Player/BP_HumanCharacter.BP_HumanCharacter_C',
    BP_Totem_C: '/Game/Blueprints/Game/Totems/BP_Totem.BP_Totem_C',
    BP_ExplosionActor_C: '/Game/Blueprints/Game/BP_ExplosionActor.BP_ExplosionActor_C',
    BP_Explosion_Nitro_C: '/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C'
};

// ============================================================================
// ログ出力・UE4文字列/オブジェクト補助関数
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

function getDHCharacterClass() {
    if (cachedDHCharacterClass.isNull()) {
        cachedDHCharacterClass = ADH_Character_GetPrivateStaticClass();
    }
    return cachedDHCharacterClass;
}

function getPlayerControllerClass() {
    if (cachedPlayerControllerClass.isNull()) {
        cachedPlayerControllerClass = APlayerController_GetPrivateStaticClass();
    }
    return cachedPlayerControllerClass;
}

function isA(objPtr, targetClassPtr) {
    if (objPtr.isNull() || targetClassPtr.isNull()) {
        return false;
    }
    var objClass = objPtr.add(UE_OFFSETS.uObjectClassPrivate).readPointer();
    if (objClass.isNull()) {
        return false;
    }
    var targetDepth = targetClassPtr.add(UE_OFFSETS.uStructChainDepth).readS32();
    var objDepth = objClass.add(UE_OFFSETS.uStructChainDepth).readS32();
    if (targetDepth < 0 || targetDepth > objDepth) {
        return false;
    }
    var chainArray = objClass.add(UE_OFFSETS.uStructChainArray).readPointer();
    if (chainArray.isNull()) {
        return false;
    }
    return chainArray.add(targetDepth * 8).readPointer().equals(targetClassPtr.add(UE_OFFSETS.uStructChainArray));
}

function isPendingKill(objPtr) {
    if (objPtr.isNull()) {
        return true;
    }
    var internalIndex = objPtr.add(UE_OFFSETS.uObjectInternalIndex).readS32();
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
    if (dataPtr.isNull() || num <= 1 || num >= 4096) {
        return '';
    }
    try {
        return dataPtr.readUtf16String(num - 1);
    } catch (e) {
        return '';
    }
}

function getUObjectName(uObjectPtr) {
    if (uObjectPtr.isNull()) {
        return '';
    }
    var namePrivatePtr = uObjectPtr.add(UE_OFFSETS.uObjectNamePrivate);
    var nameBuffer = Memory.alloc(16);
    nameBuffer.writePointer(ptr(0));
    nameBuffer.add(8).writeS32(0);
    nameBuffer.add(12).writeS32(0);
    FName_GetPlainNameString(namePrivatePtr, nameBuffer);
    var namePtr = nameBuffer.readPointer();
    var num = nameBuffer.add(8).readS32();
    var text = '';
    try {
        if (!namePtr.isNull() && num > 1 && num < 4096) {
            text = namePtr.readUtf16String(num - 1);
        }
    } catch (e) {
        text = '';
    } finally {
        if (!namePtr.isNull()) {
            FMemory_Free(namePtr);
        }
    }
    return text ? text : '';
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

function preloadSummonBlueprint(cmdName, cmdArg) {
    if (cmdName !== 'summon' || !cmdArg) {
        return;
    }
    if (SUMMON_BLUEPRINT_PATHS.hasOwnProperty(cmdArg)) {
        resolveBlueprintClass(SUMMON_BLUEPRINT_PATHS[cmdArg]);
    }
}

// ============================================================================
// ゲーム内操作
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
    if (dataPtr.isNull() || count <= 0 || count > 64) {
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

var trackedGameStatePtr = ptr(0);
var summonedThrallPlayerStates = [];
var savedEquippedSpellsByPs = {};

function pruneSummonedThrallPlayerStates(gameStatePtr) {
    if (!trackedGameStatePtr.equals(gameStatePtr)) {
        trackedGameStatePtr = gameStatePtr;
        summonedThrallPlayerStates = [];
        savedEquippedSpellsByPs = {};
        return;
    }
    var alive = [];
    for (var i = 0; i < summonedThrallPlayerStates.length; i++) {
        var ps = summonedThrallPlayerStates[i];
        if (!ps.isNull() && !isPendingKill(ps)) {
            alive.push(ps);
        }
    }
    summonedThrallPlayerStates = alive;
}

function snapshotSpellManagerEquippedSpells(playerStatePtr, spellManagerPtr) {
    if (playerStatePtr.isNull() || spellManagerPtr.isNull() || isPendingKill(spellManagerPtr)) {
        return;
    }
    var eqPtr = spellManagerPtr.add(UE_OFFSETS.spellManagerEquippedSpells);
    var dataPtr = eqPtr.readPointer();
    var count = eqPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0 || count > 16) {
        return;
    }
    var spells = [];
    for (var i = 0; i < count; i++) {
        var clsPtr = dataPtr.add(8 * i).readPointer();
        if (!clsPtr.isNull() && !isPendingKill(clsPtr)) {
            spells.push(clsPtr);
        }
    }
    if (spells.length > 0) {
        savedEquippedSpellsByPs[playerStatePtr.toString()] = spells;
    }
}

function restoreSpellManagerEquippedSpells(playerStatePtr) {
    if (playerStatePtr.isNull() || isPendingKill(playerStatePtr)) {
        return;
    }
    var saved = savedEquippedSpellsByPs[playerStatePtr.toString()];
    if (!saved || saved.length === 0) {
        return;
    }
    var spellManagerPtr = playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).readPointer();
    if (spellManagerPtr.isNull() || isPendingKill(spellManagerPtr)) {
        return;
    }
    var curCount = spellManagerPtr.add(UE_OFFSETS.spellManagerEquippedSpells + 8).readS32();
    if (curCount > 0) {
        return;
    }
    var valid = [];
    for (var i = 0; i < saved.length; i++) {
        if (!saved[i].isNull() && !isPendingKill(saved[i])) {
            valid.push(saved[i]);
        }
    }
    if (valid.length === 0) {
        return;
    }
    var buf = Memory.alloc(16 + valid.length * 8);
    var dataBuf = buf.add(16);
    for (var j = 0; j < valid.length; j++) {
        dataBuf.add(8 * j).writePointer(valid[j]);
    }
    buf.writePointer(dataBuf);
    buf.add(8).writeS32(valid.length);
    buf.add(12).writeS32(valid.length);
    ADH_SpellManager_SetEquippedSpells(spellManagerPtr, buf);
}

function addUniquePointer(tArrayPtr, elemPtr) {
    if (tArrayPtr.isNull() || elemPtr.isNull()) {
        return;
    }
    var elemBuf = Memory.alloc(8);
    elemBuf.writePointer(elemPtr);
    TArray_Ptr_AddUnique(tArrayPtr, elemBuf);
}

function makeAlignedTransform(x, y, z, yawRad) {
    var raw = Memory.alloc(0x40);
    var rem = raw.and(0xF).toInt32();
    var aligned = rem === 0 ? raw : raw.add(16 - rem);
    var halfYaw = yawRad * 0.5;
    aligned.writeFloat(0.0);
    aligned.add(0x04).writeFloat(0.0);
    aligned.add(0x08).writeFloat(Math.sin(halfYaw));
    aligned.add(0x0C).writeFloat(Math.cos(halfYaw));
    aligned.add(0x10).writeFloat(x);
    aligned.add(0x14).writeFloat(y);
    aligned.add(0x18).writeFloat(z);
    aligned.add(0x1C).writeFloat(0.0);
    aligned.add(0x20).writeFloat(1.0);
    aligned.add(0x24).writeFloat(1.0);
    aligned.add(0x28).writeFloat(1.0);
    aligned.add(0x2C).writeFloat(0.0);
    return { raw: raw, ptr: aligned };
}

function findRoleDataByOnlineName(worldPtr, targetOnlineName) {
    if (worldPtr.isNull() || isPendingKill(worldPtr) || !targetOnlineName) {
        return ptr(0);
    }
    var gameInstancePtr = worldPtr.add(UE_OFFSETS.worldGameInstance).readPointer();
    if (gameInstancePtr.isNull() || isPendingKill(gameInstancePtr)) {
        return ptr(0);
    }
    var roleListPtr = gameInstancePtr.add(UE_OFFSETS.gameInstancePlayerRoleList);
    var dataPtr = roleListPtr.readPointer();
    var count = roleListPtr.add(8).readS32();
    if (dataPtr.isNull() || count <= 0 || count > 32) {
        return ptr(0);
    }
    for (var i = 0; i < count; i++) {
        var rolePtr = dataPtr.add(i * 8).readPointer();
        if (!rolePtr.isNull() && !isPendingKill(rolePtr)) {
            if (readFStringNonOwning(rolePtr.add(UE_OFFSETS.roleDataOnlineName)) === targetOnlineName) {
                return rolePtr;
            }
        }
    }
    return ptr(0);
}

function spawnSummonedMarine(gameStatePtr, playerControllerPtr, isThrall) {
    if (gameStatePtr.isNull() || isPendingKill(gameStatePtr) || playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var worldPtr = GWorld_ptr.readPointer();
    if (worldPtr.isNull() || isPendingKill(worldPtr)) {
        return;
    }
    var targetPawn = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (targetPawn.isNull() || isPendingKill(targetPawn)) {
        return;
    }
    var rootComponent = targetPawn.add(UE_OFFSETS.actorRootComponent).readPointer();
    if (rootComponent.isNull() || isPendingKill(rootComponent)) {
        return;
    }
    var px = rootComponent.add(UE_OFFSETS.sceneComponentToWorldPos).readFloat();
    var py = rootComponent.add(UE_OFFSETS.sceneComponentToWorldPos + 4).readFloat();
    var pz = rootComponent.add(UE_OFFSETS.sceneComponentToWorldPos + 8).readFloat();
    if (!Number.isFinite(px) || !Number.isFinite(py) || !Number.isFinite(pz)) {
        return;
    }
    var qx = rootComponent.add(UE_OFFSETS.sceneComponentToWorldRot).readFloat();
    var qy = rootComponent.add(UE_OFFSETS.sceneComponentToWorldRot + 4).readFloat();
    var qz = rootComponent.add(UE_OFFSETS.sceneComponentToWorldRot + 8).readFloat();
    var qw = rootComponent.add(UE_OFFSETS.sceneComponentToWorldRot + 12).readFloat();
    var fx = 1.0 - 2.0 * (qy * qy + qz * qz);
    var fy = 2.0 * (qx * qy + qw * qz);
    var len = Math.sqrt(fx * fx + fy * fy);
    if (!Number.isFinite(len) || len < 1e-4) {
        fx = 1.0;
        fy = 0.0;
    } else {
        fx /= len;
        fy /= len;
    }
    var spawnX = px + fx * 180.0;
    var spawnY = py + fy * 180.0;
    var spawnZ = pz + 10.0;
    var facingYaw = Math.atan2(-fy, -fx);

    var humanClass = resolveBlueprintClass(SUMMON_BLUEPRINT_PATHS.BP_HumanCharacter_C);
    if (humanClass.isNull()) {
        return;
    }
    var tf = makeAlignedTransform(spawnX, spawnY, spawnZ, facingYaw);
    var spawnParams = Memory.alloc(0x40);
    FActorSpawnParameters_Ctor(spawnParams);
    spawnParams.add(0x28).writeU8(1); // AlwaysSpawn
    var spawnedPawn = UWorld_SpawnActor(worldPtr, humanClass, tf.ptr, spawnParams);
    if (spawnedPawn.isNull() || isPendingKill(spawnedPawn)) {
        return;
    }

    var gameModePtr = gameStatePtr.add(UE_OFFSETS.gameStateAuthorityGameMode).readPointer();
    var psClass = (!gameModePtr.isNull() && !isPendingKill(gameModePtr))
        ? gameModePtr.add(UE_OFFSETS.gameModePlayerStateClass).readPointer()
        : ptr(0);
    if (psClass.isNull()) {
        psClass = ADH_PlayerState_GetPrivateStaticClass();
    }
    if (psClass.isNull()) {
        return;
    }

    // 遅延スポーンで FinishSpawning 前に bIsInactive(0x20) + bIsABot(0x08) を立てることで、
    // サーバー・クライアント両方の GameState::PlayerArray (スコアボード等) に混入させない
    var dummyPs = UGameplayStatics_BeginDeferredActorSpawnFromClass(worldPtr, psClass, tf.ptr, 1, ptr(0));
    if (dummyPs.isNull() || isPendingKill(dummyPs)) {
        return;
    }
    var psFlags = dummyPs.add(UE_OFFSETS.playerStateFlags).readU8();
    dummyPs.add(UE_OFFSETS.playerStateFlags).writeU8(psFlags | 0x28);
    var marineRole = findRoleDataByOnlineName(worldPtr, 'Marine');
    if (!marineRole.isNull()) {
        dummyPs.add(UE_OFFSETS.playerStateSelectedRole).writePointer(marineRole);
    }
    UGameplayStatics_FinishSpawningActor(dummyPs, tf.ptr);

    APawn_SetPlayerState(spawnedPawn, dummyPs);
    spawnedPawn.add(UE_OFFSETS.humanLastDHPlayerState).writePointer(dummyPs);
    dummyPs.add(UE_OFFSETS.playerStateRealPawn).writePointer(spawnedPawn);
    ADH_HumanCharacter_InitFromPlayerState(spawnedPawn, dummyPs, 1);

    if (isThrall) {
        pruneSummonedThrallPlayerStates(gameStatePtr);
        ADH_PlayerState_SetIsThrall(dummyPs, 1);
        summonedThrallPlayerStates.push(dummyPs);
    }
}

function endMatchWithWinner(gameStatePtr, explorersWin) {
    if (gameStatePtr.isNull() || isPendingKill(gameStatePtr)) {
        return;
    }
    // UDH_CheatManager::EndMatch(bool bExplorersWin) と同一:
    // 1 = 探検家勝利 (PT_EXPLORERS / GOR_EXPLORERS_ESCAPED)
    // 2 = 傀儡勝利   (PT_THRALL    / GOR_EXPLORERS_DIED)
    var resultCode = explorersWin ? 1 : 2;
    ADH_GameState_SetMatchResult(gameStatePtr, resultCode, resultCode);
    var gameModePtr = gameStatePtr.add(UE_OFFSETS.gameStateAuthorityGameMode).readPointer();
    if (gameModePtr.isNull() || isPendingKill(gameModePtr)) {
        return;
    }
    var vtablePtr = gameModePtr.readPointer();
    if (vtablePtr.isNull()) {
        return;
    }
    var endMatchFnPtr = vtablePtr.add(UE_OFFSETS.gameModeVtableEndMatch).readPointer();
    if (endMatchFnPtr.isNull()) {
        return;
    }
    var endMatchFn = new NativeFunction(endMatchFnPtr, 'void', ['pointer'], 'win64');
    endMatchFn(gameModePtr);
}

function setTargetPlayerIsThrall(gameStatePtr, playerControllerPtr, isThrall) {
    if (gameStatePtr.isNull() || isPendingKill(gameStatePtr) || playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var playerStatePtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPlayerState).readPointer();
    if (playerStatePtr.isNull() || isPendingKill(playerStatePtr)) {
        return;
    }
    pruneSummonedThrallPlayerStates(gameStatePtr);
    if (isThrall) {
        ADH_PlayerState_SetIsThrall(playerStatePtr, 1);
        // クライアント側 SetEquippedSpellsFromSave(+0x7B9) は試合中1度しか走らないため、
        // 傀儡→探検家→傀儡へ戻した場合は退避済み EquippedSpells を新 SpellManager へ復元して同期する
        restoreSpellManagerEquippedSpells(playerStatePtr);
        for (var s = 0; s < summonedThrallPlayerStates.length; s++) {
            var dummyPs = summonedThrallPlayerStates[s];
            if (!dummyPs.isNull() && !isPendingKill(dummyPs)) {
                addUniquePointer(playerStatePtr.add(UE_OFFSETS.playerStateOtherThralls), dummyPs);
                addUniquePointer(dummyPs.add(UE_OFFSETS.playerStateOtherThralls), playerStatePtr);
            }
        }
        return;
    }
    // 傀儡から探検家へ安全に戻す:
    // 1. 傀儡ビジョンを無効化
    ADH_PlayerController_SetDesiresThrallVision_Impl(playerControllerPtr, 0);

    // 2. 対象および他プレイヤー(召喚傀儡海兵含む)の OtherThralls 配列から対象を除去
    var targetOtherThrallsPtr = playerStatePtr.add(UE_OFFSETS.playerStateOtherThralls);
    if (targetOtherThrallsPtr.add(8).readS32() > 0) {
        targetOtherThrallsPtr.add(8).writeS32(0);
    }
    var playerArrayPtr = gameStatePtr.add(UE_OFFSETS.gameStatePlayerArray);
    var playerListDataPtr = playerArrayPtr.readPointer();
    var playerCount = playerArrayPtr.add(8).readU32();
    if (!playerListDataPtr.isNull() && playerCount > 0 && playerCount <= 64) {
        for (var i = 0; i < playerCount; i++) {
            var otherPlayerStatePtr = playerListDataPtr.add(8 * i).readPointer();
            if (!otherPlayerStatePtr.isNull() && !isPendingKill(otherPlayerStatePtr) && !otherPlayerStatePtr.equals(playerStatePtr)) {
                removeFromOtherThrallsArray(otherPlayerStatePtr, playerStatePtr);
            }
        }
    }
    for (var d = 0; d < summonedThrallPlayerStates.length; d++) {
        var summonedPs = summonedThrallPlayerStates[d];
        if (!summonedPs.isNull() && !isPendingKill(summonedPs)) {
            removeFromOtherThrallsArray(summonedPs, playerStatePtr);
        }
    }

    // 3. SpellManager (+0x488) の装備呪文を退避してから破棄・クリアし、呪文とトーテム権限を剥奪
    var spellManagerPtr = playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).readPointer();
    if (!spellManagerPtr.isNull()) {
        if (!isPendingKill(spellManagerPtr)) {
            snapshotSpellManagerEquippedSpells(playerStatePtr, spellManagerPtr);
            destroyActorSafe(spellManagerPtr);
        }
        playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).writePointer(ptr(0));
    }

    // 4. bIsThrall (+0x572) を 0 に設定し、デリゲートと関連状態を更新
    ADH_PlayerState_SetIsThrall(playerStatePtr, 0);
}

function setTargetPlayerSpellCharge(playerControllerPtr, chargeLevel) {
    if (playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var playerStatePtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPlayerState).readPointer();
    if (playerStatePtr.isNull() || isPendingKill(playerStatePtr)) {
        return;
    }
    var spellManagerPtr = playerStatePtr.add(UE_OFFSETS.playerStateSpellManager).readPointer();
    if (spellManagerPtr.isNull() || isPendingKill(spellManagerPtr)) {
        return;
    }
    var clamped = chargeLevel < 0.0 ? 0.0 : (chargeLevel > 1.0 ? 1.0 : chargeLevel);
    ADH_SpellManager_SetSpellChargeLevel(spellManagerPtr, clamped);
}

function setHumanCharacterProperty(playerControllerPtr, propertyName, propertyValue) {
    if (playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull() || isPendingKill(pawnPtr)) {
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
    if (playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull() || isPendingKill(pawnPtr) || !isA(pawnPtr, getDHCharacterClass())) {
        return;
    }
    var inventoryManagerPtr = pawnPtr.add(UE_OFFSETS.humanInventoryManager).readPointer();
    if (inventoryManagerPtr.isNull() || isPendingKill(inventoryManagerPtr)) {
        return;
    }
    var itemClassPtr = resolveBlueprintClass(itemBlueprintPath);
    if (itemClassPtr.isNull()) {
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
    if (playerControllerPtr.isNull() || isPendingKill(playerControllerPtr)) {
        return;
    }
    var pawnPtr = playerControllerPtr.add(UE_OFFSETS.playerControllerPawn).readPointer();
    if (pawnPtr.isNull() || isPendingKill(pawnPtr) || !isA(pawnPtr, getDHCharacterClass())) {
        return;
    }
    var inventoryManagerPtr = pawnPtr.add(UE_OFFSETS.humanInventoryManager).readPointer();
    if (inventoryManagerPtr.isNull() || isPendingKill(inventoryManagerPtr)) {
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
    try {
        AGameSession_KickPlayer(gameSessionPtr, playerControllerPtr, kickReasonText);
    } finally {
        FText_Dtor(kickReasonText);
    }
}

function deliverThrallMessage(playerControllerPtr, messageText) {
    if (playerControllerPtr.isNull()) {
        return;
    }
    var messageFText = createFTextFromString(messageText);
    try {
        ADH_PlayerController_ReceiveThrallMessage(playerControllerPtr, messageFText, ptr(0));
    } finally {
        FText_Dtor(messageFText);
    }
}

// ============================================================================
// プレイヤー検索・コマンドキュー
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
    if (gameStatePtr.isNull() || isPendingKill(gameStatePtr) || !targetName) {
        return ptr(0);
    }
    var playerArrayPtr = gameStatePtr.add(UE_OFFSETS.gameStatePlayerArray);
    var playerListDataPtr = playerArrayPtr.readPointer();
    var playerCount = playerArrayPtr.add(8).readU32();
    if (playerListDataPtr.isNull() || playerCount === 0 || playerCount > 64) {
        return ptr(0);
    }
    var pcClass = getPlayerControllerClass();
    for (var i = 0; i < playerCount; i++) {
        var playerStatePtr = playerListDataPtr.add(8 * i).readPointer();
        if (playerStatePtr.isNull() || isPendingKill(playerStatePtr)) {
            continue;
        }
        var candidateName = '';
        if (matchPlayerName) {
            candidateName = readFStringNonOwning(playerStatePtr.add(UE_OFFSETS.playerStatePlayerName));
        } else {
            var selectedRolePtr = playerStatePtr.add(UE_OFFSETS.playerStateSelectedRole).readPointer();
            if (selectedRolePtr.isNull() || isPendingKill(selectedRolePtr)) {
                continue;
            }
            candidateName = readFStringNonOwning(selectedRolePtr.add(UE_OFFSETS.roleDataOnlineName));
        }
        if (candidateName === targetName) {
            var ownerController = playerStatePtr.add(UE_OFFSETS.playerStateOwnerController).readPointer();
            if (!ownerController.isNull() && !isPendingKill(ownerController) && isA(ownerController, pcClass)) {
                return ownerController;
            }
            return ptr(0);
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
        if (typeof commandText !== 'string') {
            return;
        }
        var trimmedCommand = commandText.trim();
        if (trimmedCommand.length === 0) {
            return;
        }
        var firstSpaceIdx = trimmedCommand.indexOf(' ');
        var cmdName = (firstSpaceIdx === -1 ? trimmedCommand : trimmedCommand.slice(0, firstSpaceIdx)).toLowerCase();
        var cmdArg = firstSpaceIdx === -1 ? '' : trimmedCommand.slice(firstSpaceIdx + 1).trim();
        var normalizedCommand = cmdArg.length === 0 ? cmdName : (cmdName + ' ' + cmdArg);
        if (cmdName === 'setisthrall') {
            var thrallArg = cmdArg.toLowerCase();
            var enableThrall = (thrallArg !== '0' && thrallArg !== 'false');
            setTargetPlayerIsThrall(gameStatePtr, targetController, enableThrall);
            return;
        }
        if (cmdName === 'setspellcharge') {
            var parsedCharge = parseFloat(cmdArg);
            if (Number.isFinite(parsedCharge)) {
                setTargetPlayerSpellCharge(targetController, parsedCharge);
            }
            return;
        }
        if (cmdName === 'endmatch') {
            var endArg = cmdArg.toLowerCase();
            var explorersWin = (endArg !== '0' && endArg !== 'false');
            endMatchWithWinner(gameStatePtr, explorersWin);
            return;
        }
        if (cmdName === 'summon' && (cmdArg === 'BP_HumanCharacter_C' || cmdArg === 'BP_HumanCharacter_Thrall_C')) {
            spawnSummonedMarine(gameStatePtr, targetController, cmdArg === 'BP_HumanCharacter_Thrall_C');
            return;
        }
        preloadSummonBlueprint(cmdName, cmdArg);
        if (!targetController.isNull() && targetController.add(UE_OFFSETS.playerControllerCheatManager).readPointer().isNull()) {
            ADH_PlayerController_AddCheats(targetController, 1);
        }
        var commandFString = allocateFString(normalizedCommand.length + 10);
        writeFStringValue(commandFString, normalizedCommand);
        UKismetSystemLibrary_ExecuteConsoleCommand(gameStatePtr, commandFString, targetController);
    } else if (action === 'add_inventory_item') {
        if (typeof payload.itemBlueprintPath === 'string' && typeof payload.itemCount === 'number' && Number.isFinite(payload.itemCount) && payload.itemCount > 0) {
            giveInventoryItem(targetController, payload.itemBlueprintPath, Math.floor(payload.itemCount));
        }
    } else if (action === 'set_storage_limit') {
        if (typeof payload.storageLimit === 'number' && Number.isFinite(payload.storageLimit)) {
            updateStorageLimit(targetController, Math.floor(payload.storageLimit));
        }
    } else if (action === 'kick_player') {
        kickTargetPlayer(gameStatePtr, targetController);
    } else if (action === 'send_thrall_message') {
        if (typeof payload.message === 'string') {
            deliverThrallMessage(targetController, payload.message);
        }
    } else if (action === 'set_human_property') {
        if (typeof payload.propertyName === 'string' && typeof payload.propertyValue === 'number' && Number.isFinite(payload.propertyValue)) {
            setHumanCharacterProperty(targetController, payload.propertyName, payload.propertyValue);
        }
    }
}

// ============================================================================
// ゲームフック
// ============================================================================
Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            var gameStatePtr = args[0];
            if (gameStatePtr.isNull()) {
                return;
            }
            if (!isBridgeServerRunning && Date.now() >= nextBridgeRetryAt) {
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
        } catch (e) {}
    }
});

Interceptor.attach(APlayerController_BeginPlay_addr, {
    onEnter: function (args) {
        this.playerControllerPtr = args[0];
    },
    onLeave: function () {
        try {
            if (this.playerControllerPtr && !this.playerControllerPtr.isNull() && !isPendingKill(this.playerControllerPtr)) {
                ADH_PlayerController_AddCheats(this.playerControllerPtr, 1);
            }
        } catch (e) {}
    }
});

// ============================================================================
// ローカルループバックTCPブリッジサーバー (UTF-16LE JSONを受信)
// ============================================================================
var tcpListener = null;
var isBridgeServerRunning = false;
var nextBridgeRetryAt = 0;
var MAX_BRIDGE_PACKET_BYTES = 65536;

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

function decodeUtf16LeChunks(chunks, totalBytes) {
    if (totalBytes < 2) {
        return '';
    }
    var combined = new Uint8Array(totalBytes);
    var offset = 0;
    for (var i = 0; i < chunks.length; i++) {
        combined.set(chunks[i], offset);
        offset += chunks[i].byteLength;
    }
    var codeUnits = Math.floor(totalBytes / 2);
    var chars = [];
    for (var j = 0; j < codeUnits; j++) {
        var unit = combined[j * 2] | (combined[j * 2 + 1] << 8);
        if (unit === 0) {
            break;
        }
        chars.push(String.fromCharCode(unit));
    }
    return chars.join('');
}

function handleBridgeConnection(connection) {
    var chunks = [];
    var totalBytes = 0;
    var hasCarryByte = false;
    var carryByte = 0;

    function finishRead() {
        safeCloseConnection(connection);
        var rawJson = decodeUtf16LeChunks(chunks, totalBytes);
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
    }

    function readNext() {
        connection.input.read(4096).then(function (buffer) {
            if (!buffer || buffer.byteLength === 0) {
                finishRead();
                return;
            }
            var bytes = new Uint8Array(buffer);
            var remaining = MAX_BRIDGE_PACKET_BYTES - totalBytes;
            if (bytes.byteLength > remaining) {
                chunks.push(bytes.subarray(0, remaining));
                totalBytes += remaining;
                finishRead();
                return;
            }
            chunks.push(bytes);
            totalBytes += bytes.byteLength;

            // 2バイト整列されたUTF-16LEのNUL終端を検出したら即座に読み取り完了
            var hasNull = false;
            var pos = 0;
            if (hasCarryByte) {
                if (carryByte === 0 && bytes[0] === 0) {
                    hasNull = true;
                }
                hasCarryByte = false;
                pos = 1;
            }
            if (!hasNull) {
                var end = bytes.byteLength - 1;
                while (pos < end) {
                    if (bytes[pos] === 0 && bytes[pos + 1] === 0) {
                        hasNull = true;
                        break;
                    }
                    pos += 2;
                }
                if (!hasNull && pos === end) {
                    carryByte = bytes[pos];
                    hasCarryByte = true;
                }
            }
            if (hasNull) {
                finishRead();
            } else {
                readNext();
            }
        }).catch(function (err) {
            if (totalBytes >= 2) {
                finishRead();
            } else {
                safeCloseConnection(connection);
                logBridgeError(err);
            }
        });
    }

    readNext();
}

var MAX_CONSECUTIVE_ACCEPT_ERRORS = 5;

function acceptBridgeLoop(listener, consecutiveErrors) {
    var errorStreak = consecutiveErrors || 0;
    tcpListener = listener;
    listener.accept().then(function (connection) {
        handleBridgeConnection(connection);
        setImmediate(function () { acceptBridgeLoop(listener, 0); });
    }).catch(function (err) {
        var errMsg = err && err.message ? String(err.message) : '';
        if (errMsg.indexOf('cancelled') !== -1 || errMsg.indexOf('closed') !== -1) {
            isBridgeServerRunning = false;
            nextBridgeRetryAt = Date.now() + 1000;
            return;
        }
        logBridgeError(err);
        errorStreak += 1;
        if (errorStreak >= MAX_CONSECUTIVE_ACCEPT_ERRORS) {
            safeCloseConnection(listener);
            tcpListener = null;
            isBridgeServerRunning = false;
            nextBridgeRetryAt = Date.now() + 1000;
            return;
        }
        setTimeout(function () { acceptBridgeLoop(listener, errorStreak); }, 250);
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
            nextBridgeRetryAt = Date.now() + 1000;
            logBridgeError(err);
        });
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function () {
        try {
            if (!isBridgeServerRunning && Date.now() >= nextBridgeRetryAt) {
                startBridgeServer();
            }
        } catch (e) {}
    }
});
