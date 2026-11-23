// 観光モード(広漠 / Expanse_Persistent)
// ボイラー自動給炭・過圧防止・障害物回避ルート(第1島南 -> 中央水路 -> 東海峡)の自動操舵・出口氷壁ニトロ爆破を行う。
// 注意: マップ補強_広漠.js とは併用不可 (追加氷壁が航路を塞ぐため)。

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

// ADH_Warship::Tick (0xE88272) の過圧チェック期待命令列: comiss + jbe (0x76)
// +7バイト目 (0xE88279) を 0x76 (jbe) -> 0xEB (jmp) に書き換え、過圧上昇・爆発経路を無効化
var EXPECTED_OVERPRESSURE_BYTES = [0x0F, 0x2F, 0x05, 0x4B, 0xDD, 0x5B, 0x02, 0x76, 0x12];

// 検証済みUE4構造体オフセット
var OFF_UOBJECT_NAME_PRIVATE = 0x18;
var OFF_ACTOR_ROLE = 0xF0;
var OFF_ACTOR_ROOT_COMPONENT = 0x130;
var OFF_SCENE_COMPONENT_WORLD_LOC_X = 0x1D0; // ワールド座標X (ComponentToWorld.Translation.X)
var OFF_SCENE_COMPONENT_WORLD_LOC_Y = 0x1D4; // ワールド座標Y (ComponentToWorld.Translation.Y)
var OFF_WORLD_TIME_SECONDS = 0x598; // UWorld::TimeSeconds (slomo に追従)
var OFF_GAMESTATE_WARSHIP = 0x2B0;
var OFF_GAMESTATE_SHIP_ARRIVED = 0x3D0;
var OFF_GAMESTATE_WINNING_TEAM = 0x51C;
var OFF_WARSHIP_BOILER = 0x2E0;
var OFF_BOILER_PRESSURE_LEVEL = 0x2B4;
var ROLE_AUTHORITY = 3;

// slomo 倍率に依存せず同一軌道で操舵するためのゲーム内時間間隔 (秒)
var SAILING_START_DELAY_SEC = 1.0;
var SAILING_TICK_INTERVAL_SEC = 0.25;
var WAYPOINT_CAPTURE_RADIUS = 1600.0;
var WAYPOINT_PASS_MAX_DIST = 3200.0;
var YAW_DEADBAND_DEG = 2.5;
var YAW_PROPORTIONAL_GAIN = 0.72;
var YAW_MAX_STEP_DEG = 5.5;
var RAD_TO_DEG = 180.0 / Math.PI;

var NITRO_EXPLOSION_CLASS = '/Game/Blueprints/Game/BP_Explosion_Nitro.BP_Explosion_Nitro_C';
// 出口氷河の中心座標 (BP_DestructibleIceWall_2 @ 42308.1, 14646.2)
var EXIT_GLACIER_X = 42308.07421875;
var EXIT_GLACIER_Y = 14646.22265625;
var NITRO_TRIGGER_DISTANCE = 11000.0; // 船員への爆風被弾回避と氷塊沈下完了のため遠距離で早期起爆
var NITRO_SECOND_DELAY_SEC = 0.5;
// ニトロ1発目: 出口手前の氷壁2枚 (BP_IceWall34 / BP_IceWall39) を破壊
var NITRO_GATE_LOCATION = [41874.0, 12258.0, 150.0];
// ニトロ2発目: 出口氷河 (BP_DestructibleIceWall_2) 正面を破壊
var NITRO_GLACIER_LOCATION = [41739.0, 13429.0, 200.0];
var NITRO_ROTATION = [1.0, 1.0, 1.0];

// 広漠マップ検証済み自動航行ウェイポイント (全ブロックスプライン・難破船・氷壁回避済み)
// 出発 (-40357, 2115) -> 第1島南 -> 中央水路 -> 東海峡(難破船北側) -> 出口 -> 到達領域 (52437, 30132)
var EXPANSE_WAYPOINTS = [
    { x: -34000, y: 2350 },
    { x: -24000, y: 3100 },
    { x: -15100, y: 3500 },
    { x: -12400, y: 1800 },
    { x: -6400, y: -800 },
    { x: -4200, y: -2600 },
    { x: -2300, y: -4400 },
    { x: 200, y: -5300 },
    { x: 2300, y: -3800 },
    { x: 2000, y: -200 },
    { x: 4400, y: 2800 },
    { x: 5600, y: 1700 },
    { x: 8500, y: 1000 },
    { x: 11500, y: 2100 },
    { x: 14400, y: 2800 },
    { x: 18600, y: 2150 },
    { x: 22000, y: 1650 },
    { x: 24800, y: 2100 },
    { x: 26800, y: 3900 },
    { x: 29200, y: 4500 },
    { x: 31700, y: 4300 },
    { x: 35400, y: 3600 },
    { x: 37900, y: 5600 },
    { x: 40850, y: 9800 },
    { x: 41700, y: 12258 },
    { x: 42308, y: 14646 },
    { x: 45500, y: 20000 },
    { x: 52437, y: 30132 }
];

