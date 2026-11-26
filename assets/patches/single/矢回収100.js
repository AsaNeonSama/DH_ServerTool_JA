// 矢回収100% (劣化なし)
// ADH_Character::DeathInventorySpawnPct (+0x634) を 0.5f から 1.0f へ変更し、
// 死体に刺さった矢の50%消失を防いで100%回収可能にする。

var moduleBase = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');

var ADH_Character_ctor_SpawnPctPatch_addr = moduleBase.add(0xD42DA6);
var ADH_Character_AddDeathInventory_addr = moduleBase.add(0xD46930);
var OFF_CHARACTER_DEATH_INVENTORY_SPAWN_PCT = 0x634;

// 0xD42DA6 の期待命令列: mov qword ptr [r14+634h], 3F000000h (0.5f)
// +9バイト目 (0xD42DAF) を 0x00 -> 0x80 に書き換えて即値を 3F800000h (1.0f) へ変更
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

try {
    if (matchesExpectedBytes(ADH_Character_ctor_SpawnPctPatch_addr, EXPECTED_CTOR_BYTES)) {
        Memory.patchCode(ADH_Character_ctor_SpawnPctPatch_addr.add(9), 1, function (code) {
            code.writeU8(0x80);
        });
    }
} catch (e) {}

// 注入前に生成済みのキャラクターに対しても AddDeathInventory 時に 1.0f を適用
Interceptor.attach(ADH_Character_AddDeathInventory_addr, {
    onEnter: function (args) {
        try {
            var characterPtr = args[0];
            if (!characterPtr.isNull()) {
                characterPtr.add(OFF_CHARACTER_DEATH_INVENTORY_SPAWN_PCT).writeFloat(1.0);
            }
        } catch (e) {}
    }
});
