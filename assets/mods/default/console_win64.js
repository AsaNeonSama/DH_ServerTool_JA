var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var Port = __CONSOLE_PATCH_PORT__;
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var FName_FName = new NativeFunction(base.add(0x11592A0), "void", ["pointer", "pointer", "int8"]);
var FText_FromString = new NativeFunction(base.add(0x10963B0), "pointer", ["pointer", "pointer"]);
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');
function newFName(Name) {
    var FName_Buffer = Memory.alloc(8);
    var Buffer = Memory.alloc((Name.length + 1) * 3);
    Buffer.writeUtf16String(Name);
    FName_FName(FName_Buffer,Buffer,1);
    return FName_Buffer;
}
function newFText(Text){
    var FText_Buffer = Memory.alloc(24);
    var FString_Buffer = newFString(Text.length + 3);
    writeFString(FString_Buffer, Text);
    FText_FromString(FText_Buffer, FString_Buffer);
    return FText_Buffer;
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

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');

function loadClassByName(ClassName) {
    return loadObjectByName(ClassName, UClass_GetPrivateStaticClass());
}

function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
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
    var Clazz = findClassByName(ClassName);
    if(Clazz.isNull()) {
        Clazz = loadClassByName(ClassName);
    }
    return Clazz;
}
function getObjectName(UObject) {
    var NamePrivate = UObject.add(0x18);
    var NameBuffer = newFString(100);
    FName_GetPlainNameString(NamePrivate, NameBuffer);
    return NameBuffer.readPointer().readUtf16String();
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var PropertyOffsets = {
    'bUpdateHunger' : ['int8', 0xa40],
    'bUpdateWarmth' : ['int8', 0xa41],
    'HealthBoostRegenRate' : ['float', 0xa58],
    'WarmthRegenRate' : ['float', 0xa80],
    'HungerRegenRate' : ['float', 0xa84],
    'JumpStaminaConsumption' : ['float', 0xa8c],
    'BodyTossStaminaConsumption' : ['float', 0xa90],
    'TossPawnVelocity' : ['float', 0xb1c],
    'SpiritWalkSpeedModifier' : ['float', 0xbc0],
    'MaxHealthRegenSpeed' : ['float', 0xca4],
    'WarmthBoostRemaining' : ['float', 0xff4],
    'HungerBoostRemaining' : ['float', 0xff8],
    'HealthBoostRemaining' : ['float', 0xffc],
    'MaximumStamina' : ['float', 0xa48],
    'MinimumStamina' : ['float', 0xa4c],
    'CurrentHealth' : ['float', 0x94c],
    'MaximumHealth' : ['float', 0x7e0],
    'MinHealth' : ['float', 0x1000],
    'MaxHunger' : ['float', 0x1010],
    'bSpiritWalking' : ['int8', 0xc71],
    'bPoisoned' : ['int8', 0x1250]
};
function processProperty(PropTable, PropertyName, Value, TargetObject) {
    if(PropTable.hasOwnProperty(PropertyName)) {
        var WriteType = PropTable[PropertyName][0];
        var Offset = PropTable[PropertyName][1];
        var TargetPointer = TargetObject.add(Offset);
        if(WriteType == 'int8') {
            TargetPointer.writeU8(Value);
        }
        else if(WriteType == 'int16') {
            TargetPointer.writeU16(Value);
        }
        else if(WriteType == 'int32') {
            TargetPointer.writeU32(Value);
        }
        else if(WriteType == 'int64') {
            TargetPointer.writeU64(Value);
        }
        else if(WriteType == 'float') {
            TargetPointer.writeFloat(Value);
        }
        
    }

}
function setHumanCharacter(PlayerController, PropertyName, Value) {
    if(PlayerController.isNull()) {
        return;
    }
    var Pawn = PlayerController.add(0x250).readPointer();
    if(Pawn.isNull()) {
        return;
    }
    var PawnClass = Pawn.add(0x10).readPointer();
    if(PawnClass.isNull()) {
        return;
    }
    if(getObjectName(PawnClass) == 'BP_HumanCharacter_C') {
        processProperty(PropertyOffsets, PropertyName, Value, Pawn);
    }
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var UDH_InventoryManager_AddInventory = new NativeFunction(base.add(0xDBC040), 'void', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], 'win64');
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
function addInventory(PlayerController, ItemName, Count) {
    if(PlayerController.isNull()) {
        return;
    }
    var ItemClass = getClass(ItemName);
    if(ItemClass.isNull()) {
        return;
    }
    var Buffer = Memory.alloc(8);
    var ItemArray = Memory.alloc(16 + 56);
    ItemArray.writePointer(ItemArray.add(16));
    ItemArray.add(8).writeU32(1);
    ItemArray.add(12).writeU32(1);
    InitItemState(ItemArray.readPointer(), 0, 0);

    var Pawn = PlayerController.add(0x250).readPointer();
    if(Pawn.isNull()) {
        return;
    }
    var InventoryComponent = Pawn.add(0x808).readPointer();
    if(InventoryComponent.isNull()) {
        return;
    }
    for(var j = 0; j < Count; j ++) {
        Buffer.writeU32(0);
        Buffer.add(4).writeU32(0xffffffff);
        UDH_InventoryManager_AddInventory(InventoryComponent, ItemClass, ItemArray, Buffer, Buffer.add(4), 0, ptr(0));
    }
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(base.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');
function setStorageLimit(PlayerController, Limit) {
    if(PlayerController.isNull()) {
        return;
    }
    var Pawn = PlayerController.add(0x250).readPointer();
    if(Pawn.isNull()) {
        return;
    }
    var InventoryComponent = Pawn.add(0x808).readPointer();
    if(InventoryComponent.isNull()) {
        return;
    }
    UDH_InventoryManager_SetStorageLimit(InventoryComponent, Limit);
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var AGameSession_KickPlayer = new NativeFunction(base.add(0x256B770), 'void', ['pointer', 'pointer', 'pointer']);
function kickPlayer(GameState, PlayerController) {
    if(GameState.isNull() || PlayerController.isNull()) {
        return;
    }
    var GameMode = GameState.add(0x228).readPointer();
    if(GameMode.isNull()) {
        return;
    }
    var GameSession = GameMode.add(0x278).readPointer();
    if(GameSession.isNull()) {
        return;
    }
    var FText_Message = newFText('kick');
    AGameSession_KickPlayer(GameSession, PlayerController, FText_Message);
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var ADH_PlayerController_ReceiveThrallMessage = new NativeFunction(base.add(0xEE7810), "void", ["pointer", "pointer", "pointer"]);
function sendThrallMessage(PlayerController, Message){
    if(PlayerController.isNull()) {
        return;
    }
    var FText_Message = newFText(Message);
    ADH_PlayerController_ReceiveThrallMessage(PlayerController, FText_Message, ptr(0));
}
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
var APlayerController_BeginPlay_addr = base.add(0xE2DBC0);
var ADH_PlayerController_AddCheats = new NativeFunction(base.add(0xE2B7E0), 'void', ['pointer', 'int8'], 'win64');
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var UKismetSystemLibrary_ExecuteConsoleCommand = new NativeFunction(base.add(0x25EFC50), 'void', ['pointer', 'pointer', 'pointer'], 'win64');

// Preload map for `summon <ShortClassName>` so classes not spawned by the current map
// (e.g. BP_Prey_Rat_C on Departure) are loaded into memory before UCheatManager::Summon runs.
var SummonClassPaths = {
    'BP_Prey_Rat_C': '/Game/Blueprints/AI/Prey/BP_Prey_Rat.BP_Prey_Rat_C',
    'BP_Prey_Rabbit_C': '/Game/Blueprints/AI/Prey/BP_Prey_Rabbit.BP_Prey_Rabbit_C',
    'BP_Prey_Seal_C': '/Game/Blueprints/AI/Prey/BP_Prey_Seal.BP_Prey_Seal_C',
    'BP_Predator_Wolf_C': '/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C',
    'BP_Predator_Bear_C': '/Game/Blueprints/AI/Predators/BP_Predator_Bear.BP_Predator_Bear_C',
    'BP_Cannibal_C': '/Game/Blueprints/AI/Cannibal/BP_Cannibal.BP_Cannibal_C',
    'BP_Cannibal_Crawler_C': '/Game/Blueprints/AI/Cannibal/BP_Cannibal_Crawler.BP_Cannibal_Crawler_C',
    'BP_Cannibal_Shambler_C': '/Game/Blueprints/AI/Cannibal/BP_Cannibal_Shambler.BP_Cannibal_Shambler_C',
    'BP_HumanCharacter_C': '/Game/Blueprints/Player/BP_HumanCharacter.BP_HumanCharacter_C',
    'BP_Totem_C': '/Game/Blueprints/Game/Totems/BP_Totem.BP_Totem_C',
    'BP_ExplosionActor_C': '/Game/Blueprints/Game/BP_ExplosionActor.BP_ExplosionActor_C',
    'BP_Explosion_Nitro_C': '/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C'
};

function preloadSummonTarget(CommandString) {
    if (!CommandString || CommandString.toLowerCase().indexOf('summon ') !== 0) {
        return;
    }
    var targetName = CommandString.slice(7).trim();
    if (SummonClassPaths.hasOwnProperty(targetName)) {
        getClass(SummonClassPaths[targetName]);
    }
}
function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

var Commands = [];
function addCommand(Cmd) {
    if(Commands.length >= 100) {
        return false;
    }
    Commands.push(Cmd);
    return true;
}
function getCommand() {
    if(Commands.length != 0) {
        return Commands.shift();
    }
    return null;
}

function findPlayerController(GameState, FindName, UsePlayerName) {
    if(GameState.isNull()) {
        return ptr(0);
    }
    var PlayerArray = GameState.add(0x238);
    var Players = PlayerArray.readPointer();
    var PlayerNum = PlayerArray.add(8).readU32();
    if(Players.isNull() || PlayerNum === 0) {
        return ptr(0);
    }
    for(var i = 0; i < PlayerNum; i++) {
        var PlayerState = Players.add(8 * i).readPointer();
        if(PlayerState.isNull()) {
            continue;
        }
        var NameString;
        if(UsePlayerName == 1) {
            var PlayerNamePtr = PlayerState.add(0x300).readPointer();
            if(PlayerNamePtr.isNull()) {
                continue;
            }
            NameString = PlayerNamePtr.readUtf16String();
        }
        else {
            var SelectedRole = PlayerState.add(0x590).readPointer();
            if(SelectedRole.isNull()) {
                continue;
            }
            var RoleNamePtr = SelectedRole.add(0x48).readPointer();
            if(RoleNamePtr.isNull()) {
                continue;
            }
            NameString = RoleNamePtr.readUtf16String();
        }
        if(NameString == FindName) {
            var PlayerController = PlayerState.add(0xE0).readPointer();
            return PlayerController;
        }
    }
    return ptr(0);
}


function parseController(GameState, Command) {
    var Controller = ptr(0);
    if(Command.hasOwnProperty('role')) {
        Controller = findPlayerController(GameState, Command['role'], 0);
    }
    if(Command.hasOwnProperty('player')) {
        Controller = findPlayerController(GameState, Command['player'], 1);
    }
    return Controller;
}
Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter : function(args) {
        this.GameState = args[0];
    },
    onLeave : function(ret) {
        var GameState = this.GameState;
        if(GameState.isNull()) {
            return;
        }
        if(!ServerStarted) {
            server();
        }
        var Command = getCommand();
        if(Command) {
            try {
                if(Command['op'] == 'consoleexec') {
                    var CommandString = Command['command'];
                    preloadSummonTarget(CommandString);
                    var Controller = parseController(GameState, Command);
                    if(!Controller.isNull() && Controller.add(0x338).readPointer().isNull()) {
                        ADH_PlayerController_AddCheats(Controller, 1);
                    }
                    var WrapCmd = newFString(CommandString.length + 10);
                    writeFString(WrapCmd, CommandString);
                    UKismetSystemLibrary_ExecuteConsoleCommand(GameState, WrapCmd, Controller);
                }
                else if(Command['op'] == 'additem') {
                    var ItemName = Command['itemname'];
                    var Count = Command['count'];
                    var Controller = parseController(GameState, Command);
                    addInventory(Controller, ItemName, Count);
                }
                else if(Command['op'] == 'setstorage') {
                    var Limit = Command['limit'];
                    var Controller = parseController(GameState, Command);
                    setStorageLimit(Controller, Limit);
                }
                else if(Command['op'] == 'kick') {
                    var Controller = parseController(GameState, Command);
                    kickPlayer(GameState, Controller);
                }
                else if(Command['op'] == 'thrallmsg') {
                    var Message = Command['msg'];
                    var Controller = parseController(GameState, Command);
                    sendThrallMessage(Controller, Message);
                }
                else if(Command['op'] == 'sethuman') {
                    var PropName = Command['propname'];
                    var Value = Command['value'];
                    var Controller = parseController(GameState, Command);
                    setHumanCharacter(Controller, PropName, Value);
                }
            } catch (e) {
                logInfo('Command exec error (' + Command['op'] + '): ' + e.message);
            }
        }
    }
});
Interceptor.attach(APlayerController_BeginPlay_addr, {
    onEnter : function(args) {
        this.PlayerController = args[0];
    },
    onLeave : function(ret) {
        if(!this.PlayerController.isNull()) {
            ADH_PlayerController_AddCheats(this.PlayerController, 1);
        }
    }
});

var Listener;
var ServerStarted = false;
function failure(err)
{
    if(err && err.message) {
        logInfo(err.message);
    }
}

function safeClose(connection)
{
    try {
        var res = connection.close();
        if(res && typeof res.catch === 'function') {
            res.catch(function(){});
        }
    } catch (e) {}
}

function accepted(connection)
{
    connection.input.read(4096).then(function (buffer) {
        safeClose(connection);
        if (!buffer || buffer.byteLength < 2) {
            return;
        }
        var Cmd = buffer.unwrap().readUtf16String();
        if (!Cmd || Cmd.length === 0) {
            return;
        }
        try {
            var Obj = JSON.parse(Cmd);
            if (Obj && typeof Obj === 'object') {
                addCommand(Obj);
            }
        } catch (parseErr) {
            failure(parseErr);
        }
    }).catch(function (err) {
        safeClose(connection);
        failure(err);
    });
}
function accept_loop(listener){
    Listener = listener;
    listener.accept().then(function(connection) {
        accepted(connection);
        setImmediate(function() { accept_loop(listener); });
    }).catch(function(err) {
        var msg = err && err.message ? String(err.message) : '';
        if (msg.indexOf('cancelled') !== -1 || msg.indexOf('closed') !== -1) {
            ServerStarted = false;
            return;
        }
        failure(err);
        setImmediate(function() { accept_loop(listener); });
    });
}
function listened(listener)
{
    accept_loop(listener);
}
function server()
{
    if(ServerStarted) {
        return;
    }
    ServerStarted = true;
    Socket.listen({"family": 'ipv4', 'host': '0.0.0.0', 'port': Port})
        .then(listened)
        .catch(function(err) {
            ServerStarted = false;
            failure(err);
        });
}


Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter : function(args) {
        server();
    }
});
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////