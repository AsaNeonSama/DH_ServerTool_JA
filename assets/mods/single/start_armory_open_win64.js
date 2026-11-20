var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var UDH_ArmoryLockComponent_HasCorrectCombination_addr = base.add(0xE73240);

Interceptor.attach(UDH_ArmoryLockComponent_HasCorrectCombination_addr, {
    onLeave: function (ret) {
        ret.replace(ptr(1));
    }
});
