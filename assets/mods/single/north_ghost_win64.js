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
function getGameState()
{
    var World = GWorld.readPointer();
    if(World.isNull()) {
        return ptr(0);
    }
    var AuthorityGameMode = World.add(0x118).readPointer();
    if(AuthorityGameMode.isNull()) {
        return ptr(0);
    }
    var GameState = AuthorityGameMode.add(0x280).readPointer();
    return GameState;
}


function logInfo(Info) {
    var f = new File('output.log', 'a');
    f.write('[Patches] ' + Info + '\n');
    f.close();
}

function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
}
function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}
function loadClassByName(ClassName) {
    return loadObjectByName(ClassName, UClass_GetPrivateStaticClass());
}
function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}
function getClass(ClassName) {
    var Clazz = findClassByName(ClassName);
    if (!Clazz.isNull()) {
        return Clazz;
    }
    return loadClassByName(ClassName);
}
function newFString(Length) {
    var FString = Memory.alloc(16 + Length * 2);
    FString.writePointer(FString.add(16));
    FString.add(8).writeU32(0);
    FString.add(12).writeU32(Length);
    return FString;
}
function getObjectName(UObject) {
    var NamePrivate = UObject.add(0x18);
    var NameBuffer = newFString(100);
    FName_GetPlainNameString(NamePrivate, NameBuffer);
    return NameBuffer.readPointer().readUtf16String();
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
    var RootComponent = AActor.add(0x130).readPointer();
    if(RootComponent.isNull()) {
        return ptr(0);
    }
    var ComponentToWorld = RootComponent.add(0x1C0);
    return ComponentToWorld;
}
function shuffle(array) {
    var res = [], random;
    while(array.length) {
        random = Math.floor(Math.random() * array.length);
        res.push(array[random]);
        array.splice(random, 1);
    }
    return res;
}

function Distance(Transform1, Transform2) {
    var Translation1 = Transform1.add(0x10);
    var Translation2 = Transform2.add(0x10);
    var X = Translation1.readFloat() - Translation2.readFloat();
    var Y = Translation1.add(4).readFloat() - Translation2.add(4).readFloat();
    var Z = Translation1.add(8).readFloat() - Translation2.add(8).readFloat();
    return Math.sqrt(X * X + Y * Y + Z * Z);
}

function initSpellManager(Owner, SpellChargeLevel){
    if(Owner.isNull()) {
        return ptr(0);
    }
    var World = UObject_GetWorld(Owner);
    var SpellManagerClass = getClass('/Game/Blueprints/Game/Totems/BP_PlayerSpellManager.BP_PlayerSpellManager_C');
    if(SpellManagerClass.isNull()) {
        return ptr(0);
    }
    var SpellManager = spawnActor(World, SpellManagerClass, FTransform_Identity, Owner);
    if(SpellManager.isNull()) {
        return ptr(0);
    }
    var GameInstance = UDH_GameInstance_GetInstance(SpellManager);
    if(GameInstance.isNull()) {
        return SpellManager;
    }
    SpellManager.add(0x228).writeU32(10); // Allow all 5 ThrallSpells including TS_Hush (index 4)
    var ThrallSpells = GameInstance.add(0x440);
    ADH_SpellManager_SetEquippedSpells(SpellManager, ThrallSpells);
    ADH_SpellManager_SetSpellChargeTier(SpellManager, 1 + SpellChargeLevel);
    return SpellManager;
}
var GameStarted = false;
var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var SpellManagers = [];

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter : function (args) {
        GameStarted = true;
        var GameState = getGameState();
        for(var i = 0; i < 2; i++) {
            SpellManagers.push(initSpellManager(GameState, 3));
        }
    }
});

var CastTimer = 240 + Math.random() % 120;
var CastCooldown = 5 + Math.random() % 15;
var CastLevel = Math.floor(Math.random() * 2) + 2;
var CastedTime = 0;

var PoisonTimer = 400 + Math.random() % 120;
var PoisonCooldown = 320;
var PoisonTime = 0;

var ExplosionTimer = 600 + Math.random() % 120;
var ExplosionCooldown = 480;