// 再利用バッファ (毎Tick確保を回避。execK2_GetActorRotation が FFrame.Code [+0x20] を参照)
var gStackFrame = Memory.alloc(0x40);
var gRotResult = Memory.alloc(16);
var gNewRotation = Memory.alloc(16);

// 試合ごとの自動航行状態 (ADH_GameState::HandleMatchHasStarted ごとにリセット)
var activeWorldKey = null;
var activeGameStateKey = null;
var nextWaypointIndex = 0;
var nitroStage = 0; // 0=待機, 1=ゲート用ニトロ起爆済み, 2=氷河用ニトロ起爆済み
var secondNitroAtGameSec = 0.0;
var startAtGameSec = 0.0;
var lastTickGameSec = -1.0;

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

function isExpanseWorld(worldPtr) {
    var worldName = getUObjectName(worldPtr);
    return worldName.toLowerCase().indexOf('expanse') !== -1;
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

function normalizeAngleDeg(deg) {
    var a = deg % 360.0;
    if (a > 180.0) {
        a -= 360.0;
    } else if (a < -180.0) {
        a += 360.0;
    }
    return a;
}

function getActorLocationXY(actorPtr) {
    var rootComp = actorPtr.add(OFF_ACTOR_ROOT_COMPONENT).readPointer();
    if (rootComp.isNull()) {
        return null;
    }
    var x = rootComp.add(OFF_SCENE_COMPONENT_WORLD_LOC_X).readFloat();
    var y = rootComp.add(OFF_SCENE_COMPONENT_WORLD_LOC_Y).readFloat();
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
        return null;
    }
    return { x: x, y: y };
}

function steerWarshipToward(warshipPtr, targetX, targetY, shipX, shipY) {
    var dx = targetX - shipX;
    var dy = targetY - shipY;
    if (dx * dx + dy * dy < 1.0) {
        return;
    }
    gStackFrame.add(0x20).writePointer(ptr(0));
    AActor_execK2_GetActorRotation(warshipPtr, gStackFrame, gRotResult);
    var pitch = gRotResult.readFloat();
    var currentYaw = gRotResult.add(4).readFloat();
    var roll = gRotResult.add(8).readFloat();
    if (!Number.isFinite(currentYaw)) {
        return;
    }

    var desiredYaw = Math.atan2(dy, dx) * RAD_TO_DEG;
    var yawError = normalizeAngleDeg(desiredYaw - currentYaw);
    if (Math.abs(yawError) <= YAW_DEADBAND_DEG) {
        return;
    }

    var stepYaw = yawError * YAW_PROPORTIONAL_GAIN;
    if (stepYaw > YAW_MAX_STEP_DEG) {
        stepYaw = YAW_MAX_STEP_DEG;
    } else if (stepYaw < -YAW_MAX_STEP_DEG) {
        stepYaw = -YAW_MAX_STEP_DEG;
    }

    gNewRotation.writeFloat(pitch);
    gNewRotation.add(4).writeFloat(normalizeAngleDeg(currentYaw + stepYaw));
    gNewRotation.add(8).writeFloat(roll);
    AActor_SetActorRotation(warshipPtr, gNewRotation, 0);
}

function spawnNitroAt(worldPtr, coords) {
    var nitroClass = getClassByName(NITRO_EXPLOSION_CLASS);
    if (nitroClass.isNull()) {
        return;
    }
    var location = Memory.alloc(16);
    location.writeFloat(coords[0]);
    location.add(4).writeFloat(coords[1]);
    location.add(8).writeFloat(coords[2]);

    var rotation = Memory.alloc(16);
    rotation.writeFloat(NITRO_ROTATION[0]);
    rotation.add(4).writeFloat(NITRO_ROTATION[1]);
    rotation.add(8).writeFloat(NITRO_ROTATION[2]);

    var spawnParams = Memory.alloc(0x30);
    FActorSpawnParameters_ctor(spawnParams);
    spawnParams.add(0x10).writePointer(ptr(0));
    spawnParams.add(0x28).writeU8(1); // 衝突を無視して常にスポーン (AlwaysSpawn)

    UWorld_SpawnActor(worldPtr, nitroClass, location, rotation, spawnParams);
}

