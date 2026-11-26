/// Windows `NativeFunction` prelude ensuring every native call defaults to
/// `{ exceptions: "propagate" }` so benign UE4 `UE_LOG` (`OutputDebugStringW`
/// `0x4001000A`) never triggers Frida's default VEH abort (`Error: system error`).
pub const WINDOWS_NATIVE_FUNCTION_PRELUDE: &str =
    include_str!("assets/windows_native_function_prelude.js");

/// Embedded Frida 16 -> 17 compatibility shim, loopback NDJSON log channel,
/// and isolated per-MOD execution loop.
pub(crate) const BUNDLE_RUNTIME_TEMPLATE: &str = include_str!("assets/bundle_runtime.js");
