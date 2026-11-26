// 食人鬼登攀補強（デフォルト自動注入 MOD）
// 役割:
//   - 食人鬼 (ADH_CannibalCharacter) の急斜面・垂直氷壁・岸壁でのスタックを解消する 2 段階登攀補強
//   - 基礎物理: WalkableFloorAngle=78° (WalkableFloorZ≈0.2079), MaxStepHeight>=75 (原版 128 UU 維持), 水辺這い上がり + bCanFly
//   - スマート検知: 単なる実移動距離だけでなく「目標との接近進捗・高度差(dz>=95 UU)・崖下ウロウロ・部分経路停止」を 0.7s 毎に評価
//   - 2 段階垂直登攀 (MOVE_Flying=5) + < 78° 傾角復帰:
//       Phase 1 (CLIMB_UP): 直立姿勢 (Pitch=0, Roll=0) で目標方向へ Yaw のみ向け、カプセルスイープ衝突を保ったまま壁・氷壁に沿って垂直上昇（オーバーハング時は一時後退して回避）
//       < 78° 傾角復帰:     K2_FindFloor (0x241D5A0) + 前方レイで地面傾角 < 78° をヒステリシス検知し登攀解除して通常歩行へ早期復帰
//       Phase 2 (CREST):    崖天面へ水平に踏み込み、MOVE_Falling 経由で即座に MOVE_Walking へ接地復帰
//   - 2B 軍艦・舷梯への登船完全禁止（3 層ガード）:
//       Layer 1: 船体リアルタイム OBB + ShipBase(+0x9F8) / BasedMovement(+0x298) による船上目標・船外からの登攀/跳躍遮断
//       Layer 2: 前方短距離レイキャスト (LineTraceSingleByChannel 0x2400340) による船殻・子アクタ・舷梯 (SM_BoatGangwayRails等) の精判
//                （狭い海峡で船の 3m 横にある氷壁は誤爆せず正常登攀）
//       Layer 3: UCharacterMovementComponent::IsWalkable (0x241D1E0, Interceptor.attach) による受動物理遮断（78°斜面/段差/水辺/舷梯からの乗船根絶）
//   - 原版 BP_CannibalController_C / ThreatTable / TargetCharacter は 100% 互換維持（登攀中のみ bChaseTargetCharacter を一時停止し終了時に無条件復元）
//   - 普通野生動物（狼・熊・野生ネズミ・兎・アザラシ等）には一切干渉しない（ADH_CannibalCharacter 厳格限定）
//   - フック安全設計: 0x241D1E0 のみ onLeave 1 件（カスタムスペル.js と合算 <= 2）、他はすべて onEnter または replaceFast（Frida 16/17 完全互換・GameThread 専用）

