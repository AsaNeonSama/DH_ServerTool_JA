var js_name = "madmate.js"
logInfo("START");

var log_output = false; // true false

function logInfo(Info) {
    if (log_output) {
        var f = new File('output.log', 'a');
        f.write('[Patches:' + js_name + '] ' + Info + '\n');
        f.close();
    }
}



var randomRole_on = false;

var mad_on = true;
var leader_on = true;
var thrall_p_on = true;
var thrall_c_on = true;

var mad_set_end = false;
var leader_set_end = false;
var thrall_p_set_end = false;
var thrall_c_set_end = false;

var inventoryGiven = false; // フラグを追加
var inventory_mad_set = false;
var inventory_thrall_set = false;
var inventory_leader_set = false;

var BlizzardStarted_1 = false;

var role_count_array = [];

var PS_mad = "";
var PS_leader = "";
var PS_thrall = [];

var select_mad = 0;
var select_leader = 0;
var select_thrall_p = false;
var select_thrall_c = false;


var Name_madmate = "";
var Name_thrall = [];
var Name_leader = "";

var ShuffleArray = [];

var ms_count_mad = 0;
var ms_count_leader = 0;
var ms_count_parent = 0
var ms_count_child = 0

var thrall_parent = "";
var thrall_child = "";
var thrall_parent_count = 0;

var mad_thrall = false;

