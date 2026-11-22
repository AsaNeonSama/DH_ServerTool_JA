# DH_ServerTool_JA (TypeScript + Bun)

Dread Hunger Windows Dedicated Server（原生未改造サーバー）用の統合サーバー起動・Frida 直接起動注入・Web コンソールツールです。

- 作者: **アサネオン** / **Build With Antigravity**

## 特徴
- **単一実行ファイル (`dist/DH_ServerTool_JA.exe`)**: 全 28 件の修復済み MOD パッチ (`.js`)、Web コンソール HTML、アプリアイコン、Frida ネイティブバインディングを内蔵（完全相対パス動作・`frida.dll` や `.stub` 改造不要）。
- **原生サーバー直接起動注入 (`frida.spawn`) & 連動終了**:
  - `DreadHungerServer-Win64-Shipping.exe` を一時停止状態で直接起動し、全選択 MOD を入口で注入してから再開します（`UObject::Serialize` / `UClass::SerializeDefaultObject` 系の早注入 MOD も確実に動作）。
  - Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) にサーバープロセスを紐づけるため、ツールを閉じると起動中のサーバーも自動的に終了します。
  - Windows `NativeFunction` の例外伝播 (`exceptions: "propagate"`) により UE4 `UE_LOG` (`OutputDebugStringW`) での `system error` 誤中断を防止し、`-log` サーバー出力も正常に表示されます。
- **デフォルト自動注入 MOD (質問なしで毎回適用)**:
  - `console_win64.js`（Web コンソール連携・召喚前ブループリント自動ロード・幽体状態/傀儡解除対応・ポート変数化）
  - `spell_charge_fix_win64.js`（スペルチャージ修正）
  - `death_offline_announcement_win64.js`（オフライン・死亡アナウンス日本語版）
  - `network_fix_win64.js`（ネットワーク・再接続修正：ポーカー終了・試合開始時ロックによる途中乱入防止・ゾンビ接続および残留旧Controllerの即時クリーンアップ・満員時の再接続許可・無人10秒サーバー終了タイマー解除・PlayerState フォールバック復元・分身中切断時の `RealPawn` 復元・診断ログ出力）
- **JS パッチ実行ログのファイル出力 (`./patch_runtime.log`)**:
  - Frida JS パッチのランタイムログおよびエラーは、`DH_ServerTool_JA.exe` と同階層の `./patch_runtime.log` に集約出力されます（サーバータブ `[2]` には出力しません）。
  - 試合開始ごとに上書きされ、常に最新 1 試合分のみ保持されます。
- **個別選択 MOD (`1〜22`)**:
  - `1: 役職ランダム` / `2: 開幕沈黙` / `3: マップ補強(山頂)` / `4: マップ補強(広漠)` / `5: 五サボタージュ` / `6: 狂人モード`
  - `7: 骨の短剣モード [セット]`（`bone_dagger_win64.js` + `custom_droptable_bone_dagger_win64.js` + `set_inventory_win64.js`）
  - `8: クエスト報酬` / `9: 全員バックパック` / `10: 万能砥石` / `11: 矢回収100%` / `12: 暴風雪マスケット`（`14: 開幕マスケット` と排他） / `13: 銃・弓同時所持`（銃1丁＋弓1張の最大2枠）
  - `14: 開幕マスケット`（`12: 暴風雪マスケット` と排他） / `15: 毒耐性` / `16: ミステリーモード` / `17: グレネード` / `18: 八艘飛び` / `19: スペルチャージ無限` / `20: 無限復活`
  - `21: 観光モード(山頂)`（`3: マップ補強(山頂)` / `22: 観光モード(広漠)` と排他）/ `22: 観光モード(広漠)`（`4: マップ補強(広漠)` / `21: 観光モード(山頂)` と排他）
- **2 段階の起動設定 & 自動ポート切替**:
  - 第 1 段階 `[Webコンソール設定]` で Web ポートと自動ブロードキャストを設定後、即座に Web コンソールを起動・認証 URL を出力。
  - 第 2 段階 `[サーバー・試合設定]` でサーバーポート（デフォルト `7895 <-> 7897`）・マップ・詳細設定・MOD を選択。試合終了時は 10 秒間の再設定待機（`R` で再設定、`Enter` で即再起動）後に自動切替再起動します。
- **統合 Web コンソール (Angular Neo-Brutalist UI + モバイル対応)**:
  - デフォルトで `コンソール命令` を選択済み、PC 向けカスタムドロップダウン & モバイル向けボトムシート対応、AES-256-GCM 認証付き。

## 使い方 (Windows Server)
1. `dist/DH_ServerTool_JA.exe` と `dist/config.json` を原生 `WindowsServer` フォルダ（`DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe` の上位階層）に配置します。
2. `DH_ServerTool_JA.exe` を実行し、`[Webコンソール設定]` -> `[サーバー・試合設定]` の順に設定してください（ポート項目は `Enter` でデフォルト値を使用できます）。

## `config.json` 設定項目
```json
{
  "serverPorts": [7895, 7897],
  "consolePatchPort": 7893,
  "webConsolePort": 7891,
  "checkIntervalMs": 1000,
  "restartWaitSeconds": 10,
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
ビルド成果物は `dist/DH_ServerTool_JA.exe` と `dist/config.json` に出力されます。
