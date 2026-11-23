// スペルチャージ無限 (初期チャージ加算 + 詠唱後のクールダウン短縮・再チャージ)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var GWorld = base.add(0x46ED420);
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);
var ADH_SpellManager_CastSpell_addr = base.add(0xE634F0);

function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

function resetSpells(SpellManager) {
    if (SpellManager.isNull()) {
        return;
    }
    var SpellChargeLevel = SpellManager.add(0x284);
    SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 1);
}

Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function (args) {
        try {
            var HumanCharacter = args[0];
            if (HumanCharacter.isNull()) {
                return;
            }
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            if (PlayerState.isNull()) {
                return;
            }
            var SpellManager = PlayerState.add(0x488).readPointer();
            resetSpells(SpellManager);
        } catch (e) {}
    }
});

function getArraySize(TArray) {
    if (TArray.isNull()) {
        return 0;
    }
    return TArray.add(8).readU32();
}
function getArrayItemAddr(TArray, size, index) {
    var ArrNum = getArraySize(TArray);
    if (index < 0 || index >= ArrNum)
        return null;
    var DataPtr = TArray.readPointer();
    if (DataPtr.isNull())
        return null;
    return DataPtr.add(index * size);
}

function findLiveSpellManager(GameState, spellManagerKey) {
    if (GameState.isNull() || !spellManagerKey) {
        return ptr(0);
    }
    var PlayerArray = GameState.add(0x238);
    var playerCount = getArraySize(PlayerArray);
    if (playerCount === 0 || playerCount > 64) {
        return ptr(0);
    }
    for (var i = 0; i < playerCount; i++) {
        var psEntry = getArrayItemAddr(PlayerArray, 8, i);
        if (psEntry === null) {
            continue;
        }
        var ps = psEntry.readPointer();
        if (ps.isNull()) {
            continue;
        }
        var sm = ps.add(0x488).readPointer();
        if (!sm.isNull() && sm.toString() === spellManagerKey) {
            return sm;
        }
    }
    return ptr(0);
}

function applyCooldownReduction(SpellManager) {
    if (SpellManager.isNull()) {
        return;
    }
    var SpellCooldowns = SpellManager.add(0x2A8);
    var cdSize = getArraySize(SpellCooldowns);
    if (cdSize === 0 || cdSize > 64) {
        return;
    }
    for (var i = 0; i < cdSize; i++) {
        var SpellCooldown = getArrayItemAddr(SpellCooldowns, 16, i);
        if (SpellCooldown === null) {
            continue;
        }
        var ServerUsedTime = SpellCooldown.add(8);
        ServerUsedTime.writeFloat(ServerUsedTime.readFloat() - 300);
        var SpellChargeLevel = SpellManager.add(0x284);
        SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 1);
    }
}

var pendingCooldownResets = [];

Interceptor.attach(ADH_SpellManager_CastSpell_addr, {
    onEnter : function(args) {
        try {
            var SpellManager = args[0];
            if (SpellManager.isNull()) {
                return;
            }
            var World = GWorld.readPointer();
            if (World.isNull()) {
                return;
            }
            if (pendingCooldownResets.length < 128) {
                pendingCooldownResets.push({
                    worldKey: World.toString(),
                    spellManagerKey: SpellManager.toString(),
                    dueAt: Date.now() + 1000
                });
            }
        } catch (e) {}
    }
});

// SpellManager の生存確認後にUE4ゲームスレッド上で遅延クールダウン解除を実行
Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            if (pendingCooldownResets.length === 0) {
                return;
            }
            var GameState = args[0];
            var World = GWorld.readPointer();
            if (GameState.isNull() || World.isNull()) {
                pendingCooldownResets = [];
                return;
            }
            var currentWorldKey = World.toString();
            var now = Date.now();
            var remaining = [];
            for (var i = 0; i < pendingCooldownResets.length; i++) {
                var job = pendingCooldownResets[i];
                if (job.worldKey !== currentWorldKey) {
                    continue;
                }
                if (now < job.dueAt) {
                    remaining.push(job);
                    continue;
                }
                var liveSm = findLiveSpellManager(GameState, job.spellManagerKey);
                if (!liveSm.isNull()) {
                    applyCooldownReduction(liveSm);
                }
            }
            pendingCooldownResets = remaining;
        } catch (e) {}
    }
});