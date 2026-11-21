// Mystery Mode (North Ghost / 极地恶灵 / ミステリーモード)
// Assigns 0 Thralls while displaying 2 Thralls in pre-game instructions, and drives
// AI ghost events (spells, ship/boiler explosions, and staggered random poisonings)
// safely on the UE4 Game Thread with proper per-match reset.

var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], 'win64');
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], 'win64');
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');

var ADH_SpellManager_CastSpell = new NativeFunction(base.add(0xE634A0), 'pointer', ['pointer', 'pointer', 'pointer'], 'win64');
var ADH_SpellManager_CanCast = new NativeFunction(base.add(0xE62E30), 'int8', ['pointer', 'pointer'], 'win64');
var ADH_SpellManager_SetEquippedSpells = new NativeFunction(base.add(0xE81F00), 'pointer', ['pointer', 'pointer'], 'win64');
var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], 'win64');
var ADH_SpellManager_SetSpellChargeTier = new NativeFunction(base.add(0xE82CF0), 'void', ['pointer', 'int8'], 'win64');
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');

var FTransform_Identity = base.add(0x455C4E0);
var GWorld = base.add(0x46ED420);

var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_PlayerController_ShowPreGameInstructions_addr = base.add(0xEE7E50);
var ADH_GameMode_RandomizeThralls_addr = base.add(0xDA4620);
var ADH_PlayerState_SetIsThrall_addr = base.add(0xE4E880);

// Verified UE4 struct offsets (DreadHungerServer-Win64-Shipping.exe)
var OFF_UOBJECT_CLASS_PRIVATE = 0x10;
var OFF_UOBJECT_NAME_PRIVATE = 0x18;
var OFF_ACTOR_ROLE = 0xF0;
var OFF_ACTOR_ROOT_COMPONENT = 0x130;
var OFF_SCENE_COMPONENT_WORLD_TRANSFORM = 0x1C0;
var OFF_GAMESTATE_PLAYER_ARRAY = 0x238;
var OFF_GAMESTATE_WARSHIP = 0x2B0;
var OFF_GAMESTATE_SHIP_ARRIVED = 0x3D0;
var OFF_GAMESTATE_WINNING_TEAM = 0x51C;
var OFF_WARSHIP_BOILER = 0x2E0;
var OFF_PLAYERSTATE_PAWN = 0x280;
var OFF_PLAYERSTATE_IS_DEAD = 0x570;
var OFF_PLAYERSTATE_IS_THRALL = 0x572;
var OFF_HUMAN_SHIP_BASE = 0x9F8;
var OFF_HUMAN_INSIDE_SHIP = 0xE44;
var OFF_HUMAN_POISONED = 0x1250;
var OFF_SPELLMANAGER_MAX_SPELLS = 0x228;
var OFF_SPELLMANAGER_CHARGE_LEVEL = 0x284;
var OFF_GAMEINSTANCE_THRALL_SPELLS = 0x440;
var ROLE_AUTHORITY = 3;

// Pacing configuration (seconds)
var CAST_INITIAL_MIN_SEC = 240;
var CAST_INITIAL_MAX_SEC = 360;
var CAST_COOLDOWN_MIN_SEC = 5;
var CAST_COOLDOWN_MAX_SEC = 20;

var POISON_INITIAL_MIN_SEC = 300;
var POISON_INITIAL_MAX_SEC = 520;
var POISON_COOLDOWN_MIN_SEC = 240;
var POISON_COOLDOWN_MAX_SEC = 480;
var MAX_POISON_EVENTS = 4;

var EXPLOSION_INITIAL_MIN_SEC = 600;
var EXPLOSION_INITIAL_MAX_SEC = 720;
var EXPLOSION_COOLDOWN_MIN_SEC = 480;
var EXPLOSION_COOLDOWN_MAX_SEC = 600;

function randomRange(minVal, maxVal) {
    return minVal + Math.random() * (maxVal - minVal);
}

function randomCastTier() {
    // ETotemSpellTiers: 2 = Tier 1, 3 = Tier 2
    return Math.floor(Math.random() * 2) + 2;
}

function shuffleInPlace(items) {
    for (var i = items.length - 1; i > 0; i--) {
        var j = Math.floor(Math.random() * (i + 1));
        var tmp = items[i];
        items[i] = items[j];
        items[j] = tmp;
    }
    return items;
}

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr('0xFFFFFFFFFFFFFFFF'), Buffer, 0);
}

function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}

function getClass(ClassName) {
    var UClassPtr = UClass_GetPrivateStaticClass();
    if (UClassPtr.isNull()) {
        return ptr(0);
    }
    var Clazz = findObjectByName(ClassName, UClassPtr);
    if (!Clazz.isNull()) {
        return Clazz;
    }
    return loadObjectByName(ClassName, UClassPtr);
}

