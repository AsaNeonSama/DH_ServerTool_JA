use std::fs;

use dh_gadget_injector::GadgetCache;

#[test]
fn cache_extracts_hardlinks_and_cleans_up_in_japanese_path() {
    let base = std::env::temp_dir().join(format!("dh_cache_テスト日本語_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);

    // Create a fake stale directory for an impossible dead PID
    let stale = base.join("run_99999999_1");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("dummy.txt"), b"stale").unwrap();

    let cache = GadgetCache::open(Some(&base)).expect("open Japanese cache path");
    assert!(!stale.exists(), "stale run dir should be swept on open");
    assert!(cache.cached_dll_path().is_file());

    let round_dir_path;
    {
        let round = cache
            .prepare_round_dir(1, "console.log('hello');")
            .expect("prepare round dir");
        round_dir_path = round.dir().to_path_buf();
        assert!(round.gadget_dll_path().is_file());
        assert!(round.config_path().is_file());
        assert!(round.bundle_js_path().is_file());

        let cfg: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(round.config_path()).unwrap()).unwrap();
        assert_eq!(cfg["runtime"], "qjs");
        assert_eq!(cfg["interaction"]["type"], "script");
        assert_eq!(cfg["interaction"]["path"], "bundle.js");
        assert_eq!(cfg["interaction"]["on_load"], "resume");
    }
    assert!(
        !round_dir_path.exists(),
        "RoundTempDir must clean itself up on Drop"
    );

    let _ = fs::remove_dir_all(&base);
}
