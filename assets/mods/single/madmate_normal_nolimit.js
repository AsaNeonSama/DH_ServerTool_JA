var js_name = "madmate.js";
var log_output = false; // true / false

function logInfo(Info) {
    if (!log_output) {
        return;
    }
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches:' + js_name + '] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

logInfo("START");

// ============================================================================
// Configuration Flags
// ============================================================================
var randomRole_on = false;
var mad_on = true;
var leader_on = true;
var thrall_p_on = true;
var thrall_c_on = true;

var SpellSelection = [
    [0, 4, 3, 1, 2]
];
/*
SpellSelection:
0: 傀儡召喚
1: ホワイトアウト
2: ドッペルゲンガー
3: 幽霊歩き
4: 沈黙
*/
var MaxSpellsConfigured = 0;
for (var si = 0; si < SpellSelection.length; si++) {
    if (SpellSelection[si].length > MaxSpellsConfigured) {
        MaxSpellsConfigured = SpellSelection[si].length;
    }
}

var ROLE_JAPANESE_NAMES = {
    Captain: "船長",
    Engineer: "技師",
    Hunter: "猟師",
    Cook: "料理人",
    Navigator: "航海士",
    Chaplain: "牧師",
    Marine: "海兵",
    Doctor: "医師"
};

var STORAGE_LIMIT_MAD = 8;
var STORAGE_LIMIT_LEADER = 10;
var STORAGE_LIMIT_THRALL = 8;

// ============================================================================
// Native Bindings & Addresses
// ============================================================================
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var GWorld = base.add(0x46ED420);

var ADH_PlayerState_SetPlayerRole_addr = base.add(0xE4F390);
var ADH_PlayerState_SetIsDead_addr = base.add(0xE4E700);
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var SpellManagerInit_addr = base.add(0xE58AC0);
var ADH_SpellManager_SetEquippedSpells_addr = base.add(0xE81F00);
var ADH_SpellManager_CastSpell_addr = base.add(0xE634F0);

var UDH_PlayerRoleData_FindByType = new NativeFunction(base.add(0xE33300), 'pointer', ['char', 'pointer'], 'win64');
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], 'win64');
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(base.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');
var ADH_PlayerState_GetOwningController = new NativeFunction(base.add(0xE39820), 'pointer', ['pointer'], 'win64');
var FName_FName = new NativeFunction(base.add(0x1158F20), 'void', ['pointer', 'pointer', 'int8']);
var FText_FromName = new NativeFunction(base.add(0x1096370), 'pointer', ['pointer', 'pointer']);
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), 'void', ['pointer', 'pointer', 'pointer']);

// ============================================================================
// Per-Match Mutable State (automatically reset whenever GWorld changes)
// ============================================================================
var activeWorldKey = null;
var availableRandomRoles = [1, 2, 3, 4, 5, 6, 7, 8];
var rolesAssigned = false;
var blizzardAnnounced = false;
var parentDeathCount = 0;
var parentDeath1Announced = false;
var parentDeath2Announced = false;

var thrallParentKey = "";
var thrallChildKey = "";
var madmateKey = "";
var leaderKey = "";
var madmateRoleName = "";
var leaderRoleName = "";
var thrallRoleNames = [];

var notifiedPlayerKeys = {};
var appliedStorageKeys = {};
var activeMessageTimers = [];
var spellPresetIndex = 0;

function clearAllMessageTimers() {
    for (var i = 0; i < activeMessageTimers.length; i++) {
        try {
            clearInterval(activeMessageTimers[i]);
        } catch (e) {}
    }
    activeMessageTimers = [];
}