function newFString(Length) {
    var FString = Memory.alloc(16 + Length * 2);
    FString.writePointer(FString.add(16));
    FString.add(8).writeU32(0);
    FString.add(12).writeU32(Length);
    return FString;
}

function getObjectName(UObject) {
    if (UObject.isNull()) {
        return '';
    }
    var NamePrivate = UObject.add(OFF_UOBJECT_NAME_PRIVATE);
    var NameBuffer = newFString(100);
    FName_GetPlainNameString(NamePrivate, NameBuffer);
    var DataPtr = NameBuffer.readPointer();
    if (DataPtr.isNull()) {
        return '';
    }
    var Name = DataPtr.readUtf16String();
    return Name ? Name : '';
}

function spawnActor(World, Clazz, Position, Owner) {
    if (World.isNull() || Clazz.isNull() || Position.isNull()) {
        return ptr(0);
    }
    var Parameters = Memory.alloc(0x30);
    FActorSpawnParameters_FActorSpawnParameters(Parameters);
    Parameters.add(0x10).writePointer(Owner);
    Parameters.add(0x28).writeU8(1); // ESpawnActorCollisionHandlingMethod::AlwaysSpawn
    return UWorld_SpawnActor(World, Clazz, Position, Parameters);
}

function getActorTransform(AActor) {
    if (AActor.isNull()) {
        return ptr(0);
    }
    var RootComponent = AActor.add(OFF_ACTOR_ROOT_COMPONENT).readPointer();
    if (RootComponent.isNull()) {
        return ptr(0);
    }
    return RootComponent.add(OFF_SCENE_COMPONENT_WORLD_TRANSFORM);
}

function getDistance(Transform1, Transform2) {
    if (Transform1.isNull() || Transform2.isNull()) {
        return 0;
    }
    var Translation1 = Transform1.add(0x10);
    var Translation2 = Transform2.add(0x10);
    var X = Translation1.readFloat() - Translation2.readFloat();
    var Y = Translation1.add(4).readFloat() - Translation2.add(4).readFloat();
    var Z = Translation1.add(8).readFloat() - Translation2.add(8).readFloat();
    return Math.sqrt(X * X + Y * Y + Z * Z);
}

function initSpellManager(Owner, SpellChargeLevel) {
    if (Owner.isNull()) {
        return ptr(0);
    }
    var World = UObject_GetWorld(Owner);
    if (World.isNull()) {
        return ptr(0);
    }
    var SpellManagerClass = getClass('/Game/Blueprints/Game/Totems/BP_PlayerSpellManager.BP_PlayerSpellManager_C');
    if (SpellManagerClass.isNull()) {
        return ptr(0);
    }
    var SpellManager = spawnActor(World, SpellManagerClass, FTransform_Identity, Owner);
    if (SpellManager.isNull()) {
        return ptr(0);
    }
    var GameInstance = UDH_GameInstance_GetInstance(SpellManager);
    if (GameInstance.isNull()) {
        return SpellManager;
    }
    SpellManager.add(OFF_SPELLMANAGER_MAX_SPELLS).writeU32(10); // Allow all 5 ThrallSpells including TS_Hush (index 4)
    var ThrallSpells = GameInstance.add(OFF_GAMEINSTANCE_THRALL_SPELLS);
    ADH_SpellManager_SetEquippedSpells(SpellManager, ThrallSpells);
    ADH_SpellManager_SetSpellChargeTier(SpellManager, 1 + SpellChargeLevel);
    return SpellManager;
}

// Per-match state (fully reset on every ADH_GameState::HandleMatchHasStarted)
var activeWorldKey = null;
var activeGameStateKey = null;
var lastTickMs = 0;
var spellManagers = [];

var castTimer = 0;
var castLevel = 2;
var castedTime = 0;

var poisonTimer = 0;
var poisonCount = 0;
var poisonedPlayerKeys = {};

var explosionTimer = 0;

function resetMatchState(gameState, world) {
    activeWorldKey = world.toString();
    activeGameStateKey = gameState.toString();
    lastTickMs = Date.now();
    spellManagers = [];

    for (var i = 0; i < 2; i++) {
        var manager = initSpellManager(gameState, 3);
        if (!manager.isNull()) {
            spellManagers.push(manager);
        }
    }

    castTimer = randomRange(CAST_INITIAL_MIN_SEC, CAST_INITIAL_MAX_SEC);
    castLevel = randomCastTier();
    castedTime = 0;

    poisonTimer = randomRange(POISON_INITIAL_MIN_SEC, POISON_INITIAL_MAX_SEC);
    poisonCount = 0;
    poisonedPlayerKeys = {};

    explosionTimer = randomRange(EXPLOSION_INITIAL_MIN_SEC, EXPLOSION_INITIAL_MAX_SEC);
}

