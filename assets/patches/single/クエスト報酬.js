// クエスト報酬 (未所持者へのクエスト自動配布・完了時のランダム報酬付与)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var GWorld = base.add(0x46ED420);

var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_QuestState_OnCompletedChanged_addr = base.add(0xE42EF0);

var ADH_GameState_GiveQuest = new NativeFunction(base.add(0xD973E0), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_Character_IsAlive = new NativeFunction(base.add(0xD5C850), 'int8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var UDH_InventoryManager_AddInventory = new NativeFunction(base.add(0xDBC040), 'void', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), 'void', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });

var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], { abi: 'win64', exceptions: 'propagate' });
// FString&&ムーブ版(0x10963B0)によるRPC UAF文字化け防止のため、const FString&版(0x10964E0)でFMemoryへコピー
var FText_FromString = new NativeFunction(base.add(0x10964E0), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var FText_Dtor = new NativeFunction(base.add(0x75CB30), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });

// クエスト完了時のランダム報酬プール (1つをランダム配布)
var REWARD_POOL = [
    { name: '拳銃', path: '/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C' },
    { name: '拳銃パーツ', path: '/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C' },
    { name: 'サーベル', path: '/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C' },
    { name: '砥石', path: '/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C' },
    { name: '注射器', path: '/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C' },
    { name: 'シチュー', path: '/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C' },
    { name: '骨の鍵', path: '/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C' }
];

var QUEST_GIVE_DELAY_SECONDS = 2.0;

var matchStartedAt = 0;
var questAssignedWorldKey = null;
var rewardedQuestStates = {};
var pendingDeadRewards = [];
var lastPendingRetryAt = 0;

function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}

function getClass(ClassName) {
    var StaticClass = UClass_GetPrivateStaticClass();
    if (StaticClass.isNull()) {
        return ptr(0);
    }
    var Clazz = findObjectByName(ClassName, StaticClass);
    if (Clazz.isNull()) {
        Clazz = loadObjectByName(ClassName, StaticClass);
    }
    return Clazz;
}

function newFString(Length) {
    var FString = Memory.alloc(16 + Length * 3);
    FString.writePointer(FString.add(16));
    FString.add(8).writeU32(0);
    FString.add(12).writeU32(Length);
    return FString;
}

function writeFString(FString, Data) {
    FString.add(8).writeU32(Data.length + 1);
    FString.readPointer().writeUtf16String(Data);
}

function newFText(Text) {
    var FText_Buffer = Memory.alloc(24);
    var FString_Buffer = newFString(Text.length + 3);
    writeFString(FString_Buffer, Text);
    FText_FromString(FText_Buffer, FString_Buffer);
    return FText_Buffer;
}

function sendPlayerNotice(PlayerState, Message) {
    if (PlayerState.isNull()) {
        return;
    }
    var PlayerController = PlayerState.add(0xE0).readPointer();
    if (PlayerController.isNull()) {
        return;
    }
    try {
        var FText_Message = newFText(Message);
        try {
            ADH_PlayerController_ReceiveThrallMessage(PlayerController, FText_Message, ptr(0));
        } finally {
            FText_Dtor(FText_Message);
        }
    } catch (e) {}
}

function initItemState(State, Upgraded, Tainted) {
    State.writeU32(0xffffffff);
    State.add(0x4).writeU8(1);
    State.add(0x8).writeFloat(1.0);
    State.add(0xC).writeU8(0);
    State.add(0xD).writeU8(Upgraded);
    State.add(0xE).writeU8(Tainted);
    State.add(0x10).writePointer(ptr(0));
    State.add(0x18).writePointer(ptr(0));
    State.add(0x20).writePointer(ptr(0));
    State.add(0x28).writePointer(ptr(0));
    State.add(0x30).writePointer(ptr(0));
}

function allocAligned16(size) {
    var raw = Memory.alloc(size + 16);
    var rem = raw.toUInt32() & 0xF;
    var aligned = rem === 0 ? raw : raw.add(16 - rem);
    return { raw: raw, ptr: aligned };
}