function resetMatchState(newWorldKey) {
    clearAllMessageTimers();
    activeWorldKey = newWorldKey;
    availableRandomRoles = [1, 2, 3, 4, 5, 6, 7, 8];
    rolesAssigned = false;
    blizzardAnnounced = false;
    parentDeathCount = 0;
    parentDeath1Announced = false;
    parentDeath2Announced = false;

    thrallParentKey = "";
    thrallChildKey = "";
    madmateKey = "";
    leaderKey = "";
    madmateRoleName = "";
    leaderRoleName = "";
    thrallRoleNames = [];

    notifiedPlayerKeys = {};
    appliedStorageKeys = {};
    logInfo("Match state reset for world: " + newWorldKey);
}

function ensureCurrentWorld() {
    var world = GWorld.readPointer();
    if (world.isNull()) {
        return null;
    }
    var worldKey = world.toString();
    if (activeWorldKey !== worldKey) {
        resetMatchState(worldKey);
    }
    return world;
}

// ============================================================================
// UE4 Safe Helpers
// ============================================================================
function getArraySize(TArray) {
    if (TArray.isNull()) {
        return 0;
    }
    return TArray.add(8).readU32();
}

function getArrayItemAddr(TArray, size, index) {
    var arrNum = getArraySize(TArray);
    if (index < 0 || index >= arrNum) {
        return null;
    }
    var dataPtr = TArray.readPointer();
    if (dataPtr.isNull()) {
        return null;
    }
    return dataPtr.add(index * size);
}

function getString(FString) {
    var size = getArraySize(FString);
    if (size === 0) {
        return "";
    }
    var dataPtr = FString.readPointer();
    if (dataPtr.isNull()) {
        return "";
    }
    return dataPtr.readUtf16String(size).replace(/\0+$/, "");
}

function getRoleOnlineName(PlayerState) {
    if (PlayerState.isNull()) {
        return "";
    }
    var selectedRole = PlayerState.add(0x590).readPointer();
    if (selectedRole.isNull()) {
        return "";
    }
    return getString(selectedRole.add(0x48));
}

function getJapaneseName(onlineName) {
    if (!onlineName) {
        return "なし";
    }
    return ROLE_JAPANESE_NAMES[onlineName] || onlineName;
}

function getLiveGameState() {
    var world = GWorld.readPointer();
    if (world.isNull()) {
        return ptr(0);
    }
    var gameMode = world.add(0x118).readPointer();
    if (gameMode.isNull()) {
        return ptr(0);
    }
    return gameMode.add(0x280).readPointer();
}

function findLivePlayerStateByKey(gameState, targetKey) {
    if (gameState.isNull() || !targetKey) {
        return ptr(0);
    }
    var playerArray = gameState.add(0x238);
    var count = getArraySize(playerArray);
    for (var i = 0; i < count; i++) {
        var entry = getArrayItemAddr(playerArray, 8, i);
        if (entry === null) {
            continue;
        }
        var ps = entry.readPointer();
        if (!ps.isNull() && ps.toString() === targetKey) {
            return ps;
        }
    }
    return ptr(0);
}

function resolvePlayerController(PlayerState) {
    if (PlayerState.isNull()) {
        return ptr(0);
    }
    var ctrl = ptr(0);
    try {
        ctrl = ADH_PlayerState_GetOwningController(PlayerState);
    } catch (e) {}
    if (ctrl.isNull()) {
        ctrl = PlayerState.add(0xE0).readPointer();
    }
    return ctrl;
}

function findClassByName(ClassName) {
    var buffer = Memory.alloc((ClassName.length + 1) * 2);
    buffer.writeUtf16String(ClassName);
    return StaticFindObject(UClass_GetPrivateStaticClass(), ptr(0xFFFFFFFFFFFFFFFF), buffer, 0);
}

function shuffleArray(array) {
    for (var i = array.length - 1; i > 0; i--) {
        var j = Math.floor(Math.random() * (i + 1));
        var temp = array[i];
        array[i] = array[j];
        array[j] = temp;
    }
    return array;
}

function newFName(Name) {
    var fNameBuffer = Memory.alloc(8);
    var buffer = Memory.alloc((Name.length + 4) * 2);
    buffer.writeUtf16String("   " + Name);
    FName_FName(fNameBuffer, buffer, 1);
    return fNameBuffer;
}