function collectHumanCandidates(gameState, filterFn) {
    var results = [];
    var playerArray = gameState.add(OFF_GAMESTATE_PLAYER_ARRAY);
    var players = playerArray.readPointer();
    var playerNum = playerArray.add(8).readU32();
    if (players.isNull() || playerNum === 0 || playerNum > 64) {
        return results;
    }
    for (var j = 0; j < playerNum; j++) {
        var playerState = players.add(8 * j).readPointer();
        if (playerState.isNull()) {
            continue;
        }
        var isDead = playerState.add(OFF_PLAYERSTATE_IS_DEAD).readU8();
        var isThrall = playerState.add(OFF_PLAYERSTATE_IS_THRALL).readU8();
        if (isDead === 1 || isThrall === 1) {
            continue;
        }
        var pawn = playerState.add(OFF_PLAYERSTATE_PAWN).readPointer();
        if (pawn.isNull()) {
            continue;
        }
        var pawnClass = pawn.add(OFF_UOBJECT_CLASS_PRIVATE).readPointer();
        if (getObjectName(pawnClass) !== 'BP_HumanCharacter_C') {
            continue;
        }
        if (filterFn && !filterFn(playerState, pawn)) {
            continue;
        }
        results.push({ playerState: playerState, pawn: pawn });
    }
    return results;
}

function tryPoisonOneRandomPlayer(gameState) {
    var allCandidates = collectHumanCandidates(gameState, function (_ps, pawn) {
        return pawn.add(OFF_HUMAN_POISONED).readU8() === 0;
    });
    if (allCandidates.length === 0) {
        return false;
    }

    // Prioritize players who have not been poisoned yet this match for fairer randomness
    var freshCandidates = [];
    for (var i = 0; i < allCandidates.length; i++) {
        var key = allCandidates[i].playerState.toString();
        if (!poisonedPlayerKeys[key]) {
            freshCandidates.push(allCandidates[i]);
        }
    }

    var pool = freshCandidates.length > 0 ? freshCandidates : allCandidates;
    shuffleInPlace(pool);

    var chosen = pool[Math.floor(Math.random() * pool.length)];
    chosen.pawn.add(OFF_HUMAN_POISONED).writeU8(1);
    poisonedPlayerKeys[chosen.playerState.toString()] = true;
    poisonCount += 1;
    return true;
}