var PS_temp = [
    [
        ["No=", 1],
        ['Captain', "船長"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 2],
        ['Engineer', "技師"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 3],
        ['Hunter', "猟師"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 4],
        ['Cook', "料理人"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 5],
        ['Navigator', "航海士"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 6],
        ['Chaplain', "牧師"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 7],
        ['Marine', "海兵"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ],
    [
        ["No=", 8],
        ['Doctor', "医師"],
        ["PS=", ""],
        ["Thrall=", 0],
        ["Mad=", 0],
        ["Leader=", 0],
        ["Thrall_P=", 0],
        ["Thrall_C=", 0],
        ["DeadCount", 0]
    ]
]


var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');


function log_ps_temp(PS_temp, contents) {
    logInfo(contents + "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
    for (var i = 0; i < PS_temp.length; i++) {
        logInfo("PS_temp[i]:" + PS_temp[i]);
    }
    logInfo("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
}


//random role====================================================================================================
function randomNum(max) {
    return Math.floor(Math.random() * max);
}
var roles = [1, 2, 3, 4, 5, 6, 7, 8];
/*
1 船長
2 技師
3 猟師
4 料理人
5 航海士
6 牧師
7 海兵
8 医師
 */

var ADH_PlayerState_SetPlayerRole = base.add(0xE4F390);
var UDH_PlayerRoleData_FindByType = new NativeFunction(base.add(0xE33300), "size_t", ["char", "pointer"], 'win64');
var ADH_PlayerState_SetPlayerRole_Interceptor = Interceptor.attach(ADH_PlayerState_SetPlayerRole, {
    onEnter: function(args) {
        var ADH_PlayerState = args[0];
        if (randomRole_on == true) {

            var OnlineName_old = getString(args[1].add(0x48));
            logInfo("OnlineName_old:" + OnlineName_old);

            logInfo("ADH_PlayerState random:" + ADH_PlayerState);
            var index = randomNum(roles.length);
            //logInfo("index:" + index);
            var role = roles[index];
            logInfo("role:" + role);
            var New_UDH_PlayerRoleData = UDH_PlayerRoleData_FindByType(role, ADH_PlayerState)
                //logInfo("New_UDH_PlayerRoleData:" + New_UDH_PlayerRoleData);

            //var s = '0x' + New_UDH_PlayerRoleData.toString(16)
            //logInfo("s:" + s);

            roles.splice(index, 1);
            args[1] = ptr(New_UDH_PlayerRoleData)

            var OnlineName_new = getString(args[1].add(0x48));
            logInfo("OnlineName_new:" + OnlineName_new);

            updatePSArray_new(OnlineName_new, ADH_PlayerState)
            role_count_array.push(ADH_PlayerState);

            log_ps_temp(PS_temp, "random role");

        } else if (randomRole_on == false) {

            var OnlineName_old = getString(args[1].add(0x48));
            logInfo("OnlineName_old:" + OnlineName_old);

            logInfo("ADH_PlayerState normal:" + ADH_PlayerState);

            updatePSArray_new(OnlineName_old, ADH_PlayerState)
            role_count_array.push(ADH_PlayerState);

            log_ps_temp(PS_temp, "normal role");

        }

    }
});


function updatePSArray_new(OnlineName, ADH_PlayerState) {
    for (var i = 0; i < PS_temp.length; i++) {
        //logInfo("i-1:" + i);
        //logInfo("PS_temp.length:" + PS_temp.length);
        if (PS_temp[i][1][0] == OnlineName) {
            //logInfo("i-2:" + i);
            PS_temp[i][2][1] = ADH_PlayerState;

            break;
        }
    }
}



function choosePlayer(excludePlayer) {
    //var PlayerArray = GameState.add(0x238);
    var ShuffleArray = [];
    //logInfo("getArraySize(PlayerArray):" + getArraySize(PlayerArray)); //プレイヤー数が入る
    for (var i = 0; i < PS_temp.length; i++) {
        var PlayerState = PS_temp[i][2][1];

        if (PlayerState !== "") {
            //logInfo("PlayerState:" + PlayerState);
            var IsThrall = PlayerState.add(0x572).readU8();
            var bIsDead = PlayerState.add(0x570).readU8();
            var bIsDisconnected = PlayerState.add(0x571).readU8();
            var SelectedRole = PlayerState.add(0x590).readPointer();
            var OnlineName = getString(SelectedRole.add(0x48));
            //updatePSArray(OnlineName, PlayerState);

            if (IsThrall) {
                PS_temp[i][3][1] = 1;
                if (!Name_thrall.includes(OnlineName)) {
                    Name_thrall.push(OnlineName);
                    PS_thrall.push(PlayerState);
                }
            }

            if (!bIsDead && !bIsDisconnected && OnlineName !== excludePlayer) {
                ShuffleArray.push(ptr(PlayerState));
            }
            /*if (IsThrall && !bIsDead && !bIsDisconnected && OnlineName !== excludePlayer) {
                if (!Name_thrall.includes(OnlineName)) {
                    Name_thrall.push(OnlineName);
                    //PS_thrall.push(PlayerState);
                }
            }*/
        }
    }
    if (ShuffleArray.length == 0) {
        return ptr(0);
    }
    //logInfo("ShuffleArray:" + ShuffleArray);
    return ShuffleArray[Math.ceil(Math.random() * ShuffleArray.length) % ShuffleArray.length];
}



var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');

function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
}

var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}

function InitItemState(State, Upgraded, Tainted) {
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


var UDH_InventoryManager_AddInventory = new NativeFunction(base.add(0xDBC040), 'void', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], 'win64');

function giveInventory(PlayerState, ClassName) {
    if (!inventoryGiven) { // フラグをチェック
        var HumanCharacter = PlayerState.add(0x280).readPointer();
        if (HumanCharacter.isNull()) {
            return;
        }
        var ItemClass = findClassByName(ClassName);
        var InventoryComponent = HumanCharacter.add(0x808).readPointer();
        var Array = Memory.alloc(16 + 56);
        Array.writePointer(Array.add(16));
        Array.add(8).writeU32(1);
        Array.add(12).writeU32(1);
        InitItemState(Array.readPointer(), 0, 0);
        var Buffer = Memory.alloc(8);
        Buffer.writeU32(0);
        Buffer.add(4).writeU32(0xffffffff);
        UDH_InventoryManager_AddInventory(InventoryComponent, ItemClass, Array, Buffer, Buffer.add(4), 0, ptr(0));
        //logInfo("give");
        inventoryGiven = true; // フラグを更新
    }
}

function getString(FString) {
    var Size = getArraySize(FString);
    //logInfo("Size:" + Size);
    var String = getArrayItemAddr(FString, 0, 0).readUtf16String(Size);
    return String;
}

// ====================================================================================================
//スペルチャージ----------------------------------------------------------------------------------------------------
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);

function resetSpells(SpellManager) {
    if (SpellManager.isNull()) {
        return;
    }
    var SpellChargeLevel = SpellManager.add(0x284);
    SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 0.05);
}

Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function(args) {
        var PlayerState = args[0].add(0x240).readPointer();
        var SpellManager = PlayerState.add(0x488).readPointer();
        resetSpells(SpellManager);
    }
});

//5 spells----------------------------------------------------------------------------------------------------
var SpellSelection = [
    [0, 4, 3, 1, 2]
];

var Max = 0;

for (var i = 0; i < SpellSelection.length; i++) {
    if (SpellSelection[i].length > Max) {
        Max = SpellSelection[i].length;
    }
}
/*
SpellSelection
0: 傀儡召喚
1: ホワイトアウト
2: ドッペルゲンガー
3: 幽霊歩き
4: 沈黙
*/

//SpellManager
var SpellManagerInit_addr = base.add(0xE58AC0);
var ADH_SpellManager_SetEquippedSpells_addr = base.add(0xE81F00);

