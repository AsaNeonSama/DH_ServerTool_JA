# DHA_ServerTool_JA (TypeScript + Bun)

Dread Hunger Windows Dedicated Server（原生未改造サーバー）用の統合サーバー起動・Frida 直接起動注入・Web コンソールツールです。

- 作者: **アサネオン** / **Build With Antigravity**

## 特徴
- **単一実行ファイル (`dist/DHA_ServerTool_JA.exe`)**: 全 20 件の修復済み MOD パッチ (`.js`)、Web コンソール HTML、アプリアイコン、Frida ネイティブバインディングを内蔵（完全相対パス動作・`frida.dll` や `.stub` 改造不要）。
- **原生サーバー直接起動注入 (`frida.spawn`)**:
  - `DreadHungerServer-Win64-Shipping.exe` を一時停止状態で直接起動し、全選択 MOD を入口で注入してから再開します（`UObject::Serialize` / `UClass::SerializeDefaultObject` 系の早注入 MOD も確実に動作）。
  - Windows `NativeFunction` の例外伝播 (`exceptions: "propagate"`) により UE4 `UE_LOG` (`OutputDebugStringW`) での `system error` 誤中断を防止し、`-log` サーバーウィンドウ出力も正常に表示されます。
- **デフォルト自動注入 MOD (質問なしで毎回適用)**:
  - `console_win64.js`（Web コンソール連携・召喚前ブループリント自動ロード・ポート変数化）
  - `spell_charge_fix_win64.js`（スペルチャージ修正）
  - `death_offline_announcement_win64.js`（オフライン・死亡アナウンス日本語版）
- **個別選択 MOD (`1〜15`)**:
  - `1: 役職ランダム` / `2: 五サボタージュ` / `3: グレネード` / `4: 八艘飛び` / `5: スペルチャージ無限` / `6: ミステリーモード` / `7: 毒耐性` / `8: 無限復活` / `9: マップ補強(山頂)` / `10: マップ補強(広漠)` / `11: 狂人モード`
  - `12: 骨の短剣モード [セット]`（`bone_dagger_win64.js` + `custom_droptable_bone_dagger_win64.js` + `set_inventory_win64.js`）
  - `13: 開幕沈黙`（開幕 Lv1 ハッシュ自動発動）
  - `14: 開幕マスケット`（武器庫初期開放）
  - `15: 全員バックパック`（全役職ストレージ 10 枠）
- **2 段階の起動設定 & 自動ポート切替**:
  - 第 1 段階 `[Webコンソール設定]` で Web ポートと自動ブロードキャストを設定後、即座に Web コンソールを起動・認証 URL を出力。
  - 第 2 段階 `[サーバー・試合設定]` でサーバーポート（デフォルト `7895 <-> 7897`）・マップ・詳細設定・MOD を選択。試合終了時は 10 秒間の再設定待機（`R` で再設定、`Enter` で即再起動）後に自動切替再起動します。
- **統合 Web コンソール (Angular Neo-Brutalist UI + モバイル対応)**:
  - デフォルトで `コンソール命令` を選択済み、PC 向けカスタムドロップダウン & モバイル向けボトムシート対応、AES-256-GCM 認証付き。

## 使い方 (Windows Server)
1. `dist/DHA_ServerTool_JA.exe` と `dist/config.json` を原生 `WindowsServer` フォルダ（`DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe` の上位階層）に配置します。
2. `DHA_ServerTool_JA.exe` を実行し、`[Webコンソール設定]` -> `[サーバー・試合設定]` の順に設定してください（ポート項目は `Enter` でデフォルト値を使用できます）。

## `config.json` 設定項目
```json
{
  "serverPorts": [7895, 7897],
  "consolePatchPort": 7893,
  "webConsolePort": 7891,
  "checkIntervalMs": 1000,
  "restartWaitSeconds": 10,
  "injectDelayMs": 0,
  "scriptGapMs": 0,
  "autoBroadcastDefault": true,
  "serverExecutable": "./DreadHungerServer.exe",
  "shippingExecutable": "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
  "shippingProcessName": "DreadHungerServer-Win64-Shipping.exe"
}
```

## 開発・ビルド
```bash
bun install
bun run typecheck
bun run lint
bun run build
```
ビルド成果物は `dist/DHA_ServerTool_JA.exe` と `dist/config.json` に出力されます。
