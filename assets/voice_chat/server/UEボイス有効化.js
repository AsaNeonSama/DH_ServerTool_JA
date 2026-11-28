'use strict';

// ============================================================================
// UE4 ネイティブボイス有効化 (クライアント連携必須: dh_voice_client.dll)
//
// 旧サーバー側 PAK (DH_Voice_Server_p.pak / DedicatedServerEngine.ini) の
// 機能を純JS・ゼロ資産・ディスク無変更で完全再現する。
//
// 適用内容 (UE4 メイン/ゲームスレッド上で GConfig へメモリ内反映):
//   [Engine.ini / GEngineIni]
//     [OnlineSubsystem]
//       bHasVoiceEnabled=true
//       MaxLocalTalkers=1
//       MaxRemoteTalkers=16
//       VoiceNotificationDelta=0.2
//     [Voice]
//       bEnabled=true
//     [/Script/Engine.AudioSettings]
//       VoiPSampleRate=Normal24000Hz  (24000 Hz / MaxVoiceDataSize=14336 / MaxCompressed=2048)
//   [Game.ini / GGameIni]
//     [/Script/Engine.GameSession]
//       bRequiresPushToTalk=false
//
// 設計方針:
//   - FConfigFile::Dirty (+0x50) の復元と FConfigFile::NoSave (+0x51) = 1 により
//     Saved/Config/{Engine,Game}.ini への書き出しを完全に防止
//     (次回試合で本MODを無効化すれば完全に原版挙動へ復帰)。
//   - 全 GConfig 書き込みを UE4 メイン/ゲームスレッド上のライフサイクルフック
//     (InitializeConfigSystem::onLeave / OnlineSubsystemModule / VoiceModule /
//      OnlineVoiceImpl::Init / GetVoiceSampleRate / InitGameState) からのみ実行し、
//     Frida ワーカースレッドとのデータ競合を完全に排除。
//   - Frida 16 / Frida 17 両対応 (NativePointer メソッド + exceptions: 'propagate' 明示)。
//   - onLeave フックは未競合 RVA (0x10F0110) の 1 箇所のみ (他全 MOD と競合ゼロ)。
// ============================================================================