var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], 'win64');
Interceptor.attach(SpellManagerInit_addr, {
    onEnter: function(args) {
        this.SpellManager = args[0];
    },
    onLeave: function(ret) {
        var Object = this.SpellManager;
        Object.add(0x228).writeU32(Max);
        //logInfo("Object_origin:" + Object);
    }
});



function getArraySize(TArray) {
    return TArray.add(8).readU32();
}

function getArrayItemAddr(TArray, size, index) {
    var ArrNum = getArraySize(TArray);
    if (index > ArrNum)
        return null;
    return TArray.readPointer().add(index * size);
}
//SetInThrall SpellManager



var Count = 0;
var SpellList = Memory.alloc(0x100);
SpellList.writePointer(SpellList.add(16));
SpellList.add(12).writeU32(Max);

Interceptor.attach(ADH_SpellManager_SetEquippedSpells_addr, {
    onEnter: function(args) {
        var SpellManager = args[0];
        var GameInstance = UDH_GameInstance_GetInstance(SpellManager);
        var ThrallSpells = GameInstance.add(0x440);
        var Size = getArraySize(ThrallSpells);
        if (Size == 5) {
            if (Count == SpellSelection.length) {
                Count = 0;
            }
            SpellList.add(8).writeU32(SpellSelection[Count].length);
            for (var i = 0; i < SpellSelection[Count].length; i++) {
                if (SpellSelection[Count][i] >= Size) {
                    return;
                }
                var SpellType = getArrayItemAddr(ThrallSpells, 8, SpellSelection[Count][i]);
                SpellList.add(16 + 8 * i).writePointer(SpellType.readPointer());
            }
            args[1] = SpellList;
            Count++;
        }

    }

});



//死亡判定----------------------------------------------------------------------------------------------------

var ADH_PlayerState_SetIsDead_addr = base.add(0xE4E700);
var ADH_PlayerState_SetIsThrall = new NativeFunction(base.add(0xE4E880), 'void', ['pointer', 'int8'], 'win64');
Interceptor.attach(ADH_PlayerState_SetIsDead_addr, {
    onEnter: function(args) {
        var DeadFlag = args[1] & 1;
        if (DeadFlag) { // プレイヤーの状態が死亡したときのみ
            var PlayerState = args[0];
            var IsThrall = PlayerState.add(0x572).readU8();

            for (var i = 0; i < PS_temp.length; i++) {
                var PS_entry = PS_temp[i];
                var PS = "" + PS_entry[2][1]
                if (PS == PlayerState) {
                    PS_entry[8][1]++;
                    log_ps_temp(PS_temp, "DeadFlag");

                    /*
                    if (IsThrall && PS_entry[8][1] === 2) {
                        //logInfo("傀儡だよー");
                        for (var j = 0; j < PS_thrall.length; j++) {
                            if (PS_thrall[j] !== PS) {
                                ADH_PlayerState_SetIsThrall(PS_thrall[1], 1);
                            }
                        }
                    }*/
                    break;
                }
            }
        }
    }
});


//常に監視====================================================================================================
var ADH_GameState_Tick = base.add(0xDAD420);
//var ParentChild = false;

