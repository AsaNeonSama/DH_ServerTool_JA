var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var FName_FName = new NativeFunction(base.add(0x1158F20), "void", ["pointer", "pointer", "int8"]);
var FText_FromName = new NativeFunction(base.add(0x1096370), "pointer", ["pointer","pointer"]);
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), "void", ["pointer", "pointer", "pointer"]);
var UGameplayStatics_GetPlayerController = new NativeFunction(base.add(0x25630D0), 'pointer', ['pointer', 'int32']);
var ADH_PlayerState_SetIsDead_addr = base.add(0xE4E700);
var ADH_PlayerState_SetIsDisconnected = base.add(0xE4E770);
var GWorld = base.add(0x46ED420);

/*
true: オン
false: オフ
 */

// 死亡アナウンス
var DeadNotice = false;
// 切断アナウンス
var DisconnectedNotice = true;



function GetGameState() {
    var World = GWorld.readPointer();
    if (World.isNull()) {
        return ptr(0);
    }
    var AuthorityGameMode = World.add(0x118).readPointer();
    if (AuthorityGameMode.isNull()) {
        return ptr(0);
    }
    return AuthorityGameMode.add(0x280).readPointer();
}
function newFName(Name) {
    var FName_Buffer = Memory.alloc(8);
    var Buffer = Memory.alloc((Name.length + 4) * 2);
    Buffer.writeUtf16String("   " + Name);
    FName_FName(FName_Buffer,Buffer,1)
    return FName_Buffer
}
function FNameToFText(FName){
    var FText_Buffer = Memory.alloc(24)
    FText_FromName(FText_Buffer,FName)
    return FText_Buffer
}
function SendThrallMessage(PlayerController,Message,Usound){
    if (PlayerController.isNull()) {
        return;
    }
    try {
        var FName_Message = newFName(Message);
        var FText_Message = FNameToFText(FName_Message);
        ADH_PlayerController_ReceiveThrallMessage(PlayerController,FText_Message,Usound);
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
    if (PlayerState.isNull()) {
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
    if (SelectedRole.isNull()) {
        return "不明";
    }
    var number = SelectedRole.add(0x58).readU8();
    return roleNames[number] || "不明";
}
function NotifyPlayers(GameState) {
    if (GameState.isNull()) {
        return [];
    }
    var PlayerArray = GameState.add(0x238);
    var arrSize = getArraySize(PlayerArray);
    if (arrSize === 0 || arrSize > 64) {
        return [];
    }
    var matchingPlayers = [];
    for (var i = 0; i < arrSize; i++) {
        var entry = getArrayItemAddr(PlayerArray, 8, i);
        if (entry === null) {
            continue;
        }
        var PlayerState = entry.readPointer();
        if (PlayerState.isNull()) {
            continue;
        }
        var bIsDisconnected = PlayerState.add(0x571).readU8();
        var PlayerId = PlayerState.add(0x224).readU8();
        var PlayerController = UGameplayStatics_GetPlayerController(PlayerState,PlayerId);
        if (!bIsDisconnected && !PlayerController.isNull()) {
            matchingPlayers.push(PlayerController);
        }
    }
    return matchingPlayers;
}


function SendDeathMessage(PlayerState) {
    if (PlayerState.isNull()) {
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
    if (PlayerState.isNull()) {
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
                SendDisconnectedMessage(PlayerState);
            } catch (e) {}
        }
    });
}
