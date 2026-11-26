// スペルチャージ初期値補正
var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);

function resetSpells(SpellManager) {
    // SpellManager が無効な場合は何もしない
    if (SpellManager.isNull()) {
        return;
    }
    var SpellChargeLevel = SpellManager.add(0x284);
    SpellChargeLevel.writeFloat(SpellChargeLevel.readFloat() + 0.06);
}

Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function (args) {
        try {
            var HumanCharacter = args[0];
            if (HumanCharacter.isNull()) {
                return;
            }
            // プレイヤー状態を取得
            var PlayerState = HumanCharacter.add(0x240).readPointer();
            if (PlayerState.isNull()) {
                return;
            }
            // スペル管理オブジェクトを取得
            var SpellManager = PlayerState.add(0x488).readPointer();
            // 全プレイヤーのスペルチャージを加算
            resetSpells(SpellManager);
        } catch (e) {}
    }
});