function fNameToFText(FName) {
    var fTextBuffer = Memory.alloc(24);
    FText_FromName(fTextBuffer, FName);
    return fTextBuffer;
}

function sendThrallMessage(PlayerController, Message) {
    if (PlayerController.isNull()) {
        return;
    }
    try {
        var fNameMsg = newFName(Message);
        var fTextMsg = fNameToFText(fNameMsg);
        ADH_PlayerController_ReceiveThrallMessage(PlayerController, fTextMsg, ptr(0));
    } catch (e) {}
}

// Safely repeats notification up to 5 times every 10s, validating PlayerState is
// still live in the same match World before each tick.
function sendRepeatedMessagesToPlayerKey(targetWorldKey, playerStateKey, messageList) {
    if (!playerStateKey || !messageList || messageList.length === 0) {
        return;
    }
    var sentCount = 0;
    var timerHandle = null;

    function dispatchOnce() {
        if (activeWorldKey !== targetWorldKey) {
            if (timerHandle !== null) {
                clearInterval(timerHandle);
            }
            return;
        }
        var gameState = getLiveGameState();
        var livePs = findLivePlayerStateByKey(gameState, playerStateKey);
        if (livePs.isNull() || livePs.add(0x571).readU8() !== 0) {
            if (timerHandle !== null) {
                clearInterval(timerHandle);
            }
            return;
        }
        var ctrl = resolvePlayerController(livePs);
        if (!ctrl.isNull()) {
            for (var i = 0; i < messageList.length; i++) {
                sendThrallMessage(ctrl, messageList[i]);
            }
        }
        sentCount++;
        if (sentCount >= 5 && timerHandle !== null) {
            clearInterval(timerHandle);
        }
    }

    dispatchOnce();
    timerHandle = setInterval(dispatchOnce, 10000);
    activeMessageTimers.push(timerHandle);
}

// ============================================================================
// Role Assignment (works cleanly for any player count, including 1-3p testing)
// ============================================================================
function tryAssignSpecialRoles(GameState) {
    if (rolesAssigned || GameState.isNull()) {
        return;
    }
    var playerArray = GameState.add(0x238);
    var totalPlayers = getArraySize(playerArray);
    if (totalPlayers === 0) {
        return;
    }

    var thralls = [];
    var explorers = [];
    var readyCount = 0;

    for (var i = 0; i < totalPlayers; i++) {
        var entry = getArrayItemAddr(playerArray, 8, i);
        if (entry === null) {
            continue;
        }
        var ps = entry.readPointer();
        if (ps.isNull()) {
            continue;
        }
        var roleName = getRoleOnlineName(ps);
        if (!roleName) {
            continue;
        }
        readyCount++;

        var isDead = ps.add(0x570).readU8();
        var isDisconnected = ps.add(0x571).readU8();
        var isThrall = ps.add(0x572).readU8();
        if (isDead !== 0 || isDisconnected !== 0) {
            continue;
        }

        var record = { ps: ps, key: ps.toString(), roleName: roleName };
        if (isThrall !== 0) {
            thralls.push(record);
            if (thrallRoleNames.indexOf(roleName) === -1) {
                thrallRoleNames.push(roleName);
            }
        } else {
            explorers.push(record);
        }
    }

    // Wait until every connected player has a SelectedRole assigned
    if (readyCount < totalPlayers) {
        return;
    }

    shuffleArray(thralls);
    shuffleArray(explorers);

    if (thrall_p_on && thralls.length >= 1) {
        thrallParentKey = thralls[0].key;
    }
    if (thrall_c_on && thralls.length >= 2) {
        thrallChildKey = thralls[1].key;
    }
    if (mad_on && explorers.length >= 1) {
        madmateKey = explorers[0].key;
        madmateRoleName = explorers[0].roleName;
    }
    if (leader_on && explorers.length >= 2) {
        leaderKey = explorers[1].key;
        leaderRoleName = explorers[1].roleName;
    }

    rolesAssigned = true;
    logInfo(
        "Roles assigned (players=" + totalPlayers +
        ", thralls=" + thralls.length +
        ", explorers=" + explorers.length +
        ", mad=" + madmateRoleName +
        ", leader=" + leaderRoleName + ")"
    );
}

