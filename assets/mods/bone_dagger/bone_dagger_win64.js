var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UDH_InventoryManager_AddInventory_addr = base.add(0xDBB960);
var ADH_BoneDagger_Constructor_addr = base.add(0xE57AD0);
var ADH_HumanCharacter_Died_addr = base.add(0xD50CB0);
var ADH_HumanCharacter_TakeDamage_addr = base.add(0xD73DB0);

var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], 'win64');
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], 'win64');
var ADH_Character_IsAlive = new NativeFunction(base.add(0xD5C850), 'int8', ['pointer'], 'win64');
var ADH_HumanCharacter_SetIsSpiritWalking = new NativeFunction(base.add(0xD70310), 'void', ['pointer', 'int8', 'float'], 'win64');
var UDH_InventoryManager_AddInventory = new NativeFunction(UDH_InventoryManager_AddInventory_addr, 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'pointer', 'int8', 'pointer'], 'win64');
var ADH_Inventory_DecreaseItemStack = new NativeFunction(base.add(0xDC56D0), 'void', ['pointer', 'int32', 'int8', 'int8'], 'win64');
//var UClass_CreateDefaultObject = new NativeFunction(base.add(0x11EC670), 'pointer', ['pointer'], 'win64');

function logInfo(Info) {
    var f = new File('output.log', 'a');
    f.write('[Patches] ' + Info + '\n');
    f.close();
}

var CCT_BONEDAGGER = 24;

function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
}

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
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

function unlockItem(ItemObject) {
    var bCanBeTransferred = ItemObject.add(0x811);
    bCanBeTransferred.writeU8(1);
    var bCanBeDropped = ItemObject.add(0x690);
    bCanBeDropped.writeU8(0);
}
function resetSpells(SpellManager) {
    var SpellChargeLevel = SpellManager.add(0x284);
    if(SpellChargeLevel.readFloat() + 0.55<= 1.0) {
        SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 0.55);
    }
    else {
        SpellChargeLevel.writeFloat(1.0);
    }
    
    var SpellCooldowns = SpellManager.add(0x2A8);
    for(var i = 0; i < getArraySize(SpellCooldowns); i++) {
        var SpellCooldown = getArrayItemAddr(SpellCooldowns, 16, i);
        //var SpellClass = SpellCooldown.readPointer();
        var ServerUsedTime = SpellCooldown.add(8);
        //var ClassDefaultObject = SpellClass.add(0x118).readPointer();
        //var CoolDownTime = ClassDefaultObject.add(0xD4).readFloat();

        if(ServerUsedTime.readFloat() - 30 > 0) {
            ServerUsedTime.writeFloat(ServerUsedTime.readFloat() - 30/*CoolDownTime*/);
        }
        else {
            ServerUsedTime.writeFloat(0);
        }
    }
}
var BoneDaggerName = '/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C';
function findInventory(InventoryManager, ItemClass) {
    var StoredInventory = InventoryManager.add(0x138);
    for(var i = 0; i < getArraySize(StoredInventory); i++) {
        var Inventory = getArrayItemAddr(StoredInventory, 8, i).readPointer();
        if(Inventory.isNull()) {
            continue;
        }
        var Clazz = Inventory.add(0x10).readPointer();
        if(Clazz.equals(ItemClass)) {
            return Inventory;
        }
    }
    return ptr(0);
}
Interceptor.attach(UDH_InventoryManager_AddInventory_addr, {
    onEnter : function (args) {
        var Clazz = findClassByName(BoneDaggerName);
        
        if(Clazz.isNull()) {
            return;
        }
        var TargetItem = args[1];
        if(!TargetItem.equals(Clazz)) {
            return;
        }
        var InventoryManager = args[0];
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
        
    }

});

Interceptor.attach(ADH_BoneDagger_Constructor_addr, {
    onEnter : function(args) {
        this.ItemObject = args[0];
    },
    onLeave : function(ret) {
        var ItemObject = this.ItemObject;
        var bRequiresInventorySlot = ItemObject.add(0x810);
        bRequiresInventorySlot.writeU8(1);
        unlockItem(ItemObject);
    }
});

Interceptor.attach(ADH_HumanCharacter_TakeDamage_addr, {
    onEnter : function(args) {
        var HumanCharacter = args[0];
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
            if(ItemClass.equals(Clazz) && CurrentHealth <= 30) {
                ADH_Inventory_DecreaseItemStack(SelectedInventory, 1, 1, 0);
                ADH_HumanCharacter_SetIsSpiritWalking(HumanCharacter, 1, 8);
            }
        }
    }
});
Interceptor.attach(ADH_HumanCharacter_Died_addr, {
    onEnter : function(args) {
        var HumanCharacter = args[0];
        var InventoryManager = HumanCharacter.add(0x808).readPointer();
        if(InventoryManager.isNull()) {
            return;
        }
        var StoredInventory = InventoryManager.add(0x138);
        this.BoneDagger = [];
        var Size = getArraySize(StoredInventory);
        for(var i = 0; i < Size; i++) {
            var Inventory = getArrayItemAddr(StoredInventory, 8, i).readPointer();
            var InventoryType = Inventory.add(0x290);
            if(InventoryType.readU8() == CCT_BONEDAGGER) {
                InventoryType.writeU8(0);
                this.BoneDagger.push(Inventory);
            }
        }
    },
    onLeave : function(ret) {
        var ToRecover = this.BoneDagger;
        for(var i = 0; i < ToRecover.length; i++) {
            var Inventory = ToRecover[i];
            var InventoryType = Inventory.add(0x290);
            InventoryType.writeU8(CCT_BONEDAGGER);
        }
    }
});


/*logInfo(Thread.backtrace(this.context, Backtracer.ACCURATE).map(DebugSymbol.fromAddress).join('\n')); */