
// プレイヤーの役職をランダム割り当て
var base = Module.findBaseAddress("DreadHungerServer-Win64-Shipping.exe");
var GWorld = base.add(0x46ED420);
function randomNum(max) {
    return Math.floor(Math.random() * max);
}
var activeWorldKey = null;
var roles = [1, 2, 3, 4, 5, 6, 7, 8];

var ADH_PlayerState_SetPlayerRole = base.add(0xE4F390);
var UDH_PlayerRoleData_FindByType = new NativeFunction(base.add(0xE33300), "pointer", ["char", "pointer"], { abi: 'win64', exceptions: 'propagate' });
var ADH_PlayerState_SetPlayerRole_Interceptor = Interceptor.attach(ADH_PlayerState_SetPlayerRole, {
    onEnter : function (args) {
        try {
            var world = GWorld.readPointer();
            if (!world.isNull()) {
                var worldKey = world.toString();
                if (activeWorldKey !== worldKey) {
                    activeWorldKey = worldKey;
                    roles = [1, 2, 3, 4, 5, 6, 7, 8];
                }
            }
            var ADH_PlayerState = args[0];
            if (ADH_PlayerState.isNull() || roles.length === 0) {
                return;
            }
            var index = randomNum(roles.length);
            var role = roles[index];
            var New_UDH_PlayerRoleData = UDH_PlayerRoleData_FindByType(role, ADH_PlayerState);
            if (!New_UDH_PlayerRoleData.isNull()) {
                roles.splice(index, 1);
                args[1] = New_UDH_PlayerRoleData;
            }
        } catch (e) {}
    }
});