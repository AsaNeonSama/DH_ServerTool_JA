var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var ADH_BrigCell_AddPrisoner = base.add(0xE5D180);
var ADH_BrigCell_FreePrisoner = new NativeFunction(base.add(0xE69380), 'pointer', ['pointer','pointer'], 'win64');
var ADH_BrigDoor_BeginPlay = new NativeFunction(base.add(0xE5FE20), 'pointer', ['pointer'], 'win64');
var UDH_DoorLockComponent_SetIsLocked = new NativeFunction(base.add(0xDE36B0), 'void', ['pointer','int8'], 'win64');
var ADH_DoorActor_SetOpen = new NativeFunction(base.add(0xDE3A30), 'void', ['pointer','int8','pointer','int8'], 'win64');
var ADH_BrigDoor_ADH_BrigDoor = new NativeFunction(base.add(0xE57C20), 'void', ['pointer','pointer'], 'win64');
var AActor_SetLifeSpan = new NativeFunction(base.add(0x22E9780), 'void', ['pointer', 'float'], 'win64');
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');
var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], 'win64');
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], 'win64');
var AActor_AttachToComponent = new NativeFunction(base.add(0x22CEF90), 'void', ['pointer', 'pointer', 'pointer', 'uint64'], 'win64');
var AActor_SetActorRelativeTransform = new NativeFunction(base.add(0x22E8D50), 'void', ['pointer', 'pointer', 'int8', 'pointer', 'int8'], 'win64');
var FTransform_Identity = base.add(0x455C4E0);
var FAttachmentTransformRules_Snap = base.add(0x4318720);

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

function spawnBrigDoor(BrigCell) {
    if (BrigCell.isNull()) {
        return ptr(0);
    }
    var BrigDoorClass = BrigCell.add(0x2F8).readPointer();
    if (BrigDoorClass.isNull()) {
        return ptr(0);
    }
    var World = UObject_GetWorld(BrigCell);
    if (World.isNull()) {
        return ptr(0);
    }
    var SpawnParams = Memory.alloc(0x30);
    FActorSpawnParameters_FActorSpawnParameters(SpawnParams);
    SpawnParams.add(0x10).writePointer(BrigCell);
    SpawnParams.add(0x28).writeU8(1); // AlwaysSpawn
    var NewDoor = UWorld_SpawnActor(World, BrigDoorClass, FTransform_Identity, SpawnParams);
    if (!NewDoor.isNull()) {
        var AttachComp = BrigCell.add(0x250).readPointer();
        if (!AttachComp.isNull()) {
            AActor_AttachToComponent(NewDoor, AttachComp, FAttachmentTransformRules_Snap, uint64(0));
        }
        AActor_SetActorRelativeTransform(NewDoor, BrigCell.add(0x2C0), 0, ptr(0), 0);
    }
    return NewDoor;
}

function resetBrigCell(BrigCell) {
    if (BrigCell.isNull()) {
        return;
    }
    BrigCell.add(0x2B0).writeU8(0);
    BrigCell.add(0x2F0).writePointer(ptr(0));
    var DoorSlot = BrigCell.add(0x300);
    var OldDoor = DoorSlot.readPointer();
    if (!OldDoor.isNull()) {
        AActor_SetLifeSpan(OldDoor, 0.1);
    }
    DoorSlot.writePointer(ptr(0));
    var newDoor = spawnBrigDoor(BrigCell);
    DoorSlot.writePointer(newDoor);
}
Interceptor.attach(ADH_BrigCell_AddPrisoner, {
    onEnter: function(args) {
    	this.ADH_BrigCell = args[0];
    },
    onLeave: function(ret) {
    	var ADH_BrigCell = this.ADH_BrigCell;
    	ADH_BrigCell_FreePrisoner(ADH_BrigCell,ptr(0));
    }
});
var ADH_DoorActor_SetOpen = base.add(0xDE3A30)
Interceptor.attach(ADH_DoorActor_SetOpen, {
    onEnter: function(args) {
    	this.ADH_DoorActor = args[0]
    	this.bNewIsOpen = args[1]
    },
    onLeave: function(ret) {
    	var clazz = getObjectName(this.ADH_DoorActor.add(0x10).readPointer());
    	if(clazz.includes('BP_BrigDoor')){
    		var BrigCell = this.ADH_DoorActor.add(0xE0).readPointer();
    		resetBrigCell(BrigCell);
    	}
    }
});