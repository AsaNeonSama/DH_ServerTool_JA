# DH_Tool_JA (Rust + Tauri v2 + Frida Gadget 17.27.4)

Dread Hunger Windows Dedicated Server（原生未改造サーバー）＆ Windows クライアント向けの統合ランチャー・OEP 早注入・アプリ内蔵管理コンソールツールです。

- 作者: **アサネオン** / **Build With Antigravity**

## 特徴
- **2 つの単一実行ファイル GUI (`dist/DH_ServerLauncher_JA.exe`・`dist/DH_ClientLauncher_JA.exe`)**:
  - Rust + Tauri v2 + 静的 CRT リンク (`+crt-static`) により、`VCRUNTIME140.dll` などの外部ランタイム不要・ネイティブ WebView2 デスクトップ GUI（Angular Neo-Brutalist デザイン）で単独動作します。
  - 原生サーバー／クライアントバイナリ無改造のまま、OEP ガイドスタブによる 0 秒早注入で JS パッチおよび DLL を安全に適用します（`frida.dll` や `.stub` 配置不要）。
  - 配布フォルダ `dist/` には実行ファイル (`.exe`) とクライアント配布用 `.pak` / `.sig` のみを格納します（サーバー側 `dh_voice_server.dll` は `DH_ServerLauncher_JA.exe` 内に、クライアント側 EOS 修復 DLL Plan A/B および `dh_voice_client.dll` は `DH_ClientLauncher_JA.exe` 内に zstd 圧縮・SHA-256 検証付きで内蔵）。
- **サーバーランチャー (`DH_ServerLauncher_JA.exe` / `crates/dh-server-launcher/`)**:
  - **ゼロ HTTP 公開・アプリ内蔵管理コンソール**: 外部向け HTTP ポートを一切待受せず、アプリ内の「コンソール」タブからローカルループバック (`127.0.0.1:<consolePatchPort>`) 経由で `コンソール連携.js` へ直接送信します（コマンド実行・ステータス設定・アイテム追加 44 種検索・ストレージ設定・個別／全体チャット・キック・自動ブロードキャスト対応）。
  - **デフォルト自動注入 MOD (5 種・常時有効)**:
    - `default/コンソール連携.js`（ローカル管理コンソール連携・召喚前ブループリント自動ロード・幽体状態／傀儡解除対応・ポート変数化）
    - `default/スペルチャージ修正.js`（スペルチャージ修正）
    - `default/死亡切断通知.js`（オフライン・死亡アナウンス日本語版）
    - `default/接続保護_乱入防止.js`（ネットワーク接続保護・乱入防止：`EnteringMap`/`WaitingToStart` 和集合統合による開局名簿ロック・ゾンビ接続および残留旧 Controller の即時クリーンアップ・満員時の開局メンバー許可・無人 10 秒サーバー終了タイマー解除＆CloseServer 抑止）
    - `default/食人鬼登攀補強.js`（急斜面 78° 登攀・2 段階垂直氷壁／岸壁登攀 `MOVE_Flying`＋緩斜面自動復帰・3 層防爬船＆舷梯遮断・原版 ThreatTable 互換、野生動物非干渉）
  - **3 区分 MOD セレクター & 排他ルール制御**:
    1. **クライアント導入必須 MOD (2 種)**:
       - `カスタムスペル(新呪文5種)` (`[PAK必須]`: サーバー側純 JS `assets/advanced_content/server/totem_spells/カスタムスペル.js` / クライアント側 `dist/advanced_content/totem_spells/DH_TotemSpells_Client_p.{pak,sig}`)
       - `自有音声サーバー` (`[DLL必須]`: サーバー側内蔵 `dh_voice_server.dll` をゲーム同一 UDP ポート多重化で OEP 注入 / クライアント側 `DH_ClientLauncher_JA.exe` の内蔵 **自有音声クライアント** トグルを有効化)
    2. **実用系 MOD (`1〜13`・サーバーのみ)**:
       - `1: 役職ランダム` / `2: 開幕沈黙` / `3: マップ補強(山頂)` (`21` と排他) / `4: マップ補強(広漠)` (`22` と排他) / `5: 全サボタージュ` / `6: 狂人モード` / `7: 骨の短剣モード [3本パック]` / `8: クエスト報酬` / `9: 全員バックパック` / `10: 万能砥石` / `11: 矢回収100%` / `12: 暴風雪マスケット` (`14` と排他) / `13: 銃・弓同時所持`
    3. **バラエティ・特殊 MOD (`14〜22`・サーバーのみ)**:
       - `14: 開幕マスケット` (`12` と排他) / `15: 毒耐性` / `16: ミステリーモード` / `17: グレネード` / `18: 八艘飛び` / `19: スペルチャージ無限` / `20: 無限復活` / `21: 観光モード(山頂)` (`3`・`22` と排他) / `22: 観光モード(広漠)` (`4`・`21` と排他)
  - **稼働モニター・ウォッチドッグ・ログ管理**:
    - Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) によりアプリ終了時にサーバープロセスを連動終了。
    - 2 ポート交互切替 (`7895 <-> 7897`)／固定 1 ポート維持の切替、UDP リッスン監視＆自動再起動カウントダウン、リアルタイム Frida 注入進捗バー表示。
    - Frida JS パッチ実行ログは起動時刻ごとの `./patch_runtime_YYYYMMDD_HHMMSS.log` に保存し、時系列で最新の 3 試合分のみ自動保持します。
