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
    if (AActor.isNull()) {
        return ptr(0);
    }
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
    var staticClass = UClass_GetPrivateStaticClass();
    if (staticClass.isNull()) {
        return ptr(0);
    }
    return findObjectByName(ClassName, staticClass);
}
function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}
function loadClassByName(ClassName) {
    var staticClass = UClass_GetPrivateStaticClass();
    if (staticClass.isNull()) {
        return ptr(0);
    }
    return loadObjectByName(ClassName, staticClass);
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
    if (UObject.isNull()) {
        return '';
    }
    var NamePrivate = UObject.add(0x18);
    var NameBuffer = newFString(100);
    FName_GetPlainNameString(NamePrivate, NameBuffer);
    var DataPtr = NameBuffer.readPointer();
    if (DataPtr.isNull()) {
        return '';
    }
    var text = DataPtr.readUtf16String();
    return text ? text : '';
}
/*
/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C
/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C
/Game/Blueprints/Game/BP_Explosion_Gunpowder_OffShip.BP_Explosion_Gunpowder_OffShip_C
*/
var GRENADE_PICKUP_NAMES = {
    BP_Rock_Pickup_C: true,
    BP_Stick_Pickup_C: true,
    BP_Coal_Pickup_C: true,
    BP_LeadIngot_Pickup_C: true,
    BP_Whetstone_Pickup_C: true,
    BP_BoneClub_Pickup_C: true,
    BP_CoalBarrel_Pickup_C: true,
    BP_Sword_Pickup_C: true,
    BP_WoodAxe_Pickup_C: true,
    BP_IceAxe_Pickup_C: true,
    BP_Cleaver_Pickup_C: true
};
var ADH_InventoryPickup_OnPickupMovementStopped_addr = base.add(0xDD77C0);
Interceptor.attach(ADH_InventoryPickup_OnPickupMovementStopped_addr, {
    onEnter : function(args){
        this.ADH_InventoryPickup = args[0];
    }, 
    onLeave : function(ret){
        try {
            var pickup = this.ADH_InventoryPickup;
            if (!pickup || pickup.isNull()) {
                return;
            }
            var InventoryName = getObjectName(pickup);
            if (!Object.prototype.hasOwnProperty.call(GRENADE_PICKUP_NAMES, InventoryName)) {
                return;
            }
            var Pos = getActorTransform(pickup);
            var ExplosionActor = getClass('/Game/Blueprints/Game/BP_Explosion_Gunpowder.BP_Explosion_Gunpowder_C');
            var World = UObject_GetWorld(pickup);
            if (Pos.isNull() || ExplosionActor.isNull() || World.isNull()) {
                return;
            }
            AActor_SetLifeSpan(pickup, 0.1);
            spawnActor(World, ExplosionActor, Pos);
        } catch (e) {}
    }
});