(function () {
    'use strict';

    var MODULE_NAME = 'DreadHungerServer-Win64-Shipping.exe';
    var moduleBase = Module.findBaseAddress(MODULE_NAME);
    if (!moduleBase || moduleBase.isNull()) {
        return;
    }

    var WALKABLE_SLOPE_COS_78 = Math.cos(78.0 * Math.PI / 180.0); // ≈ 0.2079117
    var WALKABLE_RECOVER_COS_75 = Math.cos(75.0 * Math.PI / 180.0); // ≈ 0.2588190（78°境界振動防止のヒステリシス）

    var CONFIG = {
        walkableFloorAngleDeg: 78.0,
        minMaxStepHeightUU: 75.0,
        meleeExemptDistUU: 220.0,
        climbExitDistUU: 180.0,
        stuckCheckIntervalSec: 0.70,
        stuckMinProgressUU: 70.0,
        elevatedDzUU: 95.0,
        elevatedMaxXyUU: 950.0,
        closeWallXyUU: 450.0,
        minCloseProgressUU: 90.0,
        minZProgressUU: 45.0,
        vaultVelZNormal: 520.0,
        vaultVelZUphill: 560.0,
        vaultUphillDeltaZUU: 40.0,
        vaultVelXY: 420.0,
        vaultCooldownSec: 1.2,
        climbFlySpeed: 900.0,
        climbVelZ: 420.0,
        climbVelXY: 210.0,
        climbBackoffVelXY: 220.0,
        climbBackoffVelZ: 280.0,
        climbBackoffSec: 0.26,
        crestVelXY: 460.0,
        crestVelZ: -40.0,
        crestDurationSec: 0.42,
        climbTopMarginUU: 35.0,
        climbMinClearanceUU: 150.0,
        climbMaxDurationSec: 4.5,
        climbMaxZGainUU: 1400.0,
        climbCooldownSec: 0.85,
        climbStuckGraceSec: 1.5,

        // < 78° 傾角復帰判定
        slopeRecoverGraceSec: 0.30,
        slopeRecoverMinZGainUU: 35.0,
        slopeCheckIntervalSec: 0.12,
        slopeRecoverMaxFloorDistUU: 45.0,
        slopeRecoverHitsNeeded: 2,

        // 2B: 軍艦・舷梯防爬ガード（Departure_Persistent 実測 ShipMesh/PhysicsMesh/ActorBounds 基準）
        shipBroadphaseRadiusUU: 3600.0,
        shipObbMinX: -2580.0,
        shipObbMaxX: 2060.0,
        shipObbMinY: -530.0,
        shipObbMaxY: 530.0,
        shipObbMinZ: -350.0,
        shipObbMaxZ: 3700.0,
        shipForwardTraceDistUU: 260.0,
        shipMidClimbTraceIntervalSec: 0.15
    };

    // DreadHungerServer-Win64-Shipping.pdb 検証済み RVA
    var RVA = {
        GUObjectArray: 0x45C40C8,
        GWorld: 0x46ED420,
        FMemory_Free: 0x105DD50,
        FName_GetPlainNameString: 0x11604F0,
        FCollisionResponseParams_DefaultResponseParam: 0x46C6EA8,

        ADH_CannibalCharacter_GetPrivateStaticClass: 0xE9F130,
        ADH_HumanCharacter_GetPrivateStaticClass: 0xEC3DA0,
        ADH_AIController_GetPrivateStaticClass: 0xE98330,
        ADH_Warship_GetPrivateStaticClass: 0xF05BF0,

        UDH_GameplayStatics_GetWarship: 0xE91C00,
        ADH_AIController_OnPossess: 0xD615A0,
        ADH_AIControllerMonster_CharacterStuck: 0xD4CA50,
        ADH_AnimalCharacter_BeginPlay: 0xD48B30,
        ADH_AnimalCharacter_Destroyed: 0xD4FDA0,
        ADH_Character_OnDied: 0xEA11C0,
        ADH_Character_IsAlive: 0xD5C850,
        ADH_GameState_Tick: 0xDAD420,
        AAIController_StopMovement: 0x2D7A130,
        AAIController_GetMoveStatus: 0x2D6BFB0,
        AAIController_HasPartialPath: 0x2D6D670,
        AActor_K2_SetActorRotation: 0x22E1260,
        AActor_GetWorldTimerManager: 0x22DDC30,
        FTimerManager_FindTimer: 0x28F2D80,
        FTimerManager_InternalClearTimer: 0x28F6A00,
        FCollisionQueryParams_Ctor: 0x23C7DE0,
        FCollisionQueryParams_AddIgnoredActor: 0x23CA2D0,
        UWorld_LineTraceSingleByChannel: 0x2400340,
        UCharacterMovementComponent_IsWalkable: 0x241D1E0,
        UCharacterMovementComponent_K2_FindFloor: 0x241D5A0,
        UCharacterMovementComponent_SetWalkableFloorAngle: 0x242E4E0,
        UCharacterMovementComponent_SetMovementMode: 0x242D7D0
    };

    // WinDump / バイナリ逆アセンブル検証済みオフセット
    var OFFSETS = {
        UObject_InternalIndex: 0x0C,
        UObject_ClassPrivate: 0x10,
        UObject_NamePrivate: 0x18,
        UStruct_ChainArray: 0x30,
        UStruct_ChainDepth: 0x38,

        UActorComponent_OwnerPrivate: 0xA0,
        USceneComponent_AttachParent: 0xC0,
        USceneComponent_ComponentToWorldQuat: 0x1C0,
        USceneComponent_ComponentToWorldTrans: 0x1D0,
        USceneComponent_ComponentToWorldScale: 0x1E0,
        UStaticMeshComponent_StaticMesh: 0x488,

        AActor_RootComponent: 0x130,
        UWorld_Levels: 0x138,
        ULevel_Actors: 0x98,
        UWorld_TimeSeconds: 0x598,
        ADH_GameState_Warship: 0x2B0,

        APawn_Controller: 0x258,
        ACharacter_CharacterMovement: 0x288,
        ACharacter_BasedMovement: 0x298,
        ADH_Character_CurrentLadder: 0x640,
        ADH_Character_Snare: 0x7A0,
        ADH_HumanCharacter_ShipBase: 0x9F8,
        ADH_AnimalCharacter_bCallingForHelp: 0x9E0,
        ADH_AnimalCharacter_StuckTimer: 0xA4C,

        ADH_AIController_TargetCharacter: 0x378,
        ADH_AIController_ControlledCharacter: 0x380,
        ADH_AIController_bChaseTargetCharacter: 0x3AD,
        ADH_AIController_StuckTimerHandle: 0x3C0,

        UMovementComponent_Velocity: 0xC4,
        UNavMovementComponent_NavAgentProps_Flags: 0xF0,
        UNavMovementComponent_Flags: 0x124,
        UNavMovementComponent_MovementState: 0x125,
        UCharacterMovementComponent_MaxStepHeight: 0x154,
        UCharacterMovementComponent_WalkableFloorAngle: 0x160,
        UCharacterMovementComponent_WalkableFloorZ: 0x164,
        UCharacterMovementComponent_MovementMode: 0x168,
        UCharacterMovementComponent_MaxFlySpeed: 0x198,
        UCharacterMovementComponent_BrakingDecelerationFlying: 0x1C0,
        UDH_CharacterMovementComponent_AutoTreadDepthSpeed: 0xB30,
        UDH_CharacterMovementComponent_bCanMoveUnderWater: 0xB34,
        UDH_CharacterMovementComponent_bAutoStepUpOutOfWater: 0xB35,
        UDH_CharacterMovementComponent_bAlwaysAutoStepOutOfWater: 0xB36,
        UDH_CharacterMovementComponent_MaxSwimDepth: 0xB38,

        FHitResult_Distance: 0x0C,
        FHitResult_ImpactNormal: 0x34,
        FHitResult_Actor: 0x6C,
        FHitResult_Component: 0x74,

        FFindFloorResult_FloorDist: 0x04,
        FFindFloorResult_HitNormalZ: 0x48,
        FFindFloorResult_HitActor: 0x78,
        FFindFloorResult_HitComponent: 0x80
    };

    var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
    var GWorld_ptr = moduleBase.add(RVA.GWorld);
    var DefaultResponseParam_ptr = moduleBase.add(RVA.FCollisionResponseParams_DefaultResponseParam);

    var FMemory_Free = new NativeFunction(moduleBase.add(RVA.FMemory_Free), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FName_GetPlainNameString = new NativeFunction(moduleBase.add(RVA.FName_GetPlainNameString), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });

    var ADH_CannibalCharacter_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_CannibalCharacter_GetPrivateStaticClass),
        'pointer', [], { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_HumanCharacter_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_HumanCharacter_GetPrivateStaticClass),
        'pointer', [], { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_AIController_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_AIController_GetPrivateStaticClass),
        'pointer', [], { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_Warship_GetPrivateStaticClass = new NativeFunction(
        moduleBase.add(RVA.ADH_Warship_GetPrivateStaticClass),
        'pointer', [], { abi: 'win64', exceptions: 'propagate' }
    );
    var UDH_GameplayStatics_GetWarship = new NativeFunction(
        moduleBase.add(RVA.UDH_GameplayStatics_GetWarship),
        'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var ADH_Character_IsAlive = new NativeFunction(
        moduleBase.add(RVA.ADH_Character_IsAlive),
        'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var AAIController_StopMovement = new NativeFunction(
        moduleBase.add(RVA.AAIController_StopMovement),
        'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var AAIController_GetMoveStatus = new NativeFunction(
        moduleBase.add(RVA.AAIController_GetMoveStatus),
        'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var AAIController_HasPartialPath = new NativeFunction(
        moduleBase.add(RVA.AAIController_HasPartialPath),
        'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var AActor_K2_SetActorRotation = new NativeFunction(
        moduleBase.add(RVA.AActor_K2_SetActorRotation),
        'uint8', ['pointer', 'pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' }
    );
    var AActor_GetWorldTimerManager = new NativeFunction(
        moduleBase.add(RVA.AActor_GetWorldTimerManager),
        'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var FTimerManager_FindTimer = new NativeFunction(
        moduleBase.add(RVA.FTimerManager_FindTimer),
        'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var FTimerManager_InternalClearTimer = new NativeFunction(
        moduleBase.add(RVA.FTimerManager_InternalClearTimer),
        'void', ['pointer', 'uint64'], { abi: 'win64', exceptions: 'propagate' }
    );
    var FCollisionQueryParams_Ctor = new NativeFunction(
        moduleBase.add(RVA.FCollisionQueryParams_Ctor),
        'pointer', ['pointer', 'uint64', 'pointer', 'uint8', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var FCollisionQueryParams_AddIgnoredActor = new NativeFunction(
        moduleBase.add(RVA.FCollisionQueryParams_AddIgnoredActor),
        'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var UWorld_LineTraceSingleByChannel = new NativeFunction(
        moduleBase.add(RVA.UWorld_LineTraceSingleByChannel),
        'uint8', ['pointer', 'pointer', 'pointer', 'pointer', 'uint32', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var UCharacterMovementComponent_K2_FindFloor = new NativeFunction(
        moduleBase.add(RVA.UCharacterMovementComponent_K2_FindFloor),
        'void', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
    );
    var UCharacterMovementComponent_SetWalkableFloorAngle = new NativeFunction(
        moduleBase.add(RVA.UCharacterMovementComponent_SetWalkableFloorAngle),
        'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' }
    );
    var UCharacterMovementComponent_SetMovementMode = new NativeFunction(
        moduleBase.add(RVA.UCharacterMovementComponent_SetMovementMode),
        'void', ['pointer', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' }
    );

    // GameThread 専用の再利用作業バッファ
    var rotatorBuf = Memory.alloc(16);
    var traceStartBuf = Memory.alloc(16);
    var traceEndBuf = Memory.alloc(16);
    var traceHitBuf = Memory.alloc(0xA0);
    var queryParamsBuf = Memory.alloc(0x80);
    var floorLocBuf = Memory.alloc(16);
    var floorResultBuf = Memory.alloc(0xA0);
    var nameFStrBuf = Memory.alloc(16);
    var zeroHitBytes = new Array(0xA0);
    for (var zi = 0; zi < 0xA0; zi++) { zeroHitBytes[zi] = 0; }

    var pinnedCallbacks = [];
    var origMonsterCharacterStuck = null;

    var cachedCannibalClass = ptr(0);
    var cachedHumanClass = ptr(0);
    var cachedDhAiCtrlClass = ptr(0);
    var cachedWarshipClass = ptr(0);
    var currentGameState = ptr(0);
    var trackedWorldKey = '';
    var trackedCannibals = {};
    var trackedMoveComps = {};
    var nextActorDiscoveryAt = 0.0;
    var gangwayCache = {};
    var gangwayCacheCount = 0;

    var shipCache = {
        warship: ptr(0),
        valid: false,
        qx: 0.0, qy: 0.0, qz: 0.0, qw: 1.0,
        tx: 0.0, ty: 0.0, tz: 0.0,
        sx: 1.0, sy: 1.0, sz: 1.0
    };

    function log(msg) {
        var line = '[食人鬼登攀補強] ' + msg;
        try {
            if (typeof send === 'function') {
                send(line);
                return;
            }
        } catch (e) {}
        try {
            console.log(line);
        } catch (e2) {}
    }

    function getCannibalClass() {
        if (cachedCannibalClass.isNull()) { cachedCannibalClass = ADH_CannibalCharacter_GetPrivateStaticClass(); }
        return cachedCannibalClass;
    }
    function getHumanClass() {
        if (cachedHumanClass.isNull()) { cachedHumanClass = ADH_HumanCharacter_GetPrivateStaticClass(); }
        return cachedHumanClass;
    }
    function getDhAiControllerClass() {
        if (cachedDhAiCtrlClass.isNull()) { cachedDhAiCtrlClass = ADH_AIController_GetPrivateStaticClass(); }
        return cachedDhAiCtrlClass;
    }
    function getWarshipClass() {
        if (cachedWarshipClass.isNull()) { cachedWarshipClass = ADH_Warship_GetPrivateStaticClass(); }
        return cachedWarshipClass;
    }

    function isPendingKill(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return true;
        }
        try {
            var internalIndex = objPtr.add(OFFSETS.UObject_InternalIndex).readS32();
            var numElements = GUObjectArray_ptr.add(0x24).readS32();
            if (internalIndex < 0 || internalIndex >= numElements) {
                return true;
            }
            var objectsPtr = GUObjectArray_ptr.add(0x10).readPointer();
            if (objectsPtr.isNull()) {
                return true;
            }
            var numChunks = GUObjectArray_ptr.add(0x2C).readS32();
            var chunkIndex = internalIndex >> 16;
            if (chunkIndex < 0 || chunkIndex >= numChunks) {
                return true;
            }
            var chunkPtr = objectsPtr.add(chunkIndex * 8).readPointer();
            if (chunkPtr.isNull()) {
                return true;
            }
            var itemPtr = chunkPtr.add((internalIndex & 0xFFFF) * 24);
            if (!itemPtr.readPointer().equals(objPtr)) {
                return true;
            }
            var flags = itemPtr.add(8).readU32();
            return ((flags >>> 29) & 1) !== 0;
        } catch (e) {
            return true;
        }
    }

    function resolveWeakObject(weakPtr) {
        if (!weakPtr || weakPtr.isNull()) {
            return ptr(0);
        }
        try {
            var idx = weakPtr.readS32();
            var ser = weakPtr.add(4).readS32();
            if (idx < 0 || ser < 1) {
                return ptr(0);
            }
            var numElements = GUObjectArray_ptr.add(0x24).readS32();
            if (idx >= numElements) {
                return ptr(0);
            }
            var objectsPtr = GUObjectArray_ptr.add(0x10).readPointer();
            if (objectsPtr.isNull()) {
                return ptr(0);
            }
            var numChunks = GUObjectArray_ptr.add(0x2C).readS32();
            var chunkIndex = idx >> 16;
            if (chunkIndex < 0 || chunkIndex >= numChunks) {
                return ptr(0);
            }
            var chunkPtr = objectsPtr.add(chunkIndex * 8).readPointer();
            if (chunkPtr.isNull()) {
                return ptr(0);
            }
            var itemPtr = chunkPtr.add((idx & 0xFFFF) * 24);
            var obj = itemPtr.readPointer();
            if (obj.isNull()) {
                return ptr(0);
            }
            var itemSer = itemPtr.add(16).readS32();
            if (itemSer !== ser) {
                return ptr(0);
            }
            var flags = itemPtr.add(8).readU32();
            if (((flags >>> 29) & 1) !== 0) {
                return ptr(0);
            }
            return obj;
        } catch (e) {
            return ptr(0);
        }
    }

    function isA(objPtr, targetClassPtr) {
        if (!objPtr || objPtr.isNull() || !targetClassPtr || targetClassPtr.isNull()) {
            return false;
        }
        try {
            var objClass = objPtr.add(OFFSETS.UObject_ClassPrivate).readPointer();
            if (objClass.isNull()) {
                return false;
            }
            var targetDepth = targetClassPtr.add(OFFSETS.UStruct_ChainDepth).readS32();
            var objDepth = objClass.add(OFFSETS.UStruct_ChainDepth).readS32();
            if (targetDepth < 0 || targetDepth > objDepth) {
                return false;
            }
            var chainArray = objClass.add(OFFSETS.UStruct_ChainArray).readPointer();
            if (chainArray.isNull()) {
                return false;
            }
            return chainArray.add(targetDepth * 8).readPointer().equals(targetClassPtr.add(OFFSETS.UStruct_ChainArray));
        } catch (e) {
            return false;
        }
    }

    function readObjectName(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return '';
        }
        nameFStrBuf.writePointer(ptr(0));
        nameFStrBuf.add(8).writeS32(0);
        nameFStrBuf.add(12).writeS32(0);
        try {
            FName_GetPlainNameString(objPtr.add(OFFSETS.UObject_NamePrivate), nameFStrBuf);
            var dp = nameFStrBuf.readPointer();
            var n = nameFStrBuf.add(8).readS32();
            if (dp.isNull() || n <= 1 || n > 1024) {
                return '';
            }
            return dp.readUtf16String(n - 1);
        } catch (e) {
            return '';
        } finally {
            var p = nameFStrBuf.readPointer();
            if (!p.isNull()) {
                try { FMemory_Free(p); } catch (e2) {}
            }
        }
    }

    function getWorldAndTime() {
        var world = GWorld_ptr.readPointer();
        if (world.isNull() || isPendingKill(world)) {
            return { world: ptr(0), timeSeconds: 0.0 };
        }
        var t = world.add(OFFSETS.UWorld_TimeSeconds).readFloat();
        return { world: world, timeSeconds: Number.isFinite(t) ? t : 0.0 };
    }

    function getActorLocation(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return null;
        }
        var rootComp = actorPtr.add(OFFSETS.AActor_RootComponent).readPointer();
        if (rootComp.isNull() || isPendingKill(rootComp)) {
            return null;
        }
        var x = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldTrans).readFloat();
        var y = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldTrans + 4).readFloat();
        var z = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldTrans + 8).readFloat();
        if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
            return null;
        }
        return { x: x, y: y, z: z };
    }

    function distance3D(a, b) {
        var dx = a.x - b.x;
        var dy = a.y - b.y;
        var dz = a.z - b.z;
        return Math.sqrt(dx * dx + dy * dy + dz * dz);
    }

    function setActorYawUpright(actorPtr, dx, dy) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return;
        }
        var horizSq = dx * dx + dy * dy;
        if (!Number.isFinite(horizSq) || horizSq <= 1.0) {
            return;
        }
        var yawDeg = Math.atan2(dy, dx) * (180.0 / Math.PI);
        if (!Number.isFinite(yawDeg)) {
            return;
        }
        try {
            rotatorBuf.writeFloat(0.0);
            rotatorBuf.add(4).writeFloat(yawDeg);
            rotatorBuf.add(8).writeFloat(0.0);
            AActor_K2_SetActorRotation(actorPtr, rotatorBuf, 0);
        } catch (e) {}
    }

    function clearActorTimer(actorPtr, handleOffset) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return;
        }
        try {
            var handlePtr = actorPtr.add(handleOffset);
            var handleVal = handlePtr.readU64();
            if (handleVal.equals(0)) {
                return;
            }
            var timerMgr = AActor_GetWorldTimerManager(actorPtr);
            if (!timerMgr.isNull()) {
                var found = FTimerManager_FindTimer(timerMgr, handlePtr);
                if (!found.isNull()) {
                    FTimerManager_InternalClearTimer(timerMgr, handleVal);
                }
            }
            handlePtr.writeU64(0);
        } catch (e) {}
    }

    // ========================================================================
    // 2B: 軍艦リアルタイム座標 / OBB / 舷梯・子アクタ識別 / レイキャスト
    // ========================================================================
    function updateWarshipCache(gsPtr) {
        shipCache.valid = false;
        shipCache.warship = ptr(0);
        if (!gsPtr || gsPtr.isNull() || isPendingKill(gsPtr)) {
            return;
        }
        try {
            var ship = gsPtr.add(OFFSETS.ADH_GameState_Warship).readPointer();
            if (ship.isNull() || isPendingKill(ship)) {
                ship = UDH_GameplayStatics_GetWarship(gsPtr);
            }
            if (!ship || ship.isNull() || isPendingKill(ship)) {
                return;
            }
            var root = ship.add(OFFSETS.AActor_RootComponent).readPointer();
            if (root.isNull() || isPendingKill(root)) {
                return;
            }
            var qx = root.add(OFFSETS.USceneComponent_ComponentToWorldQuat).readFloat();
            var qy = root.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 4).readFloat();
            var qz = root.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 8).readFloat();
            var qw = root.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 12).readFloat();
            var tx = root.add(OFFSETS.USceneComponent_ComponentToWorldTrans).readFloat();
            var ty = root.add(OFFSETS.USceneComponent_ComponentToWorldTrans + 4).readFloat();
            var tz = root.add(OFFSETS.USceneComponent_ComponentToWorldTrans + 8).readFloat();
            var sx = root.add(OFFSETS.USceneComponent_ComponentToWorldScale).readFloat();
            var sy = root.add(OFFSETS.USceneComponent_ComponentToWorldScale + 4).readFloat();
            var sz = root.add(OFFSETS.USceneComponent_ComponentToWorldScale + 8).readFloat();
            if (!Number.isFinite(tx) || !Number.isFinite(ty) || !Number.isFinite(tz) ||
                !Number.isFinite(qw) || Math.abs(sx) < 0.01 || Math.abs(sy) < 0.01 || Math.abs(sz) < 0.01) {
                return;
            }
            shipCache.warship = ship;
            shipCache.qx = qx; shipCache.qy = qy; shipCache.qz = qz; shipCache.qw = qw;
            shipCache.tx = tx; shipCache.ty = ty; shipCache.tz = tz;
            shipCache.sx = sx; shipCache.sy = sy; shipCache.sz = sz;
            shipCache.valid = true;
        } catch (e) {}
    }

    function isNearShipBroadphase(loc) {
        if (!shipCache.valid || !loc) {
            return false;
        }
        var dx = loc.x - shipCache.tx;
        var dy = loc.y - shipCache.ty;
        return Math.abs(dx) <= CONFIG.shipBroadphaseRadiusUU && Math.abs(dy) <= CONFIG.shipBroadphaseRadiusUU;
    }

    function worldToShipLocal(loc) {
        var dx = loc.x - shipCache.tx;
        var dy = loc.y - shipCache.ty;
        var dz = loc.z - shipCache.tz;
        var iqx = -shipCache.qx;
        var iqy = -shipCache.qy;
        var iqz = -shipCache.qz;
        var iqw = shipCache.qw;
        var rx = 2.0 * (iqy * dz - iqz * dy);
        var ry = 2.0 * (iqz * dx - iqx * dz);
        var rz = 2.0 * (iqx * dy - iqy * dx);
        return {
            x: (dx + iqw * rx + (iqy * rz - iqz * ry)) / shipCache.sx,
            y: (dy + iqw * ry + (iqz * rx - iqx * rz)) / shipCache.sy,
            z: (dz + iqw * rz + (iqx * ry - iqy * rx)) / shipCache.sz
        };
    }

    function isPointInShipOBB(loc, padXY, padZ) {
        if (!shipCache.valid || !loc || !isNearShipBroadphase(loc)) {
            return false;
        }
        var lp = worldToShipLocal(loc);
        return lp.x >= (CONFIG.shipObbMinX - padXY) && lp.x <= (CONFIG.shipObbMaxX + padXY) &&
               lp.y >= (CONFIG.shipObbMinY - padXY) && lp.y <= (CONFIG.shipObbMaxY + padXY) &&
               lp.z >= (CONFIG.shipObbMinZ - padZ) && lp.z <= (CONFIG.shipObbMaxZ + padZ);
    }

    function isGangwayObjectCached(objPtr, isComp) {
        if (!objPtr || objPtr.isNull()) {
            return false;
        }
        var key = (isComp ? 'c:' : 'a:') + objPtr.toString();
        var cached = gangwayCache[key];
        if (cached !== undefined) {
            return cached;
        }
        var result = false;
        try {
            var nm = readObjectName(objPtr);
            if (nm && nm.toLowerCase().indexOf('gangway') !== -1) {
                result = true;
            } else if (isComp) {
                var clsName = readObjectName(objPtr.add(OFFSETS.UObject_ClassPrivate).readPointer());
                if (clsName === 'StaticMeshComponent') {
                    var sm = objPtr.add(OFFSETS.UStaticMeshComponent_StaticMesh).readPointer();
                    if (!sm.isNull() && !isPendingKill(sm)) {
                        var smName = readObjectName(sm);
                        if (smName && smName.toLowerCase().indexOf('gangway') !== -1) {
                            result = true;
                        }
                    }
                }
            }
        } catch (e) {}
        if (gangwayCacheCount > 2048) {
            gangwayCache = {};
            gangwayCacheCount = 0;
        }
        gangwayCache[key] = result;
        gangwayCacheCount++;
        return result;
    }

    function isWarshipOrGangwayActor(actorPtr, compPtr) {
        var shipCls = getWarshipClass();
        var cur = actorPtr;
        for (var hop = 0; hop < 4 && cur && !cur.isNull(); hop++) {
            if (shipCache.valid && cur.equals(shipCache.warship)) {
                return true;
            }
            if (isA(cur, shipCls)) {
                return true;
            }
            if (hop === 0 && isGangwayObjectCached(cur, false)) {
                return true;
            }
            try {
                var root = cur.add(OFFSETS.AActor_RootComponent).readPointer();
                if (root.isNull()) { break; }
                var attachParent = root.add(OFFSETS.USceneComponent_AttachParent).readPointer();
                if (attachParent.isNull()) { break; }
                cur = attachParent.add(OFFSETS.UActorComponent_OwnerPrivate).readPointer();
            } catch (e) {
                break;
            }
        }
        if (compPtr && !compPtr.isNull()) {
            if (isGangwayObjectCached(compPtr, true)) {
                return true;
            }
            try {
                var compOwner = compPtr.add(OFFSETS.UActorComponent_OwnerPrivate).readPointer();
                if (!compOwner.isNull() && (!actorPtr || actorPtr.isNull() || !compOwner.equals(actorPtr))) {
                    if ((shipCache.valid && compOwner.equals(shipCache.warship)) || isA(compOwner, shipCls)) {
                        return true;
                    }
                }
            } catch (e2) {}
        }
        return false;
    }

    function isCharacterBasedOnShip(charPtr) {
        if (!charPtr || charPtr.isNull() || isPendingKill(charPtr)) {
            return false;
        }
        try {
            var baseComp = charPtr.add(OFFSETS.ACharacter_BasedMovement).readPointer();
            if (!baseComp.isNull() && !isPendingKill(baseComp)) {
                var baseOwner = baseComp.add(OFFSETS.UActorComponent_OwnerPrivate).readPointer();
                if (isWarshipOrGangwayActor(baseOwner, baseComp)) {
                    return true;
                }
            }
        } catch (e) {}
        return false;
    }

    function isTargetOnShip(targetPtr, targetLoc) {
        if (!targetPtr || targetPtr.isNull() || isPendingKill(targetPtr)) {
            return false;
        }
        try {
            if (isA(targetPtr, getHumanClass())) {
                var sb = targetPtr.add(OFFSETS.ADH_HumanCharacter_ShipBase).readPointer();
                if (!sb.isNull() && !isPendingKill(sb)) {
                    return true;
                }
            }
        } catch (e) {}
        if (isCharacterBasedOnShip(targetPtr)) {
            return true;
        }
        return isPointInShipOBB(targetLoc, 50.0, 20.0);
    }

    function isAiOnShip(pawnPtr, aiLoc) {
        if (isCharacterBasedOnShip(pawnPtr)) {
            return true;
        }
        return isPointInShipOBB(aiLoc, -25.0, 0.0);
    }

    function traceSinglePawnRay(worldPtr, ignoreA, ignoreB, sx, sy, sz, ex, ey, ez) {
        traceStartBuf.writeFloat(sx);
        traceStartBuf.add(4).writeFloat(sy);
        traceStartBuf.add(8).writeFloat(sz);
        traceEndBuf.writeFloat(ex);
        traceEndBuf.add(4).writeFloat(ey);
        traceEndBuf.add(8).writeFloat(ez);
        traceHitBuf.writeByteArray(zeroHitBytes);

        FCollisionQueryParams_Ctor(queryParamsBuf, 0, ptr(0), 0, ignoreA);
        if (ignoreB && !ignoreB.isNull() && !isPendingKill(ignoreB)) {
            FCollisionQueryParams_AddIgnoredActor(queryParamsBuf, ignoreB);
        }
        var ok = UWorld_LineTraceSingleByChannel(
            worldPtr,
            traceHitBuf,
            traceStartBuf,
            traceEndBuf,
            2, // ECC_Pawn
            queryParamsBuf,
            DefaultResponseParam_ptr
        );
        if ((ok & 1) === 0 || (traceHitBuf.readU8() & 1) === 0) {
            return { hit: false, isShip: false, dist: 99999.0, normZ: 1.0 };
        }
        var hitActor = resolveWeakObject(traceHitBuf.add(OFFSETS.FHitResult_Actor));
        var hitComp = resolveWeakObject(traceHitBuf.add(OFFSETS.FHitResult_Component));
        var dist = traceHitBuf.add(OFFSETS.FHitResult_Distance).readFloat();
        var normZ = traceHitBuf.add(OFFSETS.FHitResult_ImpactNormal + 8).readFloat();
        return {
            hit: true,
            isShip: isWarshipOrGangwayActor(hitActor, hitComp),
            dist: Number.isFinite(dist) ? dist : 0.0,
            normZ: Number.isFinite(normZ) ? normZ : 0.0
        };
    }

    function probeForwardWall(worldPtr, pawnPtr, targetPtr, fromLoc, ux, uy, maxDist) {
        var res = { hitAny: false, hitShip: false, hitSteepWall: false };
        if (!worldPtr || worldPtr.isNull() || !fromLoc) {
            return res;
        }
        var heights = [-35.0, 20.0, 80.0];
        for (var i = 0; i < heights.length; i++) {
            var z = fromLoc.z + heights[i];
            var r = traceSinglePawnRay(
                worldPtr, pawnPtr, targetPtr,
                fromLoc.x, fromLoc.y, z,
                fromLoc.x + ux * maxDist, fromLoc.y + uy * maxDist, z
            );
            if (r.hit) {
                res.hitAny = true;
                if (r.isShip) {
                    res.hitShip = true;
                }
                if (r.normZ < WALKABLE_SLOPE_COS_78) {
                    res.hitSteepWall = true;
                }
            }
        }
        return res;
    }

    function isShipClimbBlocked(worldPtr, entry, targetPtr, cLoc, tLoc) {
        if (isTargetOnShip(targetPtr, tLoc)) {
            return true;
        }
        if (isAiOnShip(entry.pawn, cLoc) || isPointInShipOBB(cLoc, 55.0, 20.0)) {
            return true;
        }
        var dx = tLoc.x - cLoc.x;
        var dy = tLoc.y - cLoc.y;
        var horiz = Math.sqrt(dx * dx + dy * dy);
        if (Number.isFinite(horiz) && horiz > 1.0 && (isNearShipBroadphase(cLoc) || isNearShipBroadphase(tLoc))) {
            var ux = dx / horiz;
            var uy = dy / horiz;
            var probe = probeForwardWall(worldPtr, entry.pawn, targetPtr, cLoc, ux, uy, CONFIG.shipForwardTraceDistUU);
            if (probe.hitShip) {
                return true;
            }
        }
        return false;
    }

    function getTrackedCannibal(pawnPtr) {
        if (!pawnPtr || pawnPtr.isNull() || isPendingKill(pawnPtr)) {
            return null;
        }
        var key = pawnPtr.toString();
        var entry = trackedCannibals[key];
        if (!entry) {
            return null;
        }
        try {
            var curIdx = pawnPtr.add(OFFSETS.UObject_InternalIndex).readS32();
            var curNameRaw = pawnPtr.add(OFFSETS.UObject_NamePrivate).readU64();
            if (curIdx !== entry.internalIndex || !curNameRaw.equals(entry.nameRaw)) {
                cleanupMoveCompTracking(entry);
                delete trackedCannibals[key];
                return null;
            }
        } catch (e) {
            cleanupMoveCompTracking(entry);
            delete trackedCannibals[key];
            return null;
        }
        return entry;
    }

    function cleanupMoveCompTracking(entry) {
        if (!entry) {
            return;
        }
        if (entry.moveCompKey) {
            delete trackedMoveComps[entry.moveCompKey];
        }
    }

    function configureCannibalMovement(pawnPtr) {
        if (!pawnPtr || pawnPtr.isNull() || isPendingKill(pawnPtr)) {
            return ptr(0);
        }
        var moveComp = pawnPtr.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp)) {
            return ptr(0);
        }
        try {
            UCharacterMovementComponent_SetWalkableFloorAngle(moveComp, CONFIG.walkableFloorAngleDeg);
        } catch (eAngle) {
            moveComp.add(OFFSETS.UCharacterMovementComponent_WalkableFloorAngle).writeFloat(CONFIG.walkableFloorAngleDeg);
            moveComp.add(OFFSETS.UCharacterMovementComponent_WalkableFloorZ).writeFloat(WALKABLE_SLOPE_COS_78);
        }

        // 原版 BP_Cannibal の MaxStepHeight=128.0 は維持し、75 UU 未満の個体のみ引き上げる
        var curStep = moveComp.add(OFFSETS.UCharacterMovementComponent_MaxStepHeight).readFloat();
        if (!Number.isFinite(curStep) || curStep < CONFIG.minMaxStepHeightUU) {
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxStepHeight).writeFloat(CONFIG.minMaxStepHeightUU);
        }

        // bCanJump(0x02) + bCanWalk(0x04) + bCanSwim(0x08) + bCanFly(0x10)
        var navAgentFlags = moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).writeU8(navAgentFlags | 0x1E);
        var moveStateFlags = moveComp.add(OFFSETS.UNavMovementComponent_MovementState).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_MovementState).writeU8(moveStateFlags | 0x1E);
        var navMoveFlags = moveComp.add(OFFSETS.UNavMovementComponent_Flags).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_Flags).writeU8(navMoveFlags | 0x06);

        moveComp.add(OFFSETS.UDH_CharacterMovementComponent_AutoTreadDepthSpeed).writeFloat(0.0);
        moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bCanMoveUnderWater).writeU8(1);
        moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bAutoStepUpOutOfWater).writeU8(1);
        moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bAlwaysAutoStepOutOfWater).writeU8(1);
        moveComp.add(OFFSETS.UDH_CharacterMovementComponent_MaxSwimDepth).writeFloat(5.0);
        return moveComp;
    }

    function exitCannibalWallClimb(entry, ctrlHint, nowTimeSec) {
        if (!entry) {
            return;
        }
        var wasClimbing = (entry.climbPhase !== 0);
        entry.climbPhase = 0;
        entry.stuckCount = 0;
        entry.walkableSlopeHits = 0;
        if (wasClimbing) {
            entry.climbCooldownUntil = nowTimeSec + CONFIG.climbCooldownSec;
            entry.climbProtectedUntil = nowTimeSec + CONFIG.climbStuckGraceSec;
        }
        try {
            var ctrl = (ctrlHint && !ctrlHint.isNull() && !isPendingKill(ctrlHint))
                ? ctrlHint
                : entry.savedCtrl;
            if ((!ctrl || ctrl.isNull() || isPendingKill(ctrl)) &&
                entry.pawn && !entry.pawn.isNull() && !isPendingKill(entry.pawn)) {
                ctrl = entry.pawn.add(OFFSETS.APawn_Controller).readPointer();
            }
            if (entry.savedChaseTarget !== null && ctrl && !ctrl.isNull() && !isPendingKill(ctrl) &&
                isA(ctrl, getDhAiControllerClass())) {
                ctrl.add(OFFSETS.ADH_AIController_bChaseTargetCharacter).writeU8(entry.savedChaseTarget);
            }
        } catch (eCtrl) {}
        entry.savedChaseTarget = null;
        entry.savedCtrl = ptr(0);

        if (wasClimbing) {
            try {
                if (entry.pawn && !entry.pawn.isNull() && !isPendingKill(entry.pawn)) {
                    var moveComp = entry.pawn.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
                    if (!moveComp.isNull() && !isPendingKill(moveComp)) {
                        if (typeof entry.savedBrakingFly === 'number') {
                            moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(entry.savedBrakingFly);
                        }
                        if (typeof entry.savedMaxFlySpeed === 'number') {
                            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(entry.savedMaxFlySpeed);
                        }
                        var vz = moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).readFloat();
                        if (Number.isFinite(vz) && vz > 0.0) {
                            moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(-20.0);
                        }
                        if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() === 5) {
                            UCharacterMovementComponent_SetMovementMode(moveComp, 3, 0); // MOVE_Falling -> 自動接地
                        }
                    }
                    var curLoc = getActorLocation(entry.pawn);
                    if (curLoc) {
                        entry.lastProgressLoc = { x: curLoc.x, y: curLoc.y, z: curLoc.z };
                    }
                }
            } catch (eMc) {}
        }
        entry.lastProgressDist = null;
        entry.nextStuckCheckAt = nowTimeSec + CONFIG.stuckCheckIntervalSec;
    }

    function resetAllTrackedCannibals(worldPtr) {
        var wKey = (worldPtr && !worldPtr.isNull()) ? worldPtr.toString() : '';
        if (wKey === trackedWorldKey) {
            return;
        }
        trackedWorldKey = wKey;
        var keys = Object.keys(trackedCannibals);
        for (var i = 0; i < keys.length; i++) {
            var e = trackedCannibals[keys[i]];
            if (e && e.climbPhase !== 0) {
                exitCannibalWallClimb(e, ptr(0), 0.0);
            }
        }
        trackedCannibals = {};
        trackedMoveComps = {};
        nextActorDiscoveryAt = 0.0;
        gangwayCache = {};
        gangwayCacheCount = 0;
        shipCache.valid = false;
        shipCache.warship = ptr(0);
    }

    function registerCannibalIfNeeded(pawnPtr) {
        if (!pawnPtr || pawnPtr.isNull() || isPendingKill(pawnPtr)) {
            return;
        }
        if (!isA(pawnPtr, getCannibalClass())) {
            return;
        }
        if ((ADH_Character_IsAlive(pawnPtr) & 1) === 0) {
            return;
        }
        if (getTrackedCannibal(pawnPtr)) {
            return;
        }

        var wt = getWorldAndTime();
        if (!wt.world.isNull()) {
            resetAllTrackedCannibals(wt.world);
        }

        var moveComp = configureCannibalMovement(pawnPtr);
        var moveCompKey = (!moveComp.isNull()) ? moveComp.toString() : '';
        var key = pawnPtr.toString();
        var initLoc = getActorLocation(pawnPtr);
        var startedOnShip = isAiOnShip(pawnPtr, initLoc);
        var entry = {
            pawn: pawnPtr,
            key: key,
            moveCompKey: moveCompKey,
            startedOnShip: startedOnShip,
            internalIndex: pawnPtr.add(OFFSETS.UObject_InternalIndex).readS32(),
            nameRaw: pawnPtr.add(OFFSETS.UObject_NamePrivate).readU64(),
            lastProgressLoc: initLoc ? { x: initLoc.x, y: initLoc.y, z: initLoc.z } : null,
            lastProgressDist: null,
            lastTargetKey: '',
            nextStuckCheckAt: wt.timeSeconds + CONFIG.stuckCheckIntervalSec,
            nextVaultAt: 0.0,
            stuckCount: 0,
            climbPhase: 0,
            climbStartAt: 0.0,
            climbStartZ: 0.0,
            climbMinGoalZ: 0.0,
            climbLastZCheckAt: 0.0,
            climbLastCheckZ: 0.0,
            climbBackoffUntil: 0.0,
            climbNoZCount: 0,
            climbCrestUntil: 0.0,
            climbCooldownUntil: 0.0,
            climbProtectedUntil: 0.0,
            climbNextFloorCheckAt: 0.0,
            climbNextShipTraceAt: 0.0,
            walkableSlopeHits: 0,
            savedChaseTarget: null,
            savedCtrl: ptr(0),
            savedBrakingFly: 0.0,
            savedMaxFlySpeed: 600.0
        };
        trackedCannibals[key] = entry;
        if (moveCompKey) {
            trackedMoveComps[moveCompKey] = entry;
        }
    }

    function untrackCannibal(pawnPtr) {
        if (!pawnPtr || pawnPtr.isNull()) {
            return;
        }
        var key = pawnPtr.toString();
        var entry = trackedCannibals[key];
        if (entry) {
            if (entry.climbPhase !== 0) {
                exitCannibalWallClimb(entry, ptr(0), 0.0);
            }
            cleanupMoveCompTracking(entry);
            delete trackedCannibals[key];
        }
    }

    function triggerCannibalVault(worldPtr, entry, targetPtr, cLoc, tLoc, nowTimeSec) {
        if (nowTimeSec < entry.nextVaultAt) {
            return false;
        }
        if (isShipClimbBlocked(worldPtr, entry, targetPtr, cLoc, tLoc)) {
            return false;
        }
        var moveComp = entry.pawn.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp)) {
            return false;
        }
        // MOVE_Walking(1) のときのみ短距離跳躍（段差の小障害物用）
        if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() !== 1) {
            return false;
        }
        var dx = tLoc.x - cLoc.x;
        var dy = tLoc.y - cLoc.y;
        var horiz = Math.sqrt(dx * dx + dy * dy);
        if (!Number.isFinite(horiz) || horiz <= 1.0) {
            return false;
        }
        var velZTarget = (tLoc.z > cLoc.z + CONFIG.vaultUphillDeltaZUU)
            ? CONFIG.vaultVelZUphill
            : CONFIG.vaultVelZNormal;
        var curVelZ = moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).readFloat();
        var nextVelZ = Number.isFinite(curVelZ) ? Math.max(curVelZ, velZTarget) : velZTarget;

        moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat((dx / horiz) * CONFIG.vaultVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat((dy / horiz) * CONFIG.vaultVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(nextVelZ);
        UCharacterMovementComponent_SetMovementMode(moveComp, 3, 0); // MOVE_Falling
        entry.nextVaultAt = nowTimeSec + CONFIG.vaultCooldownSec;
        return true;
    }

    function enterCannibalWallClimb(worldPtr, entry, ctrl, targetPtr, cLoc, tLoc, nowTimeSec) {
        if (isShipClimbBlocked(worldPtr, entry, targetPtr, cLoc, tLoc)) {
            return false;
        }
        var moveComp = entry.pawn.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp)) {
            return false;
        }
        var mcKey = moveComp.toString();
        if (mcKey !== entry.moveCompKey) {
            cleanupMoveCompTracking(entry);
            entry.moveCompKey = mcKey;
            trackedMoveComps[mcKey] = entry;
        }

        // 原版 ADH_AIController::Tick が StopMovement 直後に MoveToActor(bUsePathfinding=false) を再発行して
        // Velocity を上書きしないよう、登攀中のみ bChaseTargetCharacter (+0x3AD) を一時停止（終了時必ず復元）
        if (entry.savedChaseTarget === null) {
            entry.savedChaseTarget = ctrl.add(OFFSETS.ADH_AIController_bChaseTargetCharacter).readU8();
        }
        entry.savedCtrl = ctrl;
        ctrl.add(OFFSETS.ADH_AIController_bChaseTargetCharacter).writeU8(0);
        clearActorTimer(ctrl, OFFSETS.ADH_AIController_StuckTimerHandle);
        try { AAIController_StopMovement(ctrl); } catch (eStop) {}

        var origBraking = moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).readFloat();
        var origMaxFly = moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).readFloat();
        entry.savedBrakingFly = Number.isFinite(origBraking) ? origBraking : 0.0;
        entry.savedMaxFlySpeed = (Number.isFinite(origMaxFly) && origMaxFly > 0.0) ? origMaxFly : 600.0;

        var navAgentFlags = moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).writeU8(navAgentFlags | 0x1E);
        var moveStateFlags = moveComp.add(OFFSETS.UNavMovementComponent_MovementState).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_MovementState).writeU8(moveStateFlags | 0x1E);

        moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(CONFIG.climbFlySpeed);
        moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(0.0);
        entry.pawn.add(OFFSETS.ADH_AnimalCharacter_StuckTimer).writeFloat(0.0);
        UCharacterMovementComponent_SetMovementMode(moveComp, 5, 0); // MOVE_Flying

        entry.climbPhase = 1; // CLIMB_UP
        entry.climbStartAt = nowTimeSec;
        entry.climbStartZ = cLoc.z;
        entry.climbMinGoalZ = Math.max(tLoc.z + CONFIG.climbTopMarginUU, cLoc.z + CONFIG.climbMinClearanceUU);
        entry.climbLastZCheckAt = nowTimeSec + 0.25;
        entry.climbLastCheckZ = cLoc.z;
        entry.climbBackoffUntil = 0.0;
        entry.climbNoZCount = 0;
        entry.climbCrestUntil = 0.0;
        entry.climbNextFloorCheckAt = nowTimeSec + CONFIG.slopeRecoverGraceSec;
        entry.climbNextShipTraceAt = nowTimeSec + CONFIG.shipMidClimbTraceIntervalSec;
        entry.walkableSlopeHits = 0;
        entry.stuckCount = 0;
        return true;
    }

    function tickCannibalWallClimb(worldPtr, entry, ctrl, targetPtr, cLoc, tLoc, nowTimeSec) {
        var moveComp = entry.pawn.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp) || !cLoc || !tLoc) {
            exitCannibalWallClimb(entry, ctrl, nowTimeSec);
            return false;
        }
        // 2B 登攀中の船体接近・目標乗船ガード（即座に中断）
        if (isTargetOnShip(targetPtr, tLoc) || isPointInShipOBB(cLoc, 40.0, 15.0)) {
            exitCannibalWallClimb(entry, ctrl, nowTimeSec);
            return false;
        }

        var dx = tLoc.x - cLoc.x;
        var dy = tLoc.y - cLoc.y;
        var distXY = Math.sqrt(dx * dx + dy * dy);
        var dist3D = distance3D(cLoc, tLoc);
        var elapsed = nowTimeSec - entry.climbStartAt;
        var zGained = cLoc.z - entry.climbStartZ;

        if (dist3D <= CONFIG.climbExitDistUU ||
            elapsed >= CONFIG.climbMaxDurationSec ||
            zGained >= CONFIG.climbMaxZGainUU) {
            exitCannibalWallClimb(entry, ctrl, nowTimeSec);
            return false;
        }

        var ux = (Number.isFinite(distXY) && distXY > 1.0) ? (dx / distXY) : 0.0;
        var uy = (Number.isFinite(distXY) && distXY > 1.0) ? (dy / distXY) : 0.0;

        // 2B 登攀中 0.15s ごとの船殻・舷梯レイ再検査
        if (nowTimeSec >= entry.climbNextShipTraceAt) {
            entry.climbNextShipTraceAt = nowTimeSec + CONFIG.shipMidClimbTraceIntervalSec;
            if (isNearShipBroadphase(cLoc) && distXY > 1.0) {
                var shipProbe = probeForwardWall(worldPtr, entry.pawn, targetPtr, cLoc, ux, uy, CONFIG.shipForwardTraceDistUU);
                if (shipProbe.hitShip) {
                    exitCannibalWallClimb(entry, ctrl, nowTimeSec);
                    return false;
                }
            }
        }

        ctrl.add(OFFSETS.ADH_AIController_bChaseTargetCharacter).writeU8(0);
        entry.pawn.add(OFFSETS.ADH_AnimalCharacter_StuckTimer).writeFloat(0.0);
        moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(CONFIG.climbFlySpeed);
        moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(0.0);
        if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() !== 5) {
            UCharacterMovementComponent_SetMovementMode(moveComp, 5, 0);
        }
        try {
            if (AAIController_GetMoveStatus(ctrl) === 3) {
                AAIController_StopMovement(ctrl);
            }
        } catch (eSt) {}
        setActorYawUpright(entry.pawn, dx, dy);

        if (entry.climbPhase === 1) {
            // < 78° 傾角復帰判定（MOVE_Flying 中は CurrentFloor が空のため K2_FindFloor + 前方レイで判定）
            if (elapsed >= CONFIG.slopeRecoverGraceSec && zGained >= CONFIG.slopeRecoverMinZGainUU &&
                nowTimeSec >= entry.climbNextFloorCheckAt) {
                entry.climbNextFloorCheckAt = nowTimeSec + CONFIG.slopeCheckIntervalSec;
                try {
                    var wallAhead = (distXY > 1.0)
                        ? probeForwardWall(worldPtr, entry.pawn, targetPtr, cLoc, ux, uy, 145.0)
                        : { hitAny: false, hitShip: false, hitSteepWall: false };
                    if (wallAhead.hitShip) {
                        exitCannibalWallClimb(entry, ctrl, nowTimeSec);
                        return false;
                    }
                    if (wallAhead.hitSteepWall) {
                        entry.walkableSlopeHits = 0;
                    } else {
                        floorLocBuf.writeFloat(cLoc.x);
                        floorLocBuf.add(4).writeFloat(cLoc.y);
                        floorLocBuf.add(8).writeFloat(cLoc.z);
                        floorResultBuf.writeByteArray(zeroHitBytes);
                        UCharacterMovementComponent_K2_FindFloor(moveComp, floorLocBuf, floorResultBuf);
                        var flFlags = floorResultBuf.readU8();
                        var bWalkable = (flFlags & 0x02) !== 0;
                        var flDist = floorResultBuf.add(OFFSETS.FFindFloorResult_FloorDist).readFloat();
                        var flNormZ = floorResultBuf.add(OFFSETS.FFindFloorResult_HitNormalZ).readFloat();
                        var flActor = resolveWeakObject(floorResultBuf.add(OFFSETS.FFindFloorResult_HitActor));
                        var flComp = resolveWeakObject(floorResultBuf.add(OFFSETS.FFindFloorResult_HitComponent));
                        if (bWalkable && Number.isFinite(flDist) && flDist <= CONFIG.slopeRecoverMaxFloorDistUU &&
                            Number.isFinite(flNormZ) && flNormZ > WALKABLE_RECOVER_COS_75 &&
                            !isWarshipOrGangwayActor(flActor, flComp)) {
                            entry.walkableSlopeHits++;
                            if (entry.walkableSlopeHits >= CONFIG.slopeRecoverHitsNeeded) {
                                // < 78° の歩行可能斜面／崖天面に復帰 -> 登攀終了して通常歩行へ
                                exitCannibalWallClimb(entry, ctrl, nowTimeSec);
                                return false;
                            }
                        } else {
                            entry.walkableSlopeHits = 0;
                        }
                    }
                } catch (eFl) {}
            }

            var desiredZ = Math.max(tLoc.z + CONFIG.climbTopMarginUU, entry.climbMinGoalZ);
            if (cLoc.z >= desiredZ) {
                entry.climbPhase = 2;
                entry.climbCrestUntil = nowTimeSec + CONFIG.crestDurationSec;
            } else {
                if (nowTimeSec >= entry.climbLastZCheckAt) {
                    var dzStep = cLoc.z - entry.climbLastCheckZ;
                    entry.climbLastCheckZ = cLoc.z;
                    entry.climbLastZCheckAt = nowTimeSec + 0.25;
                    if (nowTimeSec >= entry.climbBackoffUntil && dzStep < 20.0) {
                        entry.climbNoZCount++;
                        if (entry.climbNoZCount >= 4) {
                            exitCannibalWallClimb(entry, ctrl, nowTimeSec);
                            return false;
                        }
                        entry.climbBackoffUntil = nowTimeSec + CONFIG.climbBackoffSec;
                    } else if (dzStep >= 20.0) {
                        entry.climbNoZCount = 0;
                    }
                }
                if (nowTimeSec < entry.climbBackoffUntil) {
                    moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(-ux * CONFIG.climbBackoffVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(-uy * CONFIG.climbBackoffVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.climbBackoffVelZ);
                } else {
                    moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(ux * CONFIG.climbVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(uy * CONFIG.climbVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.climbVelZ);
                }
                return true;
            }
        }

        // Phase 2: CREST（崖天面へ水平に押し込んで確実に接地）
        if (nowTimeSec >= entry.climbCrestUntil || distXY <= 95.0) {
            exitCannibalWallClimb(entry, ctrl, nowTimeSec);
            return false;
        }
        moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(ux * CONFIG.crestVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(uy * CONFIG.crestVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.crestVelZ);
        return true;
    }

    function discoverUntrackedCannibals(worldPtr) {
        if (!worldPtr || worldPtr.isNull() || isPendingKill(worldPtr)) {
            return;
        }
        try {
            var levelsPtr = worldPtr.add(OFFSETS.UWorld_Levels).readPointer();
            var levelCount = worldPtr.add(OFFSETS.UWorld_Levels + 8).readS32();
            if (levelsPtr.isNull() || levelCount <= 0 || levelCount > 64) {
                return;
            }
            var canCls = getCannibalClass();
            for (var li = 0; li < levelCount; li++) {
                var lvl = levelsPtr.add(li * 8).readPointer();
                if (lvl.isNull() || isPendingKill(lvl)) { continue; }
                var actorsPtr = lvl.add(OFFSETS.ULevel_Actors).readPointer();
                var actorCount = lvl.add(OFFSETS.ULevel_Actors + 8).readS32();
                if (actorsPtr.isNull() || actorCount <= 0 || actorCount > 16384) { continue; }
                for (var ai = 0; ai < actorCount; ai++) {
                    var act = actorsPtr.add(ai * 8).readPointer();
                    if (act.isNull()) { continue; }
                    if (trackedCannibals[act.toString()]) { continue; }
                    if (!isPendingKill(act) && isA(act, canCls)) {
                        registerCannibalIfNeeded(act);
                    }
                }
            }
        } catch (e) {}
    }

    function tickTrackedCannibals(wt) {
        updateWarshipCache(currentGameState);
        if (wt.timeSeconds >= nextActorDiscoveryAt) {
            nextActorDiscoveryAt = wt.timeSeconds + 1.5;
            discoverUntrackedCannibals(wt.world);
        }
        var keys = Object.keys(trackedCannibals);
        if (keys.length === 0) {
            return;
        }
        var dhAiCls = getDhAiControllerClass();
        for (var i = 0; i < keys.length; i++) {
            var k = keys[i];
            var raw = trackedCannibals[k];
            if (!raw) {
                continue;
            }
            var entry = getTrackedCannibal(raw.pawn);
            if (!entry) {
                continue;
            }
            if ((ADH_Character_IsAlive(entry.pawn) & 1) === 0) {
                if (entry.climbPhase !== 0) {
                    exitCannibalWallClimb(entry, ptr(0), wt.timeSeconds);
                }
                cleanupMoveCompTracking(entry);
                delete trackedCannibals[k];
                continue;
            }

            // 梯子登攀中・罠拘束中・仲間呼び硬直中は登攀を終了して干渉しない
            if (!entry.pawn.add(OFFSETS.ADH_Character_CurrentLadder).readPointer().isNull() ||
                !entry.pawn.add(OFFSETS.ADH_Character_Snare).readPointer().isNull() ||
                entry.pawn.add(OFFSETS.ADH_AnimalCharacter_bCallingForHelp).readU8() !== 0) {
                if (entry.climbPhase !== 0) {
                    exitCannibalWallClimb(entry, ptr(0), wt.timeSeconds);
                }
                continue;
            }

            // 原版 ThreatTable が選択した TargetCharacter (+0x378) のみを読み取り（仇恨表は一切改変しない）
            var ctrl = entry.pawn.add(OFFSETS.APawn_Controller).readPointer();
            if (ctrl.isNull() || isPendingKill(ctrl) || !isA(ctrl, dhAiCls)) {
                if (entry.climbPhase !== 0) {
                    exitCannibalWallClimb(entry, ptr(0), wt.timeSeconds);
                }
                continue;
            }
            var target = ctrl.add(OFFSETS.ADH_AIController_TargetCharacter).readPointer();
            if (target.isNull() || isPendingKill(target) || (ADH_Character_IsAlive(target) & 1) === 0) {
                if (entry.climbPhase !== 0) {
                    exitCannibalWallClimb(entry, ctrl, wt.timeSeconds);
                }
                entry.stuckCount = 0;
                entry.lastProgressDist = null;
                continue;
            }

            var cLoc = getActorLocation(entry.pawn);
            var tLoc = getActorLocation(target);
            if (!cLoc || !tLoc) {
                if (entry.climbPhase !== 0) {
                    exitCannibalWallClimb(entry, ctrl, wt.timeSeconds);
                }
                continue;
            }

            var tKey = target.toString();
            if (tKey !== entry.lastTargetKey) {
                entry.lastTargetKey = tKey;
                entry.lastProgressDist = null;
            }

            var moveCompCur = entry.pawn.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
            if (entry.startedOnShip && !moveCompCur.isNull() && !isPendingKill(moveCompCur) &&
                moveCompCur.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() === 1 &&
                !isAiOnShip(entry.pawn, cLoc)) {
                entry.startedOnShip = false;
            }

            // 垂直壁・氷壁登攀中は 2 段階ステートマシンを実行
            if (entry.climbPhase !== 0) {
                tickCannibalWallClimb(wt.world, entry, ctrl, target, cLoc, tLoc, wt.timeSeconds);
                continue;
            }

            var dist3D = distance3D(cLoc, tLoc);
            // 近接攻撃圏内 (<= 220 UU) では近接戦闘を妨げないようスキップ
            if (dist3D <= CONFIG.meleeExemptDistUU) {
                entry.stuckCount = 0;
                entry.lastProgressLoc = { x: cLoc.x, y: cLoc.y, z: cLoc.z };
                entry.lastProgressDist = dist3D;
                entry.nextStuckCheckAt = wt.timeSeconds + CONFIG.stuckCheckIntervalSec;
                continue;
            }

            if (wt.timeSeconds >= entry.nextStuckCheckAt) {
                var prevLoc = entry.lastProgressLoc;
                var prevDist = entry.lastProgressDist;
                entry.lastProgressLoc = { x: cLoc.x, y: cLoc.y, z: cLoc.z };
                entry.lastProgressDist = dist3D;
                entry.nextStuckCheckAt = wt.timeSeconds + CONFIG.stuckCheckIntervalSec;

                if (prevLoc) {
                    var moved = distance3D(cLoc, prevLoc);
                    var closedDist = (typeof prevDist === 'number') ? (prevDist - dist3D) : moved;
                    var zGainWin = cLoc.z - prevLoc.z;
                    var dz = tLoc.z - cLoc.z;
                    var dxT = tLoc.x - cLoc.x;
                    var dyT = tLoc.y - cLoc.y;
                    var distXY = Math.sqrt(dxT * dxT + dyT * dyT);
                    var canClimbNow = (wt.timeSeconds >= entry.climbCooldownUntil);

                    var moveStatus = 0;
                    var partialPath = false;
                    try { moveStatus = AAIController_GetMoveStatus(ctrl); } catch (eMs) {}
                    try { partialPath = (AAIController_HasPartialPath(ctrl) & 1) !== 0; } catch (ePp) {}

                    // 条件A: ターゲットが高所(dz >= 95 UU)で崖下ウロウロ/部分経路停滞 -> 垂直氷壁登攀へ即移行（2B 船体は遮断）
                    var elevatedStalled = (dz >= CONFIG.elevatedDzUU && distXY <= CONFIG.elevatedMaxXyUU &&
                        ((closedDist < CONFIG.minCloseProgressUU && zGainWin < CONFIG.minZProgressUU) ||
                         (distXY <= CONFIG.closeWallXyUU && (moveStatus !== 3 || partialPath))));
                    if (canClimbNow && elevatedStalled) {
                        if (enterCannibalWallClimb(wt.world, entry, ctrl, target, cLoc, tLoc, wt.timeSeconds)) {
                            tickCannibalWallClimb(wt.world, entry, ctrl, target, cLoc, tLoc, wt.timeSeconds);
                            continue;
                        }
                    }

                    // 条件B: 障害物・壁に引っかかり実移動 < 70 UU（2B 船体・舷梯は跳躍・登攀ともに遮断）
                    if (moved >= CONFIG.stuckMinProgressUU) {
                        entry.stuckCount = 0;
                    } else {
                        entry.stuckCount++;
                        if (canClimbNow && (dz >= CONFIG.elevatedDzUU || entry.stuckCount >= 2 ||
                            !triggerCannibalVault(wt.world, entry, target, cLoc, tLoc, wt.timeSeconds))) {
                            if (enterCannibalWallClimb(wt.world, entry, ctrl, target, cLoc, tLoc, wt.timeSeconds)) {
                                tickCannibalWallClimb(wt.world, entry, ctrl, target, cLoc, tLoc, wt.timeSeconds);
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }

    // 1. ADH_AIControllerMonster::CharacterStuck (replaceFast: 登攀中の追跡食人鬼のみ自爆抑止・他は完全素通し)
    var characterStuckCb = new NativeCallback(function (ctrlPtr) {
        try {
            if (ctrlPtr && !ctrlPtr.isNull() && !isPendingKill(ctrlPtr)) {
                var controlled = ctrlPtr.add(OFFSETS.ADH_AIController_ControlledCharacter).readPointer();
                var entry = getTrackedCannibal(controlled);
                if (entry) {
                    var wt = getWorldAndTime();
                    if (entry.climbPhase !== 0 || wt.timeSeconds < entry.climbProtectedUntil) {
                        return;
                    }
                }
            }
        } catch (e) {}
        if (origMonsterCharacterStuck) {
            origMonsterCharacterStuck(ctrlPtr);
        }
    }, 'void', ['pointer'], 'win64');
    pinnedCallbacks.push(characterStuckCb);
    origMonsterCharacterStuck = new NativeFunction(
        Interceptor.replaceFast(moduleBase.add(RVA.ADH_AIControllerMonster_CharacterStuck), characterStuckCb),
        'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );

    // 2. UCharacterMovementComponent::IsWalkable (2B 受動物理防爬ガード: Interceptor.attach でカスタムスペル.js と安全共存、0x241D1E0 上の onLeave 総数 <= 2)
    Interceptor.attach(moduleBase.add(RVA.UCharacterMovementComponent_IsWalkable), {
        onEnter: function (args) {
            this.blockShipWalk = false;
            try {
                var mc = args[0];
                if (!mc || mc.isNull()) {
                    return;
                }
                var entry = trackedMoveComps[mc.toString()];
                if (!entry) {
                    return; // プレイヤー・通常野生動物は O(1) 即時素通し
                }
                var hitPtr = args[1];
                if (!hitPtr || hitPtr.isNull() || (hitPtr.readU8() & 1) === 0) {
                    return;
                }
                var hitActor = resolveWeakObject(hitPtr.add(OFFSETS.FHitResult_Actor));
                var hitComp = resolveWeakObject(hitPtr.add(OFFSETS.FHitResult_Component));
                if (isWarshipOrGangwayActor(hitActor, hitComp)) {
                    if (!entry.startedOnShip) {
                        // 2B: 船外スポーンの食人鬼は船殻・舷梯・甲板縁を一切歩行不可（78°斜面・段差128・水辺這い上がり・舷梯登船を全面封鎖）
                        this.blockShipWalk = true;
                    } else if (hitPtr.add(OFFSETS.FHitResult_ImpactNormal + 8).readFloat() < 0.71) {
                        // 万一船内スポーンされた個体でも急傾斜(船縁・柱・マスト)は登らせない
                        this.blockShipWalk = true;
                    }
                }
            } catch (e) {}
        },
        onLeave: function (retval) {
            if (this.blockShipWalk) {
                retval.replace(ptr(0));
            }
        }
    });

    // 3. ADH_AIController::OnPossess (onEnter のみ・onLeave 消費 0)
    Interceptor.attach(moduleBase.add(RVA.ADH_AIController_OnPossess), {
        onEnter: function (args) {
            try {
                registerCannibalIfNeeded(args[1]);
            } catch (e) {}
        }
    });

    // 4. ADH_AnimalCharacter::BeginPlay (ワールド配置済み食人鬼の補完登録・onEnter のみ)
    Interceptor.attach(moduleBase.add(RVA.ADH_AnimalCharacter_BeginPlay), {
        onEnter: function (args) {
            try {
                registerCannibalIfNeeded(args[0]);
            } catch (e) {}
        }
    });

    // 5. ADH_AnimalCharacter::Destroyed / ADH_Character::OnDied (登攀状態復元＋即時追跡解除・onEnter のみ)
    Interceptor.attach(moduleBase.add(RVA.ADH_AnimalCharacter_Destroyed), {
        onEnter: function (args) {
            try {
                untrackCannibal(args[0]);
            } catch (e) {}
        }
    });

    Interceptor.attach(moduleBase.add(RVA.ADH_Character_OnDied), {
        onEnter: function (args) {
            try {
                untrackCannibal(args[0]);
            } catch (e) {}
        }
    });

    // 6. ADH_GameState::Tick (GameThread 監視・UWorld::TimeSeconds 駆動・onEnter のみ)
    Interceptor.attach(moduleBase.add(RVA.ADH_GameState_Tick), {
        onEnter: function (args) {
            try {
                currentGameState = args[0];
                var wt = getWorldAndTime();
                if (wt.world.isNull()) {
                    return;
                }
                resetAllTrackedCannibals(wt.world);
                tickTrackedCannibals(wt);
            } catch (e) {}
        }
    });

    log('初期化完了（食人鬼 垂直壁登攀+<78度傾角復帰 + 2B軍艦・舷梯3層防爬ガード + 原版ThreatTable互換）');
})();
