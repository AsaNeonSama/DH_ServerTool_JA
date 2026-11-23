// 骨の短剣モード (各役職の初期装備設定)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var ADH_GameMode_InitGame_addr = base.add(0xD9B220);

var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], 'win64');
var AActor_GetPrivateStaticClass = new NativeFunction(base.add(0x2984D50), 'pointer', [], 'win64');
var StaticLoadClass = new NativeFunction(base.add(0x137AB40), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer'], 'win64');
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], 'win64');
var FMemory_Realloc = new NativeFunction(base.add(0x106C3D0), 'pointer', ['pointer', 'int64', 'int32'], 'win64');

function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

function arrayResize(Array, ElementSize, Num) {
    var Allocator = Array.readPointer();
    var NewOne = FMemory_Realloc(Allocator, Num * ElementSize, 0);
    if (Num > 0 && NewOne.isNull()) {
        return false;
    }
    Array.writePointer(NewOne);
    Array.add(12).writeU32(Num);
    return true;
}

function arrayInit(Array, ElementSize, InitSize, MaxSize) {
    Array.writePointer(ptr(0));
    Array.add(8).writeU32(0);
    Array.add(12).writeU32(0);
    if (MaxSize > 0 && !arrayResize(Array, ElementSize, MaxSize)) {
        return false;
    }
    Array.add(8).writeU32(InitSize);
    return true;
}

function newFString(Length) {
    var FString = Memory.alloc(16 + Length * 2);
    FString.writePointer(FString.add(16));
    FString.add(8).writeU32(0);
    FString.add(12).writeU32(Length);
    return FString;
}

function getArraySize(TArray) {
    return TArray.add(8).readU32();
}

function getArrayItemAddr(TArray, size, index) {
    var ArrNum = getArraySize(TArray);
    if (index >= ArrNum) {
        return null;
    }
    var DataPtr = TArray.readPointer();
    if (DataPtr.isNull()) {
        return null;
    }
    return DataPtr.add(index * size);
}

function getString(FString) {
    var Size = getArraySize(FString);
    if (Size === 0) {
        return '';
    }
    var DataPtr = FString.readPointer();
    if (DataPtr.isNull()) {
        return '';
    }
    return DataPtr.readUtf16String(Size).replace(/\0+$/, '');
}

function loadActorClassByName(ClazzName) {
    var Buffer = Memory.alloc((ClazzName.length + 1) * 2);
    Buffer.writeUtf16String(ClazzName);
    return StaticLoadClass(AActor_GetPrivateStaticClass(), ptr(0), Buffer, ptr(0), 0, ptr(0));
}