function GameTick(GameState, DeltaTick) {
    var Warship = GameState.add(0x2B0).readPointer();
    if(Warship.isNull()) {
        return;
    }
    var Boiler = Warship.add(0x2E0).readPointer();
    if(Boiler.isNull()) {
        return;
    }

    var WarshipPos = getActorTransform(Warship);
    var BoilerPos = getActorTransform(Boiler);
    ExplosionTimer -= DeltaTick;
    if(ExplosionTimer < 0.0) {
        ExplosionTimer = ExplosionCooldown + Math.random() * 120;
        var c = Math.random() * 100;
        var ExplosionClassName = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C');
        if(!ExplosionClassName.isNull()) {
            if(c > 50) { 
                spawnActor(UObject_GetWorld(GameState), ExplosionClassName, WarshipPos, ptr(0));
            }
            else {
                spawnActor(UObject_GetWorld(GameState), ExplosionClassName, BoilerPos, ptr(0));
            }
        }
        
    }


    PoisonTimer -= DeltaTick;
    if(PoisonTimer < 0.0 && PoisonTime <= 3) {
        PoisonTimer = PoisonCooldown + Math.random() * 240;
        var AvailableTarget = [];
        var PlayerArray = GameState.add(0x238);
        var Players = PlayerArray.readPointer();
        var PlayerNum = PlayerArray.add(8).readU32();
        for(var j = 0; j < PlayerNum; j++) {
            var PlayerState = Players.add(8 * j).readPointer();
            if(PlayerState.isNull()) {
                continue;
            }
            var IsDead = PlayerState.add(0x570).readU8();
            var IsThrall = PlayerState.add(0x572).readU8();
            if(IsDead == 1 || IsThrall == 1) {
                continue;
            }
            var Pawn = PlayerState.add(0x280).readPointer();
            if(Pawn.isNull() || getObjectName(Pawn.add(0x10).readPointer()) != 'BP_HumanCharacter_C') {
                continue;
            }
            var bPoison = Pawn.add(0x1250).readU8();
            if(bPoison == 1) {
                continue;
            }
            AvailableTarget.push(Pawn);
        }
        
        if(AvailableTarget.length > 0) {
            var bPoison = AvailableTarget[0].add(0x1250);
            bPoison.writeU8(1);
            PoisonTime += 1;
        }
        
    }

    
    for(var i = 0; i < SpellManagers.length; i++) {
        var SpellManager = SpellManagers[i];
        if(SpellManager.isNull()) {
            continue;
        }
        if(CastedTime > 6) {
            CastedTime = 6;
        } 
        var Charge = SpellManager.add(0x284).readFloat() + DeltaTick / (1500.0 - CastedTime * 100);
        if(Charge >= 1.0) {
            SpellManager.add(0x284).writeFloat(1.0);
        }
        else {
            SpellManager.add(0x284).writeFloat(Charge);
        }
    }
    CastTimer -= DeltaTick;
    if(CastTimer < 0.0) {
        CastTimer = CastCooldown + Math.random() * 5;
        for(var i = 0; i < SpellManagers.length; i++) {
            var SpellManager = SpellManagers[i];
            if(SpellManager.isNull()) {
                continue;
            }
            var AvailableTarget = [];
            var PlayerArray = GameState.add(0x238);
            var Players = PlayerArray.readPointer();
            var PlayerNum = PlayerArray.add(8).readU32();
            for(var j = 0; j < PlayerNum; j++) {
                var PlayerState = Players.add(8 * j).readPointer();
                var IsDead = PlayerState.add(0x570).readU8();
                var IsThrall = PlayerState.add(0x572).readU8();
                if(IsDead == 1 || IsThrall == 1) {
                    continue;
                }
                var Pawn = PlayerState.add(0x280).readPointer();
                if(getObjectName(Pawn.add(0x10).readPointer()) != 'BP_HumanCharacter_C') {
                    continue;
                }
                var ShipBase = Pawn.add(0x9F8).readPointer();
                var bInsideShip = Pawn.add(0xE44).readU8();
                if(!ShipBase.isNull() || bInsideShip == 1) {
                    continue;
                }
                AvailableTarget.push(Pawn);
            }
            

            var MaxDist = 0;
            var Target = ptr(0);
            for(var j = 0; j < AvailableTarget.length; j++) {
                var Pos = getActorTransform(AvailableTarget[j]);
                var Dist = Distance(WarshipPos, Pos);
                if(Dist >= MaxDist && Dist >= 1000) {
                    MaxDist = Dist;
                    Target = AvailableTarget[j];
                }
            }
            if(!Target.isNull()) {
                var CannibalSpell = findClassByName('/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C');
                if(!CannibalSpell.isNull() && ADH_SpellManager_CanCast(SpellManager, CannibalSpell) == 1) {
                    CastedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(SpellManager, CastLevel);
                    ADH_SpellManager_CastSpell(SpellManager, CannibalSpell, Target);
                    CastLevel = Math.floor(Math.random() * 2) + 2;
                    break;
                }
            }

            var c = Math.random() * 100;

            if(c > 20) {
                var WhiteoutSpell = findClassByName('/Game/Blueprints/Game/Totems/TS_Whiteout.TS_Whiteout_C');
                if(!WhiteoutSpell.isNull() && ADH_SpellManager_CanCast(SpellManager, WhiteoutSpell) == 1) {
                    CastedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(SpellManager, CastLevel);
                    ADH_SpellManager_CastSpell(SpellManager, WhiteoutSpell, Target);
                    CastLevel = Math.floor(Math.random() * 2) + 2;
                    break;
                }
            }
            else {
                var HushSpell = findClassByName('/Game/Blueprints/Game/Totems/TS_Hush.TS_Hush_C');
                if(!HushSpell.isNull() && ADH_SpellManager_CanCast(SpellManager, HushSpell) == 1) {
                    CastedTime += 1;
                    ADH_SpellManager_SetSpellChargeTier(SpellManager, CastLevel);
                    ADH_SpellManager_CastSpell(SpellManager, HushSpell, Target);
                    CastLevel = Math.floor(Math.random() * 2) + 2;
                    break;
                }
            }
            
        }
    }
    

}


var ADH_GameState_Tick_addr = base.add(0xDAD420);
Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter : function(args) {
        if(!GameStarted) {
            return;
        }
        
        var GameState = args[0];
        var TickTime = GameState.add(0x5A8).readFloat();
        GameTick(GameState, TickTime);
    }
});

var ADH_PlayerController_ShowPreGameInstructions_addr = base.add(0xEE7E50);
Interceptor.attach(ADH_PlayerController_ShowPreGameInstructions_addr, {
    onEnter : function (args) {
        this.context.rdx = 2;
        //this.context.rdi = 2; for linux
    }
});


var ADH_GameMode_RandomizeThralls_addr = base.add(0xDA4620);
var ADH_PlayerState_SetIsThrall_addr = base.add(0xE4E880);

Interceptor.attach(ADH_GameMode_RandomizeThralls_addr, {
    onEnter : function (args) {
        Interceptor.replace(ADH_PlayerState_SetIsThrall_addr, new NativeCallback(function (PlayerState, IsThrall) {
                    return;
                }, 'void', ['pointer', 'int8']));
    }
});
