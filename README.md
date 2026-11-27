# DH_ServerTool_JA (Rust + Frida Gadget 17.27.4)

Dread Hunger Windows Dedicated Server（原生未改造サーバー）用の統合サーバー起動・OEP 早注入・Web コンソールツールです。

- 作者: **アサネオン** / **Build With Antigravity**

## 特徴
- **超軽量・単一実行ファイル (`dist/DH_ServerTool_JA.exe` / 約 12 MB)**:
  - Rust ネイティブ実装 + 静的 CRT リンク (`+crt-static`) により、`VCRUNTIME140.dll` などの外部ランタイム不要で単独動作します。
  - 修復済みサーバー MOD・コンテンツ MOD 用スクリプト (`.js`)、事前圧縮済み Web コンソール HTML (`br`/`gzip`)、アプリアイコン、zstd 圧縮済み `frida-gadget-17.27.4-windows-x86_64.dll` を内蔵（完全相対パス動作・原生サーバーバイナリ無改造・`frida.dll` や `.stub` 配置不要）。
  - デフォルト設定をバイナリに内蔵しており、初回起動時に `./config.json` を自動生成します（`config.example.json` の同梱は不要）。
- **原生サーバー OEP 早注入 (`dh-gadget-injector`) & 連動終了 (`dh-win32`)**:
  - `DreadHungerServer-Win64-Shipping.exe` を一時停止 (`CREATE_SUSPENDED | CREATE_NEW_CONSOLE`) で直接起動し、OEP ガイドスタブ経由で `frida-gadget-17.27.4` をロード・全選択 MOD を 0 秒入口で適用してから実行を再開します（`UObject::Serialize` / `UClass::SerializeDefaultObject` 系の早注入 MOD も確実に動作）。
  - Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) にサーバープロセスを紐づけるため、ツールを閉じると起動中のサーバーも自動的に終了します。
  - Windows `NativeFunction` の例外伝播 (`exceptions: "propagate"`) と Frida 16→17 互換シムをランタイム Prelude で自動適用するため、UE4 `UE_LOG` (`OutputDebugStringW`) での `system error` 誤中断を防ぎつつ、既存 MOD スクリプトの Frida 16 / 17 両対応を維持します。
- **デフォルト自動注入 MOD (質問なしで毎回適用)**:
  - `default/コンソール連携.js`（Web コンソール連携・召喚前ブループリント自動ロード・幽体状態/傀儡解除対応・ポート変数化）
  - `default/スペルチャージ修正.js`（スペルチャージ修正）
  - `default/死亡切断通知.js`（オフライン・死亡アナウンス日本語版）
  - `default/接続保護_乱入防止.js`（ネットワーク接続保護・乱入防止：`EnteringMap`/`WaitingToStart` 認証成功者の和集合統合による開局名簿ロック・ゾンビ接続および残留旧 Controller の即時クリーンアップ・満員時の開局メンバー許可・無人 10 秒サーバー終了タイマー解除＆CloseServer 抑止・プライバシー保護済み診断ログ出力）
  - `default/食人鬼登攀補強.js`（食人鬼登攀補強：急斜面 78° 登攀・2 段階垂直氷壁／岸壁登攀 `MOVE_Flying`＋`<78°` 緩斜面自動復帰・3 層防爬船＆舷梯遮断・原版 ThreatTable 互換、野生動物非干渉）
- **JS パッチ実行ログのファイル出力 (`./patch_runtime_YYYYMMDD_HHMMSS.log`・最新 3 試合分を保持)**:
  - Frida JS パッチのランタイムログおよびエラーは、ループバック NDJSON チャンネル経由で `DH_ServerTool_JA.exe` と同階層の `./patch_runtime_YYYYMMDD_HHMMSS.log`（起動時刻ごと）に出力されます（サーバータブ `[2]` には各 MOD の注入完了進捗のみ表示）。
  - サーバー起動ごとに新しいタイムスタンプ付きログファイルを作成し、時系列で最新の 3 試合分のみを自動保持します（古いログは自動削除されます）。
