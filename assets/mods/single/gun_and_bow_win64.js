// Gun + Bow Dual Carry (Max 2 ranged weapons: 1 Firearm [Musket/Pistol] + 1 Bow)
// Vanilla Dread Hunger limits players to 1 ADH_Weapon_Ranged total via UDH_InventoryManager::FindRangedWeapon.
// This patch tracks the incoming item class from all 3 callers of FindRangedWeapon
// (AddInventory, CanAddInventory, CanInventoryBeInteractedWith) and enforces:
//   - At most 1 firearm (CCT_PISTOL=26 or CCT_MUSKET=27 — Musket and Pistol are mutually exclusive)
//   - At most 1 bow (CCT_BOW=36)
//   - At most 2 ranged weapons total.

var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var UDH_InventoryManager_AddInventory_addr = base.add(0xDBB960);
var UDH_InventoryManager_CanAddInventory_addr = base.add(0xDC2150);
var ADH_PlayerController_CanInventoryBeInteractedWith_addr = base.add(0xE2F130);
var UDH_InventoryManager_FindRangedWeapon_addr = base.add(0xDC89C0);

// Verified UE4 struct offsets & EInventoryType enum values (DreadHungerServer-Win64-Shipping.exe)
var OFF_UCLASS_CDO = 0x118;
var OFF_INV_MANAGER_STORED_INVENTORY = 0x138; // TArray<ADH_Inventory*> (Data +0x138, Num +0x140)
var OFF_INVENTORY_TYPE = 0x290;               // ADH_Inventory::InventoryType (uint8 EInventoryType)
var OFF_PICKUP_INVENTORY_CLASS = 0x248;       // ADH_InventoryPickup::InventoryClass (UClass*)

var EINV_PISTOL = 26; // CCT_PISTOL (拳銃)
var EINV_MUSKET = 27; // CCT_MUSKET (マスケット)
var EINV_BOW = 36;    // CCT_BOW (弓)

var MAX_STORED_ITEMS = 256;
var MAX_RANGED_TOTAL = 2;
var MAX_STACK_DEPTH = 32;

// Reentrant call-stack of incoming InventoryClass pointers pushed by the 3 callers of FindRangedWeapon
var incomingClassStack = [];
var origFindRangedWeapon = null;

function pushIncomingClass(classPtr) {
    if (incomingClassStack.length >= MAX_STACK_DEPTH) {
        incomingClassStack.shift();
    }
    incomingClassStack.push(classPtr);
}

function popIncomingClass() {
    if (incomingClassStack.length > 0) {
        incomingClassStack.pop();
    }
}

function getRangedCategory(invType) {
    if (invType === EINV_PISTOL || invType === EINV_MUSKET) {
        return 1; // Firearm (Musket or Pistol)
    }
    if (invType === EINV_BOW) {
        return 2; // Bow
    }
    return 0; // Other
}

function resolveIncomingInventoryType(classPtr) {
    if (!classPtr || classPtr.isNull()) {
        return -1;
    }
    var cdo = classPtr.add(OFF_UCLASS_CDO).readPointer();
    if (cdo.isNull()) {
        return -1;
    }
    return cdo.add(OFF_INVENTORY_TYPE).readU8();
}

function findConflictingRangedWeapon(invManager, incomingInvType) {
    if (invManager.isNull()) {
        return ptr(0);
    }
    var storedData = invManager.add(OFF_INV_MANAGER_STORED_INVENTORY).readPointer();
    var storedNum = invManager.add(OFF_INV_MANAGER_STORED_INVENTORY + 8).readS32();
    if (storedData.isNull() || storedNum <= 0 || storedNum > MAX_STORED_ITEMS) {
        return ptr(0);
    }

    var incomingCat = getRangedCategory(incomingInvType);
    var firstRanged = ptr(0);
    var rangedCount = 0;

    for (var i = 0; i < storedNum; i++) {
        var item = storedData.add(i * 8).readPointer();
        if (item.isNull()) {
            continue;
        }
        var itemType = item.add(OFF_INVENTORY_TYPE).readU8();
        var itemCat = getRangedCategory(itemType);

        if (itemCat !== 0) {
            if (firstRanged.isNull()) {
                firstRanged = item;
            }
            rangedCount += 1;
            if (incomingCat !== 0 && itemCat === incomingCat) {
                return item;
            }
        } else if (incomingCat === 0 && itemType === incomingInvType) {
            return item;
        }
    }

    if (rangedCount >= MAX_RANGED_TOTAL) {
        return firstRanged;
    }
    return ptr(0);
}

// Pin NativeCallback in top-level scope so QuickJS GC never frees the native trampoline
var findRangedWeaponReplacement = new NativeCallback(function (invManager) {
    try {
        if (invManager.isNull()) {
            return ptr(0);
        }
        if (incomingClassStack.length > 0) {
            var incomingClass = incomingClassStack[incomingClassStack.length - 1];
            var incomingInvType = resolveIncomingInventoryType(incomingClass);
            if (incomingInvType >= 0) {
                return findConflictingRangedWeapon(invManager, incomingInvType);
            }
        }
        if (origFindRangedWeapon) {
            return origFindRangedWeapon(invManager);
        }
    } catch (e) {}
    return ptr(0);
}, 'pointer', ['pointer'], 'win64');

var findRangedWeaponTrampoline = Interceptor.replaceFast(
    UDH_InventoryManager_FindRangedWeapon_addr,
    findRangedWeaponReplacement
);
origFindRangedWeapon = new NativeFunction(
    findRangedWeaponTrampoline,
    'pointer',
    ['pointer'],
    'win64'
);

// Caller 1: UDH_InventoryManager::AddInventory(this, InventoryClass, ...)
Interceptor.attach(UDH_InventoryManager_AddInventory_addr, {
    onEnter: function (args) {
        this._pushedIncoming = false;
        try {
            var invClass = args[1];
            if (invClass && !invClass.isNull()) {
                pushIncomingClass(invClass);
                this._pushedIncoming = true;
            }
        } catch (e) {}
    },
    onLeave: function () {
        try {
            if (this._pushedIncoming) {
                popIncomingClass();
            }
        } catch (e) {}
    }
});

// Caller 2: UDH_InventoryManager::CanAddInventory(this, InventoryClass, ...)
Interceptor.attach(UDH_InventoryManager_CanAddInventory_addr, {
    onEnter: function (args) {
        this._pushedIncoming = false;
        try {
            var invClass = args[1];
            if (invClass && !invClass.isNull()) {
                pushIncomingClass(invClass);
                this._pushedIncoming = true;
            }
        } catch (e) {}
    },
    onLeave: function () {
        try {
            if (this._pushedIncoming) {
                popIncomingClass();
            }
        } catch (e) {}
    }
});

// Caller 3: ADH_PlayerController::CanInventoryBeInteractedWith(this, ADH_InventoryPickup*)
Interceptor.attach(ADH_PlayerController_CanInventoryBeInteractedWith_addr, {
    onEnter: function (args) {
        this._pushedIncoming = false;
        try {
            var pickup = args[1];
            if (pickup && !pickup.isNull()) {
                var invClass = pickup.add(OFF_PICKUP_INVENTORY_CLASS).readPointer();
                if (!invClass.isNull()) {
                    pushIncomingClass(invClass);
                    this._pushedIncoming = true;
                }
            }
        } catch (e) {}
    },
    onLeave: function () {
        try {
            if (this._pushedIncoming) {
                popIncomingClass();
            }
        } catch (e) {}
    }
});