var RegisterItemClass = new Map([
    [0, '/Game/Blueprints/Inventory/Fists/BP_Fists_Inventory.BP_Fists_Inventory_C'],
    [1, '/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C'],
    [2, '/Game/Blueprints/Inventory/Musket/BP_Musket_Inventory.BP_Musket_Inventory_C'],
    [3, '/Game/Blueprints/Inventory/Bow/BP_Bow_Inventory.BP_Bow_Inventory_C'],
    [4, '/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C'],
    [5, '/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Ammo_Inventory.BP_Flintlock_Ammo_Inventory_C'],
    [6, '/Game/Blueprints/Inventory/Bow/BP_Arrows_Inventory.BP_Arrows_Inventory_C'],
    [7, '/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C'],
    [8, '/Game/Blueprints/Inventory/WoodAxe/BP_WoodAxe_Inventory.BP_WoodAxe_Inventory_C'],
    [9, '/Game/Blueprints/Inventory/IceAxe/BP_IceAxe_Inventory.BP_IceAxe_Inventory_C'],
    [10, '/Game/Blueprints/Inventory/Cleaver/BP_Cleaver_Inventory.BP_Cleaver_Inventory_C'],
    [11, '/Game/Blueprints/Inventory/Shovel/BP_Shovel_Inventory.BP_Shovel_Inventory_C'],
    [12, '/Game/Blueprints/Inventory/Stick/BP_Stick_Inventory.BP_Stick_Inventory_C'],
    [13, '/Game/Blueprints/Inventory/Rock/BP_Rock_Inventory.BP_Rock_Inventory_C'],
    [14, '/Game/Blueprints/Inventory/Metals/BP_IronIngot_Inventory.BP_IronIngot_Inventory_C'],
    [15, '/Game/Blueprints/Inventory/Metals/BP_Nails_Inventory.BP_Nails_Inventory_C'],
    [16, '/Game/Blueprints/Inventory/Metals/BP_LeadIngot_Inventory.BP_LeadIngot_Inventory_C'],
    [17, '/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C'],
    [18, '/Game/Blueprints/Inventory/Flintlock/BP_Gunpowder_Inventory.BP_Gunpowder_Inventory_C'],
    [19, '/Game/Blueprints/Inventory/AnimalParts/BP_Sinew_Inventory.BP_Sinew_Inventory_C'],
    [20, '/Game/Blueprints/Inventory/AnimalParts/BP_WolfPelt_Inventory.BP_WolfPelt_Inventory_C'],
    [21, '/Game/Blueprints/Inventory/Tea/BP_Herbs_Inventory.BP_Herbs_Inventory_C'],
    [22, '/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C'],
    [23, '/Game/Blueprints/Inventory/Poison/BP_Antidote_Inventory.BP_Antidote_Inventory_C'],
    [24, '/Game/Blueprints/Inventory/Syringe/BP_Inventory_Laudanum.BP_Inventory_Laudanum_C'],
    [25, '/Game/Blueprints/Inventory/Meat/BP_AnimalMeat_Inventory.BP_AnimalMeat_Inventory_C'],
    [26, '/Game/Blueprints/Inventory/Meat/BP_CookedMeat_Inventory.BP_CookedMeat_Inventory_C'],
    [27, '/Game/Blueprints/Inventory/Meat/BP_HumanMeat_Inventory.BP_HumanMeat_Inventory_C'],
    [28, '/Game/Blueprints/Inventory/Meat/BP_BoneClub_Inventory.BP_BoneClub_Inventory_C'],
    [29, '/Game/Blueprints/Inventory/AnimalParts/BP_Blubber_Inventory.BP_Blubber_Inventory_C'],
    [30, '/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C'],
    [31, '/Game/Blueprints/Inventory/Tea/BP_Tea_Inventory.BP_Tea_Inventory_C'],
    [32, '/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C'],
    [33, '/Game/Blueprints/Inventory/BearTrap/BP_BearTrap_Inventory.BP_BearTrap_Inventory_C'],
    [34, '/Game/Blueprints/Inventory/Spyglass/BP_Spyglass_Inventory.BP_Spyglass_Inventory_C'],
    [35, '/Game/Blueprints/Inventory/Lantern/BP_Lantern_Inventory.BP_Lantern_Inventory_C'],
    [36, '/Game/Blueprints/Inventory/Powderkeg/BP_CoalBarrel_Inventory.BP_CoalBarrel_Inventory_C'],
    [37, '/Game/Blueprints/Inventory/Powderkeg/BP_Powderkeg_Inventory.BP_Powderkeg_Inventory_C'],
    [38, '/Game/Blueprints/Inventory/Poison/BP_Poison_Inventory.BP_Poison_Inventory_C'],
    [39, '/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C'],
    [40, '/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C'],
    [41, '/Game/Blueprints/Inventory/LockPick/BP_CaptainsKey_Inventory.BP_CaptainsKey_Inventory_C'],
    [42, '/Game/Blueprints/Inventory/Armory/BP_Code_Inventory.BP_Code_Inventory_C'],
    [43, '/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C'],
    [44, '/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C'],
]);

