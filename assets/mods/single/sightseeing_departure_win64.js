// Sightseeing Mode (Departure / Departure_Persistent)
// Automatically fuels the boiler, relieves pressure, steers the warship along
// the Departure route waypoints, and detonates the final ice barrier with nitro.
// Note: Incompatible with resources_spawner_Departure_win64.js (extra ice walls block the route).

var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var ADH_Warship_SetEnableAutoMove = new NativeFunction(base.add(0xE81E50), 'void', ['pointer', 'int8'], 'win64');
var ADH_Warship_FuelBoiler = new NativeFunction(base.add(0xE69750), 'void', ['pointer', 'float'], 'win64');
var AActor_execK2_GetActorRotation = new NativeFunction(base.add(0x2988EF0), 'void', ['pointer', 'pointer', 'pointer'], 'win64');
var AActor_SetActorRotation = new NativeFunction(base.add(0x22E1260), 'void', ['pointer', 'pointer', 'int32'], 'win64');
var UWorld_SpawnActor = new NativeFunction(base.add(0x26254A0), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer'], 'win64');
var FActorSpawnParameters_ctor = new NativeFunction(base.add(0x29584A0), 'void', ['pointer'], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], 'win64');
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');

var GWorld = base.add(0x46ED420);
var ADH_GameState_HandleMatchHasStarted_addr = base.add(0xD99080);
var ADH_GameState_Tick_addr = base.add(0xDAD420);
var ADH_Warship_Tick_OverpressureCheck_addr = base.add(0xE88272);

// Expected instruction sequence in ADH_Warship::Tick at 0xE88272:
//   0F 2F 05 4B DD 5B 02  -> comiss xmm0, [0x143445FC4] (0.7001f overpressure threshold)
//   76 12                 -> jbe +0x12 (jump to pressure-decrease branch at 0xE8828D)
// Patching byte +7 (0xE88279) from 0x76 (jbe) to 0xEB (jmp) makes the overpressure-rise
// and overpressure-explosion code paths physically unreachable at the CPU level.
var EXPECTED_OVERPRESSURE_BYTES = [0x0F, 0x2F, 0x05, 0x4B, 0xDD, 0x5B, 0x02, 0x76, 0x12];

// Verified UE4 struct offsets (DreadHungerServer-Win64-Shipping.exe)
var OFF_UOBJECT_NAME_PRIVATE = 0x18;
var OFF_ACTOR_ROLE = 0xF0;
var OFF_ACTOR_ROOT_COMPONENT = 0x130;
var OFF_SCENE_COMPONENT_WORLD_LOC_X = 0x1D0; // ComponentToWorld.Translation.X
var OFF_GAMESTATE_WARSHIP = 0x2B0;
var OFF_GAMESTATE_SHIP_ARRIVED = 0x3D0;
var OFF_GAMESTATE_WINNING_TEAM = 0x51C;
var OFF_WARSHIP_BOILER = 0x2E0;
var OFF_BOILER_PRESSURE_LEVEL = 0x2B4;
var ROLE_AUTHORITY = 3;

var SAILING_START_DELAY_MS = 1000;
var SAILING_TICK_INTERVAL_MS = 100;

var NITRO_EXPLOSION_CLASS = '/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C';
var NITRO_LOCATION = [20966.8984375, -12676.6982421875, 213.27685546875];
var NITRO_ROTATION = [1.0, 1.0, 1.0];

// Immutable Departure route waypoints (X threshold -> incremental Yaw offset in degrees)
var DEPARTURE_WAYPOINTS = [
    { x: -50854.26171875, yawOffset: 5 },
    { x: -43095.00390625, yawOffset: 15 },
    { x: -36204.8671875, yawOffset: -15 },
    { x: -29616.12109375, yawOffset: 25 },
    { x: -24594.099609375, yawOffset: -10 },
    { x: -21000.013671875, yawOffset: 30 },
    { x: -8551.7890625, yawOffset: -10 },
    { x: -5941.32177734375, yawOffset: -10 },
    { x: -2222.822265625, yawOffset: -20 },
    { x: 1808.0069580078125, yawOffset: -20 },
    { x: 6416.56787109375, yawOffset: -15 },
    { x: 9336.3857421875, yawOffset: -22 },
    { x: 15526.5, yawOffset: 0 }
];