function applyStorageAndNoticeForPlayer(HumanCharacter, PlayerState) {
    if (!rolesAssigned || HumanCharacter.isNull() || PlayerState.isNull()) {
        return;
    }
    var psKey = PlayerState.toString();
    var inventoryComponent = HumanCharacter.add(0x808).readPointer();
    var isThrall = PlayerState.add(0x572).readU8();

    if (!inventoryComponent.isNull() && !appliedStorageKeys[psKey]) {
        if (psKey === madmateKey) {
            UDH_InventoryManager_SetStorageLimit(inventoryComponent, STORAGE_LIMIT_MAD);
            appliedStorageKeys[psKey] = true;
        } else if (psKey === leaderKey) {
            UDH_InventoryManager_SetStorageLimit(inventoryComponent, STORAGE_LIMIT_LEADER);
            appliedStorageKeys[psKey] = true;
        } else if (isThrall !== 0 && (thrall_p_on || thrall_c_on)) {
            UDH_InventoryManager_SetStorageLimit(inventoryComponent, STORAGE_LIMIT_THRALL);
            appliedStorageKeys[psKey] = true;
        }
    }

    if (!notifiedPlayerKeys[psKey]) {
        if (psKey === madmateKey) {
            notifiedPlayerKeys[psKey] = true;
            sendRepeatedMessagesToPlayerKey(activeWorldKey, psKey, [
                "狂人として探検家を惑わせろ"
            ]);
        } else if (psKey === leaderKey) {
            notifiedPlayerKeys[psKey] = true;
            sendRepeatedMessagesToPlayerKey(activeWorldKey, psKey, [
                "仲間を先導し闇の傀儡を暴け"
            ]);
        } else if (psKey === thrallParentKey) {
            notifiedPlayerKeys[psKey] = true;
            sendRepeatedMessagesToPlayerKey(activeWorldKey, psKey, [
                "傀儡の親としていつも通りにやればいい"
            ]);
        } else if (psKey === thrallChildKey) {
            notifiedPlayerKeys[psKey] = true;
            sendRepeatedMessagesToPlayerKey(activeWorldKey, psKey, [
                "傀儡の子として親の背中を追え"
            ]);
        }
    }
}

function applyPendingStorageAndNotices(GameState) {
    if (!rolesAssigned || GameState.isNull()) {
        return;
    }
    var playerArray = GameState.add(0x238);
    var count = getArraySize(playerArray);
    for (var i = 0; i < count; i++) {
        var entry = getArrayItemAddr(playerArray, 8, i);
        if (entry === null) {
            continue;
        }
        var ps = entry.readPointer();
        if (ps.isNull()) {
            continue;
        }
        var pawn = ps.add(0x280).readPointer();
        if (!pawn.isNull()) {
            applyStorageAndNoticeForPlayer(pawn, ps);
        }
    }
}

// ============================================================================
// Hooks: Role Selection, Starting Inventory, 5-Spell Loadout, Death, Tick, Cast
// ============================================================================
Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function () {
        ensureCurrentWorld();
    }
});

