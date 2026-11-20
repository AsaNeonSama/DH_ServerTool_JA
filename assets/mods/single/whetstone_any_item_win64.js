var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

// 砥石によるダメージ加算値 (+5)
var UPGRADE_BONUS_DAMAGE = 5.0;

// ============================================================================
// UE4 Native Functions & Hook Addresses
// ============================================================================
var ADH_Inventory_CanBeAppliedTo_addr = moduleBase.add(0xECA220);
var ADH_Weapon_GetDamage_Impl_je_addr = moduleBase.add(0xE00F97);
var ADH_Weapon_GetDamage_addr = moduleBase.add(0xF0A7F0);
var ADH_Weapon_Ranged_Reload_addr = moduleBase.add(0xE0E600);
var ADH_InventoryPickup_Launch_addr = moduleBase.add(0xDD40A0);
var ADH_InventoryPickup_ApplyThrownDamage_addr = moduleBase.add(0xDC4CE0);

var UClass_GetPrivateStaticClass = new NativeFunction(moduleBase.add(0x11F02E0), 'pointer', [], 'win64');
var StaticFindObject = new NativeFunction(moduleBase.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var ADH_Weapon_Ranged_ReloadingConsumesAmmo = new NativeFunction(moduleBase.add(0xF0AEF0), 'uint8', ['pointer'], 'win64');
var UDH_InventoryManager_FindInventory = new NativeFunction(moduleBase.add(0xDC8730), 'pointer', ['pointer', 'pointer', 'int32'], 'win64');

var origWeaponGetDamage = null;
var origApplyThrownDamage = null;

// ============================================================================
// UE4 Struct Offsets
// ============================================================================
var OFFSETS = {
    actor_role: 0xF0,
    inventory_humanOwner: 0x268,
    inventory_bHasDurability: 0x441,
    inventory_bUpgradeItem: 0x458,
    inventory_damage: 0x868,
    inventory_slotIndex: 0x870,
    inventory_itemStatesData: 0x8B8,
    inventory_itemStatesNum: 0x8C0,
    inventory_replicatedTopItemUID: 0x8C8,
    inventory_replicatedTopUsesRemaining: 0x8CC,
    inventory_replicatedTopDurability: 0x8D0,
    inventory_replicatedTopUpgraded: 0x8D5,
    itemState_size: 0x38,
    itemState_bUpgraded: 0x0D,
    human_inventoryManager: 0x808,
    invManager_transactingInventory: 0xE0,
    weaponRanged_ammunitionClass: 0x928,
    pickup_bDamaging: 0x256,
    pickup_damageType: 0x258,
    pickup_damageRampStart: 0x260,
    pickup_maxDamageMultiplier: 0x264,
    pickup_saveStateUpgraded: 0x28D
};

var ROLE_AUTHORITY = 3;
var DEFAULT_THROWN_DAMAGE_RAMP_START = 300.0;
var DEFAULT_THROWN_MAX_DAMAGE_MULTIPLIER = 1.0;
var MAX_TRACKED_AMMO_WEAPONS = 512;

var cachedDefaultDamageTypeClass = ptr(0);

// Tracks ranged weapons currently loaded with a sharpened ammo item: weaponPtrStr -> weaponItemUID
var loadedUpgradedAmmo = {};
var loadedUpgradedAmmoCount = 0;

function setLoadedUpgradedAmmo(weaponKey, weaponUID) {
    if (!Object.prototype.hasOwnProperty.call(loadedUpgradedAmmo, weaponKey)) {
        if (loadedUpgradedAmmoCount >= MAX_TRACKED_AMMO_WEAPONS) {
            loadedUpgradedAmmo = {};
            loadedUpgradedAmmoCount = 0;
        }
        loadedUpgradedAmmoCount++;
    }
    loadedUpgradedAmmo[weaponKey] = weaponUID;
}

function clearLoadedUpgradedAmmo(weaponKey) {
    if (Object.prototype.hasOwnProperty.call(loadedUpgradedAmmo, weaponKey)) {
        delete loadedUpgradedAmmo[weaponKey];
        if (loadedUpgradedAmmoCount > 0) {
            loadedUpgradedAmmoCount--;
        }
    }
}

function isLoadedAmmoUpgraded(weaponPtr) {
    if (weaponPtr.isNull()) {
        return false;
    }
    var weaponKey = weaponPtr.toString();
    if (!Object.prototype.hasOwnProperty.call(loadedUpgradedAmmo, weaponKey)) {
        return false;
    }
    var currentUID = weaponPtr.add(OFFSETS.inventory_replicatedTopItemUID).readS32();
    return loadedUpgradedAmmo[weaponKey] === currentUID;
}

function isInventoryUpgraded(inventoryPtr) {
    if (inventoryPtr.isNull()) {
        return false;
    }
    if (inventoryPtr.add(OFFSETS.inventory_replicatedTopUpgraded).readU8() !== 0) {
        return true;
    }
    var statesCount = inventoryPtr.add(OFFSETS.inventory_itemStatesNum).readS32();
    if (statesCount > 0 && statesCount <= 256) {
        var statesData = inventoryPtr.add(OFFSETS.inventory_itemStatesData).readPointer();
        if (!statesData.isNull()) {
            var topState = statesData.add((statesCount - 1) * OFFSETS.itemState_size);
            if (topState.add(OFFSETS.itemState_bUpgraded).readU8() !== 0) {
                return true;
            }
        }
    }
    return false;
}

function getDefaultDamageTypeClass() {
    if (!cachedDefaultDamageTypeClass.isNull()) {
        return cachedDefaultDamageTypeClass;
    }
    var uClassStatic = UClass_GetPrivateStaticClass();
    var candidates = [
        '/Script/DreadHunger.DH_DamageType',
        '/Script/Engine.DamageType'
    ];
    for (var i = 0; i < candidates.length; i++) {
        var pathStr = candidates[i];
        var buf = Memory.alloc((pathStr.length + 1) * 2);
        buf.writeUtf16String(pathStr);
        var found = StaticFindObject(uClassStatic, ptr(0xFFFFFFFFFFFFFFFF), buf, 0);
        if (!found.isNull()) {
            cachedDefaultDamageTypeClass = found;
            return found;
        }
    }
    return ptr(0);
}

// ============================================================================
// 1. Allow Whetstone (bUpgradeItem == 1) to sharpen ANY un-upgraded item
//    (or repair any damaged item with durability).
//    Server executes native OnAppliedTo -> Upgrade(1), replicating the exact
//    vanilla upgrade triangle icon (bUpgraded = 1) & sharpening sound to clients.
// ============================================================================
Interceptor.attach(ADH_Inventory_CanBeAppliedTo_addr, {
    onEnter: function (args) {
        this.shouldAllowUpgrade = false;
        var sourceInventory = args[0];
        var targetInventory = args[1];
        if (sourceInventory.isNull() || targetInventory.isNull()) {
            return;
        }
        // Only override when applying an upgrade item (Whetstone)
        if (sourceInventory.add(OFFSETS.inventory_bUpgradeItem).readU8() === 0) {
            return;
        }
        // Do not consume Whetstone onto another Whetstone
        if (targetInventory.add(OFFSETS.inventory_bUpgradeItem).readU8() !== 0) {
            return;
        }
        // Allow sharpening if top item is not upgraded, or repairing if damaged
        if (!isInventoryUpgraded(targetInventory)) {
            this.shouldAllowUpgrade = true;
            return;
        }
        if (
            targetInventory.add(OFFSETS.inventory_bHasDurability).readU8() !== 0 &&
            targetInventory.add(OFFSETS.inventory_replicatedTopDurability).readFloat() < 0.999
        ) {
            this.shouldAllowUpgrade = true;
        }
    },
    onLeave: function (retval) {
        if (this.shouldAllowUpgrade) {
            retval.replace(ptr(1));
        }
    }
});

// ============================================================================
// 2. Melee & Ranged Weapon Damage Bonus
//    - Patch 0xE00F97 (je 0xE00FAA -> jmp 0xE00FAA, 0x74 -> 0xEB) so native
//      GetDamage_Implementation returns pure base Damage without adding
//      vanilla UpgradeBonusDamage and without nested JS NativeCallbacks.
//    - Wrap ADH_Weapon::GetDamage (0xF0A7F0) — called by BOTH Melee
//      (ApplyMeleeDamage) and Ranged (HitActor_Impl) — to preserve all Blueprint
//      GetDamage overrides (e.g. Sword/Shovel heavy swings) and add
//      +UPGRADE_BONUS_DAMAGE for upgraded weapon and/or upgraded loaded ammo.
// ============================================================================
if (ADH_Weapon_GetDamage_Impl_je_addr.readU8() === 0x74) {
    Memory.patchCode(ADH_Weapon_GetDamage_Impl_je_addr, 1, function (code) {
        code.writeU8(0xEB);
    });
}

var weaponGetDamageReplacement = new NativeCallback(function (weaponPtr, useStatePtr) {
    if (weaponPtr.isNull()) {
        return 0.0;
    }
    var finalDamage = origWeaponGetDamage
        ? origWeaponGetDamage(weaponPtr, useStatePtr)
        : weaponPtr.add(OFFSETS.inventory_damage).readFloat();

    if (isInventoryUpgraded(weaponPtr)) {
        finalDamage += UPGRADE_BONUS_DAMAGE;
    }
    if (isLoadedAmmoUpgraded(weaponPtr)) {
        finalDamage += UPGRADE_BONUS_DAMAGE;
    }

    return finalDamage;
}, 'float', ['pointer', 'pointer'], 'win64');

var weaponGetDamageTrampoline = Interceptor.replaceFast(
    ADH_Weapon_GetDamage_addr,
    weaponGetDamageReplacement
);
origWeaponGetDamage = new NativeFunction(
    weaponGetDamageTrampoline,
    'float',
    ['pointer', 'pointer'],
    'win64'
);

// ============================================================================
// 3. Ranged Ammo Sharpening (ADH_Weapon_Ranged::Reload @ 0xE0E600)
//    When reloading a Bow / Flintlock / Musket, inspect the top ammo item
//    about to be consumed and remember whether it was sharpened (bUpgraded).
// ============================================================================
Interceptor.attach(ADH_Weapon_Ranged_Reload_addr, {
    onEnter: function (args) {
        this.weaponPtr = ptr(0);
        this.hadAmmoToConsume = false;
        this.pendingAmmoUpgraded = false;

        var weaponPtr = args[0];
        if (weaponPtr.isNull()) {
            return;
        }
        if (weaponPtr.add(OFFSETS.actor_role).readU8() !== ROLE_AUTHORITY) {
            return;
        }
        if (ADH_Weapon_Ranged_ReloadingConsumesAmmo(weaponPtr) === 0) {
            return;
        }
        var humanOwner = weaponPtr.add(OFFSETS.inventory_humanOwner).readPointer();
        if (humanOwner.isNull()) {
            return;
        }
        var invManager = humanOwner.add(OFFSETS.human_inventoryManager).readPointer();
        if (invManager.isNull()) {
            return;
        }
        if (!invManager.add(OFFSETS.invManager_transactingInventory).readPointer().isNull()) {
            return;
        }
        var ammoClass = weaponPtr.add(OFFSETS.weaponRanged_ammunitionClass).readPointer();
        if (ammoClass.isNull()) {
            return;
        }
        var slotIndex = weaponPtr.add(OFFSETS.inventory_slotIndex).readS32();
        var ammoPtr = UDH_InventoryManager_FindInventory(invManager, ammoClass, slotIndex);
        if (ammoPtr.isNull()) {
            return;
        }
        var ammoCount = ammoPtr.add(OFFSETS.inventory_itemStatesNum).readS32();
        if (ammoCount <= 0) {
            return;
        }

        this.weaponPtr = weaponPtr;
        this.hadAmmoToConsume = true;
        this.pendingAmmoUpgraded = isInventoryUpgraded(ammoPtr);
    },
    onLeave: function () {
        if (!this.hadAmmoToConsume || this.weaponPtr.isNull()) {
            return;
        }
        if (this.weaponPtr.add(OFFSETS.inventory_replicatedTopUsesRemaining).readU8() === 0) {
            return;
        }
        var weaponKey = this.weaponPtr.toString();
        if (this.pendingAmmoUpgraded) {
            var weaponUID = this.weaponPtr.add(OFFSETS.inventory_replicatedTopItemUID).readS32();
            setLoadedUpgradedAmmo(weaponKey, weaponUID);
        } else {
            clearLoadedUpgradedAmmo(weaponKey);
        }
    }
});

// ============================================================================
// 4. Thrown pickup damage: enable damaging collision on ANY upgraded thrown
//    item (even non-weapons like Coal/Rock/Stick/Stew/Syringe/Gun/etc.)
//    and add +UPGRADE_BONUS_DAMAGE inside ApplyThrownDamage.
// ============================================================================
Interceptor.attach(ADH_InventoryPickup_Launch_addr, {
    onEnter: function (args) {
        var pickupPtr = args[0];
        var isThrown = args[1].toInt32() & 1;
        if (isThrown === 0 || pickupPtr.isNull()) {
            return;
        }
        if (pickupPtr.add(OFFSETS.pickup_saveStateUpgraded).readU8() === 0) {
            return;
        }

        // Enable thrown hit detection & damage path on non-weapon pickups
        pickupPtr.add(OFFSETS.pickup_bDamaging).writeU8(1);

        // Ensure valid DamageType & safe damage ramp multipliers
        if (pickupPtr.add(OFFSETS.pickup_damageType).readPointer().isNull()) {
            var defaultDamageType = getDefaultDamageTypeClass();
            if (!defaultDamageType.isNull()) {
                pickupPtr.add(OFFSETS.pickup_damageType).writePointer(defaultDamageType);
            }
        }
        if (pickupPtr.add(OFFSETS.pickup_damageRampStart).readFloat() <= 0.0) {
            pickupPtr.add(OFFSETS.pickup_damageRampStart).writeFloat(DEFAULT_THROWN_DAMAGE_RAMP_START);
        }
        if (pickupPtr.add(OFFSETS.pickup_maxDamageMultiplier).readFloat() < 1.0) {
            pickupPtr.add(OFFSETS.pickup_maxDamageMultiplier).writeFloat(DEFAULT_THROWN_MAX_DAMAGE_MULTIPLIER);
        }
    }
});

var applyThrownDamageReplacement = new NativeCallback(function (
    pickupPtr,
    targetActorPtr,
    baseDamage,
    hitLocationPtr,
    hitResultPtr,
    checkRamp
) {
    var effectiveDamage = baseDamage;
    if (!pickupPtr.isNull() && pickupPtr.add(OFFSETS.pickup_saveStateUpgraded).readU8() !== 0) {
        effectiveDamage = baseDamage + UPGRADE_BONUS_DAMAGE;
    }
    if (origApplyThrownDamage) {
        origApplyThrownDamage(
            pickupPtr,
            targetActorPtr,
            effectiveDamage,
            hitLocationPtr,
            hitResultPtr,
            checkRamp
        );
    }
}, 'void', ['pointer', 'pointer', 'float', 'pointer', 'pointer', 'uint8'], 'win64');

var applyThrownDamageTrampoline = Interceptor.replaceFast(
    ADH_InventoryPickup_ApplyThrownDamage_addr,
    applyThrownDamageReplacement
);
origApplyThrownDamage = new NativeFunction(
    applyThrownDamageTrampoline,
    'void',
    ['pointer', 'pointer', 'float', 'pointer', 'pointer', 'uint8'],
    'win64'
);
