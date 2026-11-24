# DH_ServerTool_JA (Rust + Frida Gadget 17.27.4)

Dread Hunger Windows Dedicated Server（原生未改造サーバー）用の統合サーバー起動・OEP 早注入・Web コンソールツールです。

- 作者: **アサネオン** / **Build With Antigravity**

## 特徴
- **超軽量・単一実行ファイル (`dist/DH_ServerTool_JA.exe` / 約 12 MB)**:
  - Rust ネイティブ実装 + 静的 CRT リンク (`+crt-static`) により、`VCRUNTIME140.dll` などの外部ランタイム不要で単独動作します。
  - 全 28 件の修復済み MOD パッチ (`.js`)、事前圧縮済み Web コンソール HTML (`br`/`gzip`)、アプリアイコン、zstd 圧縮済み `frida-gadget-17.27.4-windows-x86_64.dll` を内蔵（完全相対パス動作・原生サーバーバイナリ無改造・`frida.dll` や `.stub` 配置不要）。
  - デフォルト設定をバイナリに内蔵しており、初回起動時に `./config.json` を自動生成します（`config.example.json` の同梱は不要）。
- **原生サーバー OEP 早注入 (`dh-gadget-injector`) & 連動終了 (`dh-win32`)**:
  - `DreadHungerServer-Win64-Shipping.exe` を一時停止 (`CREATE_SUSPENDED | CREATE_NEW_CONSOLE`) で直接起動し、OEP ガイドスタブ経由で `frida-gadget-17.27.4` をロード・全選択 MOD を 0 秒入口で適用してから実行を再開します（`UObject::Serialize` / `UClass::SerializeDefaultObject` 系の早注入 MOD も確実に動作）。
  - Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) にサーバープロセスを紐づけるため、ツールを閉じると起動中のサーバーも自動的に終了します。
  - Windows `NativeFunction` の例外伝播 (`exceptions: "propagate"`) と Frida 16→17 互換シムをランタイム Prelude で自動適用するため、UE4 `UE_LOG` (`OutputDebugStringW`) での `system error` 誤中断を防ぎつつ、既存 MOD スクリプトの Frida 16 / 17 両対応を維持します。
- **デフォルト自動注入 MOD (質問なしで毎回適用)**:
  - `default/コンソール連携.js`（Web コンソール連携・召喚前ブループリント自動ロード・幽体状態/傀儡解除対応・ポート変数化）
  - `default/スペルチャージ修正.js`（スペルチャージ修正）
  - `default/死亡切断通知.js`（オフライン・死亡アナウンス日本語版）
  - `default/接続保護_乱入防止.js`（ネットワーク接続保護・乱入防止：ポーカー／試合開始時ロックによる途中乱入防止・ゾンビ接続および残留旧 Controller の即時クリーンアップ・満員時の開局メンバー許可・無人 10 秒サーバー終了タイマー解除＆CloseServer 抑止・診断ログ出力）
- **JS パッチ実行ログのファイル出力 (`./patch_runtime.log`)**:
  - Frida JS パッチのランタイムログおよびエラーは、ループバック NDJSON チャンネル経由で `DH_ServerTool_JA.exe` と同階層の `./patch_runtime.log` に集約出力されます（サーバータブ `[2]` には各 MOD の注入完了進捗のみ表示）。
  - 試合開始ごとに上書きされ、常に最新 1 試合分のみ保持されます。
- **個別選択 MOD (`1〜22`)**:
  - `1: 役職ランダム` (`single/役職ランダム.js`)
  - `2: 開幕沈黙` (`single/開幕沈黙.js`)
  - `3: マップ補強(山頂)` (`single/マップ補強_山頂.js` / `21` と排他)
  - `4: マップ補強(広漠)` (`single/マップ補強_広漠.js` / `22` と排他)
  - `5: 五サボタージュ` (`single/五サボタージュ.js`)
  - `6: 狂人モード` (`single/狂人モード.js`)
  - `7: 骨の短剣モード [セット]`（`bone_dagger/骨の短剣_本体.js` + `bone_dagger/骨の短剣_ドロップ.js` + `bone_dagger/骨の短剣_初期装備.js`）
  - `8: クエスト報酬` (`single/クエスト報酬.js`)
  - `9: 全員バックパック` (`single/全員バックパック.js`)
  - `10: 万能砥石` (`single/万能砥石.js`)
  - `11: 矢回収100%` (`single/矢回収100.js`)
  - `12: 暴風雪マスケット` (`single/暴風雪マスケット.js` / `14` と排他)
  - `13: 銃・弓同時所持` (`single/銃_弓同時所持.js` / 銃 1 丁＋弓 1 張の最大 2 枠)
  - `14: 開幕マスケット` (`single/開幕マスケット.js` / `12` と排他)
  - `15: 毒耐性` (`single/毒耐性.js`)
  - `16: ミステリーモード` (`single/ミステリーモード.js`)
  - `17: グレネード` (`single/グレネード.js`)
  - `18: 八艘飛び` (`single/八艘飛び.js`)
  - `19: スペルチャージ無限` (`single/スペルチャージ無限.js`)
  - `20: 無限復活` (`single/無限復活.js`)
  - `21: 観光モード(山頂)` (`single/観光モード_山頂.js` / `3`・`22` と排他)
  - `22: 観光モード(広漠)` (`single/観光モード_広漠.js` / `4`・`21` と排他)
