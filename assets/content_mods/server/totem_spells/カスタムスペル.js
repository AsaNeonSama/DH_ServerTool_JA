// カスタムスペル（Option C: 完全ゼロ資産・純JS UClass メモリ合成 + 5スペル全実装）
// 対象 5 スペル（クライアント側 BP_DreadGameInstance.ThrallSpells [5..9] と完全一致）:
//   [5] TS_BombMice_C   (爆弾ネズミ)     : CD 300s / 持続 300s / 原版ネズミ(BP_Prey_Rat)召喚 (7/10/13体) + 純JS専用NavMesh経路探索・急斜面登攀(AAIController) + 接近0.4s/死亡時爆発
//   [6] TS_BoomShip_C   (船体爆破)       : CD 300s / 持続 2/4/6s / STS_CASTER / 船体に 2/4/6 箇所の破口 (半径 500/1000/1500)
//   [7] TS_Teleport_C   (テレポート)     : CD 120s / 持続 2s / トーテム指定転移 + 牢獄脱出 + ニトロ破壊 + 霊界入場/出現演出
//   [8] TS_Intimidate_C (威嚇)           : CD 120s / 持続 5/10/15s / STS_CASTER(霊界同等・即発動) / 半径50m丸腰化 + 暗視野フェード + スタミナ消費 / 通知なし(効果音のみ)
//   [9] TS_Transfer_C   (エネルギー譲渡) : CD 90s  / 持続 2.5s / STS_TARGET / 仲間の傀儡のみ指定可能 / 段階別エネルギー譲渡 / 通知なし(効果音のみ)
// ※ サーバー側 .pak / .uasset / .uexp 一切不要：UE4 ネイティブ API で UBlueprintGeneratedClass をメモリ合成し
//    クライアント .pak とビット単位で一致する ClassNetCacheChecksum を維持