var RoleInitInventory = new Map([
    ['Captain', [ // 船長
        [7, 1],
        [31, 1],
        [17, 1],
    ]],
    ['Chaplain', [ // 牧師
        [11, 1],
        [40, 1],
        [17, 1],
    ]],
    ['Cook', [ // 料理人
        [10, 1],
        [30, 1],
        [17, 1],
    ]],
    ['Doctor', [ // 医師
        [10, 1],
        [24, 1],
        [22, 1],
        [17, 1],
    ]],
    ['Engineer', [ // 技師
        [8, 1],
        [36, 1],
        [17, 1],
    ]],
    ['Hunter', [ // 猟師
        [3, 1],
        [6, 4],
        [33, 2],
        [17, 1],
    ]],
    ['Marine', [ // 海兵
        [1, 1],
        [17, 1],
    ]],
    ['Navigator', [ // 航海士
        [34, 1],
        [35, 1],
        [17, 1],
    ]],
]);

var ResolvedItemClass = new Map();
var itemClassesInitialized = false;

function initItemClass() {
    if (itemClassesInitialized) {
        return true;
    }
    var TempMap = new Map();
    for (var [Id, ClassName] of RegisterItemClass) {
        var Clazz = loadActorClassByName(ClassName);
        if (Clazz.isNull()) {
            return false;
        }
        TempMap.set(Id, Clazz);
    }
    ResolvedItemClass = TempMap;
    itemClassesInitialized = true;
    return true;
}

Interceptor.attach(ADH_GameMode_InitGame_addr, {
    onEnter: function (args) {
        try {
            var GameMode = args[0];
            if (GameMode.isNull()) {
                return;
            }
            var GameInstance = UDH_GameInstance_GetInstance(GameMode);
            if (GameInstance.isNull()) {
                return;
            }
            var PlayerRoleList = GameInstance.add(0x3B8);
            if (!initItemClass()) {
                return;
            }

            var roleCount = getArraySize(PlayerRoleList);
            if (roleCount === 0 || roleCount > 64) {
                return;
            }
            for (var i = 0; i < roleCount; i++) {
                var RoleEntryPtr = getArrayItemAddr(PlayerRoleList, 8, i);
                if (RoleEntryPtr === null) {
                    continue;
                }
                var RoleData = RoleEntryPtr.readPointer();
                if (RoleData.isNull()) {
                    continue;
                }
                var OnlineName = getString(RoleData.add(0x48));
                var DesiredInventory = RoleInitInventory.get(OnlineName);
                if (!DesiredInventory || DesiredInventory.length === 0) {
                    continue;
                }
                var StartingDropTable = RoleData.add(0x128).readPointer();
                if (StartingDropTable.isNull()) {
                    continue;
                }
                var Table = StartingDropTable.add(0x40);

                if (!arrayResize(Table, 40, DesiredInventory.length)) {
                    continue;
                }
                Table.add(8).writeU32(DesiredInventory.length);
                for (var j = 0; j < DesiredInventory.length; j++) {
                    var InventoryInfo = DesiredInventory[j];
                    if (!InventoryInfo || InventoryInfo.length !== 2) {
                        continue;
                    }
                    var ClassId = InventoryInfo[0];
                    var DesiredNum = InventoryInfo[1];
                    var Clazz = ResolvedItemClass.get(ClassId);
                    if (!Clazz || Clazz.isNull()) {
                        continue;
                    }

                    var InventoryList = getArrayItemAddr(Table, 40, j);
                    if (InventoryList === null) {
                        continue;
                    }
                    if (!arrayInit(InventoryList, 8, 1, 1)) {
                        continue;
                    }
                    var ItemClass = getArrayItemAddr(InventoryList, 8, 0);
                    if (ItemClass === null) {
                        continue;
                    }
                    InventoryList.add(0x10).writeFloat(1.0);          // 重み
                    InventoryList.add(0x14).writeU32(DesiredNum);     // 最小生成数
                    InventoryList.add(0x18).writeU32(0);              // 最大生成数
                    InventoryList.add(0x1C).writeU32(1);              // スタック数
                    InventoryList.add(0x20).writeU64(0);              // パディング(8バイト)

                    ItemClass.writePointer(Clazz);
                }
            }
        } catch (e) {}
    },
});
