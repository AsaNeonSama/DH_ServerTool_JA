
var base = Module.findBaseAddress("DreadHungerServer-Win64-Shipping.exe");
var SpellSelection = [[0, 1, 3, 2, 4], [0, 1, 3, 2, 4]];
var Max = 0;
function logInfo(Info) {
    try {
        var f = new File('output.log', 'a');
        f.write('[Patches] ' + Info + '\n');
        f.close();
    } catch (e) {}
}
for(var i = 0; i < SpellSelection.length; i++)
{
    if(SpellSelection[i].length > Max) {
        Max = SpellSelection[i].length;
    }
}
/*

SpellSelection代表选定技能
0: 鬼
1: 雾
2: 分身
3: 灵界
4: 沉默

*/

//SpellManager
var ADH_SpellManager_SetEquippedSpells_addr = base.add(0xE81F00);

var UDH_GameInstance_GetInstance = new NativeFunction(base.add(0xD93240), 'pointer', ['pointer'], 'win64');

function getArraySize(TArray){
    if (TArray.isNull()) {
        return 0;
    }
    return TArray.add(8).readU32();
}
function getArrayItemAddr(TArray, size, index){
    var ArrNum = getArraySize(TArray);
    if(index < 0 || index >= ArrNum)
        return null;
    var DataPtr = TArray.readPointer();
    if (DataPtr.isNull())
        return null;
    return DataPtr.add(index * size);
}
//SetInThrall SpellManager



var Count = 0;
var SpellList = Memory.alloc(0x100);
SpellList.writePointer(SpellList.add(16));
SpellList.add(12).writeU32(Max);

Interceptor.attach(ADH_SpellManager_SetEquippedSpells_addr, {
    onEnter : function(args) {
        try {
            var SpellManager = args[0];
            if (SpellManager.isNull()) {
                return;
            }
            SpellManager.add(0x228).writeU32(Max);
            var GameInstance = UDH_GameInstance_GetInstance(SpellManager);
            if (GameInstance.isNull()) {
                return;
            }
            var ThrallSpells = GameInstance.add(0x440);
            var Size = getArraySize(ThrallSpells);
            if (Size === 0 || Size > 64) {
                return;
            }
            
            if(Count >= SpellSelection.length) {
                Count = 0;
            }
            var preset = SpellSelection[Count];
            if (!preset || preset.length > 30) {
                return;
            }
            var resolved = [];
            for(var i = 0; i < preset.length; i++) {
                if(preset[i] < 0 || preset[i] >= Size) {
                    return;
                }
                var SpellType = getArrayItemAddr(ThrallSpells, 8, preset[i]);
                if (SpellType === null) {
                    return;
                }
                resolved.push(SpellType.readPointer());
            }
            SpellList.add(8).writeU32(resolved.length);
            for (var j = 0; j < resolved.length; j++) {
                SpellList.add(16 + 8 * j).writePointer(resolved[j]);
            }
            args[1] = SpellList;
            Count++;
        } catch (e) {}
    }

});