(function () {
    'use strict';

    var MODULE_NAME = 'DreadHungerServer-Win64-Shipping.exe';
    var moduleBase = Module.findBaseAddress(MODULE_NAME);
    if (!moduleBase || moduleBase.isNull()) {
        return;
    }

    // ========================================================================
    // 1. 調整可能パラメータ
    // ========================================================================
    var CONFIG = {
        // 爆弾ネズミ (TS_BombMice): 原版 MOD (BP_SummonedMice) の移動物理 + 純JS専用 NavMesh 経路探索
        bombMiceCooldownSec: 300.0,
        bombMiceDurationSec: 300.0,
        bombMiceSummonRetrySec: 0.4,
        bombMiceCountByTier: { 2: 7, 3: 10, 4: 13 },
        bombMiceMaxHealth: 5.0,
        bombMiceSpeed: 650.0,
        bombMiceSwimSpeed: 700.0,
        bombMiceWalkableFloorAngle: 78.0,
        bombMiceMaxStepHeight: 65.0,
        bombMiceMaxAcceleration: 2400.0,
        bombMiceTurnRateYaw: 540.0,
        bombMiceMaxSwimDepth: 5.0,
        bombMiceStuckSpeed: 100.0,
        bombMiceNativeStuckJumpSec: 1.2,
        bombMiceJumpZVelocity: 420.0,
        bombMiceVaultVelZ: 510.0,
        bombMiceVaultVelXY: 400.0,
        bombMiceVaultCooldownSec: 1.1,
        bombMiceAcceptanceRadiusUU: 50.0,
        bombMiceRepathIntervalSec: 0.45,
        bombMiceCloseRepathIntervalSec: 0.25,
        bombMiceCloseRangeUU: 800.0,
        bombMiceIdleRepathCooldownSec: 0.20,
        bombMiceTargetMoveRepathUU: 120.0,
        bombMiceTargetSwitchHysteresisUU: 300.0,
        bombMiceNavProjectExtentXY: 500.0,
        bombMiceNavProjectExtentZ: 600.0,
        bombMiceNavReachableRadiusUU: 900.0,
        bombMiceDirectFallbackMaxDistUU: 1500.0,
        bombMiceStuckCheckIntervalSec: 0.65,
        bombMiceStuckMinProgressUU: 70.0,
        bombMiceWallClimbExemptDistUU: 160.0,
        bombMiceElevatedDzUU: 95.0,
        bombMiceElevatedMaxXyUU: 950.0,
        bombMiceCloseWallXyUU: 450.0,
        bombMiceMinCloseProgressUU: 90.0,
        bombMiceMinZProgressUU: 45.0,
        bombMiceClimbFlySpeed: 900.0,
        bombMiceClimbVelZ: 480.0,
        bombMiceClimbVelXY: 230.0,
        bombMiceClimbBackoffVelXY: 220.0,
        bombMiceClimbBackoffVelZ: 280.0,
        bombMiceClimbBackoffSec: 0.26,
        bombMiceCrestVelXY: 520.0,
        bombMiceCrestVelZ: -40.0,
        bombMiceCrestDurationSec: 0.42,
        bombMiceClimbTopMarginUU: 35.0,
        bombMiceClimbMinClearanceUU: 150.0,
        bombMiceClimbMaxDurationSec: 4.5,
        bombMiceClimbMaxZGainUU: 1400.0,
        bombMiceClimbCooldownSec: 0.85,
        bombMiceClimbPitchDefaultDeg: 72.0,
        bombMiceClimbPitchMinDeg: 55.0,
        bombMiceClimbPitchMaxDeg: 78.0,
        bombMiceClimbPitchBackoffDeg: 35.0,
        bombMiceSlopeRecoverGraceSec: 0.30,
        bombMiceSlopeRecoverMinZGainUU: 35.0,
        bombMiceSlopeCheckIntervalSec: 0.12,
        bombMiceSlopeRecoverMaxFloorDistUU: 38.0,
        bombMiceSlopeRecoverHitsNeeded: 2,
        shipBroadphaseRadiusUU: 3600.0,
        shipObbMinX: -2580.0,
        shipObbMaxX: 2060.0,
        shipObbMinY: -530.0,
        shipObbMaxY: 530.0,
        shipObbMinZ: -350.0,
        shipObbMaxZ: 3700.0,
        shipForwardTraceDistUU: 220.0,
        shipMidClimbTraceIntervalSec: 0.15,
        bombMiceDetourRadiusUU: 450.0,
        bombMiceDetourDurationSec: 0.85,
        bombMiceTriggerRadiusUU: 200.0,
        bombMiceFuseSec: 0.4,
        bombMiceExplosionDamage: 7.0,
        bombMiceExplosionInnerRadius: 150.0,
        bombMiceExplosionOuterRadius: 1000.0,

        // 船体爆破 (TS_BoomShip)
        boomShipCooldownSec: 300.0,
        boomShipDurationsByTier: { 2: 2.0, 3: 4.0, 4: 6.0 },
        boomShipBreachesByTier: {
            2: { count: 2, radius: 500.0 },
            3: { count: 4, radius: 1000.0 },
            4: { count: 6, radius: 1500.0 }
        },

        // テレポート (TS_Teleport)
        teleportCooldownSec: 120.0,
        teleportDurationSec: 2.0,
        teleportHealthFactorByTier: { 2: 0.8, 3: 0.9, 4: 1.0 },
        teleportSpiritWalkFallbackSec: 2.6,
        teleportSpiritWalkExitDelaySec: 0.15,

        // 威嚇 (TS_Intimidate): 通知なし・効果音のみ・霊界同等の即発動(STS_CASTER) / 半径 50m (5000 UU) / 暗視野 + スタミナ枯渇
        intimidateCooldownSec: 120.0,
        intimidateRadiusUU: 5000.0,
        intimidateDurationsByTier: { 2: 5.0, 3: 10.0, 4: 15.0 },
        intimidateCameraFadeAlpha: 0.58,
        intimidateCameraFadeTimeSec: 0.45,
        intimidateCameraFadeColorBGRA: 0xFF000000,

        // エネルギー譲渡 (TS_Transfer): 通知なし・効果音のみ・仲間の傀儡のみ指定可能
        transferCooldownSec: 90.0,
        transferDurationSec: 2.5,
        transferChargeByTier: {
            2: 0.205, // TST_ONE   (Tier 1)
            3: 0.555, // TST_TWO   (Tier 2)
            4: 1.000  // TST_THREE (Tier 3 / 最大)
        }
    };

    // ========================================================================
    // 2. 検証済み RVA / オフセット（DreadHungerServer-Win64-Shipping.pdb 照合済）
    // ========================================================================
    var RVA = {
        GUObjectArray: 0x45C40C8,
        GWorld: 0x46ED420,
        FCollisionResponseParams_DefaultResponseParam: 0x46C6EA8,
        FMemory_Realloc: 0x106C3D0,
        FMemory_Free: 0x105DD50,
        FName_CtorWchar: 0x11592A0,
        FName_GetPlainNameString: 0x11604F0,
        FText_FromString: 0x10964E0, // const FString& コピー版（RPC UAF 文字化け防止）
        FText_Dtor: 0x75CB30,

        // 動的 UClass 合成 API（UE 4.26.1 実機検証済）
        CreatePackage: 0x13692D0,
        FStaticConstructObjectParameters_Ctor: 0x13637D0,
        StaticConstructObject_Internal: 0x1378CC0,
        StaticFindObject: 0x137AAA0,
        StaticLoadObject: 0x137B290,
        UObject_GetPrivateStaticClass: 0x137EDD0,
        UClass_GetPrivateStaticClass: 0x11F02E0,
        UBlueprintGeneratedClass_GetPrivateStaticClass: 0x29AF380,
        UClass_SetSuperStruct: 0x12CEFF0,
        UClass_Bind: 0x11EAD00,
        UStruct_StaticLink: 0x12CF5D0,
        UClass_CreateDefaultObject: 0x11EC670,
        UClass_AssembleReferenceTokenStream: 0x12D9830,
        UClass_SetUpRuntimeReplicationData: 0x12CF0B0,
        UBlueprintGeneratedClass_UpdateCustomPropertyListForPostConstruction: 0x23C3C10,

        // DreadHunger / Engine StaticClass 取得
        UDH_TotemSpell_StaticClass: 0xEF7CA0,
        UDH_TotemSpell_Summoning_StaticClass: 0xEFD210,
        ADH_HumanCharacter_GetPrivateStaticClass: 0xEC3DA0,
        ADH_PlayerState_GetPrivateStaticClass: 0xEED6B0,
        ADH_PlayerController_GetPrivateStaticClass: 0xEE2F70,
        ADH_ThrallTotem_GetPrivateStaticClass: 0xEF76F0,
        ADH_PrisonerPawn_GetPrivateStaticClass: 0xEF3D00,
        ADH_NitroInventory_GetPrivateStaticClass: 0xEE4FE0,
        ADH_Warship_GetPrivateStaticClass: 0xF05BF0,
        APawn_GetPrivateStaticClass: 0x2A9E710,

        // GameInstance / GameState
        UDH_GameInstance_Init: 0xD9A200,
        ADH_GameState_HandleMatchHasStarted: 0xD99080,
        ADH_GameState_Tick: 0xDAD420,

        // クライアント RPC 診断（onEnter のみ）
        ADH_PC_SetEquippedSpells_Impl: 0xE4E2B0,
        ADH_PC_CastTotemSpell_Impl: 0xE2F3B0,

        // トーテムスペル制御
        UDH_TotemSpell_IsValidCastingTarget: 0xE756C0,
        UDH_TotemSpell_CanCast: 0xEF99A0,
        UDH_TotemSpell_DoSpellEffects: 0xEFA0E0,
        UDH_TotemSpell_OnCast: 0xEFA550,
        UDH_TotemSpell_OnExpired: 0xEFA590,
        UDH_TotemSpell_GetPlayerCaster: 0xE70040,

        // スペルエネルギー操作
        ADH_SpellManager_ModifySpellChargeLevel: 0xE76070,

        // 船体爆破 (TS_BoomShip)
        UDH_GameplayStatics_GetWarship: 0xE91C00,
        ADH_Warship_SpawnBreaches: 0xE84F90,

        // テレポート (TS_Teleport)
        AActor_K2_TeleportTo: 0x22E14B0,
        AActor_K2_SetActorRotation: 0x22E1260,
        ADH_PrisonerPawn_GetBrig: 0xE6B350,
        ADH_BrigCell_FreePrisoner: 0xE69380,
        ADH_DestructibleActor_BreakApart: 0xDBFB20,
        ADH_HumanCharacter_GetSelectedInventory: 0xD59620,
        ADH_Inventory_SetDurability: 0xDE2830,
        ADH_Inventory_DecreaseItemStack: 0xDC56D0,
        ADH_Character_SetCurrentHealth: 0xD6E5C0,

        // 爆弾ネズミ (TS_BombMice) & 純JS NavMesh 経路探索
        ADH_Character_OnDied: 0xEA11C0,
        ADH_AnimalCharacter_TakeDamage: 0xD73410,
        ADH_AnimalCharacter_Destroyed: 0xD4FDA0,
        UDH_TotemSpell_Summoning_SpawnAI: 0xE84190,
        AAIController_GetPrivateStaticClass: 0x2DE69C0,
        AAIController_MoveToActor: 0x2D70720,
        AAIController_MoveToLocation: 0x2D708B0,
        AAIController_StopMovement: 0x2D7A130,
        AAIController_GetMoveStatus: 0x2D6BFB0,
        AAIController_HasPartialPath: 0x2D6D670,
        UNavigationSystemV1_K2_ProjectPointToNavigation: 0x2C91A40,
        UNavigationSystemV1_K2_GetRandomReachablePointInRadius: 0x2C91890,
        ADH_Character_SetShouldRun: 0xD71200,
        ADH_AnimalCharacter_SetTargetCharacter: 0xD71930,
        UCharacterMovementComponent_SetMovementMode: 0x242D7D0,
        UCharacterMovementComponent_SetWalkableFloorAngle: 0x242E4E0,
        UCharacterMovementComponent_IsWalkable: 0x241D1E0,
        UCharacterMovementComponent_K2_FindFloor: 0x241D5A0,
        FCollisionQueryParams_Ctor: 0x23C7DE0,
        FCollisionQueryParams_AddIgnoredActor: 0x23CA2D0,
        UWorld_LineTraceSingleByChannel: 0x2400340,
        IDH_AITarget_Execute_IsAITargetable: 0xE9AC00,
        FActorSpawnParameters_Ctor: 0x29584A0,
        UWorld_SpawnActor: 0x2624510,
        UGameplayStatics_BeginDeferredActorSpawnFromClass: 0x25570C0,
        UGameplayStatics_FinishSpawningActor: 0x255FCF0,
        UWorld_DestroyActor: 0x2616240,

        // キャラクター／霊界／カメラフェード／スタミナ／タイマー制御
        ADH_Character_IsAlive: 0xD5C850,
        ADH_HumanCharacter_SetIsSpiritWalking: 0xD70310,
        ADH_HumanCharacter_ModifyStamina: 0xD5E0D0,
        APlayerController_ClientSetCameraFade: 0x2AA7C70,
        AActor_GetWorldTimerManager: 0x22DDC30,
        FTimerManager_FindTimer: 0x28F2D80,
        FTimerManager_InternalClearTimer: 0x28F6A00,

        // インベントリ強制丸腰制御（威嚇用）
        UDH_InventoryManager_HolsterInventory_Impl: 0xDD2450,
        UDH_InventoryManager_SelectInventory_Impl: 0xDE20C0,
        UDH_InventoryManager_UseInventory_Impl: 0xDEABB0,
        UDH_InventoryManager_AimInventory_Impl: 0xDBD3D0,
        UDH_InventoryManager_ReloadInventory_Impl: 0xDDE3A0
    };

    var OFFSETS = {
        UObject_InternalIndex: 0x0C,
        UObject_ClassPrivate: 0x10,
        UObject_NamePrivate: 0x18,
        UStruct_ChainArray: 0x30,
        UStruct_ChainDepth: 0x38,
        UClass_ClassWithin: 0xD8,
        UClass_ClassConfigName: 0xE8,
        UClass_ClassDefaultObject: 0x118,

        AActor_Owner: 0xE0,
        AActor_Instigator: 0x118,
        AActor_RootComponent: 0x130,
        UActorComponent_OwnerPrivate: 0xA0,
        USceneComponent_AttachParent: 0xC0,
        USceneComponent_ComponentToWorldQuat: 0x1C0,
        USceneComponent_ComponentToWorldTrans: 0x1D0,
        USceneComponent_ComponentToWorldScale: 0x1E0,
        UStaticMeshComponent_StaticMesh: 0x488,

        ACharacter_CharacterMovement: 0x288,
        ACharacter_BasedMovement: 0x298,
        FHitResult_Distance: 0x0C,
        FHitResult_ImpactNormal: 0x34,
        FHitResult_Actor: 0x6C,
        FHitResult_Component: 0x74,
        FFindFloorResult_FloorDist: 0x04,
        FFindFloorResult_HitNormalZ: 0x48,
        FFindFloorResult_HitActor: 0x78,
        FFindFloorResult_HitComponent: 0x80,
        UMovementComponent_Velocity: 0xC4,
        UNavMovementComponent_NavAgentProps_Flags: 0xF0,
        UNavMovementComponent_Flags: 0x124,
        UNavMovementComponent_MovementState: 0x125,
        UCharacterMovementComponent_MaxStepHeight: 0x154,
        UCharacterMovementComponent_JumpZVelocity: 0x158,
        UCharacterMovementComponent_WalkableFloorAngle: 0x160,
        UCharacterMovementComponent_WalkableFloorZ: 0x164,
        UCharacterMovementComponent_MovementMode: 0x168,
        UCharacterMovementComponent_MaxWalkSpeed: 0x18C,
        UCharacterMovementComponent_MaxWalkSpeedCrouched: 0x190,
        UCharacterMovementComponent_MaxSwimSpeed: 0x194,
        UCharacterMovementComponent_MaxFlySpeed: 0x198,
        UCharacterMovementComponent_MaxAcceleration: 0x1A0,
        UCharacterMovementComponent_BrakingDecelerationFlying: 0x1C0,
        UCharacterMovementComponent_RotationRate: 0x1E4,
        UCharacterMovementComponent_OrientFlags: 0x1F0,
        UDH_CharacterMovementComponent_AutoTreadDepthSpeed: 0xB30,
        UDH_CharacterMovementComponent_bCanMoveUnderWater: 0xB34,
        UDH_CharacterMovementComponent_bAutoStepUpOutOfWater: 0xB35,
        UDH_CharacterMovementComponent_bAlwaysAutoStepOutOfWater: 0xB36,
        UDH_CharacterMovementComponent_MaxSwimDepth: 0xB38,

        APawn_AutoPossessAI: 0x231,
        APawn_AIControllerClass: 0x238,
        APawn_Controller: 0x258,

        ADH_Character_IdleSpeed: 0x6F0,
        ADH_Character_RunSpeed: 0x6F4,
        ADH_Character_MaximumHealth: 0x7E0,
        ADH_Character_bShouldRun: 0x940,
        ADH_Character_CurrentHealth: 0x94C,
        ADH_Character_TeamID: 0x955,
        ADH_AnimalCharacter_StuckSpeed: 0x9B0,
        ADH_AnimalCharacter_StuckTime: 0x9B4,
        ADH_AnimalCharacter_StuckTimer: 0xA4C,

        ADH_ExplosionActor_Damage: 0x228,
        ADH_ExplosionActor_bDamageOnSpawn: 0x230,
        ADH_ExplosionActor_DamageType: 0x238,
        ADH_ExplosionActor_OuterRadius: 0x240,
        ADH_ExplosionActor_InnerRadius: 0x254,

        UWorld_OwningGameInstance: 0x180,
        UWorld_TimeSeconds: 0x598,

        UDH_GameInstance_ThrallSpells: 0x440,
        ADH_GameState_PlayerArray: 0x238,
        ADH_GameState_Warship: 0x2B0,

        APawn_PlayerState: 0x240,
        ADH_PlayerController_PlayerState: 0x228,
        ADH_PlayerController_Pawn: 0x250,
        ADH_PlayerController_ControlledHuman: 0x588,

        ADH_PlayerState_SpellManager: 0x488,
        ADH_PlayerState_bIsDead: 0x570,
        ADH_PlayerState_bIsDisconnected: 0x571,
        ADH_PlayerState_bIsThrall: 0x572,
        ADH_PlayerState_OtherThralls: 0x578,

        ADH_HumanCharacter_InventoryComponent: 0x808,
        ADH_HumanCharacter_ShipBase: 0x9F8,
        ADH_HumanCharacter_CurrentStamina: 0xA50,
        ADH_HumanCharacter_bSpiritWalking: 0xC71,
        ADH_HumanCharacter_SpiritWalkTimerHandle: 0xD50,

        UDH_InventoryManager_HumanOwner: 0xD8,
        UDH_InventoryManager_SelectedInventory: 0xF8,
        UDH_InventoryManager_SelectedViewClass: 0x150,

        ADH_SpellManager_SpellTierLevels: 0x230,
        ADH_SpellManager_SpellChargeLevel: 0x284,

        UDH_TotemSpell_FriendlyName: 0x28,
        UDH_TotemSpell_Description: 0x40,
        UDH_TotemSpell_OnlineCode: 0x58,
        UDH_TotemSpell_TargetingType: 0x7A,
        UDH_TotemSpell_Duration: 0x80,
        UDH_TotemSpell_CooldownTime: 0xD4,
        UDH_TotemSpell_ValidCastingTargets: 0xE0,
        UDH_TotemSpell_bAllowCorpseTargets: 0xF0,
        UDH_TotemSpell_bAllowDoppelgangerTargets: 0xF1,
        UDH_TotemSpell_Caster: 0x118,
        UDH_TotemSpell_bDefault: 0x120,
        UDH_TotemSpell_bReleased: 0x121,
        UDH_TotemSpell_CastTier: 0x135,

        UDH_TotemSpell_Summoning_AISpawnQuery: 0x138,
        UDH_TotemSpell_Summoning_AIFallbackQuery: 0x140,
        UDH_TotemSpell_Summoning_SummonedAIClasses: 0x148,
        UDH_TotemSpell_Summoning_AIControllerClass: 0x158,
        UDH_TotemSpell_Summoning_SummonedAICount: 0x160,
        UDH_TotemSpell_Summoning_SummonRetryTime: 0x1B0,
        UDH_TotemSpell_Summoning_SummonedAIs: 0x1B8,
        UDH_TotemSpell_Summoning_CurrentCastingTarget: 0x1C8
    };

    var VANILLA_PATHS = {
        TS_CannibalAttack: '/Game/Blueprints/Game/Totems/TS_CannibalAttack.TS_CannibalAttack_C',
        BP_Totem: '/Game/Blueprints/Game/Totems/BP_Totem.BP_Totem_C',
        BP_Prey_Rat: '/Game/Blueprints/AI/Prey/BP_Prey_Rat.BP_Prey_Rat_C',
        BP_Explosion_Gunpowder_OffShip: '/Game/Blueprints/Game/BP_Explosion_Gunpowder_OffShip.BP_Explosion_Gunpowder_OffShip_C',
        DT_CannibalBite: '/Game/Blueprints/Game/DamageTypes/DT_CannibalBite.DT_CannibalBite_C'
    };

    // 合成対象 5 スペル（クライアント側 BP_DreadGameInstance.ThrallSpells [5..9] と完全一致する順序）
    var CUSTOM_SPELL_SPECS = [
        {
            packagePath: '/Game/Mods/TotemSpells/BombMice/TS_BombMice',
            className: 'TS_BombMice_C',
            isSummoning: true,
            friendlyName: '爆弾ネズミ',
            description: '近づくと爆発するネズミの群れを召喚する',
            onlineCode: 'BombMice',
            targetingType: 1, // STS_TARGET
            cooldownTime: CONFIG.bombMiceCooldownSec,
            durationMap: { 1: 0.0, 2: CONFIG.bombMiceDurationSec, 3: CONFIG.bombMiceDurationSec, 4: CONFIG.bombMiceDurationSec },
            allowCorpse: 0,
            allowDoppelganger: 0
        },
        {
            packagePath: '/Game/Mods/TotemSpells/BoomShip/TS_BoomShip',
            className: 'TS_BoomShip_C',
            isSummoning: false,
            friendlyName: '船体爆破',
            description: '船の周囲に爆発を起こし船体に損傷を与える',
            onlineCode: 'BoomShip',
            targetingType: 3, // STS_CASTER
            cooldownTime: CONFIG.boomShipCooldownSec,
            durationMap: {
                1: 0.0,
                2: CONFIG.boomShipDurationsByTier[2],
                3: CONFIG.boomShipDurationsByTier[3],
                4: CONFIG.boomShipDurationsByTier[4]
            },
            allowCorpse: 0,
            allowDoppelganger: 0
        },
        {
            packagePath: '/Game/Mods/TotemSpells/Teleport/TS_Teleport',
            className: 'TS_Teleport_C',
            isSummoning: false,
            friendlyName: 'テレポート',
            description: '指定したトーテムの位置へ瞬間移動する',
            onlineCode: 'Teleport',
            targetingType: 1, // STS_TARGET
            cooldownTime: CONFIG.teleportCooldownSec,
            durationMap: { 1: 0.0, 2: CONFIG.teleportDurationSec, 3: CONFIG.teleportDurationSec, 4: CONFIG.teleportDurationSec },
            allowCorpse: 0,
            allowDoppelganger: 0
        },
        {
            packagePath: '/Game/Mods/TotemSpells/Intimidate/TS_Intimidate',
            className: 'TS_Intimidate_C',
            isSummoning: false,
            friendlyName: '威嚇',
            description: '周囲の船員を恐怖させ、一定時間アイテムを持てなくする',
            onlineCode: 'Intimidate',
            targetingType: 3, // STS_CASTER (霊界同等・即発動)
            cooldownTime: CONFIG.intimidateCooldownSec,
            durationMap: {
                1: 0.0,
                2: CONFIG.intimidateDurationsByTier[2],
                3: CONFIG.intimidateDurationsByTier[3],
                4: CONFIG.intimidateDurationsByTier[4]
            },
            allowCorpse: 0,
            allowDoppelganger: 0
        },
        {
            packagePath: '/Game/Mods/TotemSpells/Transfer/TS_Transfer',
            className: 'TS_Transfer_C',
            isSummoning: false,
            friendlyName: 'エネルギー譲渡',
            description: '自身のエネルギーを段階に応じて指定した仲間の傀儡へ分け与える',
            onlineCode: 'Transfer',
            targetingType: 1, // STS_TARGET (仲間の傀儡限定)
            cooldownTime: CONFIG.transferCooldownSec,
            durationMap: { 1: 0.0, 2: CONFIG.transferDurationSec, 3: CONFIG.transferDurationSec, 4: CONFIG.transferDurationSec },
            allowCorpse: 0,
            allowDoppelganger: 0
        }
    ];

    // ========================================================================
    // 3. ログ & UE4 ネイティブ関数バインディング
    // ========================================================================
    function log(msg) {
        var line = '[CustomSpells] ' + msg;
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

    var GUObjectArray_ptr = moduleBase.add(RVA.GUObjectArray);
    var DefaultResponseParam_ptr = moduleBase.add(RVA.FCollisionResponseParams_DefaultResponseParam);
    var WALKABLE_SLOPE_COS_78 = Math.cos(78.0 * Math.PI / 180.0);
    var WALKABLE_RECOVER_COS_75 = Math.cos(75.0 * Math.PI / 180.0);
    var FMemory_Realloc = new NativeFunction(moduleBase.add(RVA.FMemory_Realloc), 'pointer', ['pointer', 'uint64', 'uint32'], { abi: 'win64', exceptions: 'propagate' });
    var FMemory_Free = new NativeFunction(moduleBase.add(RVA.FMemory_Free), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FName_CtorWchar = new NativeFunction(moduleBase.add(RVA.FName_CtorWchar), 'void', ['pointer', 'pointer', 'int32'], { abi: 'win64', exceptions: 'propagate' });
    var FName_GetPlainNameString = new NativeFunction(moduleBase.add(RVA.FName_GetPlainNameString), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FText_FromString = new NativeFunction(moduleBase.add(RVA.FText_FromString), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FText_Dtor = new NativeFunction(moduleBase.add(RVA.FText_Dtor), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });

    var CreatePackage = new NativeFunction(moduleBase.add(RVA.CreatePackage), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FStaticConstructObjectParameters_Ctor = new NativeFunction(moduleBase.add(RVA.FStaticConstructObjectParameters_Ctor), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var StaticConstructObject_Internal = new NativeFunction(moduleBase.add(RVA.StaticConstructObject_Internal), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var StaticFindObject = new NativeFunction(moduleBase.add(RVA.StaticFindObject), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], { abi: 'win64', exceptions: 'propagate' });
    var StaticLoadObject = new NativeFunction(moduleBase.add(RVA.StaticLoadObject), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UObject_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.UObject_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var UClass_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.UClass_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var UBlueprintGeneratedClass_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.UBlueprintGeneratedClass_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var UClass_SetSuperStruct = new NativeFunction(moduleBase.add(RVA.UClass_SetSuperStruct), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UClass_Bind = new NativeFunction(moduleBase.add(RVA.UClass_Bind), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UStruct_StaticLink = new NativeFunction(moduleBase.add(RVA.UStruct_StaticLink), 'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var UClass_CreateDefaultObject = new NativeFunction(moduleBase.add(RVA.UClass_CreateDefaultObject), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UClass_AssembleReferenceTokenStream = new NativeFunction(moduleBase.add(RVA.UClass_AssembleReferenceTokenStream), 'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var UClass_SetUpRuntimeReplicationData = new NativeFunction(moduleBase.add(RVA.UClass_SetUpRuntimeReplicationData), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UBlueprintGeneratedClass_UpdateCustomPropertyListForPostConstruction = new NativeFunction(
        moduleBase.add(RVA.UBlueprintGeneratedClass_UpdateCustomPropertyListForPostConstruction),
        'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
    );

    var UDH_TotemSpell_StaticClass = new NativeFunction(moduleBase.add(RVA.UDH_TotemSpell_StaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var UDH_TotemSpell_Summoning_StaticClass = new NativeFunction(moduleBase.add(RVA.UDH_TotemSpell_Summoning_StaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_HumanCharacter_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_HumanCharacter_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_PlayerState_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerState_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_PlayerController_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PlayerController_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_ThrallTotem_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_ThrallTotem_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_PrisonerPawn_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_PrisonerPawn_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_NitroInventory_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_NitroInventory_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Warship_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.ADH_Warship_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var APawn_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.APawn_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });

    var UDH_TotemSpell_GetPlayerCaster = new NativeFunction(moduleBase.add(RVA.UDH_TotemSpell_GetPlayerCaster), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_SpellManager_ModifySpellChargeLevel = new NativeFunction(moduleBase.add(RVA.ADH_SpellManager_ModifySpellChargeLevel), 'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });

    var UDH_GameplayStatics_GetWarship = new NativeFunction(moduleBase.add(RVA.UDH_GameplayStatics_GetWarship), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Warship_SpawnBreaches = new NativeFunction(moduleBase.add(RVA.ADH_Warship_SpawnBreaches), 'void', ['pointer', 'pointer', 'int32', 'pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });

    var AActor_K2_TeleportTo = new NativeFunction(moduleBase.add(RVA.AActor_K2_TeleportTo), 'uint8', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var AActor_K2_SetActorRotation = new NativeFunction(moduleBase.add(RVA.AActor_K2_SetActorRotation), 'uint8', ['pointer', 'pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_PrisonerPawn_GetBrig = new NativeFunction(moduleBase.add(RVA.ADH_PrisonerPawn_GetBrig), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_BrigCell_FreePrisoner = new NativeFunction(moduleBase.add(RVA.ADH_BrigCell_FreePrisoner), 'uint8', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_DestructibleActor_BreakApart = new NativeFunction(moduleBase.add(RVA.ADH_DestructibleActor_BreakApart), 'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_HumanCharacter_GetSelectedInventory = new NativeFunction(moduleBase.add(RVA.ADH_HumanCharacter_GetSelectedInventory), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Inventory_SetDurability = new NativeFunction(moduleBase.add(RVA.ADH_Inventory_SetDurability), 'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Inventory_DecreaseItemStack = new NativeFunction(moduleBase.add(RVA.ADH_Inventory_DecreaseItemStack), 'void', ['pointer', 'int32', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Character_SetCurrentHealth = new NativeFunction(moduleBase.add(RVA.ADH_Character_SetCurrentHealth), 'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });

    var AAIController_GetPrivateStaticClass = new NativeFunction(moduleBase.add(RVA.AAIController_GetPrivateStaticClass), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
    var AAIController_MoveToActor = new NativeFunction(moduleBase.add(RVA.AAIController_MoveToActor), 'uint8', ['pointer', 'pointer', 'float', 'uint8', 'uint8', 'uint8', 'pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var AAIController_MoveToLocation = new NativeFunction(moduleBase.add(RVA.AAIController_MoveToLocation), 'uint8', ['pointer', 'pointer', 'float', 'uint8', 'uint8', 'uint8', 'uint8', 'pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var AAIController_StopMovement = new NativeFunction(moduleBase.add(RVA.AAIController_StopMovement), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var AAIController_GetMoveStatus = new NativeFunction(moduleBase.add(RVA.AAIController_GetMoveStatus), 'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var AAIController_HasPartialPath = new NativeFunction(moduleBase.add(RVA.AAIController_HasPartialPath), 'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UNavigationSystemV1_K2_ProjectPointToNavigation = new NativeFunction(moduleBase.add(RVA.UNavigationSystemV1_K2_ProjectPointToNavigation), 'uint8', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UNavigationSystemV1_K2_GetRandomReachablePointInRadius = new NativeFunction(moduleBase.add(RVA.UNavigationSystemV1_K2_GetRandomReachablePointInRadius), 'uint8', ['pointer', 'pointer', 'pointer', 'float', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_Character_SetShouldRun = new NativeFunction(moduleBase.add(RVA.ADH_Character_SetShouldRun), 'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_AnimalCharacter_SetTargetCharacter = new NativeFunction(moduleBase.add(RVA.ADH_AnimalCharacter_SetTargetCharacter), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UCharacterMovementComponent_SetMovementMode = new NativeFunction(moduleBase.add(RVA.UCharacterMovementComponent_SetMovementMode), 'void', ['pointer', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var UCharacterMovementComponent_SetWalkableFloorAngle = new NativeFunction(moduleBase.add(RVA.UCharacterMovementComponent_SetWalkableFloorAngle), 'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });
    var UCharacterMovementComponent_K2_FindFloor = new NativeFunction(moduleBase.add(RVA.UCharacterMovementComponent_K2_FindFloor), 'void', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FCollisionQueryParams_Ctor = new NativeFunction(moduleBase.add(RVA.FCollisionQueryParams_Ctor), 'pointer', ['pointer', 'uint64', 'pointer', 'uint8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FCollisionQueryParams_AddIgnoredActor = new NativeFunction(moduleBase.add(RVA.FCollisionQueryParams_AddIgnoredActor), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UWorld_LineTraceSingleByChannel = new NativeFunction(moduleBase.add(RVA.UWorld_LineTraceSingleByChannel), 'uint8', ['pointer', 'pointer', 'pointer', 'pointer', 'uint32', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var IDH_AITarget_Execute_IsAITargetable = new NativeFunction(moduleBase.add(RVA.IDH_AITarget_Execute_IsAITargetable), 'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FActorSpawnParameters_Ctor = new NativeFunction(moduleBase.add(RVA.FActorSpawnParameters_Ctor), 'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UWorld_SpawnActor = new NativeFunction(moduleBase.add(RVA.UWorld_SpawnActor), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UGameplayStatics_BeginDeferredActorSpawnFromClass = new NativeFunction(moduleBase.add(RVA.UGameplayStatics_BeginDeferredActorSpawnFromClass), 'pointer', ['pointer', 'pointer', 'pointer', 'uint8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UGameplayStatics_FinishSpawningActor = new NativeFunction(moduleBase.add(RVA.UGameplayStatics_FinishSpawningActor), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var UWorld_DestroyActor = new NativeFunction(moduleBase.add(RVA.UWorld_DestroyActor), 'uint8', ['pointer', 'pointer', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' });

    // GameThread 専用の再利用可能作業バッファ（毎フレームのヒープ確保を排除）
    var navPointBuf = Memory.alloc(16);
    var navOutBuf = Memory.alloc(16);
    var navExtentBuf = Memory.alloc(16);
    var fadeAlphaBuf = Memory.alloc(8);
    var rotatorBuf = Memory.alloc(16);
    var traceStartBuf = Memory.alloc(16);
    var traceEndBuf = Memory.alloc(16);
    var traceHitBuf = Memory.alloc(0xA0);
    var queryParamsBuf = Memory.alloc(0x80);
    var floorLocBuf = Memory.alloc(16);
    var floorResultBuf = Memory.alloc(0xA0);
    var zeroHitBytes = new Array(0xA0);
    for (var zhi = 0; zhi < 0xA0; zhi++) { zeroHitBytes[zhi] = 0; }

    var ADH_Character_IsAlive = new NativeFunction(moduleBase.add(RVA.ADH_Character_IsAlive), 'uint8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_HumanCharacter_SetIsSpiritWalking = new NativeFunction(moduleBase.add(RVA.ADH_HumanCharacter_SetIsSpiritWalking), 'void', ['pointer', 'int8', 'float'], { abi: 'win64', exceptions: 'propagate' });
    var ADH_HumanCharacter_ModifyStamina = new NativeFunction(moduleBase.add(RVA.ADH_HumanCharacter_ModifyStamina), 'void', ['pointer', 'float'], { abi: 'win64', exceptions: 'propagate' });
    var APlayerController_ClientSetCameraFade = new NativeFunction(moduleBase.add(RVA.APlayerController_ClientSetCameraFade), 'void', ['pointer', 'uint8', 'uint32', 'uint64', 'float', 'uint8', 'uint8'], { abi: 'win64', exceptions: 'propagate' });
    var AActor_GetWorldTimerManager = new NativeFunction(moduleBase.add(RVA.AActor_GetWorldTimerManager), 'pointer', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FTimerManager_FindTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_FindTimer), 'pointer', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
    var FTimerManager_InternalClearTimer = new NativeFunction(moduleBase.add(RVA.FTimerManager_InternalClearTimer), 'void', ['pointer', 'uint64'], { abi: 'win64', exceptions: 'propagate' });

    // GC 防止用トップレベル配列（すべての NativeCallback を常駐保持）
    var pinnedCallbacks = [];

    var cachedClasses = {
        human: ptr(0),
        playerState: ptr(0),
        playerController: ptr(0),
        thrallTotem: ptr(0),
        prisonerPawn: ptr(0),
        nitroInventory: ptr(0),
        pawn: ptr(0),
        bpTotem: ptr(0),
        bpPreyRat: ptr(0),
        bpExplosion: ptr(0),
        dtCannibalBite: ptr(0),
        aiController: ptr(0),
        warship: ptr(0)
    };

    function getWarshipClass() {
        if (cachedClasses.warship.isNull()) { cachedClasses.warship = ADH_Warship_GetPrivateStaticClass(); }
        return cachedClasses.warship;
    }

    function getAiControllerClass() {
        if (cachedClasses.aiController.isNull()) { cachedClasses.aiController = AAIController_GetPrivateStaticClass(); }
        return cachedClasses.aiController;
    }

    function getHumanClass() {
        if (cachedClasses.human.isNull()) { cachedClasses.human = ADH_HumanCharacter_GetPrivateStaticClass(); }
        return cachedClasses.human;
    }
    function getPlayerStateClass() {
        if (cachedClasses.playerState.isNull()) { cachedClasses.playerState = ADH_PlayerState_GetPrivateStaticClass(); }
        return cachedClasses.playerState;
    }
    function getPlayerControllerClass() {
        if (cachedClasses.playerController.isNull()) { cachedClasses.playerController = ADH_PlayerController_GetPrivateStaticClass(); }
        return cachedClasses.playerController;
    }
    function getThrallTotemClass() {
        if (cachedClasses.thrallTotem.isNull()) { cachedClasses.thrallTotem = ADH_ThrallTotem_GetPrivateStaticClass(); }
        return cachedClasses.thrallTotem;
    }
    function getPrisonerPawnClass() {
        if (cachedClasses.prisonerPawn.isNull()) { cachedClasses.prisonerPawn = ADH_PrisonerPawn_GetPrivateStaticClass(); }
        return cachedClasses.prisonerPawn;
    }
    function getNitroInventoryClass() {
        if (cachedClasses.nitroInventory.isNull()) { cachedClasses.nitroInventory = ADH_NitroInventory_GetPrivateStaticClass(); }
        return cachedClasses.nitroInventory;
    }
    function getPawnClass() {
        if (cachedClasses.pawn.isNull()) { cachedClasses.pawn = APawn_GetPrivateStaticClass(); }
        return cachedClasses.pawn;
    }

    function loadVanillaClass(pathStr) {
        var uClassStatic = UClass_GetPrivateStaticClass();
        var buf = Memory.allocUtf16String(pathStr);
        var found = StaticFindObject(uClassStatic, ptr('0xffffffffffffffff'), buf, 0);
        if (found && !found.isNull()) {
            addToRootSet(found);
            return found;
        }
        var loaded = StaticLoadObject(uClassStatic, ptr(0), buf, ptr(0), 0, ptr(0), 1, ptr(0));
        if (loaded && !loaded.isNull()) {
            addToRootSet(loaded);
            return loaded;
        }
        return ptr(0);
    }

    function getBpExplosionClass() {
        if (cachedClasses.bpExplosion.isNull()) {
            cachedClasses.bpExplosion = loadVanillaClass(VANILLA_PATHS.BP_Explosion_Gunpowder_OffShip);
        }
        return cachedClasses.bpExplosion;
    }

    function getDtCannibalBiteClass() {
        if (cachedClasses.dtCannibalBite.isNull()) {
            cachedClasses.dtCannibalBite = loadVanillaClass(VANILLA_PATHS.DT_CannibalBite);
        }
        return cachedClasses.dtCannibalBite;
    }

    // ========================================================================
    // 4. UE4 オブジェクト／コンテナ／文字列／タイマー補助
    // ========================================================================
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
            var withinChunkIndex = internalIndex & 0xFFFF;
            var chunkPtr = objectsPtr.add(chunkIndex * 8).readPointer();
            if (chunkPtr.isNull()) {
                return true;
            }
            var itemPtr = chunkPtr.add(withinChunkIndex * 24);
            if (!itemPtr.readPointer().equals(objPtr)) {
                return true;
            }
            var flags = itemPtr.add(8).readU32();
            return ((flags >>> 29) & 1) !== 0;
        } catch (e) {
            return true;
        }
    }

    function addToRootSet(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return false;
        }
        try {
            var internalIndex = objPtr.add(OFFSETS.UObject_InternalIndex).readS32();
            var numElements = GUObjectArray_ptr.add(0x24).readS32();
            if (internalIndex < 0 || internalIndex >= numElements) {
                return false;
            }
            var objectsPtr = GUObjectArray_ptr.add(0x10).readPointer();
            if (objectsPtr.isNull()) {
                return false;
            }
            var numChunks = GUObjectArray_ptr.add(0x2C).readS32();
            var chunkIndex = internalIndex >> 16;
            if (chunkIndex < 0 || chunkIndex >= numChunks) {
                return false;
            }
            var withinChunkIndex = internalIndex & 0xFFFF;
            var chunkPtr = objectsPtr.add(chunkIndex * 8).readPointer();
            if (chunkPtr.isNull()) {
                return false;
            }
            var itemPtr = chunkPtr.add(withinChunkIndex * 24);
            if (!itemPtr.readPointer().equals(objPtr)) {
                return false;
            }
            var flags = itemPtr.add(8).readU32();
            var nextFlags = (flags | 0x40000000) >>> 0;
            if (nextFlags !== flags) {
                itemPtr.add(8).writeU32(nextFlags);
            }
            return true;
        } catch (e) {
            return false;
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

    function readFString(fstrPtr) {
        if (!fstrPtr || fstrPtr.isNull()) {
            return '';
        }
        var dataPtr = fstrPtr.readPointer();
        var num = fstrPtr.add(8).readS32();
        if (dataPtr.isNull() || num <= 1 || num > 4096) {
            return '';
        }
        return dataPtr.readUtf16String(num - 1);
    }

    function writeOwnedFString(fstrPtr, text) {
        var oldData = fstrPtr.readPointer();
        if (!oldData.isNull()) {
            FMemory_Free(oldData);
        }
        var count = text.length + 1;
        var mem = FMemory_Realloc(ptr(0), uint64(count * 2), 2);
        if (mem.isNull()) {
            fstrPtr.writePointer(ptr(0));
            fstrPtr.add(8).writeS32(0);
            fstrPtr.add(12).writeS32(0);
            return;
        }
        mem.writeUtf16String(text);
        fstrPtr.writePointer(mem);
        fstrPtr.add(8).writeS32(count);
        fstrPtr.add(12).writeS32(count);
    }

    function writeOwnedFText(ftextPtr, text) {
        FText_Dtor(ftextPtr);
        ftextPtr.writeU64(0);
        ftextPtr.add(8).writeU64(0);
        ftextPtr.add(16).writeU64(0);

        var cap = text.length + 3;
        var fStringPtr = Memory.alloc(16 + cap * 3);
        var bufPtr = fStringPtr.add(16);
        fStringPtr.writePointer(bufPtr);
        fStringPtr.add(8).writeU32(text.length + 1);
        fStringPtr.add(12).writeU32(cap);
        bufPtr.writeUtf16String(text);
        FText_FromString(ftextPtr, fStringPtr);
    }

    // UE4 TMap<uint8, float|int32> (0x50 バイト) を FMemory 上にネイティブ互換で構築
    function writeTierTMap(mapPtr, entriesObj, isIntValue) {
        var keys = Object.keys(entriesObj);
        var count = keys.length;
        if (count === 0 || count > 8) {
            return;
        }
        var elemBytes = count * 16;
        var elemMem = FMemory_Realloc(ptr(0), uint64(elemBytes), 8);
        var hashSize = 8;
        var hashMem = FMemory_Realloc(ptr(0), uint64(hashSize * 4), 4);
        if (elemMem.isNull() || hashMem.isNull()) {
            return;
        }
        for (var h = 0; h < hashSize; h++) {
            hashMem.add(h * 4).writeS32(-1);
        }
        var allocBitMask = 0;
        for (var i = 0; i < count; i++) {
            var k = parseInt(keys[i], 10) & 0xFF;
            var v = entriesObj[keys[i]];
            var bucket = k & (hashSize - 1);
            var prevHead = hashMem.add(bucket * 4).readS32();
            var slot = elemMem.add(i * 16);
            slot.writeU64(0);
            slot.add(8).writeU64(0);
            slot.writeU8(k);
            if (isIntValue) {
                slot.add(4).writeS32(v | 0);
            } else {
                slot.add(4).writeFloat(v);
            }
            slot.add(8).writeS32(prevHead); // HashNextId
            slot.add(12).writeS32(bucket);  // HashIndex
            hashMem.add(bucket * 4).writeS32(i);
            allocBitMask = (allocBitMask | (1 << i)) >>> 0;
        }

        // TSparseArray.Elements (TArray<FSetElement>)
        mapPtr.writePointer(elemMem);
        mapPtr.add(0x08).writeS32(count);
        mapPtr.add(0x0C).writeS32(count);
        // TSparseArray.AllocationFlags (TBitArray: 4 inline uint32 + SecondaryData + NumBits + MaxBits)
        mapPtr.add(0x10).writeU32(allocBitMask);
        mapPtr.add(0x14).writeU32(0);
        mapPtr.add(0x18).writeU32(0);
        mapPtr.add(0x1C).writeU32(0);
        mapPtr.add(0x20).writePointer(ptr(0));
        mapPtr.add(0x28).writeS32(count);
        mapPtr.add(0x2C).writeS32(128);
        // FirstFreeIndex / NumFreeIndices
        mapPtr.add(0x30).writeS32(-1);
        mapPtr.add(0x34).writeS32(0);
        // Hash (TSet HashAllocator: inline 8B + SecondaryData @ +0x40 + HashSize @ +0x48)
        mapPtr.add(0x38).writeU64(0);
        mapPtr.add(0x40).writePointer(hashMem);
        mapPtr.add(0x48).writeS32(hashSize);
        mapPtr.add(0x4C).writeS32(0);
    }

    function writeOwnedPointerTArray(arrPtr, ptrList) {
        var oldData = arrPtr.readPointer();
        if (!oldData.isNull()) {
            FMemory_Free(oldData);
        }
        var n = ptrList.length;
        if (n === 0) {
            arrPtr.writePointer(ptr(0));
            arrPtr.add(8).writeS32(0);
            arrPtr.add(12).writeS32(0);
            return;
        }
        var mem = FMemory_Realloc(ptr(0), uint64(n * 8), 8);
        if (mem.isNull()) {
            return;
        }
        for (var i = 0; i < n; i++) {
            mem.add(i * 8).writePointer(ptrList[i]);
        }
        arrPtr.writePointer(mem);
        arrPtr.add(8).writeS32(n);
        arrPtr.add(12).writeS32(n);
    }

    function readObjectName(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return '';
        }
        var fstr = Memory.alloc(16);
        fstr.writePointer(ptr(0));
        fstr.add(8).writeU32(0);
        fstr.add(12).writeU32(0);
        try {
            FName_GetPlainNameString(objPtr.add(OFFSETS.UObject_NamePrivate), fstr);
            return readFString(fstr);
        } finally {
            var dataPtr = fstr.readPointer();
            if (!dataPtr.isNull()) {
                FMemory_Free(dataPtr);
            }
        }
    }

    function readObjectClassName(objPtr) {
        if (!objPtr || objPtr.isNull()) {
            return '';
        }
        var cls = objPtr.add(OFFSETS.UObject_ClassPrivate).readPointer();
        if (cls.isNull()) {
            return '';
        }
        return readObjectName(cls);
    }

    function getWorldAndTime() {
        var world = moduleBase.add(RVA.GWorld).readPointer();
        if (!world || world.isNull()) {
            return { world: ptr(0), timeSeconds: 0.0 };
        }
        var t = world.add(OFFSETS.UWorld_TimeSeconds).readFloat();
        if (!Number.isFinite(t)) {
            t = 0.0;
        }
        return { world: world, timeSeconds: t };
    }

    function getActorLocation(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return null;
        }
        var rootComp = actorPtr.add(OFFSETS.AActor_RootComponent).readPointer();
        if (!rootComp || rootComp.isNull() || isPendingKill(rootComp)) {
            return null;
        }
        var base = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldTrans);
        var x = base.readFloat();
        var y = base.add(4).readFloat();
        var z = base.add(8).readFloat();
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

    function isHumanAlive(humanPtr) {
        if (!humanPtr || humanPtr.isNull() || isPendingKill(humanPtr)) {
            return false;
        }
        if (!isA(humanPtr, getHumanClass())) {
            return false;
        }
        return (ADH_Character_IsAlive(humanPtr) & 1) !== 0;
    }

    function clearSpiritWalkTimer(humanPtr) {
        if (!humanPtr || humanPtr.isNull() || isPendingKill(humanPtr)) {
            return;
        }
        var timerHandlePtr = humanPtr.add(OFFSETS.ADH_HumanCharacter_SpiritWalkTimerHandle);
        var rawHandle = timerHandlePtr.readU64();
        if (!rawHandle.equals(0)) {
            var timerManager = AActor_GetWorldTimerManager(humanPtr);
            if (!timerManager.isNull()) {
                var timerData = FTimerManager_FindTimer(timerManager, timerHandlePtr);
                if (!timerData.isNull()) {
                    FTimerManager_InternalClearTimer(timerManager, rawHandle);
                }
            }
            timerHandlePtr.writeU64(0);
        }
    }

    function resolvePlayerStateFromActor(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return ptr(0);
        }
        var psClass = getPlayerStateClass();
        if (isA(actorPtr, psClass)) {
            return actorPtr;
        }
        if (isA(actorPtr, getPlayerControllerClass())) {
            var pcPS = actorPtr.add(OFFSETS.ADH_PlayerController_PlayerState).readPointer();
            if (!pcPS.isNull() && !isPendingKill(pcPS) && isA(pcPS, psClass)) {
                return pcPS;
            }
        }
        if (isA(actorPtr, getPawnClass())) {
            var pawnPS = actorPtr.add(OFFSETS.APawn_PlayerState).readPointer();
            if (!pawnPS.isNull() && !isPendingKill(pawnPS) && isA(pawnPS, psClass)) {
                return pawnPS;
            }
        }
        return ptr(0);
    }

    function getControllerFromPlayerState(psPtr) {
        if (!psPtr || psPtr.isNull() || isPendingKill(psPtr)) {
            return ptr(0);
        }
        var owner = psPtr.add(OFFSETS.AActor_Owner).readPointer();
        if (owner.isNull() || isPendingKill(owner)) {
            return ptr(0);
        }
        if (isA(owner, getPlayerControllerClass())) {
            return owner;
        }
        return ptr(0);
    }

    function getHumanFromPlayerState(psPtr) {
        var pc = getControllerFromPlayerState(psPtr);
        if (pc.isNull()) {
            return ptr(0);
        }
        var humanClass = getHumanClass();
        var controlled = pc.add(OFFSETS.ADH_PlayerController_ControlledHuman).readPointer();
        if (!controlled.isNull() && !isPendingKill(controlled) && isA(controlled, humanClass)) {
            return controlled;
        }
        var pawn = pc.add(OFFSETS.ADH_PlayerController_Pawn).readPointer();
        if (!pawn.isNull() && !isPendingKill(pawn) && isA(pawn, humanClass)) {
            return pawn;
        }
        return ptr(0);
    }

    function getCurrentPawnFromPlayerState(psPtr) {
        var pc = getControllerFromPlayerState(psPtr);
        if (pc.isNull()) {
            return ptr(0);
        }
        var pawn = pc.add(OFFSETS.ADH_PlayerController_Pawn).readPointer();
        if (!pawn.isNull() && !isPendingKill(pawn)) {
            return pawn;
        }
        return getHumanFromPlayerState(psPtr);
    }

    function resolveCasterPlayerState(spellPtr, inCasterPtr) {
        var ps = ptr(0);
        if (spellPtr && !spellPtr.isNull()) {
            try {
                ps = UDH_TotemSpell_GetPlayerCaster(spellPtr);
            } catch (e) {
                ps = ptr(0);
            }
            if (ps.isNull()) {
                var rawCaster = spellPtr.add(OFFSETS.UDH_TotemSpell_Caster).readPointer();
                ps = resolvePlayerStateFromActor(rawCaster);
            }
        }
        if (ps.isNull() && inCasterPtr && !inCasterPtr.isNull()) {
            ps = resolvePlayerStateFromActor(inCasterPtr);
        }
        return ps;
    }

    // ========================================================================
    // 5. Option C: 純 JS メモリ合成 UClass & BP_DreadGameInstance 登録
    // ========================================================================
    var synthesizedSpellClasses = {};
    var registeredTargetKeys = {};
    var inSpellRegistration = false;

    function configureSpellCdo(cdo, spec) {
        writeOwnedFText(cdo.add(OFFSETS.UDH_TotemSpell_FriendlyName), spec.friendlyName);
        writeOwnedFText(cdo.add(OFFSETS.UDH_TotemSpell_Description), spec.description);
        writeOwnedFString(cdo.add(OFFSETS.UDH_TotemSpell_OnlineCode), spec.onlineCode);

        cdo.add(OFFSETS.UDH_TotemSpell_TargetingType).writeU8(spec.targetingType);
        writeTierTMap(cdo.add(OFFSETS.UDH_TotemSpell_Duration), spec.durationMap, false);
        cdo.add(OFFSETS.UDH_TotemSpell_CooldownTime).writeFloat(spec.cooldownTime);
        cdo.add(OFFSETS.UDH_TotemSpell_bAllowCorpseTargets).writeU8(spec.allowCorpse ? 1 : 0);
        cdo.add(OFFSETS.UDH_TotemSpell_bAllowDoppelgangerTargets).writeU8(spec.allowDoppelganger ? 1 : 0);
        cdo.add(OFFSETS.UDH_TotemSpell_bDefault).writeU8(0);
        cdo.add(OFFSETS.UDH_TotemSpell_bReleased).writeU8(1);

        if (spec.className === 'TS_BombMice_C') {
            var cannibalCls = loadVanillaClass(VANILLA_PATHS.TS_CannibalAttack);
            if (!cannibalCls.isNull()) {
                var cannibalCdo = cannibalCls.add(OFFSETS.UClass_ClassDefaultObject).readPointer();
                if (!cannibalCdo.isNull()) {
                    cdo.add(OFFSETS.UDH_TotemSpell_Summoning_AISpawnQuery).writePointer(
                        cannibalCdo.add(OFFSETS.UDH_TotemSpell_Summoning_AISpawnQuery).readPointer()
                    );
                    cdo.add(OFFSETS.UDH_TotemSpell_Summoning_AIFallbackQuery).writePointer(
                        cannibalCdo.add(OFFSETS.UDH_TotemSpell_Summoning_AIFallbackQuery).readPointer()
                    );
                    var aiCtrlCls = getAiControllerClass();
                    if (!aiCtrlCls.isNull()) {
                        // 食人鬼 AIController(BP_CannibalController_C) の壁直進/散開/スタック自爆バグを排除し、
                        // 純JS経路探索専用の素の AAIController（UPathFollowingComponent のみ）を割り当てる
                        cdo.add(OFFSETS.UDH_TotemSpell_Summoning_AIControllerClass).writePointer(aiCtrlCls);
                    } else {
                        cdo.add(OFFSETS.UDH_TotemSpell_Summoning_AIControllerClass).writePointer(
                            cannibalCdo.add(OFFSETS.UDH_TotemSpell_Summoning_AIControllerClass).readPointer()
                        );
                    }
                }
            }
            var ratCls = loadVanillaClass(VANILLA_PATHS.BP_Prey_Rat);
            if (!ratCls.isNull()) {
                cachedClasses.bpPreyRat = ratCls;
                captureWildRatCdoDefaultsOnce(ratCls);
                writeOwnedPointerTArray(cdo.add(OFFSETS.UDH_TotemSpell_Summoning_SummonedAIClasses), [ratCls]);
            }
            writeTierTMap(
                cdo.add(OFFSETS.UDH_TotemSpell_Summoning_SummonedAICount),
                {
                    1: 0,
                    2: CONFIG.bombMiceCountByTier[2],
                    3: CONFIG.bombMiceCountByTier[3],
                    4: CONFIG.bombMiceCountByTier[4]
                },
                true
            );
            cdo.add(OFFSETS.UDH_TotemSpell_Summoning_SummonRetryTime).writeFloat(CONFIG.bombMiceSummonRetrySec);
            writeOwnedPointerTArray(cdo.add(OFFSETS.UDH_TotemSpell_ValidCastingTargets), [getHumanClass()]);
        } else if (spec.className === 'TS_Teleport_C') {
            var totemCls = loadVanillaClass(VANILLA_PATHS.BP_Totem);
            if (totemCls.isNull()) {
                totemCls = getThrallTotemClass();
            }
            cachedClasses.bpTotem = totemCls;
            writeOwnedPointerTArray(cdo.add(OFFSETS.UDH_TotemSpell_ValidCastingTargets), [totemCls]);
        } else if (spec.className === 'TS_Transfer_C') {
            writeOwnedPointerTArray(cdo.add(OFFSETS.UDH_TotemSpell_ValidCastingTargets), [getHumanClass()]);
        } else if (spec.className === 'TS_Intimidate_C' || spec.className === 'TS_BoomShip_C') {
            writeOwnedPointerTArray(cdo.add(OFFSETS.UDH_TotemSpell_ValidCastingTargets), []);
        }
    }

    function synthesizeOrGetSpellClass(spec) {
        var cached = synthesizedSpellClasses[spec.className];
        if (cached && !cached.isNull() && !isPendingKill(cached)) {
            return cached;
        }

        var fullPath = spec.packagePath + '.' + spec.className;
        var uClassStatic = UClass_GetPrivateStaticClass();
        var existing = StaticFindObject(uClassStatic, ptr('0xffffffffffffffff'), Memory.allocUtf16String(fullPath), 0);
        if (existing && !existing.isNull() && !isPendingKill(existing)) {
            synthesizedSpellClasses[spec.className] = existing;
            addToRootSet(existing);
            return existing;
        }

        var bpgcStatic = UBlueprintGeneratedClass_GetPrivateStaticClass();
        var uObjectStatic = UObject_GetPrivateStaticClass();
        var superClass = spec.isSummoning ? UDH_TotemSpell_Summoning_StaticClass() : UDH_TotemSpell_StaticClass();
        if (bpgcStatic.isNull() || uObjectStatic.isNull() || superClass.isNull()) {
            log('エラー: 基底 UClass 解決失敗 (' + spec.className + ')');
            return ptr(0);
        }

        var pkg = CreatePackage(Memory.allocUtf16String(spec.packagePath));
        if (pkg.isNull()) {
            log('エラー: CreatePackage 失敗 (' + spec.packagePath + ')');
            return ptr(0);
        }
        addToRootSet(pkg);

        var clsNameBuf = Memory.alloc(8);
        FName_CtorWchar(clsNameBuf, Memory.allocUtf16String(spec.className), 1);

        var params = Memory.alloc(0x60);
        params.writeByteArray(new Array(0x60).fill(0));
        FStaticConstructObjectParameters_Ctor(params, bpgcStatic);
        params.add(0x08).writePointer(pkg);               // Outer
        params.add(0x10).writeU64(clsNameBuf.readU64());  // Name
        params.add(0x18).writeU32(0x00000003);            // RF_Public | RF_Standalone

        var cls = StaticConstructObject_Internal(params);
        if (cls.isNull()) {
            log('エラー: StaticConstructObject_Internal 失敗 (' + spec.className + ')');
            return ptr(0);
        }

        cls.add(OFFSETS.UClass_ClassWithin).writePointer(uObjectStatic);
        var cfgNameBuf = Memory.alloc(8);
        FName_CtorWchar(cfgNameBuf, Memory.allocUtf16String('Engine'), 1);
        cls.add(OFFSETS.UClass_ClassConfigName).writeU64(cfgNameBuf.readU64());

        UClass_SetSuperStruct(cls, superClass);
        UClass_Bind(cls);
        // SetSuperStruct/Bind が基底ネイティブクラスから CLASS_Native (0x800) を引き継ぐため解除し、
        // 実 BlueprintGeneratedClass と同一フラグ (0x00440010) にして StaticConstructObject_Internal の
        // InitProperties が CDO の Duration / CooldownTime / ValidCastingTargets / SummonedAI 等を正しく複製するようにする
        var rawFlags = cls.add(0xCC).readU32();
        cls.add(0xCC).writeU32(((rawFlags & ~0x00000800) | 0x00040010) >>> 0);
        UStruct_StaticLink(cls, 1);

        var cdo = UClass_CreateDefaultObject(cls);
        UClass_AssembleReferenceTokenStream(cls, 1);
        UClass_SetUpRuntimeReplicationData(cls);

        addToRootSet(cls);
        if (!cdo.isNull()) {
            addToRootSet(cdo);
            configureSpellCdo(cdo, spec);
            // CDO の差分プロパティを CustomPropertyList へ登録し、詠唱時に StaticConstructObject_Internal で
            // 生成される各スペルインスタンスへ Duration / CooldownTime / SummonedAI 等をネイティブ複製させる
            UBlueprintGeneratedClass_UpdateCustomPropertyListForPostConstruction(cls);
        }

        synthesizedSpellClasses[spec.className] = cls;
        return cls;
    }

    function appendUniquePointerToTArray(tarrayPtr, elemPtr) {
        var dataPtr = tarrayPtr.readPointer();
        var num = tarrayPtr.add(8).readS32();
        var max = tarrayPtr.add(12).readS32();
        if (num < 0 || max < 0 || num > 256 || max > 256) {
            return { added: false, reason: 'bounds(' + num + '/' + max + ')' };
        }
        for (var i = 0; i < num; i++) {
            var cur = dataPtr.add(i * 8).readPointer();
            if (cur.equals(elemPtr)) {
                return { added: false, reason: 'exists@' + i };
            }
        }
        if (num >= max) {
            var newMax = max < 16 ? 16 : max * 2;
            var newData = FMemory_Realloc(dataPtr, uint64(newMax * 8), 8);
            if (newData.isNull()) {
                return { added: false, reason: 'realloc_failed' };
            }
            dataPtr = newData;
            tarrayPtr.writePointer(dataPtr);
            tarrayPtr.add(12).writeS32(newMax);
        }
        dataPtr.add(num * 8).writePointer(elemPtr);
        tarrayPtr.add(8).writeS32(num + 1);
        return { added: true, index: num, newNum: num + 1 };
    }

    function formatSpellArraySummary(tarrayPtr) {
        var dataPtr = tarrayPtr.readPointer();
        var num = tarrayPtr.add(8).readS32();
        if (dataPtr.isNull() || num <= 0 || num > 64) {
            return '[]';
        }
        var parts = [];
        for (var i = 0; i < num; i++) {
            var elem = dataPtr.add(i * 8).readPointer();
            parts.push('[' + i + ']=' + (elem.isNull() ? 'null' : readObjectName(elem)));
        }
        return parts.join(', ');
    }

    function registerAllCustomSpellsIntoTarget(targetPtr, sourceTag) {
        if (!targetPtr || targetPtr.isNull() || inSpellRegistration) {
            return;
        }
        var key = targetPtr.toString();
        var thrallSpellsPtr = targetPtr.add(OFFSETS.UDH_GameInstance_ThrallSpells);
        var beforeNum = thrallSpellsPtr.add(8).readS32();
        if (registeredTargetKeys[key] && beforeNum >= 5 + CUSTOM_SPELL_SPECS.length) {
            return;
        }

        inSpellRegistration = true;
        try {
            log('スペル合成・登録開始 [' + sourceTag + ' @ ' + targetPtr + '] 既存 ThrallSpells=' + beforeNum);
            var okCount = 0;
            for (var i = 0; i < CUSTOM_SPELL_SPECS.length; i++) {
                var spec = CUSTOM_SPELL_SPECS[i];
                try {
                    var clsPtr = synthesizeOrGetSpellClass(spec);
                    if (!clsPtr || clsPtr.isNull()) {
                        log('  [FAIL] 合成失敗: ' + spec.className);
                        continue;
                    }
                    var res = appendUniquePointerToTArray(thrallSpellsPtr, clsPtr);
                    okCount++;
                    log('  [OK] ' + spec.className + ' @ ' + clsPtr + ' (OnlineCode=' + spec.onlineCode + ', TT=' + spec.targetingType + ', array=' + (res.added ? ('added@' + res.index) : res.reason) + ')');
                } catch (e) {
                    log('  [ERR] ' + spec.className + ': ' + e.message);
                }
            }

            var afterNum = thrallSpellsPtr.add(8).readS32();
            if (okCount === CUSTOM_SPELL_SPECS.length) {
                registeredTargetKeys[key] = true;
            }
            log('スペル合成・登録完了 [' + sourceTag + ']: 成功=' + okCount + '/' + CUSTOM_SPELL_SPECS.length + ', ThrallSpells=' + beforeNum + ' -> ' + afterNum);
            log('ThrallSpells 一覧 [' + sourceTag + ']: ' + formatSpellArraySummary(thrallSpellsPtr));
        } finally {
            inSpellRegistration = false;
        }
    }

    function registerLiveAndCdoGameInstance(giPtr, sourceTag) {
        if (!giPtr || giPtr.isNull()) {
            return;
        }
        try {
            var giClass = giPtr.add(OFFSETS.UObject_ClassPrivate).readPointer();
            if (!giClass.isNull()) {
                var giCdo = giClass.add(OFFSETS.UClass_ClassDefaultObject).readPointer();
                if (!giCdo.isNull() && !giCdo.equals(giPtr)) {
                    registerAllCustomSpellsIntoTarget(giCdo, sourceTag + ':CDO');
                }
            }
        } catch (e) {}
        registerAllCustomSpellsIntoTarget(giPtr, sourceTag + ':LIVE');
    }

    function installGameInstanceRegistrationHooks() {
        // 1) Primary: UDH_GameInstance::Init 完了直後に CDO・LIVE 双方へ [5..9] を合成登録
        Interceptor.attach(moduleBase.add(RVA.UDH_GameInstance_Init), {
            onEnter: function (args) {
                this.gi = args[0];
            },
            onLeave: function () {
                try {
                    if (this.gi && !this.gi.isNull()) {
                        registerLiveAndCdoGameInstance(this.gi, 'UDH_GameInstance::Init');
                    }
                } catch (e) {
                    log('Init 登録例外: ' + e.message);
                }
            }
        });

        // 2) Fallback: HandleMatchHasStarted（onEnter のみ・onLeave 消費 0）
        Interceptor.attach(moduleBase.add(RVA.ADH_GameState_HandleMatchHasStarted), {
            onEnter: function () {
                try {
                    var wt = getWorldAndTime();
                    if (wt.world.isNull()) {
                        return;
                    }
                    var gi = wt.world.add(OFFSETS.UWorld_OwningGameInstance).readPointer();
                    if (gi && !gi.isNull()) {
                        registerLiveAndCdoGameInstance(gi, 'HandleMatchHasStarted');
                    }
                } catch (e) {}
            }
        });

        // 3) クライアント装備／詠唱 RPC 診断ログ（onEnter のみ・onLeave 消費 0）
        Interceptor.attach(moduleBase.add(RVA.ADH_PC_SetEquippedSpells_Impl), {
            onEnter: function (args) {
                try {
                    var arrPtr = args[1];
                    if (!arrPtr || arrPtr.isNull()) {
                        return;
                    }
                    var dataPtr = arrPtr.readPointer();
                    var num = arrPtr.add(8).readS32();
                    if (num < 0 || num > 16) {
                        return;
                    }
                    var names = [];
                    for (var i = 0; i < num; i++) {
                        var sp = dataPtr.isNull() ? ptr(0) : dataPtr.add(i * 8).readPointer();
                        names.push(sp.isNull() ? 'null' : readObjectName(sp));
                    }
                    log('[RPC SetEquippedSpells] pc=' + args[0] + ' count=' + num + ' -> [' + names.join(', ') + ']');
                } catch (e) {}
            }
        });

        Interceptor.attach(moduleBase.add(RVA.ADH_PC_CastTotemSpell_Impl), {
            onEnter: function (args) {
                try {
                    var spellCls = args[1];
                    var clsName = (!spellCls || spellCls.isNull()) ? 'null' : readObjectName(spellCls);
                    log('[RPC CastTotemSpell] pc=' + args[0] + ' spell=' + clsName + ' target=' + args[2]);
                } catch (e) {}
            }
        });
    }

    // ========================================================================
    // 6. ランタイム状態（GameThread Tick 管理・マップ遷移時に自動初期化）
    // ========================================================================
    var trackedWorldKey = '';
    var currentGameState = ptr(0);

    // 威嚇 (TS_Intimidate): humanKey -> { human, invMgr, expireTime }
    var intimidatedHumans = {};

    // テレポート (TS_Teleport): spellKey -> { totem, casterPS, castTier } / humanKey -> { human, exitAt }
    var activeTeleportTargets = {};
    var teleportSpiritWalkMap = {};

    // 爆弾ネズミ (TS_BombMice):
    //   - activeBombMiceSpells: spellKey -> { spell, instigatorHuman }
    //   - trackedBombMice: ptrKey -> 個体ごとの独立状態（純JS NavMesh 経路探索 + 接近0.4s起爆）
    //   - finishedBombMouseIds: 起爆・消滅済みの個体識別子（SummonedAIs 残存やポインタ再利用による誤追跡を完全遮断）
    //   - pendingDestroyControllers: 破棄済み爆弾ネズミに紐づいていた専用 AAIController の遅延破棄キュー
    //   - wildRatCdoDefaults: 原版 BP_Prey_Rat CDO の不変スナップショット（野生ネズミへの波及を防止）
    var activeBombMiceSpells = {};
    var trackedBombMice = {};
    var trackedBombMouseMoveComps = {};
    var finishedBombMouseIds = {};
    var wildRatCdoDefaults = null;
    var ratCdoSwapDepth = 0;
    var inBombMouseDetonation = 0;
    var pendingBombMouseDetonations = [];
    var pendingDestroyControllers = [];
    var gangwayCache = {};
    var gangwayCacheCount = 0;
    var shipCache = {
        warship: ptr(0),
        valid: false,
        qx: 0.0, qy: 0.0, qz: 0.0, qw: 1.0,
        tx: 0.0, ty: 0.0, tz: 0.0,
        sx: 1.0, sy: 1.0, sz: 1.0
    };

    function packFVector2D(fromFloat, toFloat) {
        fadeAlphaBuf.writeFloat(fromFloat);
        fadeAlphaBuf.add(4).writeFloat(toFloat);
        return fadeAlphaBuf.readU64();
    }

    function resolvePlayerControllerForIntimidate(entry) {
        if (!entry) {
            return ptr(0);
        }
        if (entry.pc && !entry.pc.isNull() && !isPendingKill(entry.pc)) {
            return entry.pc;
        }
        if (entry.ps && !entry.ps.isNull() && !isPendingKill(entry.ps)) {
            var fromPs = getControllerFromPlayerState(entry.ps);
            if (!fromPs.isNull() && !isPendingKill(fromPs)) {
                return fromPs;
            }
        }
        if (entry.human && !entry.human.isNull() && !isPendingKill(entry.human)) {
            var ctrl = entry.human.add(OFFSETS.APawn_Controller).readPointer();
            if (!ctrl.isNull() && !isPendingKill(ctrl) && isA(ctrl, getPlayerControllerClass())) {
                return ctrl;
            }
        }
        return ptr(0);
    }

    function applyIntimidateCameraFade(pcPtr, enableFading, toAlpha, holdWhenFinished) {
        if (!pcPtr || pcPtr.isNull() || isPendingKill(pcPtr)) {
            return;
        }
        try {
            APlayerController_ClientSetCameraFade(
                pcPtr,
                enableFading ? 1 : 0,
                CONFIG.intimidateCameraFadeColorBGRA,
                packFVector2D(-1.0, toAlpha),
                CONFIG.intimidateCameraFadeTimeSec,
                0, // bFadeAudio = false
                holdWhenFinished ? 1 : 0
            );
        } catch (e) {}
    }

    function clearIntimidateEntry(hKey, entry) {
        delete intimidatedHumans[hKey];
        if (!entry) {
            return;
        }
        var pc = resolvePlayerControllerForIntimidate(entry);
        if (!pc.isNull()) {
            applyIntimidateCameraFade(pc, true, 0.0, false);
        }
    }

    function resetRuntimeStateIfWorldChanged(worldPtr) {
        var key = worldPtr && !worldPtr.isNull() ? worldPtr.toString() : '';
        if (key !== trackedWorldKey) {
            trackedWorldKey = key;
            var oldIntKeys = Object.keys(intimidatedHumans);
            for (var oi = 0; oi < oldIntKeys.length; oi++) {
                clearIntimidateEntry(oldIntKeys[oi], intimidatedHumans[oldIntKeys[oi]]);
            }
            var oldMouseKeys = Object.keys(trackedBombMice);
            for (var om = 0; om < oldMouseKeys.length; om++) {
                var oldMouse = trackedBombMice[oldMouseKeys[om]];
                if (oldMouse && oldMouse.climbPhase !== 0) {
                    exitBombMouseWallClimb(oldMouse, oldMouse.controller, 0.0);
                }
            }
            intimidatedHumans = {};
            activeTeleportTargets = {};
            teleportSpiritWalkMap = {};
            activeBombMiceSpells = {};
            trackedBombMice = {};
            trackedBombMouseMoveComps = {};
            finishedBombMouseIds = {};
            pendingBombMouseDetonations = [];
            pendingDestroyControllers = [];
            gangwayCache = {};
            gangwayCacheCount = 0;
            shipCache.valid = false;
            shipCache.warship = ptr(0);
            if (ratCdoSwapDepth === 0) {
                restoreWildRatCdoDefaults();
            }
        }
    }

    function getAllPlayerStates() {
        var result = [];
        if (currentGameState.isNull() || isPendingKill(currentGameState)) {
            return result;
        }
        var paPtr = currentGameState.add(OFFSETS.ADH_GameState_PlayerArray);
        var dataPtr = paPtr.readPointer();
        var num = paPtr.add(8).readS32();
        if (dataPtr.isNull() || num <= 0 || num > 64) {
            return result;
        }
        var psClass = getPlayerStateClass();
        for (var i = 0; i < num; i++) {
            var ps = dataPtr.add(i * 8).readPointer();
            if (!ps.isNull() && !isPendingKill(ps) && isA(ps, psClass)) {
                result.push(ps);
            }
        }
        return result;
    }

    function isHumanIntimidated(humanPtr) {
        if (!humanPtr || humanPtr.isNull()) {
            return false;
        }
        var key = humanPtr.toString();
        var entry = intimidatedHumans[key];
        if (!entry) {
            return false;
        }
        var wt = getWorldAndTime();
        if (wt.world.isNull() || wt.world.toString() !== trackedWorldKey) {
            clearIntimidateEntry(key, entry);
            return false;
        }
        if (wt.timeSeconds >= entry.expireTime || isPendingKill(entry.human)) {
            clearIntimidateEntry(key, entry);
            return false;
        }
        return true;
    }

    // ========================================================================
    // 7. インベントリ強制丸腰フック（威嚇 TS_Intimidate 用）
    // ========================================================================
    var origHolsterInventory = null;
    var origSelectInventory = null;
    var origUseInventory = null;
    var origAimInventory = null;
    var origReloadInventory = null;

    function getInvHumanOwner(invMgrPtr) {
        if (!invMgrPtr || invMgrPtr.isNull() || isPendingKill(invMgrPtr)) {
            return ptr(0);
        }
        return invMgrPtr.add(OFFSETS.UDH_InventoryManager_HumanOwner).readPointer();
    }

    function forceHolsterNow(invMgrPtr) {
        if (!invMgrPtr || invMgrPtr.isNull() || isPendingKill(invMgrPtr) || !origHolsterInventory) {
            return;
        }
        var selectedInv = invMgrPtr.add(OFFSETS.UDH_InventoryManager_SelectedInventory).readPointer();
        if (selectedInv.isNull()) {
            return;
        }
        var viewClass = invMgrPtr.add(OFFSETS.UDH_InventoryManager_SelectedViewClass).readPointer();
        if (!viewClass.isNull()) {
            origHolsterInventory(invMgrPtr, 1);
        }
    }

    function installInventoryLockHooks() {
        var holsterCb = new NativeCallback(function (invMgr, bHolster) {
            try {
                if ((bHolster & 1) === 0 && isHumanIntimidated(getInvHumanOwner(invMgr))) {
                    return;
                }
            } catch (e) {}
            if (origHolsterInventory) { origHolsterInventory(invMgr, bHolster); }
        }, 'void', ['pointer', 'uint8'], 'win64');
        pinnedCallbacks.push(holsterCb);
        origHolsterInventory = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_InventoryManager_HolsterInventory_Impl), holsterCb),
            'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' }
        );

        var selectCb = new NativeCallback(function (invMgr, invSelectionPtr) {
            try {
                if (isHumanIntimidated(getInvHumanOwner(invMgr))) {
                    forceHolsterNow(invMgr);
                    return;
                }
            } catch (e) {}
            if (origSelectInventory) { origSelectInventory(invMgr, invSelectionPtr); }
        }, 'void', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(selectCb);
        origSelectInventory = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_InventoryManager_SelectInventory_Impl), selectCb),
            'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        var useCb = new NativeCallback(function (invMgr, transformPtr, useStatePtr) {
            try {
                if (isHumanIntimidated(getInvHumanOwner(invMgr))) {
                    forceHolsterNow(invMgr);
                    return;
                }
            } catch (e) {}
            if (origUseInventory) { origUseInventory(invMgr, transformPtr, useStatePtr); }
        }, 'void', ['pointer', 'pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(useCb);
        origUseInventory = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_InventoryManager_UseInventory_Impl), useCb),
            'void', ['pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        var aimCb = new NativeCallback(function (invMgr, bWantsToAim) {
            try {
                if ((bWantsToAim & 1) !== 0 && isHumanIntimidated(getInvHumanOwner(invMgr))) {
                    return;
                }
            } catch (e) {}
            if (origAimInventory) { origAimInventory(invMgr, bWantsToAim); }
        }, 'void', ['pointer', 'uint8'], 'win64');
        pinnedCallbacks.push(aimCb);
        origAimInventory = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_InventoryManager_AimInventory_Impl), aimCb),
            'void', ['pointer', 'uint8'], { abi: 'win64', exceptions: 'propagate' }
        );

        var reloadCb = new NativeCallback(function (invMgr) {
            try {
                if (isHumanIntimidated(getInvHumanOwner(invMgr))) {
                    return;
                }
            } catch (e) {}
            if (origReloadInventory) { origReloadInventory(invMgr); }
        }, 'void', ['pointer'], 'win64');
        pinnedCallbacks.push(reloadCb);
        origReloadInventory = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_InventoryManager_ReloadInventory_Impl), reloadCb),
            'void', ['pointer'], { abi: 'win64', exceptions: 'propagate' }
        );
    }

    // ========================================================================
    // 8. 全 5 スペル純 JS 実装
    // ========================================================================

    // --- [5] 爆弾ネズミ (TS_BombMice): 原版 MOD 移動物理 + 純JS専用 NavMesh 経路探索 + 1A 姿勢傾斜 + 2B 軍艦・舷梯防爬ガード ---
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
            if (itemPtr.add(16).readS32() !== ser) {
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

    function getActorYawDeg(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return 0.0;
        }
        try {
            var rootComp = actorPtr.add(OFFSETS.AActor_RootComponent).readPointer();
            if (rootComp.isNull() || isPendingKill(rootComp)) {
                return 0.0;
            }
            var qx = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldQuat).readFloat();
            var qy = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 4).readFloat();
            var qz = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 8).readFloat();
            var qw = rootComp.add(OFFSETS.USceneComponent_ComponentToWorldQuat + 12).readFloat();
            var siny = 2.0 * (qw * qz + qx * qy);
            var cosy = 1.0 - 2.0 * (qy * qy + qz * qz);
            var yaw = Math.atan2(siny, cosy) * (180.0 / Math.PI);
            return Number.isFinite(yaw) ? yaw : 0.0;
        } catch (e) {
            return 0.0;
        }
    }

    function setActorPitchYawRoll(actorPtr, pitchDeg, yawDeg, rollDeg) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return;
        }
        if (!Number.isFinite(pitchDeg) || !Number.isFinite(yawDeg) || !Number.isFinite(rollDeg)) {
            return;
        }
        try {
            rotatorBuf.writeFloat(pitchDeg);
            rotatorBuf.add(4).writeFloat(yawDeg);
            rotatorBuf.add(8).writeFloat(rollDeg);
            AActor_K2_SetActorRotation(actorPtr, rotatorBuf, 0);
        } catch (e) {}
    }

    function resetActorPitchRoll(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return;
        }
        setActorPitchYawRoll(actorPtr, 0.0, getActorYawDeg(actorPtr), 0.0);
    }

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
        return Math.abs(loc.x - shipCache.tx) <= CONFIG.shipBroadphaseRadiusUU &&
               Math.abs(loc.y - shipCache.ty) <= CONFIG.shipBroadphaseRadiusUU;
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
                var clsName = readObjectClassName(objPtr);
                if (clsName && clsName.indexOf('StaticMeshComponent') !== -1) {
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

    function isBombMouseOnShip(ratPtr, ratLoc) {
        if (isCharacterBasedOnShip(ratPtr)) {
            return true;
        }
        return isPointInShipOBB(ratLoc, -25.0, 0.0);
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

    function probeBombMouseForwardWall(worldPtr, ratPtr, targetPtr, fromLoc, ux, uy, maxDist) {
        var res = { hitAny: false, hitShip: false, hitSteepWall: false, steepestNormZ: 1.0 };
        if (!worldPtr || worldPtr.isNull() || !fromLoc) {
            return res;
        }
        var heights = [15.0, 55.0, 95.0];
        for (var i = 0; i < heights.length; i++) {
            var z = fromLoc.z + heights[i];
            var r = traceSinglePawnRay(
                worldPtr, ratPtr, targetPtr,
                fromLoc.x, fromLoc.y, z,
                fromLoc.x + ux * maxDist, fromLoc.y + uy * maxDist, z
            );
            if (r.hit) {
                res.hitAny = true;
                if (r.isShip) {
                    res.hitShip = true;
                }
                if (r.normZ < res.steepestNormZ) {
                    res.steepestNormZ = r.normZ;
                }
                if (r.normZ < WALKABLE_SLOPE_COS_78) {
                    res.hitSteepWall = true;
                }
            }
        }
        return res;
    }

    function isBombMouseShipClimbBlocked(worldPtr, mEntry, targetPtr, ratLoc, targetLoc) {
        if (isTargetOnShip(targetPtr, targetLoc)) {
            return true;
        }
        if (isBombMouseOnShip(mEntry.rat, ratLoc) || isPointInShipOBB(ratLoc, 55.0, 20.0)) {
            return true;
        }
        var dx = targetLoc.x - ratLoc.x;
        var dy = targetLoc.y - ratLoc.y;
        var horiz = Math.sqrt(dx * dx + dy * dy);
        if (Number.isFinite(horiz) && horiz > 1.0 && (isNearShipBroadphase(ratLoc) || isNearShipBroadphase(targetLoc))) {
            var probe = probeBombMouseForwardWall(worldPtr, mEntry.rat, targetPtr, ratLoc, dx / horiz, dy / horiz, CONFIG.shipForwardTraceDistUU);
            if (probe.hitShip) {
                return true;
            }
        }
        return false;
    }

    function cleanupBombMouseMoveComp(entry) {
        if (entry && entry.moveCompKey) {
            delete trackedBombMouseMoveComps[entry.moveCompKey];
        }
    }

    function writeFVector(buf, loc) {
        buf.writeFloat(loc.x);
        buf.add(4).writeFloat(loc.y);
        buf.add(8).writeFloat(loc.z);
    }

    function readFVector(buf) {
        var x = buf.readFloat();
        var y = buf.add(4).readFloat();
        var z = buf.add(8).readFloat();
        if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
            return null;
        }
        return { x: x, y: y, z: z };
    }

    function isActorAITargetable(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return false;
        }
        try {
            return (IDH_AITarget_Execute_IsAITargetable(actorPtr) & 1) !== 0;
        } catch (e) {
            return true;
        }
    }

    function isLivingNonThrallHuman(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return false;
        }
        if (!isHumanAlive(actorPtr)) {
            return false;
        }
        var ps = resolvePlayerStateFromActor(actorPtr);
        if (!ps.isNull() && !isPendingKill(ps)) {
            if (ps.add(OFFSETS.ADH_PlayerState_bIsThrall).readU8() !== 0) {
                return false;
            }
            if (ps.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() !== 0) {
                return false;
            }
        }
        return true;
    }

    function collectLivingExplorerTargets() {
        var targets = [];
        var seen = {};
        function pushCandidate(humanPtr) {
            if (!isLivingNonThrallHuman(humanPtr)) {
                return;
            }
            var hk = humanPtr.toString();
            if (seen[hk]) {
                return;
            }
            var loc = getActorLocation(humanPtr);
            if (!loc) {
                return;
            }
            seen[hk] = true;
            targets.push({
                human: humanPtr,
                key: hk,
                loc: loc,
                targetable: isActorAITargetable(humanPtr)
            });
        }

        // 1) 実プレイヤーの非傀儡探検家
        var allPS = getAllPlayerStates();
        for (var p = 0; p < allPS.length; p++) {
            var ps = allPS[p];
            if (ps.add(OFFSETS.ADH_PlayerState_bIsThrall).readU8() !== 0) { continue; }
            if (ps.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() !== 0) { continue; }
            pushCandidate(getHumanFromPlayerState(ps));
        }

        // 2) アクティブ呪文の CurrentCastingTarget（コンソール召喚した海兵(船員)等も追尾・起爆対象に含める）
        var sKeys = Object.keys(activeBombMiceSpells);
        for (var s = 0; s < sKeys.length; s++) {
            var sEntry = activeBombMiceSpells[sKeys[s]];
            if (!sEntry || !sEntry.spell || sEntry.spell.isNull() || isPendingKill(sEntry.spell)) {
                continue;
            }
            pushCandidate(sEntry.spell.add(OFFSETS.UDH_TotemSpell_Summoning_CurrentCastingTarget).readPointer());
        }

        // 3) 追跡中ネズミが保持する優先・現在ターゲット（AAIController 構造体サイズ超過読み出しを防ぐため自前保持から参照）
        var rKeys = Object.keys(trackedBombMice);
        for (var r = 0; r < rKeys.length; r++) {
            var mEntry = trackedBombMice[rKeys[r]];
            if (!mEntry || !mEntry.rat || mEntry.rat.isNull() || isPendingKill(mEntry.rat)) {
                continue;
            }
            if (mEntry.currentTarget && !mEntry.currentTarget.isNull()) {
                pushCandidate(mEntry.currentTarget);
            }
            if (mEntry.preferredTarget && !mEntry.preferredTarget.isNull()) {
                pushCandidate(mEntry.preferredTarget);
            }
        }

        return targets;
    }

    function selectBestBombMouseTarget(mEntry, ratLoc, explorers) {
        if (!explorers || explorers.length === 0) {
            return null;
        }
        // IsAITargetable を満たす生存探検家を優先し、該当ゼロ時のみ全生存探検家へフォールバック
        var pool = [];
        for (var i = 0; i < explorers.length; i++) {
            if (explorers[i].targetable) {
                pool.push(explorers[i]);
            }
        }
        if (pool.length === 0) {
            pool = explorers;
        }
        if (!ratLoc) {
            return pool[0];
        }

        var curKey = (mEntry.currentTarget && !mEntry.currentTarget.isNull() && !isPendingKill(mEntry.currentTarget))
            ? mEntry.currentTarget.toString() : '';
        var best = null;
        var bestDist = Infinity;
        var curEntry = null;
        var curDist = Infinity;

        for (var j = 0; j < pool.length; j++) {
            var cand = pool[j];
            var d = distance3D(ratLoc, cand.loc);
            if (d < bestDist) {
                bestDist = d;
                best = cand;
            }
            if (curKey && cand.key === curKey) {
                curEntry = cand;
                curDist = d;
            }
        }

        // 軽微なヒステリシス（300 UU）で等距離の2名間での往復振動を防止しつつ、より近い対象へ即座に切替
        if (curEntry && curDist <= bestDist + CONFIG.bombMiceTargetSwitchHysteresisUU) {
            return curEntry;
        }
        return best;
    }

    function scheduleBombMouseControllerCleanup(ctrlPtr) {
        if (!ctrlPtr || ctrlPtr.isNull()) {
            return;
        }
        pendingDestroyControllers.push(ctrlPtr);
    }

    function drainPendingDestroyControllers(worldPtr) {
        if (!worldPtr || worldPtr.isNull() || pendingDestroyControllers.length === 0) {
            return;
        }
        var list = pendingDestroyControllers;
        pendingDestroyControllers = [];
        for (var i = 0; i < list.length; i++) {
            var ctrl = list[i];
            try {
                if (!ctrl || ctrl.isNull() || isPendingKill(ctrl)) {
                    continue;
                }
                if (readObjectClassName(ctrl) !== 'AIController') {
                    continue;
                }
                try { AAIController_StopMovement(ctrl); } catch (e1) {}
                UWorld_DestroyActor(worldPtr, ctrl, 0, 1);
            } catch (e2) {}
        }
    }

    // 急斜面・岩段差の定向蓄力跳躍（MOVE_Walking 時のみターゲット方向へ斜め上方に跳躍・2B 船体は完全遮断）
    function triggerBombMouseVault(worldPtr, mEntry, targetPtr, ratLoc, targetLoc, nowTimeSec) {
        if (!mEntry || !mEntry.rat || mEntry.rat.isNull() || isPendingKill(mEntry.rat)) {
            return false;
        }
        if (nowTimeSec < (mEntry.nextVaultAt || 0.0)) {
            return false;
        }
        if (isBombMouseShipClimbBlocked(worldPtr, mEntry, targetPtr, ratLoc, targetLoc)) {
            return false;
        }
        try {
            var moveComp = mEntry.rat.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
            if (moveComp.isNull() || isPendingKill(moveComp)) {
                return false;
            }
            // MOVE_Walking(1) のときのみ安全に定向跳躍を発動（空中連打・水中誤発火を防止）
            if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() !== 1) {
                return false;
            }
            var curVelX = moveComp.add(OFFSETS.UMovementComponent_Velocity).readFloat();
            var curVelY = moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).readFloat();
            var curVelZ = moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).readFloat();
            var nextVelX = Number.isFinite(curVelX) ? curVelX : 0.0;
            var nextVelY = Number.isFinite(curVelY) ? curVelY : 0.0;
            if (ratLoc && targetLoc) {
                var dx = targetLoc.x - ratLoc.x;
                var dy = targetLoc.y - ratLoc.y;
                var horiz = Math.sqrt(dx * dx + dy * dy);
                if (Number.isFinite(horiz) && horiz > 1.0) {
                    nextVelX = (dx / horiz) * CONFIG.bombMiceVaultVelXY;
                    nextVelY = (dy / horiz) * CONFIG.bombMiceVaultVelXY;
                }
            }
            var nextVelZ = Number.isFinite(curVelZ)
                ? Math.max(curVelZ, CONFIG.bombMiceVaultVelZ)
                : CONFIG.bombMiceVaultVelZ;
            moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(nextVelX);
            moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(nextVelY);
            moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(nextVelZ);
            UCharacterMovementComponent_SetMovementMode(moveComp, 3, 0); // MOVE_Falling
            mEntry.nextVaultAt = nowTimeSec + CONFIG.bombMiceVaultCooldownSec;
            return true;
        } catch (e) {
            return false;
        }
    }

    // 4段階 NavMesh 経路探索（Tier 1: MoveToActor -> Tier 2: 最近傍NavMesh投影 -> Tier 3: 周辺到達可能点 -> Tier 4: 近距離直進）
    function issueBombMouseMove(mEntry, ctrl, ratLoc, targetEntry) {
        // Tier 1: 原生 RecastNavMesh によるターゲット追尾（フィルタなし・部分経路許容・衝突停止なし）
        try {
            var r1 = AAIController_MoveToActor(
                ctrl,
                targetEntry.human,
                CONFIG.bombMiceAcceptanceRadiusUU,
                0, // bStopOnOverlap = false
                1, // bUsePathfinding = true
                0, // bCanStrafe = false
                ptr(0), // FilterClass = null（制限フィルタなし）
                1  // bAllowPartialPath = true
            );
            if (r1 !== 0) {
                mEntry.lastMoveTier = 1;
                mEntry.lastTargetLoc = { x: targetEntry.loc.x, y: targetEntry.loc.y, z: targetEntry.loc.z };
                return true;
            }
        } catch (e1) {}

        // Tier 2: ターゲットが岩・船縁・段差など NavMesh 外にいる場合、最近傍の NavMesh 座標へ正確に投影
        try {
            writeFVector(navPointBuf, targetEntry.loc);
            navExtentBuf.writeFloat(CONFIG.bombMiceNavProjectExtentXY);
            navExtentBuf.add(4).writeFloat(CONFIG.bombMiceNavProjectExtentXY);
            navExtentBuf.add(8).writeFloat(CONFIG.bombMiceNavProjectExtentZ);
            var projOk = UNavigationSystemV1_K2_ProjectPointToNavigation(
                ctrl,
                navPointBuf,
                navOutBuf,
                ptr(0),
                ptr(0),
                navExtentBuf
            );
            if ((projOk & 1) !== 0 && readFVector(navOutBuf) !== null) {
                var r2 = AAIController_MoveToLocation(
                    ctrl,
                    navOutBuf,
                    40.0,
                    0, // bStopOnOverlap
                    1, // bUsePathfinding
                    1, // bProjectDestinationToNavigation
                    0, // bCanStrafe
                    ptr(0),
                    1  // bAllowPartialPath
                );
                if (r2 !== 0) {
                    mEntry.lastMoveTier = 2;
                    mEntry.lastTargetLoc = { x: targetEntry.loc.x, y: targetEntry.loc.y, z: targetEntry.loc.z };
                    return true;
                }
            }
        } catch (e2) {}

        // Tier 3: 投影範囲外（深い水上・船上高所など）ではターゲット周辺 900 UU の到達可能 NavMesh 点へ接近
        try {
            writeFVector(navPointBuf, targetEntry.loc);
            var randOk = UNavigationSystemV1_K2_GetRandomReachablePointInRadius(
                ctrl,
                navPointBuf,
                navOutBuf,
                CONFIG.bombMiceNavReachableRadiusUU,
                ptr(0),
                ptr(0)
            );
            if ((randOk & 1) !== 0 && readFVector(navOutBuf) !== null) {
                var r3 = AAIController_MoveToLocation(
                    ctrl,
                    navOutBuf,
                    40.0,
                    0,
                    1,
                    1,
                    0,
                    ptr(0),
                    1
                );
                if (r3 !== 0) {
                    mEntry.lastMoveTier = 3;
                    mEntry.lastTargetLoc = { x: targetEntry.loc.x, y: targetEntry.loc.y, z: targetEntry.loc.z };
                    return true;
                }
            }
        } catch (e3) {}

        // Tier 4: 全 NavMesh クエリ不成立かつ近距離（<= 1500 UU）時のみ一時的な直進フォールバック（2B: 船上の目標へは船外から直進しない）
        if (ratLoc && distance3D(ratLoc, targetEntry.loc) <= CONFIG.bombMiceDirectFallbackMaxDistUU &&
            !(isTargetOnShip(targetEntry.human, targetEntry.loc) && !isBombMouseOnShip(mEntry.rat, ratLoc))) {
            try {
                var r4 = AAIController_MoveToActor(
                    ctrl,
                    targetEntry.human,
                    40.0,
                    0,
                    0, // bUsePathfinding = false
                    0,
                    ptr(0),
                    1
                );
                if (r4 !== 0) {
                    mEntry.lastMoveTier = 4;
                    mEntry.lastTargetLoc = { x: targetEntry.loc.x, y: targetEntry.loc.y, z: targetEntry.loc.z };
                    return true;
                }
            } catch (e4) {}
        }

        return false;
    }

    // 2段階 垂直氷壁・岸壁登攀ステートマシン (MOVE_Flying=5 + 1A 正 Pitch 鼻上げ傾斜 + <78° 復帰 + 2B 軍艦・舷梯防爬)
    function enterBombMouseWallClimb(worldPtr, mEntry, ctrl, targetPtr, ratLoc, targetLoc, nowTimeSec) {
        if (!mEntry || !mEntry.rat || mEntry.rat.isNull() || isPendingKill(mEntry.rat)) {
            return false;
        }
        if (isBombMouseShipClimbBlocked(worldPtr, mEntry, targetPtr, ratLoc, targetLoc)) {
            return false;
        }
        var moveComp = mEntry.rat.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp)) {
            return false;
        }
        var mcKey = moveComp.toString();
        if (mcKey !== mEntry.moveCompKey) {
            cleanupBombMouseMoveComp(mEntry);
            mEntry.moveCompKey = mcKey;
            trackedBombMouseMoveComps[mcKey] = mEntry;
        }
        try {
            if (ctrl && !ctrl.isNull() && !isPendingKill(ctrl)) {
                AAIController_StopMovement(ctrl);
            }
        } catch (eStop) {}

        var origBraking = moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).readFloat();
        var origMaxFly = moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).readFloat();
        mEntry.savedBrakingFly = Number.isFinite(origBraking) ? origBraking : 0.0;
        mEntry.savedMaxFlySpeed = (Number.isFinite(origMaxFly) && origMaxFly > 0.0) ? origMaxFly : 600.0;

        var navAgentFlags = moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).writeU8(navAgentFlags | 0x1E);
        var moveStateFlags = moveComp.add(OFFSETS.UNavMovementComponent_MovementState).readU8();
        moveComp.add(OFFSETS.UNavMovementComponent_MovementState).writeU8(moveStateFlags | 0x1E);

        moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(CONFIG.bombMiceClimbFlySpeed);
        moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(0.0);
        mEntry.rat.add(OFFSETS.ADH_AnimalCharacter_StuckTimer).writeFloat(0.0);
        UCharacterMovementComponent_SetMovementMode(moveComp, 5, 0); // MOVE_Flying

        mEntry.climbPhase = 1; // CLIMB_UP
        mEntry.climbStartAt = nowTimeSec;
        mEntry.climbStartZ = ratLoc.z;
        mEntry.climbMinGoalZ = Math.max(targetLoc.z + CONFIG.bombMiceClimbTopMarginUU, ratLoc.z + CONFIG.bombMiceClimbMinClearanceUU);
        mEntry.climbLastZCheckAt = nowTimeSec + 0.25;
        mEntry.climbLastCheckZ = ratLoc.z;
        mEntry.climbBackoffUntil = 0.0;
        mEntry.climbNoZCount = 0;
        mEntry.climbCrestUntil = 0.0;
        mEntry.climbNextFloorCheckAt = nowTimeSec + CONFIG.bombMiceSlopeRecoverGraceSec;
        mEntry.climbNextShipTraceAt = nowTimeSec + CONFIG.shipMidClimbTraceIntervalSec;
        mEntry.climbPitchDeg = CONFIG.bombMiceClimbPitchDefaultDeg;
        mEntry.walkableSlopeHits = 0;
        mEntry.detourUntil = 0.0;
        mEntry.stuckCount = 0;
        return true;
    }

    function exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec) {
        if (!mEntry || mEntry.climbPhase === 0) {
            return;
        }
        mEntry.climbPhase = 0;
        mEntry.climbCooldownUntil = nowTimeSec + CONFIG.bombMiceClimbCooldownSec;
        mEntry.stuckCount = 0;
        mEntry.walkableSlopeHits = 0;
        mEntry.nextRepathAt = nowTimeSec;
        mEntry.nextIdleRetryAt = nowTimeSec;
        try {
            if (mEntry.rat && !mEntry.rat.isNull() && !isPendingKill(mEntry.rat)) {
                // 1A 必須要件: ADH_AnimalCharacter の RotationRate.Pitch=0 のため終了時に必ず Pitch=0, Roll=0 を明示復帰
                resetActorPitchRoll(mEntry.rat);
                var moveComp = mEntry.rat.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
                if (!moveComp.isNull() && !isPendingKill(moveComp)) {
                    if (typeof mEntry.savedBrakingFly === 'number') {
                        moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(mEntry.savedBrakingFly);
                    }
                    if (typeof mEntry.savedMaxFlySpeed === 'number') {
                        moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(mEntry.savedMaxFlySpeed);
                    }
                    var vz = moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).readFloat();
                    if (Number.isFinite(vz) && vz > 0.0) {
                        moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(-20.0);
                    }
                    if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() === 5) {
                        // MOVE_Falling へ戻すことで UE4 の着地判定が即座に MOVE_Walking へ接地する
                        UCharacterMovementComponent_SetMovementMode(moveComp, 3, 0);
                    }
                }
                var curLoc = getActorLocation(mEntry.rat);
                if (curLoc) {
                    mEntry.lastProgressLoc = { x: curLoc.x, y: curLoc.y, z: curLoc.z };
                }
            }
        } catch (e) {}
        mEntry.lastProgressDist = null;
        mEntry.nextStuckCheckAt = nowTimeSec + CONFIG.bombMiceStuckCheckIntervalSec;
    }

    function tickBombMouseWallClimb(worldPtr, mEntry, ctrl, targetPtr, ratLoc, targetLoc, nowTimeSec) {
        var moveComp = mEntry.rat.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (moveComp.isNull() || isPendingKill(moveComp) || !ratLoc || !targetLoc) {
            exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
            return false;
        }
        // 2B 登攀中の船体接近・目標乗船ガード（即座に中断＋Pitch/Roll 帰零）
        if (isTargetOnShip(targetPtr, targetLoc) || isPointInShipOBB(ratLoc, 40.0, 15.0)) {
            exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
            return false;
        }

        var dx = targetLoc.x - ratLoc.x;
        var dy = targetLoc.y - ratLoc.y;
        var distXY = Math.sqrt(dx * dx + dy * dy);
        var dist3D = distance3D(ratLoc, targetLoc);
        var elapsed = nowTimeSec - mEntry.climbStartAt;
        var zGained = ratLoc.z - mEntry.climbStartZ;

        if (dist3D <= CONFIG.bombMiceWallClimbExemptDistUU ||
            elapsed >= CONFIG.bombMiceClimbMaxDurationSec ||
            zGained >= CONFIG.bombMiceClimbMaxZGainUU) {
            exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
            return false;
        }

        var ux = (Number.isFinite(distXY) && distXY > 1.0) ? (dx / distXY) : 0.0;
        var uy = (Number.isFinite(distXY) && distXY > 1.0) ? (dy / distXY) : 0.0;
        var yawDeg = (distXY > 1.0) ? (Math.atan2(dy, dx) * (180.0 / Math.PI)) : getActorYawDeg(mEntry.rat);

        // 2B 登攀中 0.15s ごとの船殻・舷梯レイ再検査
        if (nowTimeSec >= mEntry.climbNextShipTraceAt) {
            mEntry.climbNextShipTraceAt = nowTimeSec + CONFIG.shipMidClimbTraceIntervalSec;
            if (isNearShipBroadphase(ratLoc) && distXY > 1.0) {
                var shipProbe = probeBombMouseForwardWall(worldPtr, mEntry.rat, targetPtr, ratLoc, ux, uy, CONFIG.shipForwardTraceDistUU);
                if (shipProbe.hitShip) {
                    exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
                    return false;
                }
            }
        }

        // 原版 ADH_AnimalCharacter::Tick のスタック強制 MOVE_Falling 化を抑止
        mEntry.rat.add(OFFSETS.ADH_AnimalCharacter_StuckTimer).writeFloat(0.0);
        moveComp.add(OFFSETS.UCharacterMovementComponent_MaxFlySpeed).writeFloat(CONFIG.bombMiceClimbFlySpeed);
        moveComp.add(OFFSETS.UCharacterMovementComponent_BrakingDecelerationFlying).writeFloat(0.0);
        if (moveComp.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() !== 5) {
            UCharacterMovementComponent_SetMovementMode(moveComp, 5, 0);
        }
        try {
            if (AAIController_GetMoveStatus(ctrl) === 3) {
                AAIController_StopMovement(ctrl);
            }
        } catch (eSt) {}

        if (mEntry.climbPhase === 1) {
            // 1A: < 78° 傾角復帰判定（MOVE_Flying 中は CurrentFloor が空のため K2_FindFloor + 前方レイで判定）
            if (elapsed >= CONFIG.bombMiceSlopeRecoverGraceSec && zGained >= CONFIG.bombMiceSlopeRecoverMinZGainUU &&
                nowTimeSec >= mEntry.climbNextFloorCheckAt) {
                mEntry.climbNextFloorCheckAt = nowTimeSec + CONFIG.bombMiceSlopeCheckIntervalSec;
                try {
                    var wallAhead = (distXY > 1.0)
                        ? probeBombMouseForwardWall(worldPtr, mEntry.rat, targetPtr, ratLoc, ux, uy, 130.0)
                        : { hitAny: false, hitShip: false, hitSteepWall: false, steepestNormZ: 1.0 };
                    if (wallAhead.hitShip) {
                        exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
                        return false;
                    }
                    if (wallAhead.hitSteepWall) {
                        mEntry.walkableSlopeHits = 0;
                        var wallSlopeDeg = Math.acos(Math.max(0.0, Math.min(1.0, wallAhead.steepestNormZ))) * (180.0 / Math.PI);
                        var targetPitch = Math.max(CONFIG.bombMiceClimbPitchMinDeg, Math.min(CONFIG.bombMiceClimbPitchMaxDeg, wallSlopeDeg * 0.85));
                        mEntry.climbPitchDeg = targetPitch;
                    } else {
                        floorLocBuf.writeFloat(ratLoc.x);
                        floorLocBuf.add(4).writeFloat(ratLoc.y);
                        floorLocBuf.add(8).writeFloat(ratLoc.z);
                        floorResultBuf.writeByteArray(zeroHitBytes);
                        UCharacterMovementComponent_K2_FindFloor(moveComp, floorLocBuf, floorResultBuf);
                        var flFlags = floorResultBuf.readU8();
                        var bWalkable = (flFlags & 0x02) !== 0;
                        var flDist = floorResultBuf.add(OFFSETS.FFindFloorResult_FloorDist).readFloat();
                        var flNormZ = floorResultBuf.add(OFFSETS.FFindFloorResult_HitNormalZ).readFloat();
                        var flActor = resolveWeakObject(floorResultBuf.add(OFFSETS.FFindFloorResult_HitActor));
                        var flComp = resolveWeakObject(floorResultBuf.add(OFFSETS.FFindFloorResult_HitComponent));
                        if (bWalkable && Number.isFinite(flDist) && flDist <= CONFIG.bombMiceSlopeRecoverMaxFloorDistUU &&
                            Number.isFinite(flNormZ) && flNormZ > WALKABLE_RECOVER_COS_75 &&
                            !isWarshipOrGangwayActor(flActor, flComp)) {
                            mEntry.walkableSlopeHits++;
                            if (mEntry.walkableSlopeHits >= CONFIG.bombMiceSlopeRecoverHitsNeeded) {
                                exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
                                return false;
                            }
                        } else {
                            mEntry.walkableSlopeHits = 0;
                        }
                    }
                } catch (eFl) {}
            }

            // Phase 1: CLIMB_UP（垂直壁・氷壁に沿ってカプセルスイープ上昇 + 正 Pitch 鼻上げ傾斜 + オーバーハング回避）
            var desiredZ = Math.max(targetLoc.z + CONFIG.bombMiceClimbTopMarginUU, mEntry.climbMinGoalZ);
            if (ratLoc.z >= desiredZ) {
                mEntry.climbPhase = 2;
                mEntry.climbCrestUntil = nowTimeSec + CONFIG.bombMiceCrestDurationSec;
                setActorPitchYawRoll(mEntry.rat, 0.0, yawDeg, 0.0);
            } else {
                if (nowTimeSec >= mEntry.climbLastZCheckAt) {
                    var dzStep = ratLoc.z - mEntry.climbLastCheckZ;
                    mEntry.climbLastCheckZ = ratLoc.z;
                    mEntry.climbLastZCheckAt = nowTimeSec + 0.25;
                    if (nowTimeSec >= mEntry.climbBackoffUntil && dzStep < 20.0) {
                        mEntry.climbNoZCount++;
                        if (mEntry.climbNoZCount >= 4) {
                            exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
                            return false;
                        }
                        mEntry.climbBackoffUntil = nowTimeSec + CONFIG.bombMiceClimbBackoffSec;
                    } else if (dzStep >= 20.0) {
                        mEntry.climbNoZCount = 0;
                    }
                }
                if (nowTimeSec < mEntry.climbBackoffUntil) {
                    setActorPitchYawRoll(mEntry.rat, CONFIG.bombMiceClimbPitchBackoffDeg, yawDeg, 0.0);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(-ux * CONFIG.bombMiceClimbBackoffVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(-uy * CONFIG.bombMiceClimbBackoffVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.bombMiceClimbBackoffVelZ);
                } else {
                    setActorPitchYawRoll(mEntry.rat, mEntry.climbPitchDeg, yawDeg, 0.0);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(ux * CONFIG.bombMiceClimbVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(uy * CONFIG.bombMiceClimbVelXY);
                    moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.bombMiceClimbVelZ);
                }
                return true;
            }
        }

        // Phase 2: CREST（Pitch=0 へ戻して崖天面へ水平に踏み込んで確実に乗せる）
        setActorPitchYawRoll(mEntry.rat, 0.0, yawDeg, 0.0);
        if (nowTimeSec >= mEntry.climbCrestUntil || distXY <= 85.0) {
            exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
            return false;
        }
        moveComp.add(OFFSETS.UMovementComponent_Velocity).writeFloat(ux * CONFIG.bombMiceCrestVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 4).writeFloat(uy * CONFIG.bombMiceCrestVelXY);
        moveComp.add(OFFSETS.UMovementComponent_Velocity + 8).writeFloat(CONFIG.bombMiceCrestVelZ);
        return true;
    }

    function driveBombMousePathfinding(worldPtr, mEntry, ratLoc, explorers, nowTimeSec) {
        if (!mEntry || !mEntry.rat || mEntry.rat.isNull() || isPendingKill(mEntry.rat)) {
            return;
        }
        var ctrl = mEntry.rat.add(OFFSETS.APawn_Controller).readPointer();
        if (ctrl.isNull() || isPendingKill(ctrl)) {
            return;
        }
        mEntry.controller = ctrl;

        var chosen = selectBestBombMouseTarget(mEntry, ratLoc, explorers);
        if (!chosen) {
            if (mEntry.climbPhase !== 0) {
                exitBombMouseWallClimb(mEntry, ctrl, nowTimeSec);
            }
            if (mEntry.currentTarget && !mEntry.currentTarget.isNull()) {
                mEntry.currentTarget = ptr(0);
                try { ADH_AnimalCharacter_SetTargetCharacter(mEntry.rat, ptr(0)); } catch (e0) {}
            }
            return;
        }

        var prevTargetKey = (mEntry.currentTarget && !mEntry.currentTarget.isNull()) ? mEntry.currentTarget.toString() : '';
        var targetChanged = (prevTargetKey !== chosen.key);
        if (targetChanged) {
            mEntry.currentTarget = chosen.human;
            mEntry.detourUntil = 0.0;
            mEntry.lastProgressDist = null;
            try {
                // ADH_AnimalCharacter::Tick のネイティブ段差スタック自動ジャンプを有効化・同期
                ADH_AnimalCharacter_SetTargetCharacter(mEntry.rat, chosen.human);
            } catch (e1) {}
        }

        // 船内スポーン個体が一度船外地面へ降りたら以降は船外個体扱い（再登船を禁止）
        var mcCur = mEntry.rat.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (mEntry.startedOnShip && !mcCur.isNull() && !isPendingKill(mcCur) &&
            mcCur.add(OFFSETS.UCharacterMovementComponent_MovementMode).readU8() === 1 &&
            !isBombMouseOnShip(mEntry.rat, ratLoc)) {
            mEntry.startedOnShip = false;
        }

        // 垂直氷壁・岸壁登攀中は 2 段階ステートマシンが移動を完全制御する
        if (mEntry.climbPhase !== 0) {
            if (tickBombMouseWallClimb(worldPtr, mEntry, ctrl, chosen.human, ratLoc, chosen.loc, nowTimeSec)) {
                return;
            }
            // 登攀終了フレームは直ちに通常 NavMesh 追尾を再発行する
            targetChanged = true;
        }

        var moveStatus = 0;
        try {
            moveStatus = AAIController_GetMoveStatus(ctrl);
        } catch (e2) {
            moveStatus = 0;
        }
        var partialPath = false;
        try {
            partialPath = (AAIController_HasPartialPath(ctrl) & 1) !== 0;
        } catch (ePp0) {
            partialPath = false;
        }

        var distToTarget = ratLoc ? distance3D(ratLoc, chosen.loc) : 9999.0;

        // 1) スマート登攀・アンチスタック監視（0.65s 毎に接近進捗 + 高度差 + 実移動を検査）
        var forceRepath = false;
        if (ratLoc && nowTimeSec >= mEntry.nextStuckCheckAt) {
            var prevLoc = mEntry.lastProgressLoc;
            var prevDist = mEntry.lastProgressDist;
            mEntry.lastProgressLoc = { x: ratLoc.x, y: ratLoc.y, z: ratLoc.z };
            mEntry.lastProgressDist = distToTarget;
            mEntry.nextStuckCheckAt = nowTimeSec + CONFIG.bombMiceStuckCheckIntervalSec;

            var dz = chosen.loc.z - ratLoc.z;
            var dxT = chosen.loc.x - ratLoc.x;
            var dyT = chosen.loc.y - ratLoc.y;
            var distXY = Math.sqrt(dxT * dxT + dyT * dyT);
            var canClimbNow = (distToTarget > CONFIG.bombMiceWallClimbExemptDistUU && nowTimeSec >= (mEntry.climbCooldownUntil || 0.0));

            if (prevLoc) {
                var movedDist = distance3D(ratLoc, prevLoc);
                var closedDist = (typeof prevDist === 'number') ? (prevDist - distToTarget) : movedDist;
                var zGainWin = ratLoc.z - prevLoc.z;

                // 条件A: ターゲットが高所(dz >= 95 UU)にいて崖下でウロウロ/部分経路停止している場合 -> 垂直登攀へ即移行（2B 船体は遮断）
                var elevatedStalled = (dz >= CONFIG.bombMiceElevatedDzUU && distXY <= CONFIG.bombMiceElevatedMaxXyUU &&
                    ((closedDist < CONFIG.bombMiceMinCloseProgressUU && zGainWin < CONFIG.bombMiceMinZProgressUU) ||
                     (distXY <= CONFIG.bombMiceCloseWallXyUU && (moveStatus !== 3 || partialPath))));
                if (canClimbNow && elevatedStalled) {
                    if (enterBombMouseWallClimb(worldPtr, mEntry, ctrl, chosen.human, ratLoc, chosen.loc, nowTimeSec)) {
                        tickBombMouseWallClimb(worldPtr, mEntry, ctrl, chosen.human, ratLoc, chosen.loc, nowTimeSec);
                        return;
                    }
                }

                // 条件B: 壁・岩に引っかかり実移動 < 70 UU（2B 船体・舷梯は跳躍・登攀ともに遮断）
                if (movedDist >= CONFIG.bombMiceStuckMinProgressUU) {
                    mEntry.stuckCount = 0;
                } else {
                    mEntry.stuckCount++;
                    if (canClimbNow && (dz >= CONFIG.bombMiceElevatedDzUU || mEntry.stuckCount >= 2 ||
                        !triggerBombMouseVault(worldPtr, mEntry, chosen.human, ratLoc, chosen.loc, nowTimeSec))) {
                        if (enterBombMouseWallClimb(worldPtr, mEntry, ctrl, chosen.human, ratLoc, chosen.loc, nowTimeSec)) {
                            tickBombMouseWallClimb(worldPtr, mEntry, ctrl, chosen.human, ratLoc, chosen.loc, nowTimeSec);
                            return;
                        }
                    } else if (mEntry.stuckCount >= 3) {
                        // 万一登攀クールダウン中の連続スタック時は周辺 NavMesh へ一時迂回（自爆は絶対にしない）
                        try {
                            writeFVector(navPointBuf, ratLoc);
                            var detourOk = UNavigationSystemV1_K2_GetRandomReachablePointInRadius(
                                ctrl,
                                navPointBuf,
                                navOutBuf,
                                CONFIG.bombMiceDetourRadiusUU,
                                ptr(0),
                                ptr(0)
                            );
                            var detourLoc = ((detourOk & 1) !== 0) ? readFVector(navOutBuf) : null;
                            if (detourLoc && distance3D(ratLoc, detourLoc) >= 120.0) {
                                var dr = AAIController_MoveToLocation(
                                    ctrl,
                                    navOutBuf,
                                    40.0,
                                    0,
                                    1,
                                    1,
                                    0,
                                    ptr(0),
                                    1
                                );
                                if (dr !== 0) {
                                    mEntry.lastMoveTier = 5;
                                    mEntry.detourUntil = nowTimeSec + CONFIG.bombMiceDetourDurationSec;
                                    mEntry.nextRepathAt = mEntry.detourUntil;
                                    mEntry.nextIdleRetryAt = nowTimeSec + CONFIG.bombMiceIdleRepathCooldownSec;
                                    return;
                                }
                            }
                        } catch (e3) {}
                    }
                    forceRepath = true;
                }
            }
        }

        // 迂回中（最大 0.85s）は移動継続を優先し、完了(Idle)なら即座に本追尾へ復帰
        if (mEntry.detourUntil > nowTimeSec) {
            if (moveStatus === 3) {
                return;
            }
            mEntry.detourUntil = 0.0;
            forceRepath = true;
        }

        var repathInterval = (distToTarget <= CONFIG.bombMiceCloseRangeUU)
            ? CONFIG.bombMiceCloseRepathIntervalSec
            : CONFIG.bombMiceRepathIntervalSec;

        var needMove = false;
        if (targetChanged || forceRepath) {
            needMove = true;
        } else if (moveStatus !== 3 && nowTimeSec >= mEntry.nextIdleRetryAt) {
            // 経路完了(部分経路終端含む)・移動停止時は即座に再計算
            needMove = true;
        } else if (nowTimeSec >= mEntry.nextRepathAt) {
            // フォールバック中・部分経路中・またはターゲットが 120 UU 以上移動した場合のみ再探索
            if (mEntry.lastMoveTier !== 1 || partialPath || !mEntry.lastTargetLoc ||
                distance3D(chosen.loc, mEntry.lastTargetLoc) >= CONFIG.bombMiceTargetMoveRepathUU) {
                needMove = true;
            } else {
                mEntry.nextRepathAt = nowTimeSec + repathInterval;
            }
        }

        if (needMove) {
            issueBombMouseMove(mEntry, ctrl, ratLoc, chosen);
            mEntry.nextRepathAt = nowTimeSec + repathInterval;
            mEntry.nextIdleRetryAt = nowTimeSec + CONFIG.bombMiceIdleRepathCooldownSec;
        }
    }

    function captureWildRatCdoDefaultsOnce(ratCls) {
        if (wildRatCdoDefaults !== null || !ratCls || ratCls.isNull()) {
            return;
        }
        var ratCdo = ratCls.add(OFFSETS.UClass_ClassDefaultObject).readPointer();
        if (ratCdo.isNull() || isPendingKill(ratCdo)) {
            return;
        }
        var ctrlCls = ratCdo.add(OFFSETS.APawn_AIControllerClass).readPointer();
        var ctrlName = (!ctrlCls.isNull() && !isPendingKill(ctrlCls)) ? readObjectName(ctrlCls) : '';
        // 爆弾ネズミ用コントローラに書き換わった状態ではなく、野生ネズミ本来の CDO のみを不変保存
        if (ctrlName === 'BP_CannibalController_C' || ctrlName === 'AIController') {
            return;
        }
        wildRatCdoDefaults = {
            cdo: ratCdo,
            autoPossess: ratCdo.add(OFFSETS.APawn_AutoPossessAI).readU8(),
            teamId: ratCdo.add(OFFSETS.ADH_Character_TeamID).readU8(),
            aiControllerClass: ctrlCls
        };
    }

    function restoreWildRatCdoDefaults() {
        if (!wildRatCdoDefaults || !wildRatCdoDefaults.cdo || wildRatCdoDefaults.cdo.isNull()) {
            return;
        }
        try {
            if (isPendingKill(wildRatCdoDefaults.cdo)) {
                return;
            }
            wildRatCdoDefaults.cdo.add(OFFSETS.APawn_AutoPossessAI).writeU8(wildRatCdoDefaults.autoPossess);
            wildRatCdoDefaults.cdo.add(OFFSETS.ADH_Character_TeamID).writeU8(wildRatCdoDefaults.teamId);
            wildRatCdoDefaults.cdo.add(OFFSETS.APawn_AIControllerClass).writePointer(wildRatCdoDefaults.aiControllerClass);
        } catch (e) {}
    }

    function buildBombMouseIdentity(ratPtr) {
        var ptrKey = ratPtr.toString();
        var internalIndex = ratPtr.add(OFFSETS.UObject_InternalIndex).readS32();
        var nameRaw = ratPtr.add(OFFSETS.UObject_NamePrivate).readU64();
        return {
            ptrKey: ptrKey,
            internalIndex: internalIndex,
            nameRaw: nameRaw,
            identityKey: ptrKey + ':' + internalIndex + ':' + nameRaw.toString()
        };
    }

    // ポインタ + InternalIndex + 64bit FName（インスタンス番号含む）の3点完全一致で検証し、
    // 野生ネズミへのヒープ再利用誤爆を 100% 遮断する
    function getTrackedBombMouse(ratPtr) {
        if (!ratPtr || ratPtr.isNull() || isPendingKill(ratPtr)) {
            return null;
        }
        var ptrKey = ratPtr.toString();
        var entry = trackedBombMice[ptrKey];
        if (!entry) {
            return null;
        }
        if (entry.detonated) {
            cleanupBombMouseMoveComp(entry);
            delete trackedBombMice[ptrKey];
            return null;
        }
        try {
            var curIdx = ratPtr.add(OFFSETS.UObject_InternalIndex).readS32();
            var curNameRaw = ratPtr.add(OFFSETS.UObject_NamePrivate).readU64();
            if (curIdx !== entry.internalIndex || !curNameRaw.equals(entry.nameRaw)) {
                // 別オブジェクト（野生ネズミ等）が同じメモリアドレスを再利用した
                cleanupBombMouseMoveComp(entry);
                delete trackedBombMice[ptrKey];
                return null;
            }
        } catch (e) {
            cleanupBombMouseMoveComp(entry);
            delete trackedBombMice[ptrKey];
            return null;
        }
        return entry;
    }

    function configureSpawnedBombMouse(ratPtr) {
        if (!ratPtr || ratPtr.isNull() || isPendingKill(ratPtr)) {
            return;
        }
        ratPtr.add(OFFSETS.ADH_Character_MaximumHealth).writeFloat(CONFIG.bombMiceMaxHealth);
        ratPtr.add(OFFSETS.ADH_Character_CurrentHealth).writeFloat(CONFIG.bombMiceMaxHealth);
        ratPtr.add(OFFSETS.ADH_Character_IdleSpeed).writeFloat(CONFIG.bombMiceSpeed);
        ratPtr.add(OFFSETS.ADH_Character_RunSpeed).writeFloat(CONFIG.bombMiceSpeed);
        ratPtr.add(OFFSETS.ADH_Character_TeamID).writeU8(3); // 敵対陣営(非傀儡船員へ敵対)
        ratPtr.add(OFFSETS.ADH_AnimalCharacter_StuckSpeed).writeFloat(CONFIG.bombMiceStuckSpeed);
        ratPtr.add(OFFSETS.ADH_AnimalCharacter_StuckTime).writeFloat(CONFIG.bombMiceNativeStuckJumpSec);

        // 原版 MOD (BP_SummonedMice) の CharMoveComp 物理設定を召喚個体のみに適用（野生ネズミ CDO は一切不変）
        var moveComp = ratPtr.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        if (!moveComp.isNull() && !isPendingKill(moveComp)) {
            // 1) NavAgentProps / MovementState: bCanSwim(0x08) + bCanFly(0x10) を召喚個体のみ有効化（野生ネズミ CDO は不変）
            var navAgentFlags = moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).readU8();
            moveComp.add(OFFSETS.UNavMovementComponent_NavAgentProps_Flags).writeU8(navAgentFlags | 0x1E);
            var moveStateFlags = moveComp.add(OFFSETS.UNavMovementComponent_MovementState).readU8();
            moveComp.add(OFFSETS.UNavMovementComponent_MovementState).writeU8(moveStateFlags | 0x1E);

            // 2) bUseAccelerationForPaths(0x02) + bUseFixedBrakingDistanceForPaths(0x04) を有効化
            //    これにより滑らかな経路加減速 + ADH_AnimalCharacter::Tick の自動ジャンプ(GetCurrentAcceleration!=0)が機能する
            var navMoveFlags = moveComp.add(OFFSETS.UNavMovementComponent_Flags).readU8();
            moveComp.add(OFFSETS.UNavMovementComponent_Flags).writeU8(navMoveFlags | 0x06);

            // 3) 速度・急斜面踏破(78度)・岩段差踏破(65 UU)・加速度・旋回性能
            try {
                UCharacterMovementComponent_SetWalkableFloorAngle(moveComp, CONFIG.bombMiceWalkableFloorAngle);
            } catch (eAngle) {
                moveComp.add(OFFSETS.UCharacterMovementComponent_WalkableFloorAngle).writeFloat(CONFIG.bombMiceWalkableFloorAngle);
                moveComp.add(OFFSETS.UCharacterMovementComponent_WalkableFloorZ).writeFloat(Math.cos(CONFIG.bombMiceWalkableFloorAngle * Math.PI / 180.0));
            }
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxStepHeight).writeFloat(CONFIG.bombMiceMaxStepHeight);
            moveComp.add(OFFSETS.UCharacterMovementComponent_JumpZVelocity).writeFloat(CONFIG.bombMiceJumpZVelocity);
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxWalkSpeed).writeFloat(CONFIG.bombMiceSpeed);
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxWalkSpeedCrouched).writeFloat(CONFIG.bombMiceSpeed);
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxSwimSpeed).writeFloat(CONFIG.bombMiceSwimSpeed);
            moveComp.add(OFFSETS.UCharacterMovementComponent_MaxAcceleration).writeFloat(CONFIG.bombMiceMaxAcceleration);
            moveComp.add(OFFSETS.UCharacterMovementComponent_RotationRate + 4).writeFloat(CONFIG.bombMiceTurnRateYaw);

            // 4) 移動方向へ滑らかに向きを合わせる（bOrientRotationToMovement=1, bUseControllerDesiredRotation=0）
            var orientFlags = moveComp.add(OFFSETS.UCharacterMovementComponent_OrientFlags).readU8();
            moveComp.add(OFFSETS.UCharacterMovementComponent_OrientFlags).writeU8((orientFlags & 0xFB) | 0x08);

            // 5) 水辺・浅瀬・氷縁でのスタック防止（原版 BP_SummonedMice 固有設定）
            moveComp.add(OFFSETS.UDH_CharacterMovementComponent_AutoTreadDepthSpeed).writeFloat(0.0);
            moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bCanMoveUnderWater).writeU8(1);
            moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bAutoStepUpOutOfWater).writeU8(1);
            moveComp.add(OFFSETS.UDH_CharacterMovementComponent_bAlwaysAutoStepOutOfWater).writeU8(1);
            moveComp.add(OFFSETS.UDH_CharacterMovementComponent_MaxSwimDepth).writeFloat(CONFIG.bombMiceMaxSwimDepth);
        }

        try {
            ADH_Character_SetShouldRun(ratPtr, 1);
        } catch (e) {}
    }

    function registerSummonedBombMouse(ratPtr, spellKey, instigatorHuman, preferredTarget, nowTimeSec, explorerTargets) {
        if (!ratPtr || ratPtr.isNull() || isPendingKill(ratPtr)) {
            return false;
        }
        if ((ADH_Character_IsAlive(ratPtr) & 1) === 0) {
            return false;
        }
        var ident = buildBombMouseIdentity(ratPtr);
        if (finishedBombMouseIds[ident.identityKey]) {
            return false;
        }
        var existing = getTrackedBombMouse(ratPtr);
        if (existing) {
            return false;
        }
        // 二重安全確認: 野生ネズミ(BP_PreyController_C / TeamID!=3)は絶対に登録しない
        var ctrl = ratPtr.add(OFFSETS.APawn_Controller).readPointer();
        var ctrlName = (!ctrl.isNull() && !isPendingKill(ctrl)) ? readObjectClassName(ctrl) : 'null';
        var teamId = ratPtr.add(OFFSETS.ADH_Character_TeamID).readU8();
        if (ctrlName === 'BP_PreyController_C' || (teamId !== 3 && ctrlName !== 'AIController')) {
            log('[TS_BombMice] 野生ネズミ誤登録防止スキップ rat=' + ratPtr + ' ctrl=' + ctrlName + ' team=' + teamId);
            return false;
        }

        configureSpawnedBombMouse(ratPtr);
        var wtCur = getWorldAndTime();
        if (!shipCache.valid && !currentGameState.isNull()) {
            updateWarshipCache(currentGameState);
        }
        var mcPtr = ratPtr.add(OFFSETS.ACharacter_CharacterMovement).readPointer();
        var mcKey = (!mcPtr.isNull() && !isPendingKill(mcPtr)) ? mcPtr.toString() : '';
        var initLoc = getActorLocation(ratPtr);
        var startedOnShip = isBombMouseOnShip(ratPtr, initLoc);
        var entry = {
            rat: ratPtr,
            controller: ctrl,
            moveCompKey: mcKey,
            startedOnShip: startedOnShip,
            ptrKey: ident.ptrKey,
            internalIndex: ident.internalIndex,
            nameRaw: ident.nameRaw,
            identityKey: ident.identityKey,
            spellKey: spellKey,
            instigatorHuman: instigatorHuman,
            preferredTarget: preferredTarget,
            currentTarget: ptr(0),
            lastTargetLoc: null,
            lastProgressLoc: initLoc ? { x: initLoc.x, y: initLoc.y, z: initLoc.z } : null,
            lastProgressDist: null,
            nextStuckCheckAt: nowTimeSec + CONFIG.bombMiceStuckCheckIntervalSec,
            stuckCount: 0,
            nextVaultAt: 0.0,
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
            climbNextFloorCheckAt: 0.0,
            climbNextShipTraceAt: 0.0,
            climbPitchDeg: CONFIG.bombMiceClimbPitchDefaultDeg,
            walkableSlopeHits: 0,
            savedBrakingFly: 0.0,
            savedMaxFlySpeed: 600.0,
            detourUntil: 0.0,
            lastMoveTier: 0,
            nextRepathAt: nowTimeSec,
            nextIdleRetryAt: nowTimeSec,
            fusing: false,
            fuseAt: 0.0,
            detonated: false
        };
        trackedBombMice[ident.ptrKey] = entry;
        if (mcKey) {
            trackedBombMouseMoveComps[mcKey] = entry;
        }
        driveBombMousePathfinding(wtCur.world, entry, initLoc, explorerTargets, nowTimeSec);
        log('[TS_BombMice] 個体登録完了 id=' + ident.identityKey + ' ctrl=' + ctrlName +
            ' tier=' + entry.lastMoveTier +
            ' target=' + (entry.currentTarget.isNull() ? 'none' : entry.currentTarget));
        return true;
    }

    function executeBombMouseDetonation(entry) {
        if (!entry || !entry.rat || entry.rat.isNull() || isPendingKill(entry.rat)) {
            return;
        }
        var wt = getWorldAndTime();
        if (wt.world.isNull()) {
            return;
        }
        var loc = getActorLocation(entry.rat);
        var expCls = getBpExplosionClass();
        var dtCls = getDtCannibalBiteClass();

        if (loc && !expCls.isNull()) {
            // ゼロ CDO 改変（遅延スポーン）:
            //   ADH_ExplosionActor::BeginPlay (0xDBE200) で初めて bDamageOnSpawn / DealDamage が発火するため、
            //   BeginDeferredActorSpawnFromClass -> 個体インスタンスのみへ Damage=7.0 等を設定 -> FinishSpawningActor とすることで
            //   BP_Explosion_Gunpowder_OffShip_C の CDO を一瞬たりとも書き換えない（火薬樽・ニトロ・ボイラーへの波及を 100% 根絶）
            inBombMouseDetonation++;
            try {
                // FTransform (48B, 16-byte aligned: Quat(0,0,0,1), Trans(x,y,z,0), Scale(1,1,1,0))
                var rawBuf = Memory.alloc(64);
                var rem = rawBuf.and(0xF).toInt32();
                var xform = rem === 0 ? rawBuf : rawBuf.add(16 - rem);
                xform.writeFloat(0.0); xform.add(4).writeFloat(0.0); xform.add(8).writeFloat(0.0); xform.add(12).writeFloat(1.0);
                xform.add(16).writeFloat(loc.x); xform.add(20).writeFloat(loc.y); xform.add(24).writeFloat(loc.z); xform.add(28).writeFloat(0.0);
                xform.add(32).writeFloat(1.0); xform.add(36).writeFloat(1.0); xform.add(40).writeFloat(1.0); xform.add(44).writeFloat(0.0);

                var instigator = (entry.instigatorHuman && !entry.instigatorHuman.isNull() && !isPendingKill(entry.instigatorHuman))
                    ? entry.instigatorHuman : ptr(0);
                var worldCtx = !instigator.isNull() ? instigator : wt.world;

                var expActor = UGameplayStatics_BeginDeferredActorSpawnFromClass(
                    worldCtx,
                    expCls,
                    xform,
                    1, // ESpawnActorCollisionHandlingMethod::AlwaysSpawn
                    ptr(0)
                );
                if (expActor && !expActor.isNull() && !isPendingKill(expActor)) {
                    if (!instigator.isNull()) {
                        expActor.add(OFFSETS.AActor_Instigator).writePointer(instigator);
                    }
                    expActor.add(OFFSETS.ADH_ExplosionActor_Damage).writeFloat(CONFIG.bombMiceExplosionDamage);
                    expActor.add(OFFSETS.ADH_ExplosionActor_InnerRadius).writeFloat(CONFIG.bombMiceExplosionInnerRadius);
                    expActor.add(OFFSETS.ADH_ExplosionActor_OuterRadius).writeFloat(CONFIG.bombMiceExplosionOuterRadius);
                    expActor.add(OFFSETS.ADH_ExplosionActor_bDamageOnSpawn).writeU8(1);
                    if (!dtCls.isNull()) {
                        expActor.add(OFFSETS.ADH_ExplosionActor_DamageType).writePointer(dtCls);
                    }
                    UGameplayStatics_FinishSpawningActor(expActor, xform);
                    log('[TS_BombMice] 爆弾ネズミ単独起爆 id=' + entry.identityKey +
                        ' t=' + wt.timeSeconds.toFixed(2) +
                        ' dmg=' + CONFIG.bombMiceExplosionDamage +
                        ' loc=(' + loc.x.toFixed(0) + ',' + loc.y.toFixed(0) + ',' + loc.z.toFixed(0) + ')');
                }
            } finally {
                inBombMouseDetonation--;
                if (inBombMouseDetonation < 0) {
                    inBombMouseDetonation = 0;
                }
            }
        }

        var ctrlToClean = entry.controller;
        if ((!ctrlToClean || ctrlToClean.isNull()) && !isPendingKill(entry.rat)) {
            ctrlToClean = entry.rat.add(OFFSETS.APawn_Controller).readPointer();
        }
        if (ctrlToClean && !ctrlToClean.isNull() && !isPendingKill(ctrlToClean) &&
            readObjectClassName(ctrlToClean) === 'AIController') {
            try { AAIController_StopMovement(ctrlToClean); } catch (eStop) {}
        }

        if (!isPendingKill(entry.rat)) {
            UWorld_DestroyActor(wt.world, entry.rat, 0, 1);
        }
        if (ctrlToClean && !ctrlToClean.isNull() && !isPendingKill(ctrlToClean) &&
            readObjectClassName(ctrlToClean) === 'AIController') {
            try { UWorld_DestroyActor(wt.world, ctrlToClean, 0, 1); } catch (eCtrl) {}
        }
    }

    function detonateBombMouse(entry) {
        if (!entry || entry.detonated) {
            return;
        }
        if (entry.climbPhase !== 0) {
            exitBombMouseWallClimb(entry, entry.controller, 0.0);
        }
        entry.detonated = true;
        finishedBombMouseIds[entry.identityKey] = true;
        cleanupBombMouseMoveComp(entry);
        delete trackedBombMice[entry.ptrKey];

        // 再入防止：他個体の起爆処理中に呼ばれた場合はキューへ積み、CDO 復元後に順次処理する
        if (inBombMouseDetonation > 0) {
            pendingBombMouseDetonations.push(entry);
            return;
        }

        executeBombMouseDetonation(entry);
        while (pendingBombMouseDetonations.length > 0) {
            var nextEntry = pendingBombMouseDetonations.shift();
            if (nextEntry) {
                executeBombMouseDetonation(nextEntry);
            }
        }
    }

    function tickBombMice(wt) {
        // 防御的整合チェック：SpawnAI 外で BP_Prey_Rat CDO が汚染されたままにならないよう常に原版へ復元
        if (ratCdoSwapDepth === 0) {
            restoreWildRatCdoDefaults();
        }
        // 破棄済み爆弾ネズミの専用 AAIController を安全に回収
        drainPendingDestroyControllers(wt.world);

        var explorerTargets = null;
        function getExplorers() {
            if (explorerTargets === null) {
                explorerTargets = collectLivingExplorerTargets();
            }
            return explorerTargets;
        }

        // 1) アクティブな TS_BombMice_C の SummonedAIs に未登録個体があれば補完登録（主登録は SpawnAI onLeave）
        var sKeys = Object.keys(activeBombMiceSpells);
        for (var i = 0; i < sKeys.length; i++) {
            var sk = sKeys[i];
            var sEntry = activeBombMiceSpells[sk];
            if (!sEntry || !sEntry.spell || sEntry.spell.isNull() || isPendingKill(sEntry.spell)) {
                delete activeBombMiceSpells[sk];
                continue;
            }
            var preferredTarget = sEntry.spell.add(OFFSETS.UDH_TotemSpell_Summoning_CurrentCastingTarget).readPointer();
            var aiArr = sEntry.spell.add(OFFSETS.UDH_TotemSpell_Summoning_SummonedAIs);
            var aiData = aiArr.readPointer();
            var aiNum = aiArr.add(8).readS32();
            if (!aiData.isNull() && aiNum > 0 && aiNum <= 64) {
                for (var j = 0; j < aiNum; j++) {
                    var rat = aiData.add(j * 8).readPointer();
                    if (rat.isNull() || isPendingKill(rat)) {
                        continue;
                    }
                    registerSummonedBombMouse(rat, sk, sEntry.instigatorHuman, preferredTarget, wt.timeSeconds, getExplorers());
                }
            }
        }

        var rKeys = Object.keys(trackedBombMice);
        if (rKeys.length === 0) {
            return;
        }

        // 2) 各爆弾ネズミを 1 体ずつ完全独立に駆動（純JS NavMesh 追尾・個体別の接近検知・0.4s 引火タイマー・単独起爆）
        var explorers = getExplorers();
        for (var k = 0; k < rKeys.length; k++) {
            var rk2 = rKeys[k];
            var rawEntry = trackedBombMice[rk2];
            if (!rawEntry) {
                continue;
            }
            var mEntry = getTrackedBombMouse(rawEntry.rat);
            if (!mEntry) {
                continue;
            }
            if ((ADH_Character_IsAlive(mEntry.rat) & 1) === 0) {
                detonateBombMouse(mEntry);
                continue;
            }
            var rLoc = getActorLocation(mEntry.rat);
            if (mEntry.fusing) {
                if (wt.timeSeconds >= mEntry.fuseAt) {
                    detonateBombMouse(mEntry);
                    continue;
                }
                // 引火中(0.4s)も原版 MOD 同様に追尾を継続し、全力疾走中の探検家へ張り付いて起爆する
                driveBombMousePathfinding(wt.world, mEntry, rLoc, explorers, wt.timeSeconds);
                continue;
            }
            driveBombMousePathfinding(wt.world, mEntry, rLoc, explorers, wt.timeSeconds);
            if (!rLoc || explorers.length === 0) {
                continue;
            }
            for (var c = 0; c < explorers.length; c++) {
                if (distance3D(rLoc, explorers[c].loc) <= CONFIG.bombMiceTriggerRadiusUU) {
                    mEntry.fusing = true;
                    mEntry.fuseAt = wt.timeSeconds + CONFIG.bombMiceFuseSec;
                    log('[TS_BombMice] 個体引火開始 id=' + mEntry.identityKey +
                        ' t=' + wt.timeSeconds.toFixed(2) + ' fuseAt=' + mEntry.fuseAt.toFixed(2));
                    break;
                }
            }
        }
    }

    // --- [6] 船体爆破 (TS_BoomShip) ---
    function handleBoomShipCanCast(spellPtr, inCasterPtr) {
        var ctx = (inCasterPtr && !inCasterPtr.isNull()) ? inCasterPtr : spellPtr;
        if (!ctx || ctx.isNull()) {
            return false;
        }
        var warship = UDH_GameplayStatics_GetWarship(ctx);
        return !warship.isNull() && !isPendingKill(warship);
    }

    function handleBoomShipDoSpellEffects(spellPtr) {
        var casterPS = resolveCasterPlayerState(spellPtr, ptr(0));
        var casterPawn = getHumanFromPlayerState(casterPS);
        if (casterPawn.isNull()) {
            casterPawn = getCurrentPawnFromPlayerState(casterPS);
        }
        var ctx = !casterPawn.isNull() ? casterPawn : spellPtr;
        var warship = UDH_GameplayStatics_GetWarship(ctx);
        if (warship.isNull() || isPendingKill(warship)) {
            log('[TS_BoomShip] 発動失敗: 軍艦(Warship)が見つかりません');
            return false;
        }
        var shipLoc = getActorLocation(warship);
        if (!shipLoc) {
            return false;
        }
        var castTier = spellPtr.add(OFFSETS.UDH_TotemSpell_CastTier).readU8();
        var tierSpec = CONFIG.boomShipBreachesByTier[castTier] || CONFIG.boomShipBreachesByTier[2];

        var locVec = Memory.alloc(12);
        locVec.writeFloat(shipLoc.x);
        locVec.add(4).writeFloat(shipLoc.y);
        locVec.add(8).writeFloat(shipLoc.z);

        ADH_Warship_SpawnBreaches(warship, casterPawn, tierSpec.count, locVec, tierSpec.radius);
        log('[TS_BoomShip] 段階' + (castTier >= 2 ? (castTier - 1) : 0) +
            ' 破口数=' + tierSpec.count + ' 半径=' + tierSpec.radius);
        return true;
    }

    // --- [7] テレポート (TS_Teleport) ---
    function handleTeleportCanCast(spellPtr, inCasterPtr) {
        var casterPS = resolveCasterPlayerState(spellPtr, inCasterPtr);
        if (casterPS.isNull() || isPendingKill(casterPS)) {
            return false;
        }
        return casterPS.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() === 0;
    }

    function handleTeleportDoSpellEffects(spellPtr, targetPtr) {
        var wt = getWorldAndTime();
        if (wt.world.isNull()) {
            return false;
        }
        resetRuntimeStateIfWorldChanged(wt.world);

        if (!targetPtr || targetPtr.isNull() || isPendingKill(targetPtr)) {
            return false;
        }
        if (!isA(targetPtr, getThrallTotemClass())) {
            return false;
        }

        var casterPS = resolveCasterPlayerState(spellPtr, ptr(0));
        if (casterPS.isNull() || isPendingKill(casterPS)) {
            return false;
        }
        var castTier = spellPtr.add(OFFSETS.UDH_TotemSpell_CastTier).readU8();

        activeTeleportTargets[spellPtr.toString()] = {
            totem: targetPtr,
            casterPS: casterPS,
            castTier: castTier
        };

        // 発動直後の霊界入場エフェクト再生（既に霊界歩行中なら維持）
        var human = getHumanFromPlayerState(casterPS);
        if (isHumanAlive(human) && human.add(OFFSETS.ADH_HumanCharacter_bSpiritWalking).readU8() === 0) {
            clearSpiritWalkTimer(human);
            ADH_HumanCharacter_SetIsSpiritWalking(human, 1, CONFIG.teleportSpiritWalkFallbackSec);
            teleportSpiritWalkMap[human.toString()] = {
                human: human,
                exitAt: wt.timeSeconds + CONFIG.teleportSpiritWalkFallbackSec
            };
        }

        log('[TS_Teleport] 詠唱開始 段階' + (castTier >= 2 ? (castTier - 1) : 0) + ' target=' + targetPtr);
        return true;
    }

    function handleTeleportOnExpired(spellPtr) {
        var sKey = spellPtr.toString();
        var tp = activeTeleportTargets[sKey];
        delete activeTeleportTargets[sKey];
        if (!tp) {
            return;
        }

        var wt = getWorldAndTime();
        var casterPS = tp.casterPS;
        if (!casterPS || casterPS.isNull() || isPendingKill(casterPS)) {
            return;
        }

        // 投獄中 (ADH_PrisonerPawn) の場合は牢獄から解放してから移動
        var curPawn = getCurrentPawnFromPlayerState(casterPS);
        if (!curPawn.isNull() && !isPendingKill(curPawn) && isA(curPawn, getPrisonerPawnClass())) {
            var brig = ADH_PrisonerPawn_GetBrig(curPawn);
            if (!brig.isNull() && !isPendingKill(brig)) {
                ADH_BrigCell_FreePrisoner(brig, casterPS);
            }
        }

        var human = getHumanFromPlayerState(casterPS);
        if (!isHumanAlive(human)) {
            return;
        }

        // トーテムが移動前に破壊されていた場合は霊界解除のみ行う
        if (!tp.totem || tp.totem.isNull() || isPendingKill(tp.totem)) {
            var deadEntry = teleportSpiritWalkMap[human.toString()];
            if (deadEntry && !wt.world.isNull()) {
                deadEntry.exitAt = wt.timeSeconds + CONFIG.teleportSpiritWalkExitDelaySec;
            }
            return;
        }

        // ニトログリセリン所持時の破壊／耐久0処理（原版 BreakNitro と同一仕様）
        var selectedInv = ADH_HumanCharacter_GetSelectedInventory(human);
        if (!selectedInv.isNull() && !isPendingKill(selectedInv) && isA(selectedInv, getNitroInventoryClass())) {
            if (tp.castTier === 2) {
                ADH_Inventory_DecreaseItemStack(selectedInv, 1, 0, 1);
            } else {
                ADH_Inventory_SetDurability(selectedInv, 0.0);
            }
        }

        var totemLoc = getActorLocation(tp.totem);
        if (totemLoc) {
            var destLoc = Memory.alloc(12);
            destLoc.writeFloat(totemLoc.x);
            destLoc.add(4).writeFloat(totemLoc.y);
            destLoc.add(8).writeFloat(totemLoc.z + 100.0);

            var destRot = Memory.alloc(12);
            destRot.writeFloat(0.0);
            destRot.add(4).writeFloat(0.0);
            destRot.add(8).writeFloat(0.0);

            AActor_K2_TeleportTo(human, destLoc, destRot);

            // Tier 1 (TST_ONE) のみトーテム破壊
            if (tp.castTier === 2) {
                ADH_DestructibleActor_BreakApart(tp.totem, 0);
            }

            // 段階別体力倍率（Tier1=0.8, Tier2=0.9, Tier3=1.0）
            var hpFactor = CONFIG.teleportHealthFactorByTier[tp.castTier];
            if (typeof hpFactor === 'number' && hpFactor > 0.0 && hpFactor < 1.0) {
                var curHp = human.add(OFFSETS.ADH_Character_CurrentHealth).readFloat();
                if (Number.isFinite(curHp) && curHp > 0.0) {
                    ADH_Character_SetCurrentHealth(human, curHp * hpFactor);
                }
            }
            log('[TS_Teleport] 転移完了 段階' + (tp.castTier >= 2 ? (tp.castTier - 1) : 0) +
                ' dest=(' + totemLoc.x.toFixed(0) + ',' + totemLoc.y.toFixed(0) + ',' + (totemLoc.z + 100).toFixed(0) + ')');
        }

        var entry = teleportSpiritWalkMap[human.toString()];
        if (entry && !wt.world.isNull()) {
            entry.exitAt = wt.timeSeconds + CONFIG.teleportSpiritWalkExitDelaySec;
        }
    }

    // --- [8] 威嚇 (TS_Intimidate): 通知なし・効果音のみ・霊界同等の即発動 ---
    function handleIntimidateDoSpellEffects(spellPtr, targetPtr) {
        var wt = getWorldAndTime();
        if (wt.world.isNull()) {
            return false;
        }
        resetRuntimeStateIfWorldChanged(wt.world);

        var casterPS = resolveCasterPlayerState(spellPtr, ptr(0));
        var casterHuman = getHumanFromPlayerState(casterPS);
        var castTier = spellPtr.add(OFFSETS.UDH_TotemSpell_CastTier).readU8();
        var duration = CONFIG.intimidateDurationsByTier[castTier] || 5.0;

        // STS_CASTER: 術者自身を中心に範囲判定
        var center = getActorLocation(casterHuman);
        if (!center) {
            center = getActorLocation(targetPtr);
        }
        if (!center) {
            log('[TS_Intimidate] 発動失敗: 術者座標を取得できません');
            return false;
        }

        var expireAt = wt.timeSeconds + duration;
        var allPS = getAllPlayerStates();
        var hitCount = 0;

        for (var i = 0; i < allPS.length; i++) {
            var ps = allPS[i];
            if (ps.add(OFFSETS.ADH_PlayerState_bIsThrall).readU8() !== 0) { continue; }
            if (ps.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() !== 0) { continue; }

            var human = getHumanFromPlayerState(ps);
            if (!isHumanAlive(human)) { continue; }

            var loc = getActorLocation(human);
            if (!loc || distance3D(center, loc) > CONFIG.intimidateRadiusUU) { continue; }

            var invMgr = human.add(OFFSETS.ADH_HumanCharacter_InventoryComponent).readPointer();
            var pc = getControllerFromPlayerState(ps);
            if (pc.isNull()) {
                var pawnCtrl = human.add(OFFSETS.APawn_Controller).readPointer();
                if (!pawnCtrl.isNull() && !isPendingKill(pawnCtrl) && isA(pawnCtrl, getPlayerControllerClass())) {
                    pc = pawnCtrl;
                }
            }
            var hKey = human.toString();
            var prev = intimidatedHumans[hKey];
            var nextExpire = prev ? Math.max(prev.expireTime, expireAt) : expireAt;

            intimidatedHumans[hKey] = {
                human: human,
                ps: ps,
                pc: pc,
                invMgr: invMgr,
                expireTime: nextExpire
            };

            if (!invMgr.isNull() && !isPendingKill(invMgr)) {
                if (origAimInventory) { origAimInventory(invMgr, 0); }
                forceHolsterNow(invMgr);
            }

            // 暗視野カメラフェード（ClientSetCameraFade: 行動阻害なし・現在値から滑らかに 0.58 黒フェード保持）
            if (!pc.isNull()) {
                applyIntimidateCameraFade(pc, true, CONFIG.intimidateCameraFadeAlpha, true);
            }

            // スタミナをちょうど 0.0 まで清空（負の疲労値や強制5秒遅延を課さず、即座に通常どおり回復開始）
            try {
                var curStam = human.add(OFFSETS.ADH_HumanCharacter_CurrentStamina).readFloat();
                if (Number.isFinite(curStam) && curStam > 0.0) {
                    ADH_HumanCharacter_ModifyStamina(human, -curStam);
                }
            } catch (eStam) {}
            hitCount++;
        }

        // 通知なし（クライアント側 sfx_IntimidateCast 効果音のみ再生）
        log('[TS_Intimidate] 段階' + (castTier >= 2 ? (castTier - 1) : 0) +
            ' 持続=' + duration + 's 半径=' + CONFIG.intimidateRadiusUU + 'UU 命中=' + hitCount + '人');
        return true;
    }

    // --- [9] エネルギー譲渡 (TS_Transfer): 仲間の傀儡のみ厳格指定 ---
    function isLivingThrallHumanTarget(actorPtr) {
        if (!actorPtr || actorPtr.isNull() || isPendingKill(actorPtr)) {
            return false;
        }
        if (!isHumanAlive(actorPtr)) {
            return false;
        }
        var ps = resolvePlayerStateFromActor(actorPtr);
        if (ps.isNull() || isPendingKill(ps)) {
            return false;
        }
        if (ps.add(OFFSETS.ADH_PlayerState_bIsThrall).readU8() === 0) {
            return false;
        }
        if (ps.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() !== 0) {
            return false;
        }
        var sm = ps.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
        return !sm.isNull() && !isPendingKill(sm);
    }

    function hasLivingThrallTeammate(casterPS) {
        if (!casterPS || casterPS.isNull() || isPendingKill(casterPS)) {
            return false;
        }
        var psClass = getPlayerStateClass();
        function checkCandidate(ps) {
            if (!ps || ps.isNull() || ps.equals(casterPS) || isPendingKill(ps) || !isA(ps, psClass)) {
                return false;
            }
            if (ps.add(OFFSETS.ADH_PlayerState_bIsThrall).readU8() === 0) { return false; }
            if (ps.add(OFFSETS.ADH_PlayerState_bIsDead).readU8() !== 0) { return false; }
            var sm = ps.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
            return !sm.isNull() && !isPendingKill(sm);
        }

        var otherPtr = casterPS.add(OFFSETS.ADH_PlayerState_OtherThralls);
        var otherData = otherPtr.readPointer();
        var otherNum = otherPtr.add(8).readS32();
        if (!otherData.isNull() && otherNum > 0 && otherNum <= 64) {
            for (var i = 0; i < otherNum; i++) {
                if (checkCandidate(otherData.add(i * 8).readPointer())) { return true; }
            }
        }
        var allPS = getAllPlayerStates();
        for (var j = 0; j < allPS.length; j++) {
            if (checkCandidate(allPS[j])) { return true; }
        }
        return false;
    }

    function readTransferDeltaForTier(allySpellMgr, castTier) {
        if (castTier === 4) {
            return CONFIG.transferChargeByTier[4];
        }
        try {
            var mapData = allySpellMgr.add(OFFSETS.ADH_SpellManager_SpellTierLevels).readPointer();
            var mapNum = allySpellMgr.add(OFFSETS.ADH_SpellManager_SpellTierLevels + 8).readS32();
            if (!mapData.isNull() && mapNum > 0 && mapNum <= 16) {
                for (var i = 0; i < mapNum; i++) {
                    var elem = mapData.add(i * 16);
                    var key = elem.readU8();
                    if (key === castTier) {
                        var val = elem.add(4).readFloat();
                        if (Number.isFinite(val) && val > 0.0 && val <= 1.0) {
                            return Math.min(1.0, val + 0.005);
                        }
                    }
                }
            }
        } catch (e) {}
        return CONFIG.transferChargeByTier[castTier] || 0.0;
    }

    function handleTransferCanCast(spellPtr, inCasterPtr) {
        var casterPS = resolveCasterPlayerState(spellPtr, inCasterPtr);
        return hasLivingThrallTeammate(casterPS);
    }

    function handleTransferDoSpellEffects(spellPtr, targetPtr) {
        var casterPS = resolveCasterPlayerState(spellPtr, ptr(0));
        if (casterPS.isNull() || isPendingKill(casterPS)) {
            return false;
        }
        var castTier = spellPtr.add(OFFSETS.UDH_TotemSpell_CastTier).readU8();
        var tierDisplay = castTier >= 2 ? (castTier - 1) : 0;
        if (tierDisplay <= 0 || tierDisplay > 3) {
            return false;
        }

        // 厳格チェック: 指定先が生存中の仲間傀儡(自分以外)でなければ拒否（エネルギー・CD 消費なし）
        if (!isLivingThrallHumanTarget(targetPtr)) {
            log('[TS_Transfer] ターゲット拒否: 仲間の傀儡ではありません target=' + targetPtr);
            return false;
        }
        var allyPS = resolvePlayerStateFromActor(targetPtr);
        if (allyPS.isNull() || allyPS.equals(casterPS)) {
            log('[TS_Transfer] ターゲット拒否: 自分自身または無効対象');
            return false;
        }
        var allySM = allyPS.add(OFFSETS.ADH_PlayerState_SpellManager).readPointer();
        if (allySM.isNull() || isPendingKill(allySM)) {
            return false;
        }

        var delta = readTransferDeltaForTier(allySM, castTier);
        if (delta <= 0.0) {
            return false;
        }

        var beforeCharge = allySM.add(OFFSETS.ADH_SpellManager_SpellChargeLevel).readFloat();
        ADH_SpellManager_ModifySpellChargeLevel(allySM, delta);
        var afterCharge = allySM.add(OFFSETS.ADH_SpellManager_SpellChargeLevel).readFloat();

        // 通知なし（クライアント側 Transfer_cast 効果音のみ再生）
        log('[TS_Transfer] 段階' + tierDisplay + ' (+' + delta.toFixed(3) + ') 味方チャージ: ' +
            beforeCharge.toFixed(3) + ' -> ' + afterCharge.toFixed(3));
        return true;
    }

    // ========================================================================
    // 9. トーテムスペル実行フック（Interceptor.replaceFast：onLeave 消費 0）
    // ========================================================================
    var origIsValidCastingTarget = null;
    var origCanCast = null;
    var origDoSpellEffects = null;
    var origOnCast = null;
    var origOnExpired = null;

    function installSpellExecutionHooks() {
        // 9-0. UDH_TotemSpell::IsValidCastingTarget(this, AActor const* InTarget) -> uint8
        var validTargetCb = new NativeCallback(function (spellPtr, targetPtr) {
            try {
                var clsName = readObjectClassName(spellPtr);
                if (clsName === 'TS_Transfer_C') {
                    return isLivingThrallHumanTarget(targetPtr) ? 1 : 0;
                }
            } catch (e) {}
            return origIsValidCastingTarget ? origIsValidCastingTarget(spellPtr, targetPtr) : 0;
        }, 'uint8', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(validTargetCb);
        origIsValidCastingTarget = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_TotemSpell_IsValidCastingTarget), validTargetCb),
            'uint8', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-1. UDH_TotemSpell::CanCast(this, AActor const* InCaster) -> uint8
        var canCastCb = new NativeCallback(function (spellPtr, inCasterPtr) {
            try {
                var clsName = readObjectClassName(spellPtr);
                if (clsName === 'TS_Transfer_C') { return handleTransferCanCast(spellPtr, inCasterPtr) ? 1 : 0; }
                if (clsName === 'TS_Intimidate_C') { return 1; }
                if (clsName === 'TS_BoomShip_C') { return handleBoomShipCanCast(spellPtr, inCasterPtr) ? 1 : 0; }
                if (clsName === 'TS_Teleport_C') { return handleTeleportCanCast(spellPtr, inCasterPtr) ? 1 : 0; }
            } catch (e) {
                log('CanCast 例外: ' + e.message);
            }
            return origCanCast ? origCanCast(spellPtr, inCasterPtr) : 0;
        }, 'uint8', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(canCastCb);
        origCanCast = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_TotemSpell_CanCast), canCastCb),
            'uint8', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-2. UDH_TotemSpell::DoSpellEffects(this, AActor* SpellTarget) -> uint8
        var doEffectsCb = new NativeCallback(function (spellPtr, targetPtr) {
            var clsName = '';
            try {
                clsName = readObjectClassName(spellPtr);
                if (clsName === 'TS_Transfer_C') { return handleTransferDoSpellEffects(spellPtr, targetPtr) ? 1 : 0; }
                if (clsName === 'TS_Intimidate_C') { return handleIntimidateDoSpellEffects(spellPtr, targetPtr) ? 1 : 0; }
                if (clsName === 'TS_BoomShip_C') { return handleBoomShipDoSpellEffects(spellPtr) ? 1 : 0; }
                if (clsName === 'TS_Teleport_C') { return handleTeleportDoSpellEffects(spellPtr, targetPtr) ? 1 : 0; }
            } catch (e) {
                log('DoSpellEffects 独自ハンドラ例外 (' + clsName + '): ' + e.message);
                return 0;
            }

            // TS_BombMice_C はネイティブ UDH_TotemSpell_Summoning::DoSpellEffects_Implementation を実行し召喚を追跡
            var res = origDoSpellEffects ? origDoSpellEffects(spellPtr, targetPtr) : 0;
            if (clsName === 'TS_BombMice_C' && (res & 1) !== 0) {
                try {
                    var wt = getWorldAndTime();
                    if (!wt.world.isNull()) { resetRuntimeStateIfWorldChanged(wt.world); }
                    var casterPS = resolveCasterPlayerState(spellPtr, ptr(0));
                    var instigatorHuman = getHumanFromPlayerState(casterPS);
                    activeBombMiceSpells[spellPtr.toString()] = {
                        spell: spellPtr,
                        instigatorHuman: instigatorHuman
                    };
                    log('[TS_BombMice] 召喚開始 spell=' + spellPtr + ' tier=' + spellPtr.add(OFFSETS.UDH_TotemSpell_CastTier).readU8());
                } catch (e2) {
                    log('TS_BombMice 追跡登録例外: ' + e2.message);
                }
            }
            return res;
        }, 'uint8', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(doEffectsCb);
        origDoSpellEffects = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_TotemSpell_DoSpellEffects), doEffectsCb),
            'uint8', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-3. UDH_TotemSpell::OnCast(this, AActor const* WorldContext) -> void
        var onCastCb = new NativeCallback(function (spellPtr, worldContextPtr) {
            try {
                var clsName = readObjectClassName(spellPtr);
                if (clsName === 'TS_Transfer_C' || clsName === 'TS_Intimidate_C' || clsName === 'TS_BoomShip_C' || clsName === 'TS_Teleport_C') {
                    return;
                }
            } catch (e) {}
            if (origOnCast) { origOnCast(spellPtr, worldContextPtr); }
        }, 'void', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(onCastCb);
        origOnCast = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_TotemSpell_OnCast), onCastCb),
            'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-4. UDH_TotemSpell::OnExpired(this, AActor const* WorldContext) -> void
        var onExpiredCb = new NativeCallback(function (spellPtr, worldContextPtr) {
            var clsName = '';
            try {
                clsName = readObjectClassName(spellPtr);
                if (clsName === 'TS_Teleport_C') {
                    handleTeleportOnExpired(spellPtr);
                    return;
                }
                if (clsName === 'TS_Transfer_C' || clsName === 'TS_Intimidate_C' || clsName === 'TS_BoomShip_C') {
                    return;
                }
                if (clsName === 'TS_BombMice_C') {
                    // 持続終了で消滅するネズミは爆発させずに追跡解除＆完了マーク＆専用コントローラ遅延回収
                    var expSpellKey = spellPtr.toString();
                    delete activeBombMiceSpells[expSpellKey];
                    var rKeys = Object.keys(trackedBombMice);
                    for (var i = 0; i < rKeys.length; i++) {
                        var expEntry = trackedBombMice[rKeys[i]];
                        if (expEntry && expEntry.spellKey === expSpellKey) {
                            if (expEntry.climbPhase !== 0) {
                                exitBombMouseWallClimb(expEntry, expEntry.controller, 0.0);
                            }
                            expEntry.detonated = true;
                            finishedBombMouseIds[expEntry.identityKey] = true;
                            cleanupBombMouseMoveComp(expEntry);
                            scheduleBombMouseControllerCleanup(expEntry.controller);
                            delete trackedBombMice[rKeys[i]];
                        }
                    }
                }
            } catch (e) {
                log('OnExpired 例外 (' + clsName + '): ' + e.message);
            }
            if (origOnExpired) { origOnExpired(spellPtr, worldContextPtr); }
        }, 'void', ['pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(onExpiredCb);
        origOnExpired = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.UDH_TotemSpell_OnExpired), onExpiredCb),
            'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-5. UDH_TotemSpell_Summoning::SpawnAI（爆弾ネズミ召喚時のみ BP_Prey_Rat CDO の AIControllerClass /
        //      AutoPossessAI / TeamID を一時的に純JS経路探索用 AAIController へ切替え、
        //      onLeave で即座に不変スナップショット(wildRatCdoDefaults)へ復元し、新規生成個体のみを独立登録）
        //      ※ 0xE84190 上で唯一のフック（onLeave 計 1 <= 2）
        Interceptor.attach(moduleBase.add(RVA.UDH_TotemSpell_Summoning_SpawnAI), {
            onEnter: function (args) {
                this.didSwap = false;
                this.spellPtr = ptr(0);
                this.prevAiCount = 0;
                try {
                    var spellPtr = args[0];
                    if (readObjectClassName(spellPtr) !== 'TS_BombMice_C') {
                        return;
                    }
                    var ratCls = cachedClasses.bpPreyRat;
                    if (ratCls.isNull()) {
                        ratCls = loadVanillaClass(VANILLA_PATHS.BP_Prey_Rat);
                        cachedClasses.bpPreyRat = ratCls;
                    }
                    if (ratCls.isNull()) {
                        return;
                    }
                    captureWildRatCdoDefaultsOnce(ratCls);
                    var ratCdo = ratCls.add(OFFSETS.UClass_ClassDefaultObject).readPointer();
                    if (ratCdo.isNull() || isPendingKill(ratCdo)) {
                        return;
                    }
                    var desiredCtrlCls = spellPtr.add(OFFSETS.UDH_TotemSpell_Summoning_AIControllerClass).readPointer();
                    this.spellPtr = spellPtr;
                    var aiArr = spellPtr.add(OFFSETS.UDH_TotemSpell_Summoning_SummonedAIs);
                    var aiNum = aiArr.add(8).readS32();
                    this.prevAiCount = (aiNum >= 0 && aiNum <= 64) ? aiNum : 0;

                    ratCdoSwapDepth++;
                    this.didSwap = true;
                    ratCdo.add(OFFSETS.APawn_AutoPossessAI).writeU8(1); // PlacedInWorld のみ（SpawnDefaultController に委譲）
                    ratCdo.add(OFFSETS.ADH_Character_TeamID).writeU8(3); // 食人鬼陣営
                    if (!desiredCtrlCls.isNull() && !isPendingKill(desiredCtrlCls)) {
                        ratCdo.add(OFFSETS.APawn_AIControllerClass).writePointer(desiredCtrlCls);
                    }
                } catch (e) {
                    if (this.didSwap) {
                        ratCdoSwapDepth = Math.max(0, ratCdoSwapDepth - 1);
                        if (ratCdoSwapDepth === 0) { restoreWildRatCdoDefaults(); }
                        this.didSwap = false;
                    }
                }
            },
            onLeave: function () {
                try {
                    if (this.didSwap) {
                        ratCdoSwapDepth = Math.max(0, ratCdoSwapDepth - 1);
                        if (ratCdoSwapDepth === 0) {
                            restoreWildRatCdoDefaults();
                        }
                    }
                    if (this.spellPtr && !this.spellPtr.isNull() && !isPendingKill(this.spellPtr)) {
                        var sk = this.spellPtr.toString();
                        var sEntry = activeBombMiceSpells[sk];
                        var instigatorHuman = sEntry ? sEntry.instigatorHuman : getHumanFromPlayerState(resolveCasterPlayerState(this.spellPtr, ptr(0)));
                        var preferredTarget = this.spellPtr.add(OFFSETS.UDH_TotemSpell_Summoning_CurrentCastingTarget).readPointer();
                        var aiArr = this.spellPtr.add(OFFSETS.UDH_TotemSpell_Summoning_SummonedAIs);
                        var aiData = aiArr.readPointer();
                        var aiNum = aiArr.add(8).readS32();
                        var wt = getWorldAndTime();
                        if (!aiData.isNull() && aiNum > this.prevAiCount && aiNum <= 64) {
                            var explorers = collectLivingExplorerTargets();
                            for (var j = this.prevAiCount; j < aiNum; j++) {
                                var newRat = aiData.add(j * 8).readPointer();
                                registerSummonedBombMouse(newRat, sk, instigatorHuman, preferredTarget, wt.timeSeconds, explorers);
                            }
                        }
                    }
                } catch (e) {}
            }
        });

        // 9-6. ADH_AnimalCharacter::TakeDamage（爆弾ネズミ同士の爆風連鎖を 100% 遮断・onLeave 消費 0）
        //      原版 BP_SummonedMice の DamageResistances[DT_CannibalBite_C] = 100.0 と同等の爆風耐性を付与し、
        //      さらに野生ネズミ（追跡外）には一切干渉しない
        var origAnimalTakeDamage = null;
        var animalTakeDamageCb = new NativeCallback(function (animalPtr, damageAmount, damageEventPtr, instigatorPtr, damageCauserPtr) {
            try {
                var bombEntry = getTrackedBombMouse(animalPtr);
                if (bombEntry) {
                    // 他の爆弾ネズミの起爆中（inBombMouseDetonation > 0）、または DT_CannibalBite_C 由来ダメージを完全無効化
                    if (inBombMouseDetonation > 0) {
                        return 0.0;
                    }
                    if (damageEventPtr && !damageEventPtr.isNull()) {
                        var dtCls = getDtCannibalBiteClass();
                        var dmgTypePtr = damageEventPtr.add(0x08).readPointer();
                        if (!dtCls.isNull() && !dmgTypePtr.isNull() && dmgTypePtr.equals(dtCls)) {
                            return 0.0;
                        }
                    }
                }
            } catch (e) {}
            return origAnimalTakeDamage
                ? origAnimalTakeDamage(animalPtr, damageAmount, damageEventPtr, instigatorPtr, damageCauserPtr)
                : 0.0;
        }, 'float', ['pointer', 'float', 'pointer', 'pointer', 'pointer'], 'win64');
        pinnedCallbacks.push(animalTakeDamageCb);
        origAnimalTakeDamage = new NativeFunction(
            Interceptor.replaceFast(moduleBase.add(RVA.ADH_AnimalCharacter_TakeDamage), animalTakeDamageCb),
            'float', ['pointer', 'float', 'pointer', 'pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' }
        );

        // 9-7. ADH_AnimalCharacter::Destroyed（アクタ破棄時に即座に追跡解除し、ヒープ再利用での野生ネズミ誤爆を根絶・onEnter のみ）
        Interceptor.attach(moduleBase.add(RVA.ADH_AnimalCharacter_Destroyed), {
            onEnter: function (args) {
                try {
                    var animalPtr = args[0];
                    var entry = getTrackedBombMouse(animalPtr);
                    if (entry) {
                        if (entry.climbPhase !== 0) {
                            exitBombMouseWallClimb(entry, entry.controller, 0.0);
                        }
                        entry.detonated = true;
                        finishedBombMouseIds[entry.identityKey] = true;
                        cleanupBombMouseMoveComp(entry);
                        scheduleBombMouseControllerCleanup(entry.controller);
                        delete trackedBombMice[entry.ptrKey];
                    }
                } catch (e) {}
            }
        });

        // 9-8. ADH_Character::OnDied（追跡中の爆弾ネズミ撃破時即爆発 + 威嚇中の被弾者死亡時フェード即時解除・onEnter のみ）
        Interceptor.attach(moduleBase.add(RVA.ADH_Character_OnDied), {
            onEnter: function (args) {
                try {
                    var charPtr = args[0];
                    if (charPtr && !charPtr.isNull()) {
                        var intKey = charPtr.toString();
                        if (intimidatedHumans[intKey]) {
                            clearIntimidateEntry(intKey, intimidatedHumans[intKey]);
                        }
                    }
                    var entry = getTrackedBombMouse(charPtr);
                    if (entry) {
                        detonateBombMouse(entry);
                    }
                } catch (e) {}
            }
        });
        // 9-9. UCharacterMovementComponent::IsWalkable (2B 受動物理防爬ガード: Interceptor.attach で食人鬼登攀補強.js と安全共存、0x241D1E0 上の onLeave 総数 <= 2)
        Interceptor.attach(moduleBase.add(RVA.UCharacterMovementComponent_IsWalkable), {
            onEnter: function (args) {
                this.blockShipWalk = false;
                try {
                    var mc = args[0];
                    if (!mc || mc.isNull()) {
                        return;
                    }
                    var entry = trackedBombMouseMoveComps[mc.toString()];
                    if (!entry) {
                        return; // プレイヤー・野生ネズミ・その他野生動物は O(1) 即時素通し
                    }
                    var hitPtr = args[1];
                    if (!hitPtr || hitPtr.isNull() || (hitPtr.readU8() & 1) === 0) {
                        return;
                    }
                    var hitActor = resolveWeakObject(hitPtr.add(OFFSETS.FHitResult_Actor));
                    var hitComp = resolveWeakObject(hitPtr.add(OFFSETS.FHitResult_Component));
                    if (isWarshipOrGangwayActor(hitActor, hitComp)) {
                        if (!entry.startedOnShip) {
                            // 2B: 船外召喚の爆弾ネズミは船殻・舷梯・甲板縁を一切歩行不可
                            this.blockShipWalk = true;
                        } else if (hitPtr.add(OFFSETS.FHitResult_ImpactNormal + 8).readFloat() < 0.71) {
                            // 船上召喚個体でも急傾斜(船縁・マスト等)は登らせない
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
    }

    // ========================================================================
    // 10. GameThread Tick（威嚇維持・テレポート霊界出現・爆弾ネズミ起爆）
    //     ※ onEnter のみ（Frida 16.7.19 の >=3 onLeave 回避）
    // ========================================================================
    function installGameStateTickHook() {
        Interceptor.attach(moduleBase.add(RVA.ADH_GameState_Tick), {
            onEnter: function (args) {
                try {
                    currentGameState = args[0];
                    var wt = getWorldAndTime();
                    if (wt.world.isNull()) {
                        return;
                    }
                    resetRuntimeStateIfWorldChanged(wt.world);
                    updateWarshipCache(currentGameState);

                    // 1) 威嚇 (TS_Intimidate) 丸腰維持＆期限切れ/死亡時の暗視野フェード解除
                    var intKeys = Object.keys(intimidatedHumans);
                    for (var i = 0; i < intKeys.length; i++) {
                        var ik = intKeys[i];
                        var ie = intimidatedHumans[ik];
                        if (!ie || wt.timeSeconds >= ie.expireTime || !isHumanAlive(ie.human)) {
                            clearIntimidateEntry(ik, ie);
                            continue;
                        }
                        forceHolsterNow(ie.invMgr);
                    }

                    // 2) テレポート (TS_Teleport) 霊界出現エフェクトの実行＆後処理
                    var tpKeys = Object.keys(teleportSpiritWalkMap);
                    for (var j = 0; j < tpKeys.length; j++) {
                        var tk = tpKeys[j];
                        var te = teleportSpiritWalkMap[tk];
                        if (!te || !isHumanAlive(te.human)) {
                            delete teleportSpiritWalkMap[tk];
                            continue;
                        }
                        if (wt.timeSeconds >= te.exitAt) {
                            delete teleportSpiritWalkMap[tk];
                            clearSpiritWalkTimer(te.human);
                            if (te.human.add(OFFSETS.ADH_HumanCharacter_bSpiritWalking).readU8() !== 0) {
                                ADH_HumanCharacter_SetIsSpiritWalking(te.human, 0, 0.0);
                            }
                        }
                    }

                    // 3) 爆弾ネズミ (TS_BombMice) 召喚追跡＆接近0.4s起爆
                    tickBombMice(wt);
                } catch (e) {}
            }
        });
    }

    // ========================================================================
    // 11. 起動処理
    // ========================================================================
    installGameInstanceRegistrationHooks();
    installInventoryLockHooks();
    installSpellExecutionHooks();
    installGameStateTickHook();
    log('カスタムスペル Option C 初期化完了（完全ゼロ資産・純JS UClass 合成 / 5スペル実装）');
})();