function makeTransform(x, y, z) {
    var block = allocAligned16(0x30);
    var t = block.ptr;
    // 回転クォータニオン (単位元: 0, 0, 0, 1)
    t.add(0x00).writeFloat(0.0);
    t.add(0x04).writeFloat(0.0);
    t.add(0x08).writeFloat(0.0);
    t.add(0x0C).writeFloat(1.0);
    // 位置 (x, y, z)
    t.add(0x10).writeFloat(x);
    t.add(0x14).writeFloat(y);
    t.add(0x18).writeFloat(z);
    t.add(0x1C).writeFloat(0.0);
    // スケール (1, 1, 1)
    t.add(0x20).writeFloat(1.0);
    t.add(0x24).writeFloat(1.0);
    t.add(0x28).writeFloat(1.0);
    t.add(0x2C).writeFloat(0.0);
    return block;
}

function resolvePlayerPawn(PlayerState) {
    if (PlayerState.isNull()) {
        return ptr(0);
    }
    var Pawn = PlayerState.add(0x280).readPointer();
    if (!Pawn.isNull()) {
        return Pawn;
    }
    var PlayerController = PlayerState.add(0xE0).readPointer();
    if (!PlayerController.isNull()) {
        Pawn = PlayerController.add(0x250).readPointer();
        if (!Pawn.isNull()) {
            return Pawn;
        }
    }
    return ptr(0);
}

function spawnRewardPickupAtPawn(Pawn, ItemClass) {
    var World = GWorld.readPointer();
    if (World.isNull() || Pawn.isNull() || ItemClass.isNull()) {
        return false;
    }
    var DefaultObject = ItemClass.add(0x118).readPointer();
    if (DefaultObject.isNull()) {
        return false;
    }
    var PickupClass = DefaultObject.add(0x2A8).readPointer();
    if (PickupClass.isNull()) {
        return false;
    }
    var RootComponent = Pawn.add(0x130).readPointer();
    if (RootComponent.isNull()) {
        return false;
    }
    // USceneComponent::ComponentToWorld.Translation (船上でも有効なワールド座標XYZ)
    var x = RootComponent.add(0x1D0).readFloat();
    var y = RootComponent.add(0x1D4).readFloat();
    var z = RootComponent.add(0x1D8).readFloat() + 45.0;
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
        return false;
    }

    var transformBlock = makeTransform(x, y, z);
    var spawnParams = Memory.alloc(0x40);
    FActorSpawnParameters_FActorSpawnParameters(spawnParams);
    spawnParams.add(0x28).writeU8(1); // 衝突を無視して常にスポーン (AlwaysSpawn)

    var Spawned = UWorld_SpawnActor(World, PickupClass, transformBlock.ptr, spawnParams);
    return !Spawned.isNull();
}

function pickRandomReward() {
    var idx = Math.floor(Math.random() * REWARD_POOL.length);
    if (idx < 0 || idx >= REWARD_POOL.length) {
        idx = 0;
    }
    return REWARD_POOL[idx];
}

// 戻り値: 'inventory' | 'dropped' | 'defer' | 'failed'
function tryGrantReward(PlayerState, Reward) {
    if (PlayerState.isNull()) {
        return 'failed';
    }
    var Pawn = resolvePlayerPawn(PlayerState);
    if (Pawn.isNull() || ADH_Character_IsAlive(Pawn) === 0) {
        return 'defer';
    }
    var InventoryComponent = Pawn.add(0x808).readPointer();
    if (InventoryComponent.isNull()) {
        return 'defer';
    }
    var ItemClass = getClass(Reward.path);
    if (ItemClass.isNull()) {
        logInfo('[Quest] Failed to load reward class: ' + Reward.path);
        return 'failed';
    }

    var OutBuffer = Memory.alloc(8);
    OutBuffer.writeU32(0);             // 追加個数
    OutBuffer.add(4).writeU32(0xffffffff); // スロット番号

    var ItemArray = Memory.alloc(16 + 56);
    ItemArray.writePointer(ItemArray.add(16));
    ItemArray.add(8).writeU32(1);
    ItemArray.add(12).writeU32(1);
    initItemState(ItemArray.readPointer(), 0, 0);

    UDH_InventoryManager_AddInventory(
        InventoryComponent,
        ItemClass,
        ItemArray,
        OutBuffer,
        OutBuffer.add(4),
        0,
        ptr(0)
    );

    var NumItemsAdded = OutBuffer.readU32();
    if (NumItemsAdded > 0) {
        sendPlayerNotice(PlayerState, 'クエスト完了報酬: ' + Reward.name + ' を獲得しました！');
        return 'inventory';
    }

    // インベントリ満杯時は足元にドロップして報酬消失を防止
    if (spawnRewardPickupAtPawn(Pawn, ItemClass)) {
        sendPlayerNotice(PlayerState, 'クエスト完了報酬: ' + Reward.name + ' を足元にドロップしました！');
        return 'dropped';
    }

    return 'failed';
}