function advanceWaypoints(shipX, shipY) {
    while (nextWaypointIndex < EXPANSE_WAYPOINTS.length - 1) {
        var wp = EXPANSE_WAYPOINTS[nextWaypointIndex];
        var dx = wp.x - shipX;
        var dy = wp.y - shipY;
        var dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < WAYPOINT_CAPTURE_RADIUS) {
            nextWaypointIndex += 1;
            continue;
        }
        // 前方通過判定: 直前区間からウェイポイントを通過済みなら次へ進める
        if (nextWaypointIndex > 0 && dist < WAYPOINT_PASS_MAX_DIST) {
            var prevWp = EXPANSE_WAYPOINTS[nextWaypointIndex - 1];
            var inX = wp.x - prevWp.x;
            var inY = wp.y - prevWp.y;
            var pastDot = (shipX - wp.x) * inX + (shipY - wp.y) * inY;
            if (pastDot > 0) {
                nextWaypointIndex += 1;
                continue;
            }
        }
        break;
    }
}

function getWorldTimeSeconds(worldPtr) {
    var t = worldPtr.add(OFF_WORLD_TIME_SECONDS).readFloat();
    if (Number.isFinite(t)) {
        return t;
    }
    return Date.now() / 1000.0;
}

function tickSightseeingSailing(gameStatePtr, worldPtr, gameSec) {
    var warship = gameStatePtr.add(OFF_GAMESTATE_WARSHIP).readPointer();
    if (warship.isNull()) {
        return;
    }
    var boiler = warship.add(OFF_WARSHIP_BOILER).readPointer();
    if (boiler.isNull()) {
        return;
    }
    var shipLoc = getActorLocationXY(warship);
    if (!shipLoc) {
        return;
    }

    boiler.add(OFF_BOILER_PRESSURE_LEVEL).writeFloat(0.0);
    ADH_Warship_FuelBoiler(warship, 1.0);
    ADH_Warship_SetEnableAutoMove(warship, 1);

    // 出口氷壁と破壊可能氷河を船の前方で安全に爆破除去
    if (nitroStage === 0) {
        var gdx = EXIT_GLACIER_X - shipLoc.x;
        var gdy = EXIT_GLACIER_Y - shipLoc.y;
        if (Math.sqrt(gdx * gdx + gdy * gdy) <= NITRO_TRIGGER_DISTANCE) {
            nitroStage = 1;
            secondNitroAtGameSec = gameSec + NITRO_SECOND_DELAY_SEC;
            spawnNitroAt(worldPtr, NITRO_GATE_LOCATION);
        }
    } else if (nitroStage === 1 && gameSec >= secondNitroAtGameSec) {
        nitroStage = 2;
        spawnNitroAt(worldPtr, NITRO_GLACIER_LOCATION);
    }

    advanceWaypoints(shipLoc.x, shipLoc.y);
    var targetWp = EXPANSE_WAYPOINTS[nextWaypointIndex];
    steerWarshipToward(warship, targetWp.x, targetWp.y, shipLoc.x, shipLoc.y);
}

Interceptor.attach(ADH_GameState_HandleMatchHasStarted_addr, {
    onEnter: function (args) {
        try {
            var gameState = args[0];
            var world = GWorld.readPointer();
            if (gameState.isNull() || world.isNull() || !isExpanseWorld(world)) {
                activeWorldKey = null;
                activeGameStateKey = null;
                return;
            }
            var nowGameSec = getWorldTimeSeconds(world);
            activeWorldKey = world.toString();
            activeGameStateKey = gameState.toString();
            nextWaypointIndex = 0;
            nitroStage = 0;
            secondNitroAtGameSec = 0.0;
            startAtGameSec = nowGameSec + SAILING_START_DELAY_SEC;
            lastTickGameSec = -1.0;
        } catch (e) {}
    }
});

Interceptor.attach(ADH_GameState_Tick_addr, {
    onEnter: function (args) {
        try {
            if (activeWorldKey === null) {
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
            var gameSec = getWorldTimeSeconds(world);
            if (gameSec < startAtGameSec) {
                return;
            }
            if (lastTickGameSec >= 0.0 && (gameSec - lastTickGameSec) < SAILING_TICK_INTERVAL_SEC) {
                return;
            }
            lastTickGameSec = gameSec;
            tickSightseeingSailing(gameState, world, gameSec);
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
