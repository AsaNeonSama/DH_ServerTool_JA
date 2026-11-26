// スペルチャージ無限 (原版5種＋カスタムスペル全種対応: 詠唱・死亡時のクールダウン完全ゼロ化 & リアルタイム再チャージ)
// 検証済み根拠 (DreadHungerServer-Win64-Shipping.pdb / 逆アセンブリ):
//   - ADH_SpellManager::CastSpell(inner) (0xE634F0) の引数は args[0]=WorldContext, args[1]=SpellManager, args[2]=SpellType
//   - ADH_SpellManager::ResetSpellCooldown (0xE80390) が SpellCooldowns (+0x2A8) へ ServerUsedTime を記録する唯一の詠唱経路 (0xE638B0)
//   - ADH_SpellManager::ResetSpells (0xE80500) が死亡時 (OnPawnDied 0xE4405E) に全装備スペルをクールダウン化しチャージを 0 に戻す
//   - ADH_PlayerState::ClearSpellCooldowns (0xE30040) と同じく TArray<FSpellCooldown>::ResizeTo (0x768CF0) で空配列化すると、
//     サーバー・クライアント双方で GetTimeSinceLastCast (0xE71670) -> 0.0f / GetSpellCooldown (0xE70F30) -> 0.0f となり、
//     CooldownTime=300s (TS_BombMice_C / TS_BoomShip_C 等) やクライアント時刻差・slomo 下でも確実にクールダウンが 0 になる
//   - 開幕沈黙.js 等の GameState 所有一時 SpellManager に干渉しないよう Owner (+0xE0) が ADH_PlayerState の個体のみ対象とする
(function () {
    'use strict';

    var MODULE_NAME = 'DreadHungerServer-Win64-Shipping.exe';
    var moduleBase = Module.findBaseAddress(MODULE_NAME);
    if (!moduleBase || moduleBase.isNull()) {
        return;
    }

    var RVA = {
        GUObjectArray: 0x45C40C8,
        GWorld: 0x46ED420,
        TArray_FSpellCooldown_ResizeTo: 0x768CF0,
        ADH_HumanCharacter_AddStartingInventory: 0xD46F10,
        ADH_GameState_HandleMatchHasStarted: 0xD99080,
        ADH_GameState_Tick: 0xDAD420,
        ADH_SpellManager_CastSpell: 0xE634F0,
        ADH_SpellManager_ResetSpellCooldown: 0xE80390,
        ADH_SpellManager_ResetSpells: 0xE80500,
        ADH_SpellManager_SetEquippedSpells: 0xE81F00,
        ADH_SpellManager_SetSpellChargeLevel: 0xE82CC0,
        ADH_PlayerState_GetPrivateStaticClass: 0xEED6B0,
        ADH_SpellManager_GetPrivateStaticClass: 0xEF7340
    };

    var OFFSETS = {
        UObject_InternalIndex: 0x0C,
        UObject_ClassPrivate: 0x10,
        UObject_NamePrivate: 0x18,
        UStruct_ChainArray: 0x30,
        UStruct_ChainDepth: 0x38,
        AActor_Owner: 0xE0,
        AActor_Role: 0xF0,
        UWorld_TimeSeconds: 0x598,
        ADH_GameState_PlayerArray: 0x238,
        APawn_PlayerState: 0x240,
        ADH_PlayerState_SpellManager: 0x488,
        ADH_SpellManager_CooldownMultiplier: 0x280,
        ADH_SpellManager_SpellChargeLevel: 0x284,
        ADH_SpellManager_SpellCooldowns: 0x2A8
    };

    var FULL_SPELL_CHARGE = 1.0;
    var MAX_PENDING_JOBS = 128;
    var ROLE_AUTHORITY = 3;

    var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
    var GWorld_ptr = moduleBase.add(RVA.GWorld);

    var TArray_FSpellCooldown_ResizeTo = new NativeFunction(
        moduleBase.add(RVA.TArray_FSpellCooldown_ResizeTo),
        'void',
        ['pointer', 'int32'],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_SpellManager_SetSpellChargeLevel = new NativeFunction(
        moduleBase.add(RVA.ADH_SpellManager_SetSpellChargeLevel),
        'void',
        ['pointer', 'float'],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_PlayerState_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_PlayerState_GetPrivateStaticClass),
        'pointer',
        [],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_SpellManager_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_SpellManager_GetPrivateStaticClass),
        'pointer',
        [],
        { abi: 'win64', exceptions: 'propagate' }
    );

    // QuickJS GC によるトランポリン解放防止用トップレベル保持配列
    var pinnedCallbacks = [];
    var cachedPlayerStateClass = ptr(0);
    var cachedSpellManagerClass = ptr(0);
    var origResetSpellCooldown = null;
    var origResetSpells = null;

    function log(msg) {
        var line = '[スペルチャージ無限] ' + msg;
        try {
            if (typeof send === 'function') {
                send(line);
                return;
            }
        } catch (e) {}
        try {
            console.log(line);
        } catch (e2) {}
    }

    function getPlayerStateClass() {
        if (cachedPlayerStateClass.isNull()) {
            cachedPlayerStateClass = ADH_PlayerState_GetPrivateStaticClass();
        }
        return cachedPlayerStateClass;
    }

    function getSpellManagerClass() {
        if (cachedSpellManagerClass.isNull()) {
            cachedSpellManagerClass = ADH_SpellManager_GetPrivateStaticClass();
        }
        return cachedSpellManagerClass;
    }

    function isPendingKill(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return true;
        }
        try {
            var internalIndex = objPtr.add(OFFSETS.UObject_InternalIndex).readS32();
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
        } catch (e) {
            return true;
        }
    }

    function isA(objPtr, targetClassPtr) {
        if (!objPtr || objPtr.isNull() || !targetClassPtr || targetClassPtr.isNull()) {
            return false;
        }
        try {
            var objClass = objPtr.add(OFFSETS.UObject_ClassPrivate).readPointer();
            if (objClass.isNull()) {
                return false;
            }
            var targetDepth = targetClassPtr.add(OFFSETS.UStruct_ChainDepth).readS32();
            var objDepth = objClass.add(OFFSETS.UStruct_ChainDepth).readS32();
            if (targetDepth < 0 || targetDepth > objDepth) {
                return false;
            }
            var chainArray = objClass.add(OFFSETS.UStruct_ChainArray).readPointer();
            if (chainArray.isNull()) {
                return false;
            }
            return chainArray.add(targetDepth * 8).readPointer().equals(targetClassPtr.add(OFFSETS.UStruct_ChainArray));
        } catch (e) {
            return false;
        }
    }

    function isValidSpellManager(smPtr) {
        if (!smPtr || smPtr.isNull() || isPendingKill(smPtr)) {
            return false;
        }
        return isA(smPtr, getSpellManagerClass());
    }

    // 開幕沈黙.js 等の GameState 所有一時 SpellManager を除外し、実プレイヤーの SpellManager のみ対象とする
    function isPlayerSpellManager(smPtr) {
        if (!isValidSpellManager(smPtr)) {
            return false;
        }
        if (smPtr.add(OFFSETS.AActor_Role).readU8() !== ROLE_AUTHORITY) {
            return false;
        }
        var owner = smPtr.add(OFFSETS.AActor_Owner).readPointer();
        if (owner.isNull() || isPendingKill(owner)) {
            return false;
        }
        return isA(owner, getPlayerStateClass());
    }

    function getWorldAndTime() {
        var world = GWorld_ptr.readPointer();
        if (!world || world.isNull()) {
            return { world: ptr(0), worldKey: '', timeSeconds: 0.0 };
        }
        var t = world.add(OFFSETS.UWorld_TimeSeconds).readFloat();
        if (!Number.isFinite(t)) {
            t = 0.0;
        }
        return { world: world, worldKey: world.toString(), timeSeconds: t };
    }

    function getArraySize(tArrayPtr) {
        if (!tArrayPtr || tArrayPtr.isNull()) {
            return 0;
        }
        return tArrayPtr.add(8).readS32();
    }

    // ADH_PlayerState::ClearSpellCooldowns (0xE30040) と同一手順で SpellCooldowns (+0x2A8) を完全消去
    function clearSpellCooldowns(spellManager) {
        if (!isPlayerSpellManager(spellManager)) {
            return;
        }
        // サーバー側判定の保険として CooldownMultiplier (+0x280) も 0.0f に固定
        spellManager.add(OFFSETS.ADH_SpellManager_CooldownMultiplier).writeFloat(0.0);

        var cdArray = spellManager.add(OFFSETS.ADH_SpellManager_SpellCooldowns);
        var cdNum = cdArray.add(8).readS32();
        var cdMax = cdArray.add(12).readS32();
        if (cdNum < 0 || cdMax < 0 || cdNum > 256 || cdMax > 256) {
            return;
        }
        if (cdNum !== 0 || cdMax !== 0) {
            cdArray.add(8).writeS32(0);
            if (cdMax !== 0) {
                TArray_FSpellCooldown_ResizeTo(cdArray, 0);
            }
        }
    }

    function refillSpellManager(spellManager) {
        if (!isPlayerSpellManager(spellManager)) {
            return;
        }
        clearSpellCooldowns(spellManager);
        ADH_SpellManager_SetSpellChargeLevel(spellManager, FULL_SPELL_CHARGE);
    }

    // 詠唱完了後の次 Tick でチャージを 1.0f へ即時復帰させる GameThread キュー (UWorld::TimeSeconds 基準)
    var pendingSpellRestores = [];

    function enqueueSpellManagerRestore(spellManager) {
        if (!isPlayerSpellManager(spellManager)) {
            return;
        }
        var wt = getWorldAndTime();
        if (wt.world.isNull()) {
            return;
        }
        var smKey = spellManager.toString();
        var internalIndex = spellManager.add(OFFSETS.UObject_InternalIndex).readS32();
        var nameRaw = spellManager.add(OFFSETS.UObject_NamePrivate).readU64();

        for (var i = 0; i < pendingSpellRestores.length; i++) {
            var existing = pendingSpellRestores[i];
            if (existing.worldKey === wt.worldKey && existing.spellManagerKey === smKey) {
                existing.internalIndex = internalIndex;
                existing.nameRaw = nameRaw;
                existing.dueTime = wt.timeSeconds;
                return;
            }
        }
        if (pendingSpellRestores.length >= MAX_PENDING_JOBS) {
            return;
        }
        pendingSpellRestores.push({
            worldKey: wt.worldKey,
            spellManagerPtr: spellManager,
            spellManagerKey: smKey,
            internalIndex: internalIndex,
            nameRaw: nameRaw,
            dueTime: wt.timeSeconds
        });
    }

    function resolveLiveSpellManager(gameState, job) {
        if (!gameState || gameState.isNull() || !job || !job.spellManagerKey) {
            return ptr(0);
        }
        // 1) 通常プレイヤー: GameState->PlayerArray から生存中の SpellManager を検索
        var playerArray = gameState.add(OFFSETS.ADH_GameState_PlayerArray);
        var playerCount = getArraySize(playerArray);
        var dataPtr = playerArray.readPointer();
        if (!dataPtr.isNull() && playerCount > 0 && playerCount <= 64) {
            for (var i = 0; i < playerCount; i++) {
                var ps = dataPtr.add(i * 8).readPointer();
                if (ps.isNull() || isPendingKill(ps)) {
                    continue;
                }
                var sm = ps.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
                if (!sm.isNull() && sm.toString() === job.spellManagerKey && isPlayerSpellManager(sm)) {
                    return sm;
                }
            }
        }

        // 2) PlayerArray 外のプレイヤー SpellManager: 3点同一性 (ptr + InternalIndex + FName) で厳密検証
        var candidate = job.spellManagerPtr;
        if (candidate && isPlayerSpellManager(candidate)) {
            var curIdx = candidate.add(OFFSETS.UObject_InternalIndex).readS32();
            var curNameRaw = candidate.add(OFFSETS.UObject_NamePrivate).readU64();
            if (curIdx === job.internalIndex && curNameRaw.equals(job.nameRaw)) {
                return candidate;
            }
        }
        return ptr(0);
    }

    // 1) ADH_SpellManager::ResetSpellCooldown (0xE80390) を replaceFast (onLeave 消費 0):
    //    原版・カスタムスペル問わず詠唱直後のクールダウン登録自体を遮断し、次 Tick でチャージを 1.0f へ復帰
    var resetSpellCooldownCb = new NativeCallback(function (spellManager, spellType) {
        try {
            if (isPlayerSpellManager(spellManager)) {
                clearSpellCooldowns(spellManager);
                enqueueSpellManagerRestore(spellManager);
                return;
            }
        } catch (e) {}
        if (origResetSpellCooldown !== null) {
            try {
                origResetSpellCooldown(spellManager, spellType);
            } catch (e2) {}
        }
    }, 'void', ['pointer', 'pointer'], 'win64');
    pinnedCallbacks.push(resetSpellCooldownCb);
    var origResetSpellCooldownPtr = Interceptor.replaceFast(
        moduleBase.add(RVA.ADH_SpellManager_ResetSpellCooldown),
        resetSpellCooldownCb
    );
    origResetSpellCooldown = new NativeFunction(
        origResetSpellCooldownPtr,
        'void',
        ['pointer', 'pointer'],
        { abi: 'win64', exceptions: 'propagate' }
    );

    // 2) ADH_SpellManager::ResetSpells (0xE80500) を replaceFast (onLeave 消費 0):
    //    死亡時 (OnPawnDied) に全装備スペルへクールダウンが付与されチャージが 0 になるのを防止
    var resetSpellsCb = new NativeCallback(function (spellManager) {
        try {
            if (isPlayerSpellManager(spellManager)) {
                refillSpellManager(spellManager);
                return;
            }
        } catch (e) {}
        if (origResetSpells !== null) {
            try {
                origResetSpells(spellManager);
            } catch (e2) {}
        }
    }, 'void', ['pointer'], 'win64');
    pinnedCallbacks.push(resetSpellsCb);
    var origResetSpellsPtr = Interceptor.replaceFast(
        moduleBase.add(RVA.ADH_SpellManager_ResetSpells),
        resetSpellsCb
    );
    origResetSpells = new NativeFunction(
        origResetSpellsPtr,
        'void',
        ['pointer'],
        { abi: 'win64', exceptions: 'propagate' }
    );

    // 3) ADH_SpellManager::CastSpell(inner) (0xE634F0) 保険フック (onEnter のみ):
    //    args[0]=WorldContext, args[1]=SpellManager。CastSpell 後に確実にチャージ復帰を予約
    Interceptor.attach(moduleBase.add(RVA.ADH_SpellManager_CastSpell), {
        onEnter: function (args) {
            try {
                var spellManager = args[1];
                if (isPlayerSpellManager(spellManager)) {
                    enqueueSpellManagerRestore(spellManager);
                }
            } catch (e) {}
        }
    });

    // 4) 呪文装備時 (onEnter のみ): 途中傀儡化(/thrall 等)直後でも即座に満タン化・クールダウン消去
    Interceptor.attach(moduleBase.add(RVA.ADH_SpellManager_SetEquippedSpells), {
        onEnter: function (args) {
            try {
                var spellManager = args[0];
                if (isPlayerSpellManager(spellManager)) {
                    refillSpellManager(spellManager);
                }
            } catch (e) {}
        }
    });

    // 5) 初期装備付与時 (onEnter のみ): チャージ満タン＋クールダウン全消去
    Interceptor.attach(moduleBase.add(RVA.ADH_HumanCharacter_AddStartingInventory), {
        onEnter: function (args) {
            try {
                var humanChar = args[0];
                if (!humanChar || humanChar.isNull() || isPendingKill(humanChar)) {
                    return;
                }
                var playerState = humanChar.add(OFFSETS.APawn_PlayerState).readPointer();
                if (playerState.isNull() || isPendingKill(playerState)) {
                    return;
                }
                var spellManager = playerState.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
                refillSpellManager(spellManager);
            } catch (e) {}
        }
    });

    // 6) 試合開始時 (onEnter のみ): 全プレイヤーの SpellManager を初期満タン化・クールダウン消去
    Interceptor.attach(moduleBase.add(RVA.ADH_GameState_HandleMatchHasStarted), {
        onEnter: function (args) {
            try {
                var gameState = args[0];
                if (!gameState || gameState.isNull() || isPendingKill(gameState)) {
                    return;
                }
                var playerArray = gameState.add(OFFSETS.ADH_GameState_PlayerArray);
                var playerCount = getArraySize(playerArray);
                var dataPtr = playerArray.readPointer();
                if (dataPtr.isNull() || playerCount <= 0 || playerCount > 64) {
                    return;
                }
                for (var i = 0; i < playerCount; i++) {
                    var ps = dataPtr.add(i * 8).readPointer();
                    if (ps.isNull() || isPendingKill(ps)) {
                        continue;
                    }
                    var sm = ps.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
                    refillSpellManager(sm);
                }
            } catch (e) {}
        }
    });

    // 7) GameThread Tick (onEnter のみ): UWorld::TimeSeconds 基準で待機キューを安全に消費しチャージを満タン復帰
    Interceptor.attach(moduleBase.add(RVA.ADH_GameState_Tick), {
        onEnter: function (args) {
            try {
                if (pendingSpellRestores.length === 0) {
                    return;
                }
                var gameState = args[0];
                var wt = getWorldAndTime();
                if (!gameState || gameState.isNull() || wt.world.isNull()) {
                    pendingSpellRestores = [];
                    return;
                }
                var remaining = [];
                for (var i = 0; i < pendingSpellRestores.length; i++) {
                    var job = pendingSpellRestores[i];
                    if (job.worldKey !== wt.worldKey) {
                        continue;
                    }
                    if (wt.timeSeconds < job.dueTime) {
                        remaining.push(job);
                        continue;
                    }
                    var liveSm = resolveLiveSpellManager(gameState, job);
                    if (!liveSm.isNull()) {
                        refillSpellManager(liveSm);
                    }
                }
                pendingSpellRestores = remaining;
            } catch (e) {}
        }
    });

    log('初期化完了 (原版＋カスタムスペル対応: クールダウン完全ゼロ化 & 即時フルチャージ復帰)');
})();
