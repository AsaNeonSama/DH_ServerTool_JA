// 全サボタージュ (条件付き全呪文装備: 通常時は原版5種、カスタムスペル導入時は全10種など登録済み全呪文を自動装備)
// 検証済み根拠 (DreadHungerServer-Win64-Shipping.pdb / 逆アセンブリ):
//   - ADH_SpellManager::SetEquippedSpells (0xE81F00) は bDefault (+0x120) || bReleased (+0x121) を満たす
//     UDH_TotemSpell 派生クラスを EquippedSpells (+0x288) へコピー後、要素数 > MaxSpells (+0x228) ならランダム削除する
//   - MaxSpells (+0x228) を装備総数へ動的拡張すれば 5 種超 (10 種等) も上限切り捨てなしでクライアントへ複製される
//   - クライアント UDH_RadialMenuSpellsWidget::SetSpells (0xF3CC40) / PopulateOptions (0xF36E40) は
//     EquippedSpells.Num() に応じて放射状メニュー項目を動的生成するため 10 呪文構成にネイティブ対応する
//   - 開幕沈黙.js 等の GameState 所有一時 SpellManager へ干渉しないよう Owner (+0xE0) が ADH_PlayerState 以外なら素通りする
(function () {
    'use strict';

    var MODULE_NAME = 'DreadHungerServer-Win64-Shipping.exe';
    var moduleBase = Module.findBaseAddress(MODULE_NAME);
    if (!moduleBase || moduleBase.isNull()) {
        return;
    }

    // 原版5呪文の装備順 (0:食人鬼召喚 / 1:ホワイトアウト / 3:霊界歩き / 2:ドッペルゲンガー / 4:沈黙)
    // カスタムスペル導入時 (ThrallSpells.Num > 5) はこの後ろへ [5..Size-1] を自動連結して全呪文を装備する
    var VANILLA_SPELL_ORDER = [0, 1, 3, 2, 4];
    var MAX_EQUIPPED_SPELLS = 64;
    var ROLE_AUTHORITY = 3;

    var RVA = {
        GUObjectArray: 0x45C40C8,
        GWorld: 0x46ED420,
        UDH_GameInstance_GetInstance: 0xD93240,
        ADH_GameState_HandleMatchHasStarted: 0xD99080,
        ADH_SpellManager_SetEquippedSpells: 0xE81F00,
        ADH_PlayerState_GetPrivateStaticClass: 0xEED6B0,
        UDH_TotemSpell_GetPrivateStaticClass: 0xEF7CA0
    };

    var OFFSETS = {
        UObject_InternalIndex: 0x0C,
        UObject_ClassPrivate: 0x10,
        UStruct_ChainArray: 0x30,
        UStruct_ChainDepth: 0x38,
        UClass_ClassDefaultObject: 0x118,
        AActor_Owner: 0xE0,
        AActor_Role: 0xF0,
        ADH_GameState_PlayerArray: 0x238,
        ADH_PlayerState_SpellManager: 0x488,
        UDH_GameInstance_ThrallSpells: 0x440,
        ADH_SpellManager_MaxSpells: 0x228,
        ADH_SpellManager_EquippedSpells: 0x288,
        UDH_TotemSpell_bDefault: 0x120,
        UDH_TotemSpell_bReleased: 0x121
    };

    var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
    var GWorld_ptr = moduleBase.add(RVA.GWorld);

    var UDH_GameInstance_GetInstance = new NativeFunction(
        moduleBase.add(RVA.UDH_GameInstance_GetInstance),
        'pointer',
        ['pointer'],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_SpellManager_SetEquippedSpells = new NativeFunction(
        moduleBase.add(RVA.ADH_SpellManager_SetEquippedSpells),
        'void',
        ['pointer', 'pointer'],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_PlayerState_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_PlayerState_GetPrivateStaticClass),
        'pointer',
        [],
        { abi: 'win64', exceptions: 'propagate' }
    );
    var UDH_TotemSpell_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.UDH_TotemSpell_GetPrivateStaticClass),
        'pointer',
        [],
        { abi: 'win64', exceptions: 'propagate' }
    );

    // 動的容量対応の傀儡用スペルリスト構築バッファ (TArray<TSubclassOf<UDH_TotemSpell>>: 最大64呪文)
    var spellListBuffer = Memory.alloc(16 + MAX_EQUIPPED_SPELLS * 8);
    spellListBuffer.writePointer(spellListBuffer.add(16));
    spellListBuffer.add(8).writeS32(0);
    spellListBuffer.add(12).writeS32(0);

    var cachedPlayerStateClass = ptr(0);
    var cachedTotemSpellClass = ptr(0);
    var inSelfApply = false;
    var lastLoggedModeKey = '';

    function log(msg) {
        var line = '[全サボタージュ] ' + msg;
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

    function getTotemSpellClass() {
        if (cachedTotemSpellClass.isNull()) {
            cachedTotemSpellClass = UDH_TotemSpell_GetPrivateStaticClass();
        }
        return cachedTotemSpellClass;
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

    function isSubclassOf(clsPtr, baseClassPtr) {
        if (!clsPtr || clsPtr.isNull() || !baseClassPtr || baseClassPtr.isNull()) {
            return false;
        }
        try {
            var baseDepth = baseClassPtr.add(OFFSETS.UStruct_ChainDepth).readS32();
            var clsDepth = clsPtr.add(OFFSETS.UStruct_ChainDepth).readS32();
            if (baseDepth < 0 || baseDepth > clsDepth) {
                return false;
            }
            var chainArray = clsPtr.add(OFFSETS.UStruct_ChainArray).readPointer();
            if (chainArray.isNull()) {
                return false;
            }
            return chainArray.add(baseDepth * 8).readPointer().equals(baseClassPtr.add(OFFSETS.UStruct_ChainArray));
        } catch (e) {
            return false;
        }
    }

    function getArraySize(tArrayPtr) {
        if (!tArrayPtr || tArrayPtr.isNull()) {
            return 0;
        }
        return tArrayPtr.add(8).readS32();
    }

    function getArrayItemAddr(tArrayPtr, elemSize, index) {
        var arrNum = getArraySize(tArrayPtr);
        if (index < 0 || index >= arrNum) {
            return null;
        }
        var dataPtr = tArrayPtr.readPointer();
        if (dataPtr.isNull()) {
            return null;
        }
        return dataPtr.add(index * elemSize);
    }

    function isEquippableSpellClass(spellCls) {
        if (!spellCls || spellCls.isNull() || isPendingKill(spellCls)) {
            return false;
        }
        if (!isSubclassOf(spellCls, getTotemSpellClass())) {
            return false;
        }
        var cdo = spellCls.add(OFFSETS.UClass_ClassDefaultObject).readPointer();
        if (cdo.isNull() || isPendingKill(cdo)) {
            return false;
        }
        var bDefault = cdo.add(OFFSETS.UDH_TotemSpell_bDefault).readU8();
        var bReleased = cdo.add(OFFSETS.UDH_TotemSpell_bReleased).readU8();
        return bDefault !== 0 || bReleased !== 0;
    }

    function buildSpellPlan(contextObj) {
        if (!contextObj || contextObj.isNull()) {
            return null;
        }
        var gameInstance = UDH_GameInstance_GetInstance(contextObj);
        if (gameInstance.isNull() || isPendingKill(gameInstance)) {
            return null;
        }
        var thrallSpells = gameInstance.add(OFFSETS.UDH_GameInstance_ThrallSpells);
        var size = getArraySize(thrallSpells);
        if (size <= 0 || size > MAX_EQUIPPED_SPELLS) {
            return null;
        }

        // 通常時 (size === 5): 原版と完全一致する [0, 1, 3, 2, 4]
        // カスタムスペル導入時 (size > 5): [0, 1, 3, 2, 4] の直後に [5..size-1] を追加して全種装備
        var indices = [];
        if (size >= VANILLA_SPELL_ORDER.length) {
            for (var i = 0; i < VANILLA_SPELL_ORDER.length; i++) {
                indices.push(VANILLA_SPELL_ORDER[i]);
            }
            for (var extra = VANILLA_SPELL_ORDER.length; extra < size; extra++) {
                indices.push(extra);
            }
        } else {
            for (var k = 0; k < size; k++) {
                indices.push(k);
            }
        }

        var resolved = [];
        var seenKeys = {};
        for (var m = 0; m < indices.length; m++) {
            var itemAddr = getArrayItemAddr(thrallSpells, 8, indices[m]);
            if (itemAddr === null) {
                continue;
            }
            var spellCls = itemAddr.readPointer();
            if (!isEquippableSpellClass(spellCls)) {
                continue;
            }
            var key = spellCls.toString();
            if (seenKeys[key]) {
                continue;
            }
            seenKeys[key] = true;
            resolved.push(spellCls);
        }

        if (resolved.length === 0 || resolved.length > MAX_EQUIPPED_SPELLS) {
            return null;
        }
        return {
            spells: resolved,
            thrallSpellCount: size,
            hasCustomSpells: size > VANILLA_SPELL_ORDER.length
        };
    }

    function writeSpellListBuffer(spells) {
        var count = spells.length;
        spellListBuffer.writePointer(spellListBuffer.add(16));
        spellListBuffer.add(8).writeS32(count);
        spellListBuffer.add(12).writeS32(count);
        for (var i = 0; i < count; i++) {
            spellListBuffer.add(16 + 8 * i).writePointer(spells[i]);
        }
    }

    function logModeOnce(plan) {
        var world = GWorld_ptr.readPointer();
        var worldKey = (!world || world.isNull()) ? 'none' : world.toString();
        var modeKey = worldKey + ':' + plan.thrallSpellCount + ':' + plan.spells.length;
        if (modeKey === lastLoggedModeKey) {
            return;
        }
        lastLoggedModeKey = modeKey;
        if (plan.hasCustomSpells) {
            log('カスタムスペル検出 (ThrallSpells=' + plan.thrallSpellCount + ') -> 全 ' + plan.spells.length + ' 呪文を装備モードへ自動拡張');
        } else {
            log('通常モード (ThrallSpells=' + plan.thrallSpellCount + ') -> 原版 ' + plan.spells.length + ' 呪文を装備');
        }
    }

    // 1) ADH_SpellManager::SetEquippedSpells (0xE81F00) フック (onEnter のみ・onLeave 消費 0)
    Interceptor.attach(moduleBase.add(RVA.ADH_SpellManager_SetEquippedSpells), {
        onEnter: function (args) {
            try {
                if (inSelfApply) {
                    return;
                }
                var spellManager = args[0];
                if (!spellManager || spellManager.isNull() || isPendingKill(spellManager)) {
                    return;
                }
                if (spellManager.add(OFFSETS.AActor_Role).readU8() !== ROLE_AUTHORITY) {
                    return;
                }
                // 開幕沈黙.js 等の GameState 所有一時 SpellManager には干渉しない
                var owner = spellManager.add(OFFSETS.AActor_Owner).readPointer();
                if (owner.isNull() || isPendingKill(owner) || !isA(owner, getPlayerStateClass())) {
                    return;
                }

                var plan = buildSpellPlan(spellManager);
                if (!plan) {
                    return;
                }

                spellManager.add(OFFSETS.ADH_SpellManager_MaxSpells).writeS32(plan.spells.length);
                writeSpellListBuffer(plan.spells);
                args[1] = spellListBuffer;
                logModeOnce(plan);
            } catch (e) {}
        }
    });

    // 2) 試合開始時フォールバック (onEnter のみ・onLeave 消費 0):
    //    万一カスタムスペル登録より先に SetEquippedSpells が呼ばれたプレイヤーが居た場合も全呪文へ再同期
    Interceptor.attach(moduleBase.add(RVA.ADH_GameState_HandleMatchHasStarted), {
        onEnter: function (args) {
            try {
                var gameState = args[0];
                if (!gameState || gameState.isNull() || isPendingKill(gameState)) {
                    return;
                }
                var plan = buildSpellPlan(gameState);
                if (!plan) {
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
                    if (sm.isNull() || isPendingKill(sm)) {
                        continue;
                    }
                    if (sm.add(OFFSETS.AActor_Role).readU8() !== ROLE_AUTHORITY) {
                        continue;
                    }
                    var curEquippedNum = getArraySize(sm.add(OFFSETS.ADH_SpellManager_EquippedSpells));
                    if (curEquippedNum === plan.spells.length) {
                        continue;
                    }
                    sm.add(OFFSETS.ADH_SpellManager_MaxSpells).writeS32(plan.spells.length);
                    writeSpellListBuffer(plan.spells);
                    inSelfApply = true;
                    try {
                        ADH_SpellManager_SetEquippedSpells(sm, spellListBuffer);
                    } finally {
                        inSelfApply = false;
                    }
                }
                logModeOnce(plan);
            } catch (e) {}
        }
    });

    log('初期化完了 (通常5種 / カスタムスペル導入時は全種自動装備)');
})();