// Reusable buffers (avoid per-tick allocations and fix 16-byte FFrame overflow in original script;
// AActor::execK2_GetActorRotation accesses FFrame.Code at [rdx + 0x20]).
var gStackFrame = Memory.alloc(0x40);
var gRotResult = Memory.alloc(16);
var gNewRotation = Memory.alloc(16);

// Per-match sailing state (reset on every ADH_GameState::HandleMatchHasStarted)
var activeWorldKey = null;
var activeGameStateKey = null;
var nextWaypointIndex = 0;
var nitroSpawned = false;
var startAtMs = 0;
var lastTickMs = 0;

function getUObjectName(uObjectPtr) {
    if (uObjectPtr.isNull()) {
        return '';
    }
    var capacity = 128;
    var fStringPtr = Memory.alloc(16 + capacity * 2);
    fStringPtr.writePointer(fStringPtr.add(16));
    fStringPtr.add(8).writeU32(0);
    fStringPtr.add(12).writeU32(capacity);
    FName_GetPlainNameString(uObjectPtr.add(OFF_UOBJECT_NAME_PRIVATE), fStringPtr);
    var dataPtr = fStringPtr.readPointer();
    if (dataPtr.isNull()) {
        return '';
    }
    var text = dataPtr.readUtf16String();
    return text ? text : '';
}

function isDepartureWorld(worldPtr) {
    var worldName = getUObjectName(worldPtr);
    return worldName.toLowerCase().indexOf('departure') !== -1;
}

function findObjectByName(objectName, clazz) {
    var buffer = Memory.alloc((objectName.length + 1) * 2);
    buffer.writeUtf16String(objectName);
    return StaticFindObject(clazz, ptr('0xFFFFFFFFFFFFFFFF'), buffer, 0);
}