- **クライアントランチャー (`DH_ClientLauncher_JA.exe` / `crates/dh-client-launcher/`)**:
  - **内蔵クライアント MOD のみ（ワンクリック有効化・外部 DLL/JS インポート不要・デフォルト引数固定起動）**:
    - **EOS 再接続・同ポート再起動修正 (`crates/dh-client-eos-fix/`)**: Plan A（標準・19 パッチ）と Plan B（非公開 URI 検証版・20 パッチ）を排他トグルでワンクリック切替。非公開／カスタム部屋のロビー検索権限・`MEMBERS` 1000B 拡張・途中切断／クラッシュ再起動・Steam 招待再接続・固定ポート連続再起動を修正します。
    - **自有音声クライアント (`crates/dh-client-voice/` / `dh_voice_client.dll`)**: 追加ポート不要のゲーム同一 UDP ポート多重化 (`DHV2`)・3D ボイスエンジン（WASAPI キャプチャ＆ループバック参照 → 80 Hz HPF + WebRTC `aec3` + RNNoise `nnnoiseless` + VAD/AGC → ゲーム内蔵 `libopus` 48 kHz 32 kbps In-Band FEC/DTX/PLC → 話者別 Adaptive Jitter Buffer → `BeforeRecvAudioRendered` へ出力、ロビー復帰時は EOS ボイスへ自動切替）。サーバー側 **自有音声サーバー** (`dh_voice_server.dll`) と連携します。
  - Steam 環境変数 (`SteamAppId=1418630` / `SteamGameId=1418630`) を子プロセスへ安全に引き渡して起動し、設定は `./launcher_config.json` に永続化されます。

## 使い方
### 1. サーバー側 (`DH_ServerLauncher_JA.exe`)
1. `dist/DH_ServerLauncher_JA.exe` を原生 `WindowsServer` フォルダ（`DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe` の上位階層）に配置します。
2. `DH_ServerLauncher_JA.exe` を起動すると初回に `./launcher_config.json` が自動生成されます。「基本設定」タブでポートやパスを確認し、「サーバー起動・MOD」タブでマップ・試合ルール・MOD を選択して起動します。
3. 「コンソール」タブから起動中サーバーへ管理コマンド・アイテム付与・ステータス変更・チャット通知を直接送信できます。

### 2. クライアント側 (`DH_ClientLauncher_JA.exe` & `.pak`)
1. `dist/DH_ClientLauncher_JA.exe` を起動し、Steam 版 `DreadHunger-Win64-Shipping.exe` のパスを選択（または自動検出）します。
2. 内蔵トグルから **EOS 修復 (Plan A / Plan B)** や **自有音声クライアント** を有効化してゲームを起動します（初期状態: EOS Plan B = ON、自有音声クライアント = OFF）。
3. サーバー側で `カスタムスペル(新呪文5種)` を有効にする場合は、`dist/advanced_content/totem_spells/DH_TotemSpells_Client_p.{pak,sig}` をクライアントの `DreadHunger/Content/Paks/~mods/` に配置してください。

## サーバー `launcher_config.json` 設定項目 (自動生成デフォルト)
```json
{
  "serverPorts": [7895, 7897],
  "switchPortOnRestart": true,
  "consolePatchPort": 7893,
  "restartWaitSeconds": 10,
  "autoBroadcastDefault": true,
  "serverExecutable": "./DreadHungerServer.exe",
  "shippingExecutable": "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
  "shippingProcessName": "DreadHungerServer-Win64-Shipping.exe"
}
```
- `switchPortOnRestart`: 再起動時のサーバーポート切替設定（`true`: 2 ポート交互切替 / `false`: ポート 1 固定維持）。
- `consolePatchPort`: アプリ内蔵コンソールと `コンソール連携.js` 間のローカルループバック (`127.0.0.1`) 専用 TCP ポート（外部には公開されません）。