- **コンテンツ MOD（クライアント `.pak` 併用・サーバー側ゼロ資産）**:
  - `1: カスタムスペル(新呪文5種)`（サーバー側は純 JS でランタイム合成、クライアント配布用 `.pak` / `.sig` は `assets/advanced_content/client/paked/totem_spells/` に格納・配布出力先 `dist/advanced_content/totem_spells/`）
- **クライアント EOS 再接続・同ポート再起動修正 DLL (`dist/client_fix/eos_fix/plan_a/eos_fix.dll`・`dist/client_fix/eos_fix/plan_b/eos_fix.dll` / 約 225 KB)**:
  - Rust ネイティブ実装 (`crates/dh-client-eos-fix/`) + 静的 CRT (`+crt-static`) による単体クライアント DLL（`VCRUNTIME140.dll` 不要・Steam オーバーレイ / OBS や `DH_TotemSpells_Client_p.pak` と完全非干渉）。`dsound.dll` の全 12 固定序号エクスポートを保持しているため、DLL インジェクタによる `eos_fix.dll` 直接注入と、ゲーム本体フォルダへ `dsound.dll` にリネーム配置するゼロインジェクタ起動の両方に対応します。
  - **Plan A（標準・19 パッチ、`plan_a/eos_fix.dll`）**:
    - 非公開／カスタム部屋の EOS ロビー検索権限を `EOS_LPL_PUBLICADVERTISED (0)` に補正しつつ、`LOBBYSEARCH` 属性を `bShouldAdvertise` 連動に制御し、試合終了・リセット・セッション更新完了時（`OnMatchEnded` / `ResetCurrentData` / `OnOnlineSessionUpdateComplete`）に EOS 公開設定が勝手にオフへ戻される挙動も全面抑止します。
    - `SetLobbyAttributes` の 256B `MEMBERS` 切り詰めを最大 1000B（UTF-8 境界安全）へ拡張し、8 人全員を EOS メンバーリストに保持するとともに、試合開始後の `MEMBERS` 上書き保護・`ConnectToIP` 時の `STARTUTCDATETIME` 自動設定・進行中ロビー検索許可・Steam「ゲームに参加」(`+connect_lobby`) 維持によって船員の途中切断・クラッシュ再起動・Steam 招待再接続を全面修正します。
    - `UDH_GameInstance + 0x570`（前回接続 URL）の試合中重複接続抑止を維持したまま、ロビー復帰時（`PostUpdate`）・セッション参加完了時（`OnJoinSessionComplete`）・試合終了／リセット時（`OnMatchEnded` / `ResetCurrentData` / `OnDestroyComplete`）に `ResetURI` と `bHasGameStarted` を適切にリセットすることで、無限再接続を起こさずに固定ポートでの連続再起動（同端重開）と途中切断からの再接続を安定化します。
  - **Plan B（Private URI 検証版・20 パッチ、`plan_b/eos_fix.dll` / `--features private-uri`）**:
    - Plan A の全 19 パッチに加え、`FOnlineSessionEOS::AddLobbyAttribute`（`0x9CA894`）で `"URI"` 属性のみ `EOS_LAT_PRIVATE (1)`（ロビー参加メンバー限定公開）に切り替え、入室前の EOS 照会から専用サーバー IP を秘匿します。
- **個別選択 MOD (`1〜22`・サーバー単体動作)**:
  - `1: 役職ランダム` (`single/役職ランダム.js`)
  - `2: 開幕沈黙` (`single/開幕沈黙.js`)
  - `3: マップ補強(山頂)` (`single/マップ補強_山頂.js` / `21` と排他)
  - `4: マップ補強(広漠)` (`single/マップ補強_広漠.js` / `22` と排他)
  - `5: 全サボタージュ` (`single/全サボタージュ.js`)
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
- **2 段階の起動設定 & 双タブ TUI & 再起動時ポート切替（切替あり / 固定ポート選択可）**:
  - 第 1 段階 `[Webコンソール設定]` で Web ポートと自動ブロードキャストを設定後、即座に Web コンソールを起動し双タブ TUI へ移行（タブ `[1]` で認証 URL のクリップボードコピー・操作ログ閲覧が可能）。
  - 第 2 段階 `[サーバー・試合設定]` で再起動時のポート切替有無（`1:切替あり(2ポート交互)` デフォルト `7895 <-> 7897` / `0:固定ポート(1ポート)`）・マップ・詳細設定・MOD（クライアント `.pak` 必須のコンテンツ MOD は洋紅色表示で区別）を選択。試合終了時および UDP ウォッチドッグ検知時は 10 秒間の再設定待機（`R` で再設定、`Enter` で即再起動）後に設定に従って切替または同ポート維持で自動再起動します。