function loadObjectByName(objectName, clazz) {
    var buffer = Memory.alloc((objectName.length + 1) * 2);
    buffer.writeUtf16String(objectName);
    return StaticLoadObject(clazz, ptr(0), buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}

function getClassByName(className) {
    var uClassPtr = UClass_GetPrivateStaticClass();
    if (uClassPtr.isNull()) {
        return ptr(0);
    }
    var found = findObjectByName(className, uClassPtr);
    if (!found.isNull()) {
        return found;
    }
    return loadObjectByName(className, uClassPtr);
}

function getActorLocationX(actorPtr) {
    var rootComp = actorPtr.add(OFF_ACTOR_ROOT_COMPONENT).readPointer();
    if (rootComp.isNull()) {
        return NaN;
    }
    return rootComp.add(OFF_SCENE_COMPONENT_WORLD_LOC_X).readFloat();
}

function applyYawOffset(warshipPtr, offsetYaw) {
    if (offsetYaw === 0) {
        return;
    }
    gStackFrame.add(0x20).writePointer(ptr(0));
    AActor_execK2_GetActorRotation(warshipPtr, gStackFrame, gRotResult);
    var pitch = gRotResult.readFloat();
    var yaw = gRotResult.add(4).readFloat();
    var roll = gRotResult.add(8).readFloat();
    if (!Number.isFinite(pitch) || !Number.isFinite(yaw) || !Number.isFinite(roll)) {
        return;
    }

    gNewRotation.writeFloat(pitch);
    gNewRotation.add(4).writeFloat(yaw + offsetYaw);
    gNewRotation.add(8).writeFloat(roll);
    AActor_SetActorRotation(warshipPtr, gNewRotation, 0);
}

function spawnExitNitro(worldPtr) {
    var nitroClass = getClassByName(NITRO_EXPLOSION_CLASS);
    if (nitroClass.isNull()) {
        return;
    }
    var location = Memory.alloc(16);
    location.writeFloat(NITRO_LOCATION[0]);
    location.add(4).writeFloat(NITRO_LOCATION[1]);
    location.add(8).writeFloat(NITRO_LOCATION[2]);

    var rotation = Memory.alloc(16);
    rotation.writeFloat(NITRO_ROTATION[0]);
    rotation.add(4).writeFloat(NITRO_ROTATION[1]);
    rotation.add(8).writeFloat(NITRO_ROTATION[2]);

    var spawnParams = Memory.alloc(0x30);
    FActorSpawnParameters_ctor(spawnParams);
    spawnParams.add(0x10).writePointer(ptr(0));
    spawnParams.add(0x28).writeU8(1); // ESpawnActorCollisionHandlingMethod::AlwaysSpawn

    UWorld_SpawnActor(worldPtr, nitroClass, location, rotation, spawnParams);
}

function tickSightseeingSailing(gameStatePtr, worldPtr) {
    var warship = gameStatePtr.add(OFF_GAMESTATE_WARSHIP).readPointer();
    if (warship.isNull()) {
        return;
    }
    var boiler = warship.add(OFF_WARSHIP_BOILER).readPointer();
    if (boiler.isNull()) {
        return;
    }
    var shipX = getActorLocationX(warship);
    if (!Number.isFinite(shipX)) {
        return;
    }

    boiler.add(OFF_BOILER_PRESSURE_LEVEL).writeFloat(0.0);
    ADH_Warship_FuelBoiler(warship, 1.0);
    ADH_Warship_SetEnableAutoMove(warship, 1);

    if (nextWaypointIndex >= DEPARTURE_WAYPOINTS.length) {
        if (!nitroSpawned) {
            nitroSpawned = true;
            spawnExitNitro(worldPtr);
        }
        return;
    }

    var waypoint = DEPARTURE_WAYPOINTS[nextWaypointIndex];
    if (shipX >= waypoint.x) {
        applyYawOffset(warship, waypoint.yawOffset);
        nextWaypointIndex += 1;
    }
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function (args) {
        try {
            var gameState = args[0];
            var world = GWorld.readPointer();
            if (gameState.isNull() || world.isNull() || !isDepartureWorld(world)) {
                activeWorldKey = null;
                activeGameStateKey = null;
                return;
            }
            activeWorldKey = world.toString();
            activeGameStateKey = gameState.toString();
            nextWaypointIndex = 0;
            nitroSpawned = false;
            startAtMs = Date.now() + SAILING_START_DELAY_MS;
            lastTickMs = 0;
        } catch (e) {}
    }
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            if (activeWorldKey === null) {
                return;
            }
            var now = Date.now();
            if (now < startAtMs || now - lastTickMs < SAILING_TICK_INTERVAL_MS) {
                return;
            }
            var gameState = args[0];
            if (gameState.isNull() || gameState.toString() !== activeGameStateKey) {
                return;
            }
            if (gameState.add(OFF_ACTOR_ROLE).readU8() !== ROLE_AUTHORITY) {
                return;
            }
            var world = GWorld.readPointer();
            if (world.isNull() || world.toString() !== activeWorldKey) {
                activeWorldKey = null;
                activeGameStateKey = null;
                return;
            }
            if (
                gameState.add(OFF_GAMESTATE_SHIP_ARRIVED).readU8() !== 0 ||
                gameState.add(OFF_GAMESTATE_WINNING_TEAM).readU8() !== 0
            ) {
                activeWorldKey = null;
                activeGameStateKey = null;
                return;
            }
            lastTickMs = now;
            tickSightseeingSailing(gameState, world);
        } catch (e) {}
    }
});

function matchesExpectedBytes(addr, expected) {
    var raw = addr.readByteArray(expected.length);
    if (!raw) {
        return false;
    }
    var view = new Uint8Array(raw);
    for (var i = 0; i < expected.length; i++) {
        if (view[i] !== expected[i]) {
            return false;
        }
    }
    return true;
}

try {
    if (matchesExpectedBytes(ADH_Warship_Tick_OverpressureCheck_addr, EXPECTED_OVERPRESSURE_BYTES)) {
        Memory.patchCode(ADH_Warship_Tick_OverpressureCheck_addr.add(7), 1, function (code) {
            code.writeU8(0xEB);
        });
    }
} catch (e) {}