Interceptor.attach(ADH_PlayerState_SetPlayerRole_addr, {
    onEnter: function (args) {
        ensureCurrentWorld();
        var playerState = args[0];
        var roleData = args[1];
        if (playerState.isNull() || roleData.isNull()) {
            return;
        }
        if (randomRole_on && availableRandomRoles.length > 0) {
            var index = Math.floor(Math.random() * availableRandomRoles.length);
            var roleId = availableRandomRoles[index];
            var newRoleData = UDH_PlayerRoleData_FindByType(roleId, playerState);
            if (!newRoleData.isNull()) {
                availableRandomRoles.splice(index, 1);
                args[1] = newRoleData;
            }
        }
    }
});

Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function (args) {
        ensureCurrentWorld();
        var humanCharacter = args[0];
        if (humanCharacter.isNull()) {
            return;
        }
        var playerState = humanCharacter.add(0x240).readPointer();
        if (playerState.isNull()) {
            return;
        }

        // Slight initial spell charge boost (+0.05)
        var spellManager = playerState.add(0x488).readPointer();
        if (!spellManager.isNull()) {
            var spellChargePtr = spellManager.add(0x284);
            spellChargePtr.writeFloat(spellChargePtr.readFloat() + 0.05);
        }

        var gameState = getLiveGameState();
        if (!gameState.isNull()) {
            tryAssignSpecialRoles(gameState);
            applyStorageAndNoticeForPlayer(humanCharacter, playerState);
        }
    }
});

// 5-spell loadout for Thralls (preserves higher MaxSpells if another MOD raised it)
Interceptor.attach(SpellManagerInit_addr, {
    onEnter: function (args) {
        this.spellManager = args[0];
    },
    onLeave: function () {
        var sm = this.spellManager;
        if (sm.isNull()) {
            return;
        }
        var currentMax = sm.add(0x228).readU32();
        if (currentMax < MaxSpellsConfigured) {
            sm.add(0x228).writeU32(MaxSpellsConfigured);
        }
    }
});

var spellListBuffer = Memory.alloc(0x100);
spellListBuffer.writePointer(spellListBuffer.add(16));
spellListBuffer.add(12).writeU32(MaxSpellsConfigured);

Interceptor.attach(ADH_SpellManager_SetEquippedSpells_addr, {
    onEnter: function (args) {
        var spellManager = args[0];
        if (spellManager.isNull()) {
            return;
        }
        var gameInstance = UDH_GameInstance_GetInstance(spellManager);
        if (gameInstance.isNull()) {
            return;
        }
        var thrallSpells = gameInstance.add(0x440);
        var spellPoolSize = getArraySize(thrallSpells);
        if (spellPoolSize === 5) {
            if (spellPresetIndex >= SpellSelection.length) {
                spellPresetIndex = 0;
            }
            var preset = SpellSelection[spellPresetIndex];
            spellListBuffer.add(8).writeU32(preset.length);
            for (var i = 0; i < preset.length; i++) {
                if (preset[i] >= spellPoolSize) {
                    return;
                }
                var spellTypePtr = getArrayItemAddr(thrallSpells, 8, preset[i]);
                if (spellTypePtr === null) {
                    return;
                }
                spellListBuffer.add(16 + 8 * i).writePointer(spellTypePtr.readPointer());
            }
            args[1] = spellListBuffer;
            spellPresetIndex++;
        }
    }
});

Interceptor.attach(ADH_PlayerState_SetIsDead_addr, {
    onEnter: function (args) {
        var deadFlag = args[1].toInt32() & 1;
        if (deadFlag === 0) {
            return;
        }
        var playerState = args[0];
        if (playerState.isNull()) {
            return;
        }
        if (thrallParentKey && playerState.toString() === thrallParentKey) {
            parentDeathCount++;
            logInfo("Thrall parent death count: " + parentDeathCount);
        }
    }
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        var gameState = args[0];
        if (gameState.isNull()) {
            return;
        }
        ensureCurrentWorld();

        if (!rolesAssigned) {
            tryAssignSpecialRoles(gameState);
        }
        if (rolesAssigned) {
            applyPendingStorageAndNotices(gameState);
        }

        // Notify Madmate when Thrall Parent dies (1st and 2nd death, once each)
        if (madmateKey) {
            if (parentDeathCount === 1 && !parentDeath1Announced) {
                parentDeath1Announced = true;
                sendRepeatedMessagesToPlayerKey(activeWorldKey, madmateKey, [
                    "力が湧いてきたようだ"
                ]);
            } else if (parentDeathCount >= 2 && !parentDeath2Announced) {
                parentDeath2Announced = true;
                sendRepeatedMessagesToPlayerKey(activeWorldKey, madmateKey, [
                    "今こそ復讐の時だ"
                ]);
            }
        }

        // Reveal Madmate & Thralls to Leader when Blizzard begins
        var blizzardStarted = gameState.add(0x359).readU8();
        if (!blizzardAnnounced && blizzardStarted === 1) {
            blizzardAnnounced = true;
            if (leaderKey) {
                var jpThralls = thrallRoleNames.map(getJapaneseName);
                var jpMadmate = getJapaneseName(madmateRoleName);
                var leaderMessages = [
                    "",
                    "狂人：" + jpMadmate,
                    "傀儡：" + (jpThralls.length > 0 ? jpThralls.join(", ") : "なし")
                ];
                sendRepeatedMessagesToPlayerKey(activeWorldKey, leaderKey, leaderMessages);
            }
        }
    }
});

