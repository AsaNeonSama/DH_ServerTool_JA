var base = Module.findBaseAddress("DreadHungerServer-Win64-Shipping.exe");
var JumpScale = 4.0;
var UCharacterMovementComponent_DoJump_addr = base.add(0x2419404);
var ADH_Character_TakeFallingDamage_addr = base.add(0xD74200);
Interceptor.attach(UCharacterMovementComponent_DoJump_addr, {
    onEnter : function(args) {
        var Component = this.context.rbx;
        var VectorZ = Component.add(0xCC);
        VectorZ.writeFloat(VectorZ.readFloat() * JumpScale);
    }
});

Interceptor.replace(ADH_Character_TakeFallingDamage_addr, new NativeCallback(function(){}, 'void', ['pointer', 'pointer', 'pointer']));