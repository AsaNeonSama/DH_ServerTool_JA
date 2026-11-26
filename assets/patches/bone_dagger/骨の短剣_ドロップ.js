// 骨の短剣モード (カスタムドロップテーブル)
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UDH_DropTable_GetGuaranteedDrops_addr = base.add(0xDC9010);
var UDH_DropTable_GetRandomDrops_addr = base.add(0xDC9260);
var FName_GetPlainNameString = new NativeFunction(base.add(0x11604F0), 'void', ['pointer', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var FMemory_Realloc = new NativeFunction(base.add(0x106C3D0), 'pointer', ['pointer', 'int64', 'int32'], { abi: 'win64', exceptions: 'propagate' });
var UClass_GetPrivateStaticClass = new NativeFunction(base.add(0x11F02E0), 'pointer', [], { abi: 'win64', exceptions: 'propagate' });
var StaticLoadObject = new NativeFunction(base.add(0x137B290), 'pointer', ['pointer', 'pointer', 'pointer', 'pointer', 'int32', 'pointer', 'int8', 'pointer'], { abi: 'win64', exceptions: 'propagate' });
var StaticFindObject = new NativeFunction(base.add(0x137AAA0), 'pointer', ['pointer', 'pointer', 'pointer', 'int8'], { abi: 'win64', exceptions: 'propagate' });
function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}

function newFString(Length) {
    var FString = Memory.alloc(16 + Length * 2);
    FString.writePointer(FString.add(16));
    FString.add(8).writeU32(0);
    FString.add(12).writeU32(Length);
    return FString;
}

function getObjectName(UObject) {
    if (UObject.isNull()) {
        return '';
    }
    var NamePrivate = UObject.add(0x18);
    var NameBuffer = newFString(100);
    FName_GetPlainNameString(NamePrivate, NameBuffer);
    var DataPtr = NameBuffer.readPointer();
    if (DataPtr.isNull()) {
        return '';
    }
    var text = DataPtr.readUtf16String();
    return text ? text : '';
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
var ItemTable = new Map([
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
    [44, '/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C']
/*
アイテムID一覧:
0: 拳
遠距離武器: 1: 拳銃 / 2: マスケット / 3: 弓 / 4: 銃パーツ
弾薬: 5: 弾丸 / 6: 矢
近接武器: 7: サーベル / 8: 斧 / 9: ピッケル / 10: 包丁 / 11: シャベル
素材: 12: 木材 / 13: 石 / 14: 鉄インゴット / 15: 釘 / 16: 鉛インゴット / 17: 石炭 / 18: 火薬 / 19: 腱 / 20: 毛皮 / 21: 薬草
回復: 22: 注射器 / 23: 解毒剤 / 24: アヘンチンキ
食料: 25: 生肉 / 26: 焼いた肉 / 27: 人肉 / 28: 骨の棍棒 / 29: 脂肪 / 30: シチュー / 31: お茶
道具: 32: 砥石 / 33: トラバサミ / 34: 望遠鏡 / 35: ランタン / 36: 石炭樽 / 37: 火薬樽 / 38: 毒薬 / 39: ニトログリセリン
鍵: 40: 骨の鍵 / 41: 船長の鍵
その他: 42: 武器庫の暗証番号 / 43: 骨の短剣 / 44: クエスト
*/
]);

function weightedItems(boostedIds, boostedWeight, baseIds, baseWeight) {
    var list = [];
    for (var i = 0; i < boostedIds.length; i++) {
        for (var r = 0; r < boostedWeight; r++) {
            list.push(boostedIds[i]);
        }
    }
    for (var j = 0; j < baseIds.length; j++) {
        for (var k = 0; k < baseWeight; k++) {
            list.push(baseIds[j]);
        }
    }
    return list;
}

var CustomDropTables = {
    "DT_HumanSkeleton" : [ // 人骨
        {
            "Items": [],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [],
            "Weight": 10, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 3, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [19],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [28],
            "Weight": 20, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [24],
            "Weight": 2, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [43], // 骨の短剣 (重み 5 -> 5.6、約+11%)
            "Weight": 5.6, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_OfficerSkeleton" : [ // 士官の人骨
        {
            "Items": [],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [19],
            "Weight": 8, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [28],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 2, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [7],
            "Weight": 15, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [43], // 骨の短剣 (重み 2 -> 2.2、約+10%)
            "Weight": 2.2, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [5],
            "Weight": 8, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 3, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_HumanCorpse" : [ // 死体
        {
            "Items": [27],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [],
            "Weight": 10, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 3, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 3, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [43], // 骨の短剣 (重み 5 -> 5.6、約+11%)
            "Weight": 5.6, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [19],
            "Weight": 4, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [28],
            "Weight": 20, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [24],
            "Weight": 2, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_CaptainsTrunk" : [ // 船長宝箱
        {
            "Items": weightedItems([43], 7, [36, 22, 23], 6), // 骨の短剣確率 25.0% -> 28.0% (約+12%)
            "Weight": 1, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [31],
            "Weight": 1, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [33, 24, 20],
            "Weight": 1, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [17, 35],
            "Weight": 1, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_StrongBox" : [ // 金庫
        {
            "Items": [4, 32, 22, 34, 9, 23],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": weightedItems([43], 7, [17, 33, 20, 36, 24, 35], 6), // 骨の短剣確率 14.3% -> 16.3% (約+14%)
            "Weight": 10, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [15, 18, 5],
            "Weight": 3, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 5, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [31, 26],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_StrongBoxThrall" : [ // 金庫(傀儡)
        {
            "Items": weightedItems([43], 7, [4, 32, 22, 9, 23], 6), // 骨の短剣確率 16.7% -> 18.9% (約+13%)
            "Weight": 30, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [17, 33, 20, 36, 24, 35, 34],
            "Weight": 10, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [38, 37],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 5, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [15, 18, 5],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 5, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
        {
            "Items": [31, 26],
            "Weight": 5, // 重み
            "GuaranteedSpawns": 0, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
    "DT_ArmoryBox_Musket" : [ // 武器庫クレート
        {
            "Items": [1, 2, 2, 3],
            "Weight": 30, // 重み
            "GuaranteedSpawns": 1, // 最小生成数
            "MaximumSpawns": 1, // 最大生成数
            "StackSizeToSpawn": 1 // スタック数
        },
    ],
};
/*
参照用ドロップテーブル名一覧:
- DT_HumanSkeleton (人骨), DT_OfficerSkeleton (士官の人骨), DT_HumanCorpse (死体)
- DT_CaptainsTrunk (船長宝箱), DT_StorageTrunk_Supplies (大物資箱), DT_StorageTrunk_Supplies_Low (小物資箱)
- DT_ArmoryBox_Musket (武器庫クレート), DT_CoalSled (石炭ソリ), DT_CoalSled_High (大石炭ソリ)
- DT_StorageTrunk_Ammo (弾薬箱), DT_StorageTrunk_Nitro (ニトロ箱), DT_StrongBox (金庫), DT_StrongBoxThrall (傀儡金庫)
- DT_ShipCoalStorage (ボイラー室収納箱), DT_ShipStorage (船内収納箱), DT_Stove (ストーブ), DT_ShipBoiler (ボイラー), DT_MedicalSupplies (医療箱)
- 死体剥ぎ取り系 (Items の肉IDは取得不可防止のため変更不可):
  DT_Cannibal (人肉=27), DT_Bear (生肉=25 x6), DT_Wolf (生肉=25 x3), DT_SealPrey (生肉=25 x1), DT_SmallPrey (ウサギ/ネズミ 生肉=25 x1)
*/
function loadClassByName(ClassName) {
    return loadObjectByName(ClassName, UClass_GetPrivateStaticClass());
}
function findClassByName(ClassName) {
    return findObjectByName(ClassName, UClass_GetPrivateStaticClass());
}

function loadObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticLoadObject(Clazz, ptr(0), Buffer, ptr(0), 0, ptr(0), 1, ptr(0));
}

function findObjectByName(ObjectName, Clazz) {
    var Buffer = Memory.alloc((ObjectName.length + 1) * 2);
    Buffer.writeUtf16String(ObjectName);
    return StaticFindObject(Clazz, ptr(0xFFFFFFFFFFFFFFFF), Buffer, 0);
}


function getItemClass(ClassName) {
    var Clazz = findClassByName(ClassName);
    if (Clazz.isNull()) {
        Clazz = loadClassByName(ClassName);
    }
    return Clazz;
}
var patchedDropTables = {};

function applyCustomDropTable(DropTable) {
    if (DropTable.isNull()) {
        return;
    }
    var ObjectName = getObjectName(DropTable);
    if (!CustomDropTables.hasOwnProperty(ObjectName)) {
        return;
    }
    var Table = DropTable.add(0x40);
    var CustomDropTable = CustomDropTables[ObjectName];
    var Length = CustomDropTable.length;
    var TableKey = DropTable.toString();
    var CurrentDataKey = Table.readPointer().toString();
    if (patchedDropTables[TableKey] === CurrentDataKey && Table.add(8).readU32() === Length) {
        return;
    }

    // TArray変更前に全アイテムの UClass* を事前解決
    var ResolvedEntries = [];
    for (var i = 0; i < Length; i++) {
        var InventoryInfo = CustomDropTable[i];
        var ResolvedItems = [];
        for (var j = 0; j < InventoryInfo.Items.length; j++) {
            var ItemName = ItemTable.get(InventoryInfo.Items[j]);
            if (!ItemName) {
                continue;
            }
            var ItemClass = getItemClass(ItemName);
            if (!ItemClass.isNull()) {
                ResolvedItems.push(ItemClass);
            }
        }
        ResolvedEntries.push({
            Info: InventoryInfo,
            Items: ResolvedItems,
        });
    }

    if (!arrayResize(Table, 40, Length)) {
        return;
    }
    Table.add(8).writeU32(Length);
    for (var k = 0; k < Length; k++) {
        var InventoryList = getArrayItemAddr(Table, 40, k);
        if (InventoryList === null) {
            continue;
        }
        var Entry = ResolvedEntries[k];
        var EntryInfo = Entry.Info;
        var EntryItems = Entry.Items;

        var List = InventoryList;
        var Weight = InventoryList.add(0x10);
        var GuaranteedSpawns = InventoryList.add(0x14);
        var MaximumSpawns = InventoryList.add(0x18);
        var StackSizeToSpawn = InventoryList.add(0x1C);

        Weight.writeFloat(EntryInfo.Weight);
        MaximumSpawns.writeU32(EntryInfo.MaximumSpawns);
        StackSizeToSpawn.writeU32(EntryInfo.StackSizeToSpawn);
        GuaranteedSpawns.writeU32(EntryInfo.GuaranteedSpawns);
        InventoryList.add(0x20).writeU64(0); // CurrentSpawns / パディング

        var ItemNum = EntryItems.length;
        if (!arrayInit(List, 8, ItemNum, ItemNum)) {
            continue;
        }
        for (var m = 0; m < ItemNum; m++) {
            var itemSlot = getArrayItemAddr(List, 8, m);
            if (itemSlot !== null) {
                itemSlot.writePointer(EntryItems[m]);
            }
        }
    }

    patchedDropTables[TableKey] = Table.readPointer().toString();
}

Interceptor.attach(UDH_DropTable_GetGuaranteedDrops_addr, {
    onEnter: function (args) {
        try {
            applyCustomDropTable(args[0]);
        } catch (e) {}
    },
});

Interceptor.attach(UDH_DropTable_GetRandomDrops_addr, {
    onEnter: function (args) {
        try {
            applyCustomDropTable(args[0]);
        } catch (e) {}
    },
});