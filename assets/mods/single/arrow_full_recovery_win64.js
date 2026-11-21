// Full Arrow Recovery (No Decay)
// Vanilla Dread Hunger sets ADH_Character::DeathInventorySpawnPct (+0x634) = 0.5f,
// causing arrows embedded in corpses (added via BP_Bow_Inventory -> AddDeathInventory)
// to lose 50% on death. Setting DeathInventorySpawnPct = 1.0f preserves 100% of arrows
// without affecting any other loot or item system.

var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var ADH_Character_ctor_SpawnPctPatch_addr = moduleBase.add(0xD42DA6);
var ADH_Character_AddDeathInventory_addr = moduleBase.add(0xD46930);
var OFF_CHARACTER_DEATH_INVENTORY_SPAWN_PCT = 0x634;

// Expected instruction at 0xD42DA6:
//   49 C7 86 34 06 00 00 00 00 00 3F  -> mov qword ptr [r14+634h], 3F000000h (0.5f)
// Patching byte +9 (0xD42DAF) from 0x00 to 0x80 changes immediate to 3F800000h (1.0f),
// keeping the sign-extended upper 32 bits (+0x638) identical (0x00000000).
var EXPECTED_CTOR_BYTES = [0x49, 0xC7, 0x86, 0x34, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3F];

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

if (matchesExpectedBytes(ADH_Character_ctor_SpawnPctPatch_addr, EXPECTED_CTOR_BYTES)) {
    Memory.patchCode(ADH_Character_ctor_SpawnPctPatch_addr.add(9), 1, function (code) {
        code.writeU8(0x80);
    });
}

// Also enforce DeathInventorySpawnPct = 1.0f on any character that receives embedded arrows
// (covers actors spawned before this script was injected).
Interceptor.attach(ADH_Character_AddDeathInventory_addr, {
    onEnter: function (args) {
        var characterPtr = args[0];
        if (!characterPtr.isNull()) {
            characterPtr.add(OFF_CHARACTER_DEATH_INVENTORY_SPAWN_PCT).writeFloat(1.0);
        }
    }
});
