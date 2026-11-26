// 開幕沈黙 (試合開始0.5秒後にTier1の沈黙を自動発動)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], { abi: 'win64', exceptions: 'propagate' });

var ADH_SpellManager_CastSpell = new NativeFunction(base.add(0xE634A0), 'pointer', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_SpellManager_CastSpellStatic = new NativeFunction(base.add(0xE63990), 'pointer', ['pointer', 'pointer', 'pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
var ADH_SpellManager_SetEquippedSpells = new NativeFunction(base.add(0xE81F00), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_SpellManager_SetSpellChargeTier = new NativeFunction(base.add(0xE82CF0), 'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });

var FTransform_Identity = base.add(0x455C4E0);
var GWorld = base.add(0x46ED420);
var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var ADH_GameState_Tick_addr = base.add(0xDAD420);

// ETotemSpellTiers: 0=未定義, 1=Tier0, 2=Tier1(30秒), 3=Tier2(45秒), 4=Tier3(60秒)
var SPELL_TIER_LEVEL_1 = 2;
var HUSH_SPELL_CLASS = '/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C';
var SPELL_MANAGER_CLASS = '/Game/Blueprints/Game/Totems/BP_PlayerSpellManager.BP_PlayerSpellManager_C';

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
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

function spawnActor(World, Clazz, Position, Owner) {
    if (World.isNull() || Clazz.isNull() || Position.isNull()) {
        return ptr(0);
    }
    var Parameters = Memory.alloc(0x30);
    FActorSpawnParameters_FActorSpawnParameters(Parameters);
    Parameters.add(0x10).writePointer(Owner);
    Parameters.add(0x28).writeU8(1); // 衝突を無視して常にスポーン (AlwaysSpawn)
    return UWorld_SpawnActor(World, Clazz, Position, Parameters);
}

var ActiveSpellManagers = [];

function castOpeningHush(GameState) {
    if (GameState.isNull()) {
        return false;
    }
    var World = UObject_GetWorld(GameState);
    if (World.isNull()) {
        return false;
    }
    var SpellManagerClass = getClass(SPELL_MANAGER_CLASS);
    var SpellClass = getClass(HUSH_SPELL_CLASS);
    if (SpellManagerClass.isNull() || SpellClass.isNull()) {
        return false;
    }

    // 沈黙未装備でも全クライアントへ ActiveSpells を同期するため、ワールド所有の SpellManager を生成
    var SpellManager = spawnActor(World, SpellManagerClass, FTransform_Identity, GameState);
    if (SpellManager.isNull()) {
        return false;
    }
    ActiveSpellManagers.push(SpellManager);

    // ワールド所有の一時 SpellManager には TS_Hush 単体のみを装備（他プレイヤーの装備へ影響させない）
    SpellManager.add(0x228).writeU32(1);
    var SingleSpellArr = Memory.alloc(24);
    SingleSpellArr.writePointer(SingleSpellArr.add(16));
    SingleSpellArr.add(8).writeU32(1);
    SingleSpellArr.add(12).writeU32(1);
    SingleSpellArr.add(16).writePointer(SpellClass);
    ADH_SpellManager_SetEquippedSpells(SpellManager, SingleSpellArr);

    // Tier 1 (TST_ONE=2、30秒) を設定し、チャージ残量 (+0x284) が閾値を満たすよう補正
    ADH_SpellManager_SetSpellChargeTier(SpellManager, SPELL_TIER_LEVEL_1);
    if (SpellManager.add(0x284).readFloat() < 0.37) {
        SpellManager.add(0x284).writeFloat(0.37);
    }

    var Spell = ADH_SpellManager_CastSpell(SpellManager, SpellClass, ptr(0));
    if (Spell.isNull()) {
        Spell = ADH_SpellManager_CastSpellStatic(GameState, SpellClass, ptr(0), SPELL_TIER_LEVEL_1);
    }
    return !Spell.isNull();
}

var pendingWorldKey = null;
var pendingCastAtMs = 0;
var lastCastWorldKey = null;

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function () {
        try {
            var World = GWorld.readPointer();
            if (World.isNull()) {
                return;
            }
            var WorldKey = World.toString();
            if (lastCastWorldKey === WorldKey) {
                return;
            }
            ActiveSpellManagers = [];
            // ポーカー画面終了後に確実に同期されるよう、InProgress 遷移から0.5秒後に発動
            pendingWorldKey = WorldKey;
            pendingCastAtMs = Date.now() + 500;
        } catch (e) {}
    },
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            if (pendingWorldKey === null || Date.now() < pendingCastAtMs) {
                return;
            }
            var GameState = args[0];
            if (GameState.isNull()) {
                return;
            }
            var World = GWorld.readPointer();
            if (World.isNull() || World.toString() !== pendingWorldKey) {
                return;
            }
            var worldToCast = pendingWorldKey;
            pendingWorldKey = null;
            if (castOpeningHush(GameState)) {
                lastCastWorldKey = worldToCast;
            }
        } catch (e) {}
    },
});