// ============================================================================
// Thrall Child Spell Restrictions (unlock progressively as Parent dies)
// ============================================================================
var TOTEM_SPELL_PATHS = {
    spiritWalk: '/Game/Blueprints/Game/Totems/TS_SpiritWalk.TS_SpiritWalk_C',
    whiteout: '/Game/Blueprints/Game/Totems/TS_Whiteout.TS_Whiteout_C',
    hush: '/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C',
    doppelganger: '/Game/Blueprints/Game/Totems/TS_Doppelganger.TS_Doppelganger_C',
    cannibalAttack: '/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C'
};

var spellClassesInitialized = false;
var WhiteoutClass = ptr(0);
var HushClass = ptr(0);
var DoppelgangerClass = ptr(0);
var CannibalAttackClass = ptr(0);

function initSpellClasses() {
    var whiteout = findClassByName(TOTEM_SPELL_PATHS.whiteout);
    var hush = findClassByName(TOTEM_SPELL_PATHS.hush);
    var doppelganger = findClassByName(TOTEM_SPELL_PATHS.doppelganger);
    var cannibal = findClassByName(TOTEM_SPELL_PATHS.cannibalAttack);
    if (whiteout.isNull() || hush.isNull() || doppelganger.isNull() || cannibal.isNull()) {
        return false;
    }
    WhiteoutClass = whiteout;
    HushClass = hush;
    DoppelgangerClass = doppelganger;
    CannibalAttackClass = cannibal;
    return true;
}

Interceptor.attach(ADH_SpellManager_CastSpell_addr, {
    onEnter: function (args) {
        if (!thrallChildKey) {
            return;
        }
        if (!spellClassesInitialized) {
            if (!initSpellClasses()) {
                return;
            }
            spellClassesInitialized = true;
        }

        var spellManager = args[0];
        var spellType = args[2];
        if (spellManager.isNull() || spellType.isNull()) {
            return;
        }
        var playerState = spellManager.add(0xE0).readPointer();
        if (playerState.isNull() || playerState.toString() !== thrallChildKey) {
            return;
        }

        var isDead = playerState.add(0x570).readU8();
        if (isDead !== 0) {
            return;
        }

        var playerController = resolvePlayerController(playerState);
        if (playerController.isNull()) {
            return;
        }
        var controlledHuman = playerController.add(0x588).readPointer();
        if (controlledHuman.isNull()) {
            return;
        }

        var blocked = false;
        if (parentDeathCount < 2 && (spellType.equals(WhiteoutClass) || spellType.equals(CannibalAttackClass))) {
            blocked = true;
        } else if (parentDeathCount < 1 && (spellType.equals(HushClass) || spellType.equals(DoppelgangerClass))) {
            blocked = true;
        }

        if (blocked) {
            args[2] = ptr(0);
            var healthBoostRemaining = controlledHuman.add(0xFFC);
            healthBoostRemaining.writeFloat(healthBoostRemaining.readFloat() + 8.0);
        }
    }
});
