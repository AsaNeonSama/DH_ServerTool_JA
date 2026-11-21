/**
 * Prepended to every injected Frida script on Windows so all `NativeFunction`
 * calls default to `{ exceptions: "propagate" }`.
 *
 * Without this, Frida's default `"steal"` Vectored Exception Handler intercepts
 * benign Windows `OutputDebugStringW` (`0x4001000A`) raised inside UE4 `UE_LOG`
 * (e.g. `UCheatManager::Summon`, `UWorld::SpawnActor`, `AddInventory`) and
 * aborts the native call mid-execution with `Error: system error`.
 */
export const WINDOWS_NATIVE_FUNCTION_PRELUDE: string =
  '(function(){var _NF=NativeFunction;function _PNF(a,r,t,o){var p=(o===undefined||o===null)?{exceptions:"propagate"}:(typeof o==="string"?{abi:o,exceptions:"propagate"}:Object.assign({exceptions:"propagate"},o));return new _NF(a,r,t,p);}_PNF.prototype=_NF.prototype;globalThis.NativeFunction=_PNF;})();\n';

/**
 * Executed once at process entry (before `frida.resume`) to clear the stale
 * inherited `STARTF_USESTDHANDLES` handles set by `frida.spawn`, allowing UE4's
 * `FWindowsConsoleOutputDevice::Show(true)` -> `AllocConsole()` to bind
 * `STD_OUTPUT_HANDLE` to the newly created `-log` server console window.
 */
export const WINDOWS_CONSOLE_BOOTSTRAP_SCRIPT: string =
  '(function(){try{var addr=Module.findExportByName("kernel32.dll","SetStdHandle");if(!addr){return;}var SetStdHandle=new NativeFunction(addr,"int32",["int32","pointer"],{abi:"win64",exceptions:"propagate"});SetStdHandle(-10,ptr(0));SetStdHandle(-11,ptr(0));SetStdHandle(-12,ptr(0));}catch(e){}})();';
