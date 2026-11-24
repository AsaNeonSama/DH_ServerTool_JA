var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
// FNamePool汚染・FString&&ムーブ由来のUAF(0x10963B0)・リーク防止のため、
// const FString&版(0x10964E0)と FText::~FText(0x75CB30)を使用
var FText_FromString = new NativeFunction(base.add(0x10964E0), "pointer", ["pointer", "pointer"], "win64");
var FText_Dtor = new NativeFunction(base.add(0x75CB30), "void", ["pointer"], "win64");
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), "void", ["pointer", "pointer", "pointer"], "win64");
var APlayerController_GetPrivateStaticClass = new NativeFunction(base.add(0x2AA84E0), "pointer", [], "win64");
var ADH_PlayerState_SetIsDead_addr = base.add(0xE4E700);
var ADH_PlayerState_SetIsDisconnected = base.add(0xE4E770);
var GWorld = base.add(0x46ED420);
var GUObjectArray_ptr = base.add(0x45C40C8);
var cachedPlayerControllerClass = ptr(0);

/*
true: オン
false: オフ
 */

// 死亡アナウンス
var DeadNotice = false;
// 切断アナウンス
var DisconnectedNotice = true;


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
    var objClass = objPtr.add(0x10).readPointer();
    if (objClass.isNull()) {
        return false;
    }
    var targetDepth = targetClassPtr.add(0x38).readS32();
    var objDepth = objClass.add(0x38).readS32();
    if (targetDepth < 0 || targetDepth > objDepth) {
        return false;
    }
    var chainArray = objClass.add(0x30).readPointer();
    if (chainArray.isNull()) {
        return false;
    }
    return chainArray.add(targetDepth * 8).readPointer().equals(targetClassPtr.add(0x30));
}

function isPendingKill(objPtr) {
    if (objPtr.isNull()) {
        return true;
    }
    var internalIndex = objPtr.add(0x0C).readS32();
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

function GetGameState() {
    var World = GWorld.readPointer();
    if (World.isNull() || isPendingKill(World)) {
        return ptr(0);
    }
    var AuthorityGameMode = World.add(0x118).readPointer();
    if (AuthorityGameMode.isNull() || isPendingKill(AuthorityGameMode)) {
        return ptr(0);
    }
    return AuthorityGameMode.add(0x280).readPointer();
}

function createFTextFromString(text) {
    var fTextPtr = Memory.alloc(24);
    var capacityChars = text.length + 3;
    var fStringPtr = Memory.alloc(16 + capacityChars * 2);
    fStringPtr.writePointer(fStringPtr.add(16));
    fStringPtr.add(8).writeU32(text.length + 1);
    fStringPtr.add(12).writeU32(capacityChars);
    fStringPtr.add(16).writeUtf16String(text);
    FText_FromString(fTextPtr, fStringPtr);
    return fTextPtr;
}

function SendThrallMessage(PlayerController, Message, Usound) {
    if (PlayerController.isNull() || isPendingKill(PlayerController)) {
        return;
    }
    try {
        var FText_Message = createFTextFromString(Message);
        try {
            ADH_PlayerController_ReceiveThrallMessage(PlayerController, FText_Message, Usound);
        } finally {
            FText_Dtor(FText_Message);
        }
    } catch (e) {}
}

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

function getPlayerRoleName(PlayerState) {
    if (PlayerState.isNull() || isPendingKill(PlayerState)) {
        return "不明";
    }
    var roleNames = {
        1: "船長",
        2: "エンジニア",
        3: "猟師",
        4: "船の料理人",
        5: "航海士",
        6: "牧師",
        7: "イギリス海兵",
        8: "医師"
    };
    var SelectedRole = PlayerState.add(0x590).readPointer();
    if (SelectedRole.isNull() || isPendingKill(SelectedRole)) {
        return "不明";
    }
    var number = SelectedRole.add(0x58).readU8();
    return roleNames[number] || "不明";
}

function NotifyPlayers(GameState) {
    if (GameState.isNull() || isPendingKill(GameState)) {
        return [];
    }
    var PlayerArray = GameState.add(0x238);
    var arrSize = getArraySize(PlayerArray);
    if (arrSize === 0 || arrSize > 64) {
        return [];
    }
    var pcClass = getPlayerControllerClass();
    var matchingPlayers = [];
    for (var i = 0; i < arrSize; i++) {
        var entry = getArrayItemAddr(PlayerArray, 8, i);
        if (entry === null) {
            continue;
        }
        var PlayerState = entry.readPointer();
        if (PlayerState.isNull() || isPendingKill(PlayerState)) {
            continue;
        }
        var bIsDisconnected = PlayerState.add(0x571).readU8();
        if (bIsDisconnected) {
            continue;
        }
        var PlayerController = PlayerState.add(0xE0).readPointer();
        if (!PlayerController.isNull() && !isPendingKill(PlayerController) && isA(PlayerController, pcClass)) {
            matchingPlayers.push(PlayerController);
        }
    }
    return matchingPlayers;
}


function SendDeathMessage(PlayerState) {
    if (PlayerState.isNull() || isPendingKill(PlayerState)) {
        return;
    }
    if ((PlayerState.add(0x22A).readU8() & 0x20) !== 0) {
        return;
    }
    var name = getPlayerRoleName(PlayerState);
    var DeathMessages = {
        0: `${name} が死亡しました`,
        1: `${name} があの世へ旅立ちました〜`
    };
    var DeathCount = PlayerState.add(0x630).readU32();
    var GameState = GetGameState();
    var DeathNews = DeathMessages[DeathCount] || `${name} が死亡しました`;
    var PlayerController = NotifyPlayers(GameState);
    for (var i = 0; i < PlayerController.length; i++) {
        SendThrallMessage(PlayerController[i], DeathNews, ptr(0));
    }
}

function SendDisconnectedMessage(PlayerState) {
    if (PlayerState.isNull() || isPendingKill(PlayerState)) {
        return;
    }
    if ((PlayerState.add(0x22A).readU8() & 0x20) !== 0) {
        return;
    }
    var name = getPlayerRoleName(PlayerState);
    var GameState = GetGameState();
    var DisconnectedNews = `${name} がゲームから切断されました!`;
    var PlayerController = NotifyPlayers(GameState);
    for (var i = 0; i < PlayerController.length; i++) {
        SendThrallMessage(PlayerController[i], DisconnectedNews, ptr(0));
    }
}

if (DeadNotice) {
    Interceptor.attach(ADH_PlayerState_SetIsDead_addr, {
        onEnter: function(args) {
            try {
                var PlayerState = args[0];
                if (PlayerState.isNull()) {
                    return;
                }
                var DeadFlag = args[1].toInt32() & 1;
                if (DeadFlag) {
                    SendDeathMessage(PlayerState);
                }
            } catch (e) {}
        }
    });
}


if (DisconnectedNotice) {
    Interceptor.attach(ADH_PlayerState_SetIsDisconnected, {
        onEnter: function(args) {
            try {
                var PlayerState = args[0];
                if (PlayerState.isNull()) {
                    return;
                }
                var discFlag = args[1].toInt32() & 1;
                if (!discFlag || PlayerState.add(0x571).readU8() !== 0) {
                    return;
                }
                SendDisconnectedMessage(PlayerState);
            } catch (e) {}
        }
    });
}
