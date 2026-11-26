// 骨の短剣モード (短剣本体)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UDH_InventoryManager_AddInventory_addr = base.add(0xDBB960);
var ADH_BoneDagger_Constructor_addr = base.add(0xE57AD0);
var ADH_HumanCharacter_Died_addr = base.add(0xD50CB0);
var ADH_HumanCharacter_TakeDamage_addr = base.add(0xD73DB0);

var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], { abi: 'win64', exceptions: 'propagate' });
var ADH_Character_IsAlive = new NativeFunction(base.add(0xD5C850), 'int8', ['pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_HumanCharacter_SetIsSpiritWalking = new NativeFunction(base.add(0xD70310), 'void', ['pointer', 'int8', 'float'], { abi: 'win64', exceptions: 'propagate' });
var UDH_InventoryManager_AddInventory = new NativeFunction(UDH_InventoryManager_AddInventory_addr, 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var ADH_Inventory_DecreaseItemStack = new NativeFunction(base.add(0xDC56D0), 'void', ['pointer', 'int32', 'int8', 'int8'], { abi: 'win64', exceptions: 'propagate' });
// 未使用: UClass::CreateDefaultObject (0x11EC670)

function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

var CCT_BONEDAGGER = 24;

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
function getArraySize(TArray) {
    if (TArray.isNull()) {
        return 0;
    }
    return TArray.add(8).readU32();
}
function getArrayItemAddr(TArray, size, index) {
    var ArrNum = getArraySize(TArray);
    if (index < 0 || index >= ArrNum)
        return null;
    var DataPtr = TArray.readPointer();
    if (DataPtr.isNull())
        return null;
    return DataPtr.add(index * size);
}

function unlockItem(ItemObject) {
    if (ItemObject.isNull()) {
        return;
    }
    var bCanBeTransferred = ItemObject.add(0x811);
    bCanBeTransferred.writeU8(1);
    var bCanBeDropped = ItemObject.add(0x690);
    bCanBeDropped.writeU8(0);
}
function resetSpells(SpellManager) {
    if (SpellManager.isNull()) {
        return;
    }
    var SpellChargeLevel = SpellManager.add(0x284);
    if(SpellChargeLevel.readFloat() + 0.55<= 1.0) {
        SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 0.55);
    }
    else {
        SpellChargeLevel.writeFloat(1.0);
    }
    
    var SpellCooldowns = SpellManager.add(0x2A8);
    var cdSize = getArraySize(SpellCooldowns);
    if (cdSize > 64) {
        return;
    }
    for(var i = 0; i < cdSize; i++) {
        var SpellCooldown = getArrayItemAddr(SpellCooldowns, 16, i);
        if (SpellCooldown === null) {
            continue;
        }
        var ServerUsedTime = SpellCooldown.add(8);

        if(ServerUsedTime.readFloat() - 30 > 0) {
            ServerUsedTime.writeFloat(ServerUsedTime.readFloat() - 30/* クールダウン短縮(秒) */);
        }
        else {
            ServerUsedTime.writeFloat(0);
        }
    }
}
var BoneDaggerName = '/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C';
function findInventory(InventoryManager, ItemClass) {
    if (InventoryManager.isNull() || ItemClass.isNull()) {
        return ptr(0);
    }
    var StoredInventory = InventoryManager.add(0x138);
    var invSize = getArraySize(StoredInventory);
    if (invSize > 256) {
        return ptr(0);
    }
    for(var i = 0; i < invSize; i++) {
        var entry = getArrayItemAddr(StoredInventory, 8, i);
        if (entry === null) {
            continue;
        }
        var Inventory = entry.readPointer();
        if(Inventory.isNull()) {
            continue;
        }
        var Clazz = Inventory.add(0x10).readPointer();
        if(!Clazz.isNull() && Clazz.equals(ItemClass)) {
            return Inventory;
        }
    }
    return ptr(0);
}
Interceptor.attach(UDH_InventoryManager_AddInventory_addr, {
    onEnter : function (args) {
        try {
            var InventoryManager = args[0];
            var TargetItem = args[1];
            if (InventoryManager.isNull() || TargetItem.isNull()) {
                return;
            }
            var Clazz = findClassByName(BoneDaggerName);
            if(Clazz.isNull()) {
                return;
            }
            if(!TargetItem.equals(Clazz)) {
                return;
            }
            var Unlock = 1;
            var HumanOwner = InventoryManager.add(0xD8).readPointer();
            
            if(!HumanOwner.isNull()) {
                var IsAlive = ADH_Character_IsAlive(HumanOwner);
                if(IsAlive) {
                    var PlayerState = HumanOwner.add(0x240).readPointer();
                    if(!PlayerState.isNull()){
                        var IsThrall = PlayerState.add(0x572).readU8();
                        if(IsThrall) {
                            Unlock = 0;
                            var BoneDagger = findInventory(InventoryManager, Clazz);
                            if(!BoneDagger.isNull()) {
                                var SpellManager = PlayerState.add(0x488).readPointer();
                                if(!SpellManager.isNull()) {
                                    resetSpells(SpellManager);
                                }
                            }
                        }
                    }
                    
                }
            }
            var DefaultObject = Clazz.add(0x118).readPointer();
            if(DefaultObject.isNull()) {
                return;
            }
            var bRequiresInventorySlot = DefaultObject.add(0x810);
            bRequiresInventorySlot.writeU8(Unlock);
            unlockItem(DefaultObject);
        } catch (e) {}
    }

});

Interceptor.attach(ADH_BoneDagger_Constructor_addr, {
    onEnter : function(args) {
        this.ItemObject = args[0];
    },
    onLeave : function(ret) {
        try {
            var ItemObject = this.ItemObject;
            if (!ItemObject || ItemObject.isNull()) {
                return;
            }
            var bRequiresInventorySlot = ItemObject.add(0x810);
            bRequiresInventorySlot.writeU8(1);
            unlockItem(ItemObject);
        } catch (e) {}
    }
});

Interceptor.attach(ADH_HumanCharacter_TakeDamage_addr, {
    onEnter : function(args) {
        try {
            var HumanCharacter = args[0];
            if (HumanCharacter.isNull()) {
                return;
            }
            var bSpiritWalking = HumanCharacter.add(0xC71).readU8();
            if(bSpiritWalking) {
                return;
            }
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            if(PlayerState.isNull()) {
                return;
            }
            var InventoryManager = HumanCharacter.add(0x808).readPointer();
            if(InventoryManager.isNull()) {
                return;
            }
            var SelectedInventory = InventoryManager.add(0xF8).readPointer();
            if(SelectedInventory.isNull()) {
                return;
            }
            var Clazz = findClassByName(BoneDaggerName);
            if(Clazz.isNull()) {
                return;
            }
            var IsThrall = PlayerState.add(0x572).readU8();
            if(!IsThrall) {
                var ItemClass = SelectedInventory.add(0x10).readPointer();
                var CurrentHealth = HumanCharacter.add(0x94C).readFloat();
                if(!ItemClass.isNull() && ItemClass.equals(Clazz) && CurrentHealth <= 30) {
                    ADH_Inventory_DecreaseItemStack(SelectedInventory, 1, 1, 0);
                    ADH_HumanCharacter_SetIsSpiritWalking(HumanCharacter, 1, 8);
                }
            }
        } catch (e) {}
    }
});
Interceptor.attach(ADH_HumanCharacter_Died_addr, {
    onEnter : function(args) {
        this.BoneDagger = [];
        try {
            var HumanCharacter = args[0];
            if (HumanCharacter.isNull()) {
                return;
            }
            var InventoryManager = HumanCharacter.add(0x808).readPointer();
            if(InventoryManager.isNull()) {
                return;
            }
            var StoredInventory = InventoryManager.add(0x138);
            var Size = getArraySize(StoredInventory);
            if (Size > 256) {
                return;
            }
            for(var i = 0; i < Size; i++) {
                var entry = getArrayItemAddr(StoredInventory, 8, i);
                if (entry === null) {
                    continue;
                }
                var Inventory = entry.readPointer();
                if (Inventory.isNull()) {
                    continue;
                }
                var InventoryType = Inventory.add(0x290);
                if(InventoryType.readU8() == CCT_BONEDAGGER) {
                    InventoryType.writeU8(0);
                    this.BoneDagger.push(Inventory);
                }
            }
        } catch (e) {}
    },
    onLeave : function(ret) {
        try {
            var ToRecover = this.BoneDagger;
            if (!ToRecover) {
                return;
            }
            for(var i = 0; i < ToRecover.length; i++) {
                var Inventory = ToRecover[i];
                if (Inventory && !Inventory.isNull()) {
                    var InventoryType = Inventory.add(0x290);
                    InventoryType.writeU8(CCT_BONEDAGGER);
                }
            }
        } catch (e) {}
    }
});


// デバッグ用バックトレース出力 (無効)