function assignQuestsToMissingPlayers(GameState) {
    if (GameState.isNull()) {
        return;
    }
    var PlayerArray = GameState.add(0x238);
    var Players = PlayerArray.readPointer();
    var PlayerNum = PlayerArray.add(8).readU32();
    if (Players.isNull() || PlayerNum === 0 || PlayerNum > 64) {
        return;
    }
    for (var i = 0; i < PlayerNum; i++) {
        var PlayerState = Players.add(8 * i).readPointer();
        if (PlayerState.isNull()) {
            continue;
        }
        var bIsDisconnected = PlayerState.add(0x571).readU8();
        if (bIsDisconnected !== 0) {
            continue;
        }
        var ExistingQuest = PlayerState.add(0x478).readPointer();
        if (ExistingQuest.isNull()) {
            try {
                ADH_GameState_GiveQuest(GameState, PlayerState);
            } catch (e) {}
        }
    }
}

function isPlayerStateInGame(GameState, TargetPlayerState) {
    if (GameState.isNull() || TargetPlayerState.isNull()) {
        return false;
    }
    var PlayerArray = GameState.add(0x238);
    var Players = PlayerArray.readPointer();
    var PlayerNum = PlayerArray.add(8).readU32();
    if (Players.isNull() || PlayerNum === 0 || PlayerNum > 64) {
        return false;
    }
    for (var i = 0; i < PlayerNum; i++) {
        var PlayerState = Players.add(8 * i).readPointer();
        if (!PlayerState.isNull() && PlayerState.equals(TargetPlayerState)) {
            return true;
        }
    }
    return false;
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function () {
        matchStartedAt = Date.now();
        questAssignedWorldKey = null;
        rewardedQuestStates = {};
        pendingDeadRewards = [];
        lastPendingRetryAt = 0;
    }
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            var GameState = args[0];
            if (GameState.isNull()) {
                return;
            }
            var now = Date.now();

            // マッチ開始後2秒時点でクエスト未付与のプレイヤーに自動配布
            // (骨の短剣モードの初期装備で既に付与済みならスキップ)
            if (matchStartedAt > 0 && now - matchStartedAt >= QUEST_GIVE_DELAY_SECONDS * 1000) {
                var World = GWorld.readPointer();
                if (!World.isNull()) {
                    var worldKey = World.toString();
                    if (questAssignedWorldKey !== worldKey) {
                        questAssignedWorldKey = worldKey;
                        assignQuestsToMissingPlayers(GameState);
                    }
                }
            }

            // 死亡中にクエスト完了したプレイヤーが復活したら報酬を再配布
            if (pendingDeadRewards.length > 0 && now - lastPendingRetryAt >= 1000) {
                lastPendingRetryAt = now;
                var remaining = [];
                for (var i = 0; i < pendingDeadRewards.length; i++) {
                    var item = pendingDeadRewards[i];
                    if (!isPlayerStateInGame(GameState, item.playerState)) {
                        continue;
                    }
                    var status = tryGrantReward(item.playerState, item.reward);
                    if (status === 'defer') {
                        remaining.push(item);
                    }
                }
                pendingDeadRewards = remaining;
            }
        } catch (e) {}
    }
});

Interceptor.attach(ADH_QuestState_OnCompletedChanged_addr, {
    onEnter: function (args) {
        try {
            var QuestState = args[0];
            if (QuestState.isNull()) {
                return;
            }
            var bCompleted = QuestState.add(0x228).readU8();
            if (bCompleted === 0) {
                return;
            }
            var QuestKey = QuestState.toString();
            if (rewardedQuestStates[QuestKey]) {
                return;
            }
            var PlayerState = QuestState.add(0x238).readPointer();
            if (PlayerState.isNull()) {
                return;
            }
            rewardedQuestStates[QuestKey] = true;

            var Reward = pickRandomReward();
            var status = tryGrantReward(PlayerState, Reward);
            if (status === 'defer') {
                pendingDeadRewards.push({
                    playerState: PlayerState,
                    reward: Reward
                });
                sendPlayerNotice(PlayerState, 'クエスト完了！復活時に報酬 (' + Reward.name + ') が配布されます');
            }
        } catch (e) {}
    }
});