Interceptor.attach(ADH_GameState_Tick, {
    onEnter: function(args) {
        var GameState = args[0];
        if (GameState.isNull()) {
            return;
        }

        var PlayerArray = GameState.add(0x238);
        var PA_size = getArraySize(PlayerArray);
        //logInfo("PA_size:" + PA_size);
        if (role_count_array.length >= 1) {
            if (role_count_array.length == PA_size) {
                //logInfo("true");

                //thrall parent
                if (select_thrall_p == false && thrall_p_on == true && thrall_p_set_end == false) {
                    shuffle_Array(PS_temp)
                    var ChooseOne = choosePlayer("");
                    var PlayerState = "" + ChooseOne;
                    var SelectedRole = ChooseOne.add(0x590).readPointer();
                    var OnlineName = SelectedRole.add(0x48);

                    for (var i = 0; i < PS_temp.length; i++) {
                        if (PS_temp[i][2][1] == PlayerState && PS_temp[i][3][1] == 1 && PS_temp[i][6][1] == 0 && PS_temp[i][7][1] == 0) {
                            PS_temp[i][6][1] = 1;

                            thrall_parent = ChooseOne;
                            //sortByNo(PS_temp);
                            log_ps_temp(PS_temp, "thrall parent set");

                            thrall_p_set_end = true;
                            select_thrall_p = true; // カウンターをインクリメント
                            break;
                        }
                    }
                }
                //thrall child
                if (select_thrall_c == false && thrall_c_on == true && thrall_c_set_end == false) {
                    shuffle_Array(PS_temp)
                    var ChooseOne = choosePlayer("");
                    var PlayerState = "" + ChooseOne;
                    var SelectedRole = ChooseOne.add(0x590).readPointer();
                    var OnlineName = SelectedRole.add(0x48);

                    for (var i = 0; i < PS_temp.length; i++) {
                        if (PS_temp[i][2][1] == PlayerState && PS_temp[i][3][1] == 1 && PS_temp[i][6][1] == 0 && PS_temp[i][7][1] == 0) {
                            PS_temp[i][7][1] = 1;


                            thrall_child = ChooseOne;
                            //sortByNo(PS_temp);
                            log_ps_temp(PS_temp, "thrall child set");

                            thrall_c_set_end = true;
                            select_thrall_c = true; // カウンターをインクリメント
                            break;
                        }
                    }
                }

                //unique role set--------------------------------------------------
                //mad
                if (select_mad <= 1 && mad_on == true && mad_set_end == false && (thrall_p_set_end == true && thrall_c_set_end == true)) {
                    shuffle_Array(PS_temp)
                    var ChooseOne = choosePlayer("");
                    var PlayerState = "" + ChooseOne;
                    var SelectedRole = ChooseOne.add(0x590).readPointer();
                    var OnlineName = SelectedRole.add(0x48);

                    for (var i = 0; i < PS_temp.length; i++) {
                        if (PS_temp[i][2][1] == PlayerState && PS_temp[i][3][1] == 0 && PS_temp[i][4][1] == 0 && PS_temp[i][5][1] == 0) {
                            PS_temp[i][4][1] = 1;

                            PS_mad = ChooseOne;
                            Name_madmate = getString(OnlineName); // グローバル変数に値を設定

                            //sortByNo(PS_temp);
                            log_ps_temp(PS_temp, "mad set");

                            mad_set_end = true;
                            select_mad++; // カウンターをインクリメント
                            break;
                        }
                    }
                }
                //leader
                if (select_leader <= 1 && leader_on == true && leader_set_end == false && (thrall_p_set_end == true && thrall_c_set_end == true)) {
                    shuffle_Array(PS_temp)
                    var ChooseOne = choosePlayer("");
                    var PlayerState = "" + ChooseOne;
                    var SelectedRole = ChooseOne.add(0x590).readPointer();
                    var OnlineName = SelectedRole.add(0x48);

                    for (var i = 0; i < PS_temp.length; i++) {
                        if (PS_temp[i][2][1] == PlayerState && PS_temp[i][3][1] == 0 && PS_temp[i][4][1] == 0 && PS_temp[i][5][1] == 0) {
                            PS_temp[i][5][1] = 1;

                            PS_leader = ChooseOne;
                            Name_leader = getString(OnlineName); // グローバル変数に値を設定

                            //sortByNo(PS_temp);
                            log_ps_temp(PS_temp, "leader set");

                            leader_set_end = true;
                            select_leader++; // カウンターをインクリメント
                            break;
                        }
                    }
                }

            }
        }

        //thrall parent child
        /*
        if (!ParentChild) {
            //logInfo("PS_thrall.length" + PS_thrall.length);
            //logInfo("PS_thrall.length === 2" + PS_thrall.length === 2);
            if (PS_thrall.length === 2) {
                shuffle_Array(PS_thrall)
                for (var i = 0; i < PS_thrall.length; i++) {
                    logInfo("PS_thrall[i]:" + PS_thrall[i]);
                    if (thrall_parent === "") {
                        thrall_parent = PS_thrall[i];
                        PS_temp[i][6][1] = 1;
                    } else if (thrall_child === "") {
                        thrall_child = PS_thrall[i];
                        PS_temp[i][7][1] = 1;
                        ParentChild = true;
                    }
                }
            }
        }
        */


        var BlizzardStarted = GameState.add(0x359).readU8();



        if (ms_count_parent == 0) {
            if (PS_mad !== "") {
                //logInfo("thrall_parentだよ");

                if (thrall_parent_count == 1) {
                    //下から表示
                    var MessageList = [
                        "力が湧いてきたようだ",
                    ]
                    var ADH_PlayerState = PS_mad
                    var PlayerId = ADH_PlayerState.add(0x224).readU8()
                    var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

                    //logInfo("ADH_PlayerState:" + ADH_PlayerState);
                    //logInfo("PlayerId:" + PlayerId);
                    //logInfo("PlayerController:" + PlayerController);
                    SendMessages(PlayerController, MessageList)
                } else if (thrall_parent_count == 2) {
                    //下から表示
                    var MessageList = [
                        "今こそ復讐の時だ",
                    ]
                    var ADH_PlayerState = PS_mad
                    var PlayerId = ADH_PlayerState.add(0x224).readU8()
                    var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

                    //logInfo("ADH_PlayerState:" + ADH_PlayerState);
                    //logInfo("PlayerId:" + PlayerId);
                    //logInfo("PlayerController:" + PlayerController);
                    SendMessages(PlayerController, MessageList)
                    ms_count_parent++;
                }
            }
        }



        if (!BlizzardStarted_1) {
            if (BlizzardStarted == 1) {


                // Name_thrallとName_madmateに対応する日本語名を取得
                var JapaneseNames_thrall = Name_thrall.map(getJapaneseName);
                var JapaneseName_madmate = getJapaneseName(Name_madmate);

                //下から表示
                var MessageList_leader = [
                    "",
                    "狂人：" + JapaneseName_madmate,
                    "傀儡：" + JapaneseNames_thrall.join(", "),
                ]

                if (PS_leader !== "") {
                    var ADH_PlayerState_leader = PS_leader
                    logInfo("ADH_PlayerState_leader:" + ADH_PlayerState_leader);
                    var PlayerIde_leader = ADH_PlayerState_leader.add(0x224).readU8()
                    var PlayerControllere_leader = UGameplayStatics_GetPlayerController(ADH_PlayerState_leader, PlayerIde_leader)

                    //logInfo("ADH_PlayerState:" + ADH_PlayerState);
                    //logInfo("PlayerId:" + PlayerId);
                    //logInfo("PlayerController:" + PlayerController);
                    SendMessages(PlayerControllere_leader, MessageList_leader)
                }

                logInfo("MessageList_leader:" + MessageList_leader);

                BlizzardStarted_1 = true; // フラグを更新
            }
        }



    }
});


