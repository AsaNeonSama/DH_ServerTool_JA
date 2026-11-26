// 八艘飛び (ジャンプ力4倍 + 落下ダメージ無効化)
var base = Module.findBaseAddress("DreadHungerServer-Win64-Shipping.exe");
var JumpScale = 4.0;
var UCharacterMovementComponent_DoJump_addr = base.add(0x2419404);
var ADH_Character_TakeFallingDamage_addr = base.add(0xD74200);
Interceptor.attach(UCharacterMovementComponent_DoJump_addr, {
    onEnter : function(args) {
        try {
            var Component = this.context.rbx;
            if (!Component || Component.isNull()) {
                return;
            }
            var VectorZ = Component.add(0xCC);
            var curZ = VectorZ.readFloat();
            if (Number.isFinite(curZ)) {
                VectorZ.writeFloat(curZ * JumpScale);
            }
        } catch (e) {}
    }
});

// GCによるトランポリン解放防止のためトップレベルで NativeCallback を保持
var noopTakeFallingDamage = new NativeCallback(function(){}, 'void', ['pointer', 'pointer', 'pointer'], 'win64');
Interceptor.replace(ADH_Character_TakeFallingDamage_addr, noopTakeFallingDamage);