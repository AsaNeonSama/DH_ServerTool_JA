var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], 'win64');
var AActor_SetLifeSpan = new NativeFunction(base.add(0x22E9780), 'void', ['pointer', 'float'], 'win64');
var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], 'win64');
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');

function getActorTransform(AActor) {
    var RootComponent = AActor.add(0x130).readPointer();
    if (RootComponent.isNull()) {
        return ptr(0);
    }
    var ComponentToWorld = RootComponent.add(0x1C0);
    return ComponentToWorld;
}
function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}
function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
}
function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}
function loadClassByName(ClassName) {
    return loadObjectByName(ClassName, UClass_GetPrivateStaticClass());
}
function getClass(ClassName) {
    var Clazz = findClassByName(ClassName);
    if (!Clazz.isNull()) {
        return Clazz;
    }
    return loadClassByName(ClassName);
}
function spawnActor(World, Clazz, Position) {
    if (World.isNull() || Clazz.isNull() || Position.isNull()) {
        return ptr(0);
    }
    var Parameters = Memory.alloc(0x30);
    FActorSpawnParameters_FActorSpawnParameters(Parameters);
    Parameters.add(0x28).writeU8(1); // ESpawnActorCollisionHandlingMethod::AlwaysSpawn
    return UWorld_SpawnActor(World, Clazz, Position, Parameters);
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
/*
/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C
/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C
/Game/Blueprints/Game/BP_Explosion_Gunpowder_OffShip.BP_Explosion_Gunpowder_OffShip_C
*/
var ADH_InventoryPickup_OnPickupMovementStopped_addr = base.add(0xDD77C0)
Interceptor.attach(ADH_InventoryPickup_OnPickupMovementStopped_addr, {
    onEnter : function(args){
        this.ADH_InventoryPickup = args[0]
    }, 
    onLeave : function(ret){
        var InventoryName = getObjectName(this.ADH_InventoryPickup)
        //杂物类
        if(InventoryName == "BP_Rock_Pickup_C"){ //石头
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_Stick_Pickup_C"){ //木板
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_Coal_Pickup_C"){ //煤炭
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_LeadIngot_Pickup_C"){ //铅锭
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_Whetstone_Pickup_C"){ //磨刀石
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_BoneClub_Pickup_C"){ //骨棒
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_CoalBarrel_Pickup_C"){ //煤炭桶
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        //武器类
        else if(InventoryName == "BP_Sword_Pickup_C"){ //军刀
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_WoodAxe_Pickup_C"){ //木斧
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_IceAxe_Pickup_C"){ //冰镐
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
        else if(InventoryName == "BP_Cleaver_Pickup_C"){ //菜刀
            var Pos = getActorTransform(this.ADH_InventoryPickup)
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C')
            var World = UObject_GetWorld(this.ADH_InventoryPickup)
            AActor_SetLifeSpan(this.ADH_InventoryPickup,0.1)
            var Explosion = spawnActor(World, ExplosionActor, Pos)
        }
    }
});