/*
if (select_mad <= 1) { // フラグをチェック
    //logInfo("select_mad:" + select_mad);
    Interceptor.attach(ADH_GameState_Tick, {
        onEnter: function(args) {
            //var GameState = args[0];
            //logInfo("GameState: " + GameState);
            var ChooseOne = choosePlayer("");
            var SelectedRole = ChooseOne.add(0x590).readPointer();
            var OnlineName = SelectedRole.add(0x48);
            Name_madmate = getString(OnlineName); // グローバル変数に値を設定


            PS_mad = ChooseOne;
            //logInfo("GameState: " + GameState);
            //logInfo("PS_mad select: " + PS_mad);

            if (!ChooseOne.isNull()) {
                //giveInventory(ChooseOne, '/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C');
                //logInfo("madmate selected: " + Name_madmate);

                // select_leaderを選択
                var ChooseLeader = choosePlayer(Name_madmate);
                PS_leader = ChooseLeader;
                var SelectedRoleLeader = ChooseLeader.add(0x590).readPointer();
                var OnlineNameLeader = SelectedRoleLeader.add(0x48);
                Name_leader = getString(OnlineNameLeader); // グローバル変数に値を設定

                if (!ChooseLeader.isNull()) {
                    //logInfo("Leader selected: " + Name_leader);
                }
            }


        }
    });

    select_mad++; // カウンターをインクリメント
}
*/