function runGhostTick(gameState, world, deltaSec) {
    var warship = gameState.add(OFF_GAMESTATE_WARSHIP).readPointer();
    if (warship.isNull()) {
        return;
    }
    var boiler = warship.add(OFF_WARSHIP_BOILER).readPointer();
    if (boiler.isNull()) {
        return;
    }

    var warshipPos = getActorTransform(warship);
    var boilerPos = getActorTransform(boiler);

    // 1. Periodic random ship / boiler gunpowder explosion
    explosionTimer -= deltaSec;
    if (explosionTimer <= 0.0) {
        explosionTimer = randomRange(EXPLOSION_COOLDOWN_MIN_SEC, EXPLOSION_COOLDOWN_MAX_SEC);
        var explosionClass = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C');
        if (!explosionClass.isNull()) {
            var spawnPos = Math.random() < 0.5 ? warshipPos : boilerPos;
            if (!spawnPos.isNull()) {
                spawnActor(world, explosionClass, spawnPos, ptr(0));
            }
        }
    }

    // 2. Strictly staggered single-target random poisoning (never poisons multiple players at once)
    if (poisonCount < MAX_POISON_EVENTS) {
        poisonTimer -= deltaSec;
        if (poisonTimer <= 0.0) {
            tryPoisonOneRandomPlayer(gameState);
            poisonTimer = randomRange(POISON_COOLDOWN_MIN_SEC, POISON_COOLDOWN_MAX_SEC);
        }
    }

    // 3. Recharge AI ghost SpellManagers
    var effectiveCasts = castedTime > 6 ? 6 : castedTime;
    var chargeGain = deltaSec / (1500.0 - effectiveCasts * 100.0);
    for (var i = 0; i < spellManagers.length; i++) {
        var manager = spellManagers[i];
        if (manager.isNull()) {
            continue;
        }
        var nextCharge = manager.add(OFF_SPELLMANAGER_CHARGE_LEVEL).readFloat() + chargeGain;
        manager.add(OFF_SPELLMANAGER_CHARGE_LEVEL).writeFloat(nextCharge >= 1.0 ? 1.0 : nextCharge);
    }

    // 4. Cast AI ghost spells on isolated players / global spells
    castTimer -= deltaSec;
    if (castTimer <= 0.0) {
        castTimer = randomRange(CAST_COOLDOWN_MIN_SEC, CAST_COOLDOWN_MAX_SEC);
        var outdoorCandidates = collectHumanCandidates(gameState, function (_ps, pawn) {
            var shipBase = pawn.add(OFF_HUMAN_SHIP_BASE).readPointer();
            var bInsideShip = pawn.add(OFF_HUMAN_INSIDE_SHIP).readU8();
            return shipBase.isNull() && bInsideShip === 0;
        });

        var maxDist = 0;
        var farthestTarget = ptr(0);
        if (!warshipPos.isNull()) {
            for (var j = 0; j < outdoorCandidates.length; j++) {
                var pos = getActorTransform(outdoorCandidates[j].pawn);
                var dist = getDistance(warshipPos, pos);
                if (dist >= maxDist && dist >= 1000) {
                    maxDist = dist;
                    farthestTarget = outdoorCandidates[j].pawn;
                }
            }
        }

        for (var k = 0; k < spellManagers.length; k++) {
            var spellManager = spellManagers[k];
            if (spellManager.isNull()) {
                continue;
            }

            if (!farthestTarget.isNull()) {
                var cannibalSpell = getClass('/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C');
                if (!cannibalSpell.isNull() && ADH_SpellManager_CanCast(spellManager, cannibalSpell) === 1) {
                    castedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(spellManager, castLevel);
                    ADH_SpellManager_CastSpell(spellManager, cannibalSpell, farthestTarget);
                    castLevel = randomCastTier();
                    break;
                }
            }

            if (Math.random() > 0.2) {
                var whiteoutSpell = getClass('/Game/Blueprints/Game/Totems/TS_Whiteout.TS_Whiteout_C');
                if (!whiteoutSpell.isNull() && ADH_SpellManager_CanCast(spellManager, whiteoutSpell) === 1) {
                    castedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(spellManager, castLevel);
                    ADH_SpellManager_CastSpell(spellManager, whiteoutSpell, farthestTarget);
                    castLevel = randomCastTier();
                    break;
                }
            } else {
                var hushSpell = getClass('/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C');
                if (!hushSpell.isNull() && ADH_SpellManager_CanCast(spellManager, hushSpell) === 1) {
                    castedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(spellManager, castLevel);
                    ADH_SpellManager_CastSpell(spellManager, hushSpell, farthestTarget);
                    castLevel = randomCastTier();
                    break;
                }
            }
        }
    }
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function (args) {
        this.gameState = args[0];
    },
    onLeave: function () {
        try {
            var gameState = this.gameState;
            var world = GWorld.readPointer();
            if (!gameState || gameState.isNull() || world.isNull()) {
                activeWorldKey = null;
                activeGameStateKey = null;
                return;
            }
            if (gameState.add(OFF_ACTOR_ROLE).readU8() !== ROLE_AUTHORITY) {
                return;
            }
            resetMatchState(gameState, world);
        } catch (e) {}
    }
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            if (activeWorldKey === null) {
                return;
            }
            var gameState = args[0];
            if (gameState.isNull() || gameState.toString() !== activeGameStateKey) {
                return;
            }
            if (gameState.add(OFF_ACTOR_ROLE).readU8() !== ROLE_AUTHORITY) {
                return;
            }
            var world = GWorld.readPointer();
            if (world.isNull() || world.toString() !== activeWorldKey) {
                activeWorldKey = null;
                activeGameStateKey = null;
                spellManagers = [];
                return;
            }
            if (
                gameState.add(OFF_GAMESTATE_SHIP_ARRIVED).readU8() !== 0 ||
                gameState.add(OFF_GAMESTATE_WINNING_TEAM).readU8() !== 0
            ) {
                activeWorldKey = null;
                activeGameStateKey = null;
                spellManagers = [];
                return;
            }

            var nowMs = Date.now();
            var deltaSec = (nowMs - lastTickMs) / 1000.0;
            lastTickMs = nowMs;
            if (!Number.isFinite(deltaSec) || deltaSec <= 0) {
                return;
            }
            if (deltaSec > 1.0) {
                deltaSec = 1.0;
            }

            runGhostTick(gameState, world, deltaSec);
        } catch (e) {}
    }
});

// Tell players there are 2 Thralls in the pre-game briefing
Interceptor.attach(ADH_PlayerController_ShowPreGameInstructions_addr, {
    onEnter: function (args) {
        args[1] = ptr(2);
    }
});

// Prevent RandomizeThralls from assigning any real Thralls without double-replacing SetIsThrall
var inRandomizeThralls = false;
Interceptor.attach(ADH_GameMode_RandomizeThralls_addr, {
    onEnter: function () {
        inRandomizeThralls = true;
    },
    onLeave: function () {
        inRandomizeThralls = false;
    }
});

Interceptor.attach(ADH_PlayerState_SetIsThrall_addr, {
    onEnter: function (args) {
        if (inRandomizeThralls) {
            args[1] = ptr(0);
        }
    }
});