- **統合 Web コンソール (Angular Neo-Brutalist UI + モバイル対応)**:
  - デフォルトで `コンソール命令` を選択済み、PC 向けカスタムドロップダウン & モバイル向けボトムシート対応、AES-256-GCM 認証・チャレンジ防重放付き。

## 使い方 (Windows Server)
1. `dist/DH_ServerTool_JA.exe` のみを原生 `WindowsServer` フォルダ（`DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe` の上位階層）に配置します。
2. `DH_ServerTool_JA.exe` を実行すると初回起動時にデフォルト設定の `./config.json` が自動作成されます。`[Webコンソール設定]` -> `[サーバー・試合設定]` の順に設定してください（ポート項目は `Enter` でデフォルト値を使用できます）。
3. コンテンツ MOD（`カスタムスペル` など）を有効化する場合は、参加クライアント側で `dist/advanced_content/<MOD名>/`（ソース配置: `assets/advanced_content/client/paked/<MOD名>/`）配下の対応 `.pak` / `.sig` を `DreadHunger/Content/Paks/~mods/` に配置してください（サーバー側への `.pak` 配置は不要です）。
4. 固定ポートでの同端再起動や非公開カスタム部屋の途中切断再接続を有効化する場合は、少なくとも**部屋ホスト側**で `dist/client_fix/eos_fix/plan_a/eos_fix.dll`（または入室前 EOS IP 秘匿検証版の `dist/client_fix/eos_fix/plan_b/eos_fix.dll`）を `DreadHunger-Win64-Shipping.exe` に注入するか、`dsound.dll` へリネームしてゲーム本体の `DreadHunger/Binaries/Win64/`（`DreadHunger-Win64-Shipping.exe` と同階層）に配置してください（参加メンバーはバニラ状態でも同端重開・クラッシュ再起動再接続・Steam 招待再接続が機能します。ホスト途中クラッシュ時の EOS オーナー移譲まで保護したい場合は全参加者への導入を推奨します。`.pak` とは独立しており、`DH_TotemSpells_Client_p.pak` との併用時も競合しません）。