## ワークスペース・資産構成
- `crates/dh-win32/`: Win32 API ラッパー（Job Object・プロセス制御・PEB/OEP 読み取り・UDP/TCP テーブル・ローカル時刻取得・コンソール／クリップボード）
- `crates/dh-gadget-injector/`: 独立再利用可能な Frida Gadget 17.27.4 & マルチ DLL OEP 早注入・キャッシュ管理・16→17 互換 Prelude・ループバック NDJSON ログチャンネル
- `crates/dh-voice-proto/`: 同一 UDP ポート多重化用 `DHV2` バイナリプロトコル（`Hello`/`HelloAck`/`PeerMap`/`VoiceFrame`/`Bye` コーデック）
- `crates/dh-server-voice/`: サーバー用同一 UDP ポート 3D ボイス中継 DLL (`dh_voice_server.dll`、追加ポート不要)
- `crates/dh-server-launcher/`: サーバーランチャー GUI 本体 (`DH_ServerLauncher_JA.exe`、`dh_voice_server.dll` 内蔵)
- `crates/dh-client-launcher/`: クライアントランチャー GUI 本体 (`DH_ClientLauncher_JA.exe`、EOS Fix Plan A/B & `dh_voice_client.dll` 内蔵)
- `crates/dh-client-eos-fix/`: クライアント用 EOS 再接続・同ポート再起動修正 DLL（Plan A 19 パッチ／Plan B 20 パッチ `--features private-uri`）
- `crates/dh-client-voice/`: クライアント用純 Rust 3D ボイスエンジン DLL (`dh_voice_client.dll`、WASAPI + AEC3 + RNNoise + Opus 48 kHz FEC/PLC)
- `ui/server/`・`ui/client/`: Tauri v2 静的フロントエンド (HTML / CSS / Vanilla JS)
- `assets/patches/`: デフォルト自動注入・サーバー単体 MOD スクリプト (`.js`)
- `assets/advanced_content/server/`: コンテンツ MOD 用サーバー側純 JS スクリプト（`totem_spells/`）
- `assets/advanced_content/client/paked/`: コンテンツ MOD 用クライアント配布 `.pak` / `.sig`
- `assets/advanced_content/client/source/`: クライアント `.pak` 展開済みソース資産
- `assets/voice_chat/`: 自有音声サーバー／自有音声クライアント用ステージング DLL（`server/dh_voice_server.dll`・`client/dh_voice_client.dll`）

## 開発・ビルド
> **注意**: `.cargo/config.toml`（静的 CRT `+crt-static` 設定）を確実に適用するため、すべての `cargo` コマンドは **リポジトリルート（Cargo Workspace ルート）** で実行してください。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p dh-client-eos-fix --features private-uri --all-targets -- -D warnings
cargo test --workspace
cargo test -p dh-client-eos-fix --features private-uri
cargo build --release -p dh-client-eos-fix
cargo build --release -p dh-client-eos-fix --features private-uri
cargo build --release -p dh-server-voice -p dh-client-voice
cargo build --release -p dh-server-launcher -p dh-client-launcher
```

配布用 `dist/` 出力構成（git 無視対象・`.exe` および `.pak`/`.sig` のみ）:
- `dist/DH_ServerLauncher_JA.exe`
- `dist/DH_ClientLauncher_JA.exe`
- `dist/advanced_content/totem_spells/DH_TotemSpells_Client_p.pak`
- `dist/advanced_content/totem_spells/DH_TotemSpells_Client_p.sig`

## ライセンス / License

本プロジェクトは **[DH_Tool_JA Custom Source-Available Non-Commercial License](./LICENSE)**（日本語・简体中文・English の 3 言語を収録。解釈に相違がある場合は簡体字中国語版を基準とします）の下で公開されています。

- **著作権帰属**: ゲーム『Dread Hunger』本体および公式オリジナル資産の知的財産権はニュージーランドの **Digital Confectioners Ltd.** に帰属し、ゲーム内に含まれる一部の UE4 有料商用アセットの権利帰属は原アセットの本来の権利関係・ライセンス条項および公式の購入・許諾状況に従います。本プロジェクトは自己ホスト専用サーバー向けの非公式コミュニティツールであり、無保証（AS IS）で提供されます。
- **許可**: メンテナーが著作権を留保する独自オリジナル部分について、個人または非営利目的での学習、技術交流、自己ホスト運用、改変、および共有を無料で行えます（互換性のため付随するゲームの公開取得可能なアセットについても、メンテナー側からの追加制限は設けませんが、元の権利関係・法令遵守は利用者自身の責任となります）。
- **義務**: 二次開発・改変・再配布を行う場合は、**完全なソースコードの公開（Source-Available）**、**本ライセンスの継承**、および**本ライセンス全文・元の著作権表示・出典の保持**が必要です。
- **禁止**: クローズドソース化、暗号化・難読化のみでの配布、直接・間接を問わない一切の商業利用・収益化、および特定プラットフォームや私設サーバーでの独占利用は禁止されています。

正式な条件・権利境界・公式および原アセット権利者向け削除窓口については、必ず [LICENSE](./LICENSE) の全文をご確認ください。
