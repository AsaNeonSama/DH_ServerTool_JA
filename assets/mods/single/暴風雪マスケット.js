// 暴風雪マスケット
// 終盤の暴風雪開始後 (ADH_GameStateBase::bBlizzardStarted +0x359)、暗証番号なしで武器庫を開放可能にする。
// 暴風雪開始前は通常通り動作する。

var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UDH_ArmoryLockComponent_HasCorrectCombination_addr = base.add(0xE73240);
var UObject_GetWorld = new NativeFunction(base.add(0x13075A0), 'pointer', ['pointer'], 'win64');
var GWorld = base.add(0x46ED420);

var OFF_UWORLD_GAMESTATE = 0x120;
var OFF_GAMESTATE_BLIZZARD_STARTED = 0x359;

Interceptor.attach(UDH_ArmoryLockComponent_HasCorrectCombination_addr, {
    onEnter: function (args) {
        this.armoryLockComponent = args[0];
    },
    onLeave: function (ret) {
        try {
            if ((ret.toInt32() & 1) !== 0) {
                return;
            }
            var world = ptr(0);
            var lockComp = this.armoryLockComponent;
            if (lockComp && !lockComp.isNull()) {
                world = UObject_GetWorld(lockComp);
            }
            if (world.isNull()) {
                world = GWorld.readPointer();
            }
            if (world.isNull()) {
                return;
            }
            var gameState = world.add(OFF_UWORLD_GAMESTATE).readPointer();
            if (gameState.isNull()) {
                return;
            }
            var blizzardStarted = gameState.add(OFF_GAMESTATE_BLIZZARD_STARTED).readU8();
            if (blizzardStarted !== 0) {
                ret.replace(ptr(1));
            }
        } catch (e) {}
    }
});
