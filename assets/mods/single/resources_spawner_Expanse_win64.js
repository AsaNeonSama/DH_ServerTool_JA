
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var FTransform_Identity = base.add(0x455C4E0);
var FActorSpawnParameters_FActorSpawnParameters = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], 'win64');
var UWorld_SpawnActor = new NativeFunction(base.add(0x2624510), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var AActor_SetActorScale3D = new NativeFunction(base.add(0x22E8E90), 'void', ['pointer', 'pointer'], 'win64');
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], 'win64');

var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');

var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');


function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

function findClassByName(ClassName) {
    var staticClass = UClass_GetPrivateStaticClass();
    if (staticClass.isNull()) {
        return ptr(0);
    }
    return findObjectByName(ClassName, staticClass);
}
function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}
function loadClassByName(ClassName) {
    var staticClass = UClass_GetPrivateStaticClass();
    if (staticClass.isNull()) {
        return ptr(0);
    }
    return loadObjectByName(ClassName, staticClass);
}
function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
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
function getClass(ClassName) {
    var Clazz = findClassByName(ClassName);
    if(!Clazz.isNull()) {
        return Clazz; 
    }
    return loadClassByName(ClassName);
}
function makeTransform(Rot, Pos, Scale) {
    // UWorld::SpawnActor executes aligned SSE `andps xmm0, xmmword ptr [rdi]` on FTransform;
    // force 16-byte alignment and retain backing buffer against GC.
    var RawBuffer = Memory.alloc(16 * 4);
    var TransformBuffer = RawBuffer.add(15).and(ptr('0xFFFFFFFFFFFFFFF0'));
    TransformBuffer._backing = RawBuffer;

    TransformBuffer.add(0).writeFloat(Rot[0]);
    TransformBuffer.add(4).writeFloat(Rot[1]);
    TransformBuffer.add(8).writeFloat(Rot[2]);
    TransformBuffer.add(12).writeFloat(1.0); // Valid unit quaternion W

    TransformBuffer.add(16).writeFloat(Pos[0]);
    TransformBuffer.add(20).writeFloat(Pos[1]);
    TransformBuffer.add(24).writeFloat(Pos[2]);
    TransformBuffer.add(28).writeFloat(0.0);

    TransformBuffer.add(32).writeFloat(Scale[0]);
    TransformBuffer.add(36).writeFloat(Scale[1]);
    TransformBuffer.add(40).writeFloat(Scale[2]);
    TransformBuffer.add(44).writeFloat(0.0);
    return TransformBuffer;
}

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
var ActorToSpawn = [
    ['/Game/Blueprints/AI/Prey/BP_Prey_Rat.BP_Prey_Rat_C', //左巨型老鼠
        [0, 0, 0], //朝向
        [-15019.61, -3055.90, 575.00], //位置 (地表 Z=748.46 へ落下ゼロでスナップ着地)
        [20, 20, 20] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Bear.BP_Predator_Bear_C',//中岛熊
        [0, 0, 0], //朝向
        [-3537.25, 4185.95, 55.15], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C', //中岛狼
        [0, 0, 0], //朝向
        [-4745.48, 3453.55, 165.75], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C', //中岛狼
        [0, 0, 0], //朝向
        [-4775.48, 3453.55, 165.75], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C', //中岛狼
        [0, 0, 0], //朝向
        [-4805.48, 3453.55, 165.75], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C', //中岛狼
        [0, 0, 0], //朝向
        [-4845.48, 3453.55, 265.75], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/AI/Predators/BP_Predator_Wolf.BP_Predator_Wolf_C', //中岛狼
        [0, 0, 0], //朝向
        [-4875.48, 3453.55, 265.75], //位置
        [1, 1, 1] //大小
    ],
	['/Game/Blueprints/Environment/EnvironmentContianers/BP_CoveredCrate_Storage_Nitro.BP_CoveredCrate_Storage_Nitro_C', //甘油车
        [0, 0, 0], //朝向
        [-3891.89, 2710.32, 120.65], //位置
        [1, 1, 1] //大小
    ],
    ['/Game/Blueprints/Environment/DestructibleShip/BP_DestructibleIceWall.BP_DestructibleIceWall_C', //左冰山
        [0, 0, 0], //朝向
        [-6576.14, -1756.27, -75.50], //位置
        [0.7, 0.7, 0.7] //大小
    ],
    ['/Game/Blueprints/Environment/DestructibleShip/BP_DestructibleIceWall.BP_DestructibleIceWall_C', //右冰山
        [0, 0, 0], //朝向
        [-3741.89, 10318.94, -100.15], //位置
        [0.7, 0.7, 0.7] //大小
    ],

];
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);
var lastSpawnedWorld = null;
Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter : function(args) {},
    onLeave : function(ret) {
        try {
            var World = GWorld.readPointer();
            var GameState = getGameState();
            if (World.isNull() || GameState.isNull()) {
                return;
            }
            var WorldKey = World.toString();
            if (lastSpawnedWorld === WorldKey) {
                return;
            }
            lastSpawnedWorld = WorldKey;
            for(var i = 0; i < ActorToSpawn.length; i++) {
                try {
                    var ActorClass = getClass(ActorToSpawn[i][0]);
                    if (ActorClass.isNull()) {
                        continue;
                    }
                    var Scale = ActorToSpawn[i][3];
                    var Transform = makeTransform(ActorToSpawn[i][1], ActorToSpawn[i][2], Scale);
                    var Actor = spawnActor(World, ActorClass, Transform, GameState);
                    if (!Actor.isNull() && (Scale[0] !== 1 || Scale[1] !== 1 || Scale[2] !== 1)) {
                        var ScaleVec = Memory.alloc(16);
                        ScaleVec.add(0).writeFloat(Scale[0]);
                        ScaleVec.add(4).writeFloat(Scale[1]);
                        ScaleVec.add(8).writeFloat(Scale[2]);
                        AActor_SetActorScale3D(Actor, ScaleVec);
                    }
                } catch (e) {}
            }
        } catch (e) {}
    }
});