- **2 段階の起動設定 & 双タブ TUI & 自動ポート切替**:
  - 第 1 段階 `[Webコンソール設定]` で Web ポートと自動ブロードキャストを設定後、即座に Web コンソールを起動し双タブ TUI へ移行（タブ `[1]` で認証 URL のクリップボードコピー・操作ログ閲覧が可能）。
  - 第 2 段階 `[サーバー・試合設定]` でサーバーポート（デフォルト `7895 <-> 7897`）・マップ・詳細設定・MOD を選択。試合終了時は 10 秒間の再設定待機（`R` で再設定、`Enter` で即再起動）後に自動切替再起動します。
- **統合 Web コンソール (Angular Neo-Brutalist UI + モバイル対応)**:
  - デフォルトで `コンソール命令` を選択済み、PC 向けカスタムドロップダウン & モバイル向けボトムシート対応、AES-256-GCM 認証・チャレンジ防重放付き。

## 使い方 (Windows Server)
1. `dist/DH_ServerTool_JA.exe` のみを原生 `WindowsServer` フォルダ（`DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe` の上位階層）に配置します。
2. `DH_ServerTool_JA.exe` を実行すると初回起動時にデフォルト設定の `./config.json` が自動作成されます。`[Webコンソール設定]` -> `[サーバー・試合設定]` の順に設定してください（ポート項目は `Enter` でデフォルト値を使用できます）。

## `config.json` 設定項目 (自動生成デフォルト)
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

## ワークスペース構成
- `crates/dh-win32/`: Win32 API ラッパー（Job Object・プロセス制御・PEB/OEP 読み取り・UDP/TCP テーブル・コンソール／クリップボード）
- `crates/dh-gadget-injector/`: 独立再利用可能な Frida Gadget 17.27.4 OEP 早注入・キャッシュ管理・16→17 互換 Prelude・ループバック NDJSON ログチャンネル
- `crates/dh-server-tool/`: メイン実行ファイル (`DH_ServerTool_JA.exe`)（設定・AES-256-GCM・Web コンソール・双タブ TUI・CLI・MOD レジストリ・Supervisor）

## 開発・ビルド
> **注意**: `.cargo/config.toml`（静的 CRT `+crt-static` 設定）を確実に適用するため、すべての `cargo` コマンドは **`controller/` ディレクトリ内** で実行してください。

```bash
cd controller
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

ビルド成果物は `target/release/DH_ServerTool_JA.exe`（配布用コピー先: `dist/DH_ServerTool_JA.exe`）に出力されます。

## ライセンス / License

本プロジェクトは **[DH_ServerTool_JA Custom Source-Available Non-Commercial License (Version 1.2)](./LICENSE)**（日本語・简体中文・English の 3 言語を収録。解釈に相違がある場合は簡体字中国語版を基準とします）の下で公開されています。

- **著作権帰属**: ゲーム『Dread Hunger』本体およびすべてのオリジナル資産の知的財産権は、ニュージーランドの **Digital Confectioners Ltd.** に帰属します。本プロジェクトは自己ホスト専用サーバー向けの非公式コミュニティツールであり、無保証（AS IS）で提供されます。
- **許可**: 個人または非営利目的での学習、技術交流、自己ホスト運用、改変、および共有は無料で行えます。
- **義務**: 二次開発・改変・再配布を行う場合は、**完全なソースコードの公開（Source-Available）**、**本ライセンスの継承**、および**本ライセンス全文・元の著作権表示・出典の保持**が必要です。
- **禁止**: クローズドソース化、暗号化・難読化のみでの配布、直接・間接を問わない一切の商業利用・収益化、および特定プラットフォームや私設サーバーでの独占利用は禁止されています。

正式な条件・権利境界・公式削除窓口については、必ず [LICENSE](./LICENSE) の全文をご確認ください。