function log_role_check(PlayerState, Name, contents) {
    logInfo(contents + "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
    //logInfo("HumanCharacter:" + HumanCharacter);
    logInfo("PlayerState:" + PlayerState);
    //logInfo("InventoryComponent:" + InventoryComponent);
    //logInfo("SelectedRole:" + SelectedRole);
    //logInfo("OnlineName:" + OnlineName);
    logInfo("----------");
    logInfo("Name:" + Name);
    logInfo("Name_madmate:" + Name_madmate);
    logInfo("Name_thrall:" + Name_thrall);
    logInfo("Name_leader:" + Name_leader);
    logInfo("----------");
    logInfo("PS_mad:" + PS_mad);
    logInfo("PS_leader:" + PS_leader);
    logInfo("PS_thrall:" + PS_thrall);
    logInfo("----------");
    logInfo("thrall_parent:" + thrall_parent);
    logInfo("thrall_child:" + thrall_child);
    logInfo("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
}



//インベントリ設定====================================================================================================
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(base.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');
//全体のインベントリを設定
/*var StorageLimit_all = new Map([
    ['Captain', 9],
    ['Chaplain', 9],
    ['Cook', 9],
    ['Doctor', 9],
    ['Engineer', 9],
    ['Hunter', 9],
    ['Marine', 9],
    ['Navigator', 9]
]);
Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function(args) {
        var HumanCharacter = args[0];
        var PlayerState = HumanCharacter.add(0x240).readPointer();
        var InventoryComponent = HumanCharacter.add(0x808).readPointer();
        var SelectedRole = PlayerState.add(0x590).readPointer();

        var OnlineName = SelectedRole.add(0x48);
        var Name = getString(OnlineName);
        if (StorageLimit_all.has(Name)) {
            UDH_InventoryManager_SetStorageLimit(InventoryComponent, StorageLimit_all.get(Name));
        }
    }
});*/

var StorageLimit_mad = new Map([
    ['Captain', 8],
    ['Chaplain', 8],
    ['Cook', 8],
    ['Doctor', 8],
    ['Engineer', 8],
    ['Hunter', 8],
    ['Marine', 8],
    ['Navigator', 8]
]);

if (!inventory_mad_set && mad_on == true) { // フラグをチェック
    Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
        onEnter: function(args) {
            var HumanCharacter = args[0];
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            var InventoryComponent = HumanCharacter.add(0x808).readPointer();
            var SelectedRole = PlayerState.add(0x590).readPointer();

            var OnlineName = SelectedRole.add(0x48);
            var Name = getString(OnlineName);

            //sortByNo(PS_temp);
            logInfo("■mad====================================================================================================");
            log_ps_temp(PS_temp, "inv mad set");
            log_role_check(PlayerState, Name, "mad role check")

            if (StorageLimit_mad.has(Name) && Name_madmate == Name) {
                UDH_InventoryManager_SetStorageLimit(InventoryComponent, StorageLimit_mad.get(Name));
                //giveInventory(PlayerState, '/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C');
            }


            //下から表示
            var MessageList = [
                "狂人として探検家を惑わせろ",
            ]
            logInfo("MessageList:" + MessageList);

            var ADH_PlayerState = PS_mad
            var PlayerId = ADH_PlayerState.add(0x224).readU8()
            var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

            //logInfo("ADH_PlayerState:" + ADH_PlayerState);
            //logInfo("PlayerId:" + PlayerId);
            //logInfo("PlayerController:" + PlayerController);
            if (ms_count_mad == 0) {
                SendMessages(PlayerController, MessageList)
                ms_count_mad++;
            }
        }
    });

    inventory_mad_set = true; // フラグを更新
}

var StorageLimit_leader = new Map([
    ['Captain', 10],
    ['Chaplain', 10],
    ['Cook', 10],
    ['Doctor', 10],
    ['Engineer', 10],
    ['Hunter', 10],
    ['Marine', 10],
    ['Navigator', 10]
]);
if (!inventory_leader_set && leader_on == true) { // フラグをチェック
    Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
        onEnter: function(args) {
            var HumanCharacter = args[0];
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            var InventoryComponent = HumanCharacter.add(0x808).readPointer();
            var SelectedRole = PlayerState.add(0x590).readPointer();

            var OnlineName = SelectedRole.add(0x48);
            var Name = getString(OnlineName);

            //sortByNo(PS_temp);
            logInfo("■leader====================================================================================================");
            log_ps_temp(PS_temp, "inv leader set");
            log_role_check(PlayerState, Name, "leader role check")

            if (StorageLimit_leader.has(Name) && Name_leader == Name) {
                UDH_InventoryManager_SetStorageLimit(InventoryComponent, StorageLimit_leader.get(Name));
                //giveInventory(PlayerState, '/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C');
            }


            //下から表示
            var MessageList = [
                "仲間を先導し闇の傀儡を暴け",
            ]
            logInfo("MessageList:" + MessageList);

            var ADH_PlayerState = PS_leader
            var PlayerId = ADH_PlayerState.add(0x224).readU8()
            var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

            //logInfo("ADH_PlayerState:" + ADH_PlayerState);
            //logInfo("PlayerId:" + PlayerId);
            //logInfo("PlayerController:" + PlayerController);
            if (ms_count_leader == 0) {
                SendMessages(PlayerController, MessageList)
                ms_count_leader++;
            }
        }
    });

    inventory_leader_set = true; // フラグを更新
}

var StorageLimit_thrall = new Map([
    ['Captain', 8],
    ['Chaplain', 8],
    ['Cook', 8],
    ['Doctor', 8],
    ['Engineer', 8],
    ['Hunter', 8],
    ['Marine', 8],
    ['Navigator', 8]
]);