## `config.json` 設定項目 (自動生成デフォルト)
```json
{
  "serverPorts": [7895, 7897],
  "switchPortOnRestart": true,
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
- `switchPortOnRestart`: 再起動時のサーバーポート切替のデフォルト設定（`true`: 2 ポート交互切替 / `false`: 固定 1 ポート維持。既存 `config.json` で省略時は `true` にフォールバック）。

## ワークスペース・資産構成
- `crates/dh-win32/`: Win32 API ラッパー（Job Object・プロセス制御・PEB/OEP 読み取り・UDP/TCP テーブル・コンソール／クリップボード）
- `crates/dh-gadget-injector/`: 独立再利用可能な Frida Gadget 17.27.4 OEP 早注入・キャッシュ管理・16→17 互換 Prelude・ループバック NDJSON ログチャンネル
- `crates/dh-server-tool/`: メイン実行ファイル (`DH_ServerTool_JA.exe`)（設定・AES-256-GCM・Web コンソール・双タブ TUI・CLI・MOD レジストリ・Supervisor）
- `crates/dh-client-eos-fix/`: クライアント用 EOS 再接続・同ポート再起動修正 DLL（`eos_fix.dll` 直接注入／`dsound.dll` プロキシ両対応、Plan A 19 パッチ標準版／Plan B 20 パッチ `--features private-uri`：EOS ロビー検索権限・公開状態保持・`MEMBERS` 1000B 拡張・Steam 招待維持・`STARTUTCDATETIME` 補完・`ResetURI` / `bHasGameStarted` 制御）
- `assets/patches/`: デフォルト自動注入・サーバー単体 MOD スクリプト (`.js`)
- `assets/advanced_content/server/`: コンテンツ MOD 用サーバー側スクリプト (`.js`)
- `assets/advanced_content/client/paked/`: コンテンツ MOD 用クライアント配布 `.pak` / `.sig`
- `assets/advanced_content/client/source/`: クライアント `.pak` 展開済みソース資産
- `assets/console/`・`assets/languages/`: Web コンソールテンプレートおよび言語ファイル

## 開発・ビルド
> **注意**: `.cargo/config.toml`（静的 CRT `+crt-static` 設定）を確実に適用するため、すべての `cargo` コマンドは **リポジトリルート（Cargo Workspace ルート）** で実行してください。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p dh-client-eos-fix --features private-uri --all-targets -- -D warnings
cargo test --workspace
cargo test -p dh-client-eos-fix --features private-uri
cargo build --release
cargo build --release -p dh-client-eos-fix --features private-uri
```

配布用 `dist/` 出力構成（git 無視対象）:
- `dist/DH_ServerTool_JA.exe`: サーバーコントローラ本体（`target/release/DH_ServerTool_JA.exe` からコピー）
- `dist/advanced_content/<MOD名>/*.{pak,sig}`: クライアント配布用 `.pak` / `.sig` のみ（`assets/advanced_content/client/paked/` から MOD 別サブフォルダを維持してコピー。例: `dist/advanced_content/totem_spells/DH_TotemSpells_Client_p.{pak,sig}`。`source/`・`.uasset`/`.uexp`・サーバー `.js` は同梱しません）
- `dist/client_fix/eos_fix/plan_a/eos_fix.dll`: クライアント EOS 修正 DLL Plan A（標準 19 パッチ、デフォルトリリースビルド）
- `dist/client_fix/eos_fix/plan_b/eos_fix.dll`: クライアント EOS 修正 DLL Plan B（20 パッチ、`--features private-uri` リリースビルド）

## ライセンス / License

本プロジェクトは **[DH_ServerTool_JA Custom Source-Available Non-Commercial License](./LICENSE)**（日本語・简体中文・English の 3 言語を収録。解釈に相違がある場合は簡体字中国語版を基準とします）の下で公開されています。

- **著作権帰属**: ゲーム『Dread Hunger』本体および公式オリジナル資産の知的財産権はニュージーランドの **Digital Confectioners Ltd.** に帰属し、ゲーム内に含まれる一部の UE4 有料商用アセットの権利帰属は原アセットの本来の権利関係・ライセンス条項および公式の購入・許諾状況に従います。本プロジェクトは自己ホスト専用サーバー向けの非公式コミュニティツールであり、無保証（AS IS）で提供されます。
- **許可**: メンテナーが著作権を留保する独自オリジナル部分について、個人または非営利目的での学習、技術交流、自己ホスト運用、改変、および共有を無料で行えます（互換性のため付随するゲームの公開取得可能なアセットについても、メンテナー側からの追加制限は設けませんが、元の権利関係・法令遵守は利用者自身の責任となります）。
- **義務**: 二次開発・改変・再配布を行う場合は、**完全なソースコードの公開（Source-Available）**、**本ライセンスの継承**、および**本ライセンス全文・元の著作権表示・出典の保持**が必要です。
- **禁止**: クローズドソース化、暗号化・難読化のみでの配布、直接・間接を問わない一切の商業利用・収益化、および特定プラットフォームや私設サーバーでの独占利用は禁止されています。

正式な条件・権利境界・公式および原アセット権利者向け削除窓口については、必ず [LICENSE](./LICENSE) の全文をご確認ください。
