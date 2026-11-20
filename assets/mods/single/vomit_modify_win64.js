var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UClass_SerializeDefaultObject_addr = base.add(0x11C3530);
var GWorld = base.add(0x46ED420);
var VomitCurve = [0.0, 0.11, 0.12, 0.13, 0.14, 0.15, 0.16, 0.17, 0.18, 0.19, 0.20, 0.21, 0.22, 0.23, 0.24, 0.25, 0.88, 0.89, 0.975, 0.99, 1.0];
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');
var FMemory_Realloc = new NativeFunction(base.add(0x106C3D0), 'pointer', ['pointer', 'int64', 'int32'], 'win64');
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
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

function arrayResize(Array, ElementSize, Num) {
    var Allocator = Array.readPointer();
    Array.add(12).writeU32(Num);
    var NewOne = FMemory_Realloc(Allocator, Num * ElementSize, 0);
    Array.writePointer(NewOne);
}
function getArraySize(TArray) {
    return TArray.add(8).readU32();
}
function getArrayItemAddr(TArray, size, index) {
    var ArrNum = getArraySize(TArray);
    if (index > ArrNum)
        return null;
    return TArray.readPointer().add(index * size);
}

function applyVomitCurveToCdo(CDO) {
    if (CDO.isNull()) {
        return;
    }
    var PoisonReserveHealth = CDO.add(0xC10).readPointer();
    if (PoisonReserveHealth.isNull()) {
        return;
    }
    var FloatCurve = PoisonReserveHealth.add(0x30);
    var FloatKeys = FloatCurve.add(0x70);
    arrayResize(FloatKeys, 28, VomitCurve.length);
    FloatKeys.add(8).writeU32(VomitCurve.length);
    for(var i = 0; i < VomitCurve.length; i++) {
        var CurveKey = getArrayItemAddr(FloatKeys, 28, i);
        CurveKey.add(0x0).writeU8(0);
        CurveKey.add(0x1).writeU8(0);
        CurveKey.add(0x2).writeU8(0);
        CurveKey.add(0x4).writeFloat(i);
        CurveKey.add(0x8).writeFloat(VomitCurve[i]);
        CurveKey.add(0xC).writeFloat(0);
        CurveKey.add(0x10).writeFloat(0);
        CurveKey.add(0x14).writeFloat(0);
        CurveKey.add(0x18).writeFloat(0);
    }
}

Interceptor.attach(UClass_SerializeDefaultObject_addr, {
    onEnter : function(args) {
        this.UClass = args[0];
        this.ClassDefaultObject = args[1];
    },
    onLeave : function(ret) {
        if(this.UClass.isNull() || getObjectName(this.UClass) != 'BP_HumanCharacter_C') {
            return;
        }
        applyVomitCurveToCdo(this.ClassDefaultObject);
    }
});

// Fallback for late external-attach mode (when UE4 is already initialized and CDO already serialized)
try {
    if (!GWorld.readPointer().isNull()) {
        var HumanClassName = '/Game/Blueprints/Player/BP_HumanCharacter.BP_HumanCharacter_C';
        var NameBuf = Memory.alloc((HumanClassName.length + 1) * 2);
        NameBuf.writeUtf16String(HumanClassName);
        var HumanClass = StaticFindObject(UClass_GetPrivateStaticClass(), ptr(0xFFFFFFFFFFFFFFFF), NameBuf, 0);
        if (!HumanClass.isNull()) {
            var ExistingCdo = HumanClass.add(0x118).readPointer();
            applyVomitCurveToCdo(ExistingCdo);
        }
    }
} catch (e) {}