if (!inventory_thrall_set && (thrall_p_on == true || thrall_c_on == true)) { // フラグをチェック
    Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
        onEnter: function(args) {
            var HumanCharacter = args[0];
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            var InventoryComponent = HumanCharacter.add(0x808).readPointer();
            var SelectedRole = PlayerState.add(0x590).readPointer();
            var IsThrall = PlayerState.add(0x572).readU8();

            var OnlineName = SelectedRole.add(0x48);
            var Name = getString(OnlineName);

            //sortByNo(PS_temp);
            logInfo("■thrall====================================================================================================");
            log_ps_temp(PS_temp, "inv thrall set");
            log_role_check(PlayerState, Name, "mad thrall check")

            if (StorageLimit_thrall.has(Name) && IsThrall == 1) {
                UDH_InventoryManager_SetStorageLimit(InventoryComponent, StorageLimit_thrall.get(Name));
            }



            var ps = "" + PlayerState
            var psp = "" + thrall_parent
            var psc = "" + thrall_child

            if (psp == ps) {
                //logInfo("thrall_parentだよ");

                //下から表示
                var MessageList_parent = [
                    "傀儡の親としていつも通りにやればいい",
                ]
                logInfo("MessageList_parent:" + MessageList_parent);
                var ADH_PlayerState = thrall_parent
                var PlayerId = ADH_PlayerState.add(0x224).readU8()
                var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

                //logInfo("ADH_PlayerState:" + ADH_PlayerState);
                //logInfo("PlayerId:" + PlayerId);
                //logInfo("PlayerController:" + PlayerController);
                if (ms_count_parent == 0) {
                    SendMessages(PlayerController, MessageList_parent)
                    ms_count_parent++;
                }
            }
            if (psc == ps) {
                //logInfo("thrall_childだよ");
                //下から表示
                var MessageList_child = [
                    "傀儡の子として親の背中を追え",
                ]
                logInfo("MessageList_child:" + MessageList_child);
                var ADH_PlayerState = thrall_child
                var PlayerId = ADH_PlayerState.add(0x224).readU8()
                var PlayerController = UGameplayStatics_GetPlayerController(ADH_PlayerState, PlayerId)

                //logInfo("ADH_PlayerState:" + ADH_PlayerState);
                //logInfo("PlayerId:" + PlayerId);
                //logInfo("PlayerController:" + PlayerController);
                if (ms_count_child == 0) {
                    SendMessages(PlayerController, MessageList_child)
                    ms_count_child++;
                }
            }

        }
    });

    inventory_thrall_set = true; // フラグを更新
}


//アナウンス====================================================================================================
var FName_FName = new NativeFunction(base.add(0x1158F20), "void", ["pointer", "pointer", "int8"]);
var FText_FromName = new NativeFunction(base.add(0x1096370), "pointer", ["pointer", "pointer"]);
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), "void", ["pointer", "pointer", "pointer"]);
var UGameplayStatics_GetPlayerController = new NativeFunction(base.add(0x25630D0), 'pointer', ['pointer', 'int32']);

function newFName(Name) {
    var FName_Buffer = Memory.alloc(8);
    var Buffer = Memory.alloc((Name.length + 4) * 2);
    Buffer.writeUtf16String("   " + Name);
    FName_FName(FName_Buffer, Buffer, 1)
    return FName_Buffer
}

function FNameToFText(FName) {
    var FText_Buffer = Memory.alloc(24)
    FText_FromName(FText_Buffer, FName)
    return FText_Buffer
}

function SendThrallMessage(PlayerController, Message, Usound) {
    var FName_Message = newFName(Message)
    var FText_Message = FNameToFText(FName_Message)
    ADH_PlayerController_ReceiveThrallMessage(PlayerController, FText_Message, Usound)
}



function SendMessages(PlayerController, MessageList) {
    var count = 0;
    var timer = null;

    function sendMessages() {
        for (let i = 0; i < MessageList.length; ++i) {
            SendThrallMessage(PlayerController, MessageList[i], ptr(0))
        }
        count++;
        if (count == 5) {
            clearInterval(timer);
        }
    }
    sendMessages();
    timer = setInterval(sendMessages, 10000);
}




// 名前から日本語名を取得する関数
function getJapaneseName(name) {
    for (var i = 0; i < PS_temp.length; i++) {
        if (PS_temp[i][1][0] === name) {
            return PS_temp[i][1][1];
        }
    }
    return null; // 該当する名前がない場合
}


function shuffle_Array(array) {
    for (var i = array.length - 1; i > 0; i--) {
        var j = Math.floor(Math.random() * (i + 1));
        var temp = array[i];
        array[i] = array[j];
        array[j] = temp;
    }
    return array;
}

function sortByNo(array) {
    return array.sort(function(a, b) {
        var noA = a[0][1]; // aのNoの値
        var noB = b[0][1]; // bのNoの値
        return noA - noB; // 昇順にソート
    });
}



var ADH_SpellManager_CastSpell_addr = base.add(0xE634F0);