(function () {
    'use strict';

    var MODULE_NAME = 'DreadHungerServer-Win64-Shipping.exe';
    var moduleBase = Module.findBaseAddress(MODULE_NAME);
    if (!moduleBase || moduleBase.isNull()) {
        return;
    }

    // PDB (DreadHungerServer-Win64-Shipping.pdb) で検証済みの RVA 一覧
    var RVA = {
        GConfig: 0x4581920,
        GEngineIni: 0x458BB40,
        GGameIni: 0x458BBF0,
        GWorld: 0x46ED420,
        UVOIPStatics_bHasReadSampleRate: 0x46ED550,
        UVOIPStatics_CachedSampleRate: 0x46ED554,

        FConfigCacheIni_InitializeConfigSystem: 0x10F0110,
        FConfigCacheIni_Find: 0x10E7810,
        FConfigCacheIni_SetString: 0x10FA360,

        FOnlineSubsystemModule_StartupModule: 0x8C18A0,
        FVoiceModule_StartupModule: 0x2D343D0,
        FOnlineVoiceImpl_Init: 0x8FF000,

        Online_GetVoiceInterface: 0x8FD700,
        UVOIPStatics_GetVoiceSampleRate: 0x2968540,
        UVOIPStatics_GetMaxVoiceDataSize: 0x2967BC0,
        UVOIPStatics_GetMaxCompressedVoiceDataSize: 0x2967A40,
        AGameModeBase_InitGameState: 0x2569A30
    };

    var OFF = {
        fStringNum: 0x08,
        fConfigFileDirty: 0x50,
        fConfigFileNoSave: 0x51,
        refCtrlSharedCount: 0x08,
        gameModeGameSession: 0x278,
        gameSessionRequiresPushToTalk: 0x22D
    };

    var TARGET_SAMPLE_RATE = 24000;

    // 適用する INI 設定一覧 (DH_Voice_Server_p.pak 完全互換 + bRequiresPushToTalk=false)
    var ENGINE_INI_OVERRIDES = [
        ['OnlineSubsystem', 'bHasVoiceEnabled', 'true'],
        ['OnlineSubsystem', 'MaxLocalTalkers', '1'],
        ['OnlineSubsystem', 'MaxRemoteTalkers', '16'],
        ['OnlineSubsystem', 'VoiceNotificationDelta', '0.2'],
        ['Voice', 'bEnabled', 'true'],
        ['/Script/Engine.AudioSettings', 'VoiPSampleRate', 'Normal24000Hz']
    ];

    var GAME_INI_OVERRIDES = [
        ['/Script/Engine.GameSession', 'bRequiresPushToTalk', 'false']
    ];

    // グローバルアドレス
    var GConfig_ptr = moduleBase.add(RVA.GConfig);
    var GEngineIni_ptr = moduleBase.add(RVA.GEngineIni);
    var GGameIni_ptr = moduleBase.add(RVA.GGameIni);
    var GWorld_ptr = moduleBase.add(RVA.GWorld);
    var bHasReadSampleRate_ptr = moduleBase.add(RVA.UVOIPStatics_bHasReadSampleRate);
    var cachedSampleRate_ptr = moduleBase.add(RVA.UVOIPStatics_CachedSampleRate);

    // ネイティブ関数バインディング (単独 Frida 16 互換のため exceptions: 'propagate' 明示)
    var fnOpt = { abi: 'win64', exceptions: 'propagate' };

    var fnConfigFind = new NativeFunction(
        moduleBase.add(RVA.FConfigCacheIni_Find),
        'pointer',
        ['pointer', 'pointer', 'uint8'],
        fnOpt
    );

    var fnConfigSetString = new NativeFunction(
        moduleBase.add(RVA.FConfigCacheIni_SetString),
        'void',
        ['pointer', 'pointer', 'pointer', 'pointer', 'pointer'],
        fnOpt
    );

    var fnOnlineGetVoiceInterface = new NativeFunction(
        moduleBase.add(RVA.Online_GetVoiceInterface),
        'pointer',
        ['pointer', 'pointer', 'uint64'],
        fnOpt
    );

    var fnGetVoiceSampleRate = new NativeFunction(
        moduleBase.add(RVA.UVOIPStatics_GetVoiceSampleRate),
        'int32',
        [],
        fnOpt
    );

    var fnGetMaxVoiceDataSize = new NativeFunction(
        moduleBase.add(RVA.UVOIPStatics_GetMaxVoiceDataSize),
        'uint32',
        [],
        fnOpt
    );

    var fnGetMaxCompressedVoiceDataSize = new NativeFunction(
        moduleBase.add(RVA.UVOIPStatics_GetMaxCompressedVoiceDataSize),
        'uint32',
        [],
        fnOpt
    );

    var configApplied = false;
    var runtimeVerified = false;

    function isFStringNonEmpty(fstrPtr) {
        if (!fstrPtr || fstrPtr.isNull()) {
            return false;
        }
        var dataPtr = fstrPtr.readPointer();
        var num = fstrPtr.add(OFF.fStringNum).readS32();
        return !dataPtr.isNull() && num > 1;
    }

    // FConfigFile::Dirty (+0x50) を復元しつつ NoSave (+0x51) = 1 としてメモリ上のみ上書き
    function applyIniEntriesNoSave(gConfig, iniFStringPtr, entries) {
        if (gConfig.isNull() || !isFStringNonEmpty(iniFStringPtr)) {
            return 0;
        }
        var cfgFile = fnConfigFind(gConfig, iniFStringPtr, 0);
        var hasCfgFile = !cfgFile.isNull();
        var prevDirty = hasCfgFile ? cfgFile.add(OFF.fConfigFileDirty).readU8() : 0;

        var applied = 0;
        try {
            for (var i = 0; i < entries.length; i++) {
                var sectionBuf = Memory.allocUtf16String(entries[i][0]);
                var keyBuf = Memory.allocUtf16String(entries[i][1]);
                var valueBuf = Memory.allocUtf16String(entries[i][2]);
                fnConfigSetString(gConfig, sectionBuf, keyBuf, valueBuf, iniFStringPtr);
                applied += 1;
            }
        } finally {
            if (hasCfgFile) {
                cfgFile.add(OFF.fConfigFileDirty).writeU8(prevDirty);
                // FConfigFile::Write (0x10FCC0E) の NoSave ガードを有効化しディスク汚染を防止
                cfgFile.add(OFF.fConfigFileNoSave).writeU8(1);
            }
        }
        return applied;
    }

    // UE4 メイン/ゲームスレッド上からのみ呼び出される冪等な設定反映関数
    function ensureVoiceConfigApplied(triggerLabel) {
        if (configApplied) {
            return true;
        }
        try {
            var gConfig = GConfig_ptr.readPointer();
            if (gConfig.isNull()) {
                return false;
            }
            if (!isFStringNonEmpty(GEngineIni_ptr) || !isFStringNonEmpty(GGameIni_ptr)) {
                return false;
            }

            var engineCount = applyIniEntriesNoSave(gConfig, GEngineIni_ptr, ENGINE_INI_OVERRIDES);
            var gameCount = applyIniEntriesNoSave(gConfig, GGameIni_ptr, GAME_INI_OVERRIDES);
            if (engineCount !== ENGINE_INI_OVERRIDES.length || gameCount !== GAME_INI_OVERRIDES.length) {
                return false;
            }

            // 遅延注入等でサンプルレートがキャッシュ済みであれば 24000 Hz へ同期
            if (bHasReadSampleRate_ptr.readU8() !== 0) {
                cachedSampleRate_ptr.writeS32(TARGET_SAMPLE_RATE);
            }

            configApplied = true;
            send(
                '[UEボイス有効化] 設定適用完了 (' + triggerLabel + '): Engine=' +
                engineCount + '件 / Game=' + gameCount + '件 (メモリ専用・INI無変更)'
            );
            return true;
        } catch (e) {
            send('[UEボイス有効化] 設定適用エラー (' + triggerLabel + '): ' + String(e));
            return false;
        }
    }

    // ゲームスレッド上で 1 回だけ実稼働状態を確認し、一時 TSharedPtr の参照カウントを安全に解放する
    function verifyVoiceRuntimeOnce(gameModePtr) {
        if (runtimeVerified) {
            return;
        }
        runtimeVerified = true;

        try {
            ensureVoiceConfigApplied('InitGameState');

            var pttValue = -1;
            if (gameModePtr && !gameModePtr.isNull()) {
                var gameSession = gameModePtr.add(OFF.gameModeGameSession).readPointer();
                if (!gameSession.isNull()) {
                    if (gameSession.add(OFF.gameSessionRequiresPushToTalk).readU8() !== 0) {
                        gameSession.add(OFF.gameSessionRequiresPushToTalk).writeU8(0);
                    }
                    pttValue = gameSession.add(OFF.gameSessionRequiresPushToTalk).readU8();
                }
            }

            var world = GWorld_ptr.readPointer();
            var outShared = Memory.alloc(16);
            outShared.writePointer(ptr(0));
            outShared.add(8).writePointer(ptr(0));

            fnOnlineGetVoiceInterface(outShared, world, 0);
            var voiceObj = outShared.readPointer();
            var refCtrl = outShared.add(8).readPointer();
            var voiceAvailable = !voiceObj.isNull();

            // OnlineSubsystem が保持する所有参照 (count >= 2) を維持したまま一時コピー参照のみ解放
            if (!refCtrl.isNull()) {
                var sharedCount = refCtrl.add(OFF.refCtrlSharedCount).readS32();
                if (sharedCount > 1) {
                    refCtrl.add(OFF.refCtrlSharedCount).writeS32(sharedCount - 1);
                }
            }

            var sampleRate = fnGetVoiceSampleRate();
            var maxVoiceSize = fnGetMaxVoiceDataSize();
            var maxCompressedSize = fnGetMaxCompressedVoiceDataSize();

            send(
                '[UEボイス有効化] 稼働確認: VoiceInterface=' + (voiceAvailable ? '有効' : '無効') +
                ' / SampleRate=' + sampleRate + 'Hz' +
                ' / MaxVoiceDataSize=' + maxVoiceSize + 'B' +
                ' / MaxCompressed=' + maxCompressedSize + 'B' +
                ' / PushToTalk=' + (pttValue === 0 ? '無効' : String(pttValue))
            );
        } catch (e) {
            send('[UEボイス有効化] 稼働確認エラー: ' + String(e));
        }
    }

    // 1. 主要適用ポイント: FConfigCacheIni::InitializeConfigSystem 完了直後 (onLeave x1)
    try {
        Interceptor.attach(moduleBase.add(RVA.FConfigCacheIni_InitializeConfigSystem), {
            onLeave: function () {
                try {
                    ensureVoiceConfigApplied('InitializeConfigSystem');
                } catch (e) {}
            }
        });
    } catch (e) {
        send('[UEボイス有効化] InitializeConfigSystem フック失敗: ' + String(e));
    }

    // 2. 起動順フォールバック (全て UE4 メイン/ゲームスレッド・onEnter のみ・他MOD競合ゼロ)
    var earlyMainThreadHooks = [
        [RVA.FOnlineSubsystemModule_StartupModule, 'OnlineSubsystemModule::StartupModule'],
        [RVA.FVoiceModule_StartupModule, 'VoiceModule::StartupModule'],
        [RVA.FOnlineVoiceImpl_Init, 'OnlineVoiceImpl::Init']
    ];

    for (var hi = 0; hi < earlyMainThreadHooks.length; hi++) {
        (function (rva, label) {
            try {
                Interceptor.attach(moduleBase.add(rva), {
                    onEnter: function () {
                        try {
                            ensureVoiceConfigApplied(label);
                        } catch (e) {}
                    }
                });
            } catch (e) {
                send('[UEボイス有効化] ' + label + ' フック失敗: ' + String(e));
            }
        })(earlyMainThreadHooks[hi][0], earlyMainThreadHooks[hi][1]);
    }

    // 3. UVOIPStatics::GetVoiceSampleRate 入口ガード (onEnter のみ)
    try {
        Interceptor.attach(moduleBase.add(RVA.UVOIPStatics_GetVoiceSampleRate), {
            onEnter: function () {
                try {
                    ensureVoiceConfigApplied('GetVoiceSampleRate');
                    if (bHasReadSampleRate_ptr.readU8() !== 0 && cachedSampleRate_ptr.readS32() !== TARGET_SAMPLE_RATE) {
                        cachedSampleRate_ptr.writeS32(TARGET_SAMPLE_RATE);
                    }
                } catch (e) {}
            }
        });
    } catch (e) {
        send('[UEボイス有効化] GetVoiceSampleRate フック失敗: ' + String(e));
    }

    // 4. ゲームスレッド初期化確認 (AGameModeBase::InitGameState、onEnter のみ・他MOD競合ゼロ)
    try {
        Interceptor.attach(moduleBase.add(RVA.AGameModeBase_InitGameState), {
            onEnter: function (args) {
                try {
                    verifyVoiceRuntimeOnce(args[0]);
                } catch (e) {}
            }
        });
    } catch (e) {
        send('[UEボイス有効化] InitGameState フック失敗: ' + String(e));
    }
})();
