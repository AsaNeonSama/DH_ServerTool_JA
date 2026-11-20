var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');
var ADH_HumanCharacter_AddStartingInventory_addr = base.add(0xD46F10);
var UDH_InventoryManager_SetStorageLimit = new NativeFunction(base.add(0xDE3DA0), 'void', ['pointer', 'int32'], 'win64');

var StorageLimit = new Map([
    ['Captain', 10],
    ['Chaplain', 10],
    ['Cook', 10],
    ['Doctor', 10],
    ['Engineer', 10],
    ['Hunter', 10],
    ['Marine', 10],
    ['Navigator', 10],
]);

function getArraySize(TArray) {
    return TArray.add(8).readU32();
}

function getString(FString) {
    var Size = getArraySize(FString);
    if (Size === 0) {
        return '';
    }
    var DataPtr = FString.readPointer();
    if (DataPtr.isNull()) {
        return '';
    }
    return DataPtr.readUtf16String(Size).replace(/\0+$/, '');
}

Interceptor.attach(ADH_HumanCharacter_AddStartingInventory_addr, {
    onEnter: function (args) {
        var HumanCharacter = args[0];
        if (HumanCharacter.isNull()) {
            return;
        }
        var PlayerState = HumanCharacter.add(0x240).readPointer();
        var InventoryComponent = HumanCharacter.add(0x808).readPointer();
        if (PlayerState.isNull() || InventoryComponent.isNull()) {
            return;
        }
        var SelectedRole = PlayerState.add(0x590).readPointer();
        if (SelectedRole.isNull()) {
            return;
        }
        var Name = getString(SelectedRole.add(0x48));
        if (StorageLimit.has(Name)) {
            UDH_InventoryManager_SetStorageLimit(InventoryComponent, StorageLimit.get(Name));
        }
    },
});