var ADH_PlayerState_GetOwningController = new NativeFunction(base.add(0xE39820), 'pointer', ['pointer'], 'win64');



var TotemSpells = [
    '/Game/Blueprints/Game/Totems/TS_SpiritWalk.TS_SpiritWalk_C',
    '/Game/Blueprints/Game/Totems/TS_Whiteout.TS_Whiteout_C',
    '/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C',
    '/Game/Blueprints/Game/Totems/TS_Doppelganger.TS_Doppelganger_C',
    '/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C'
];

var SpiritWalkClass = null;
var WhiteoutClass = null;
var HushClass = null;
var DoppelgangerClass = null;
var CannibalAttackClass = null;

function initSpellClass() {
    for (var i = 0; i < TotemSpells.length; i++) {
        var Clazz = findClassByName(TotemSpells[i]);
        if (Clazz.isNull()) {
            return false;
        }
        if (TotemSpells[i] === '/Game/Blueprints/Game/Totems/TS_SpiritWalk.TS_SpiritWalk_C') {
            SpiritWalkClass = Clazz;
        }
        if (TotemSpells[i] === '/Game/Blueprints/Game/Totems/TS_Whiteout.TS_Whiteout_C') {
            WhiteoutClass = Clazz;
        }
        if (TotemSpells[i] === '/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C') {
            HushClass = Clazz;
        }
        if (TotemSpells[i] === '/Game/Blueprints/Game/Totems/TS_Doppelganger.TS_Doppelganger_C') {
            DoppelgangerClass = Clazz;
        }
        if (TotemSpells[i] === '/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C') {
            CannibalAttackClass = Clazz;
        }
    }
    return true;
}

var Init = false;
Interceptor.attach(ADH_SpellManager_CastSpell_addr, {
    onEnter: function(args) {
        if (!Init) {
            if (!initSpellClass()) {
                return;
            }
            Init = true;
        }

        var SpellType = args[2];
        var SpellManager = args[0];
        var PlayerState = SpellManager.add(0xE0).readPointer();
        logInfo("PlayerState:" + PlayerState);


        var bIsDead = PlayerState.add(0x570).readU8();
        logInfo("bIsDead:" + bIsDead);


        var ps = "" + PlayerState
        var psp = "" + thrall_parent
        var psc = "" + thrall_child
        logInfo("ps:" + ps);
        logInfo("psp:" + psp);
        logInfo("psc:" + psc);

        var result = null;

        for (var i = 0; i < PS_temp.length; i++) {
            if (PS_temp[i][2][1] == psp) {
                result = PS_temp[i][8][1];
                thrall_parent_count = result
                    //logInfo("result:" + result);
                break;
            }
        }

        logInfo("result:" + result);

        var PlayerController = ADH_PlayerState_GetOwningController(PlayerState);
        if (PlayerController.isNull()) {
            return;
        }
        var ControlledHuman = PlayerController.add(0x588).readPointer();
        if (ControlledHuman.isNull()) {
            return;
        }


        // TS_SpiritWalk.TS_SpiritWalk_C以外の呪文の発動をキャンセルする
        if (ps == psc && result < 2 && !bIsDead) {
            if (SpellType.equals(WhiteoutClass)) {
                args[2] = ptr(0); // 無効なアドレスに変更してキャストをキャンセル

                var HealthBoostRemaining = ControlledHuman.add(0xFFC);
                HealthBoostRemaining.writeFloat(HealthBoostRemaining.readFloat() + 8);
            }
        }
        if (ps == psc && result < 1 && !bIsDead) {
            if (SpellType.equals(HushClass)) {
                args[2] = ptr(0); // 無効なアドレスに変更してキャストをキャンセル

                var HealthBoostRemaining = ControlledHuman.add(0xFFC);
                HealthBoostRemaining.writeFloat(HealthBoostRemaining.readFloat() + 8);
            }
        }
        if (ps == psc && result < 1 && !bIsDead) {
            if (SpellType.equals(DoppelgangerClass)) {
                args[2] = ptr(0); // 無効なアドレスに変更してキャストをキャンセル

                var HealthBoostRemaining = ControlledHuman.add(0xFFC);
                HealthBoostRemaining.writeFloat(HealthBoostRemaining.readFloat() + 8);
            }
        }
        if (ps == psc && result < 2 && !bIsDead) {
            if (SpellType.equals(CannibalAttackClass)) {
                args[2] = ptr(0); // 無効なアドレスに変更してキャストをキャンセル

                var HealthBoostRemaining = ControlledHuman.add(0xFFC);
                HealthBoostRemaining.writeFloat(HealthBoostRemaining.readFloat() + 8);
            }
        }
    }
});