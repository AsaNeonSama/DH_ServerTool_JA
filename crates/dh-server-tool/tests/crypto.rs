use dh_server_tool::crypto::{
    decode_canonical_b64url, encode_b64url_nopad, AesGcmSessionCipher, COOKIE_AAD,
};

#[test]
fn roundtrip_and_aad_binding() {
    let cipher = AesGcmSessionCipher::generate();
    assert_eq!(decode_canonical_b64url(cipher.psk()).unwrap().len(), 32);

    let aad = Some("REQ:/dh-console/console_command");
    let plaintext = r#"{"player_name":"Captain","command":"summon BP_Prey_Rat_C"}"#;
    let enc = cipher.encrypt(plaintext, aad);

    assert_eq!(cipher.decrypt(&enc, aad).as_deref(), Some(plaintext));
    assert!(cipher.verify(&enc, plaintext, aad));
    assert!(!cipher.verify(&enc, "wrong", aad));

    // Wrong AAD must fail
    assert_eq!(
        cipher.decrypt(&enc, Some("RES:/dh-console/console_command")),
        None
    );
    assert_eq!(cipher.decrypt(&enc, None), None);
}

#[test]
fn handles_empty_plaintext_and_unicode() {
    let cipher = AesGcmSessionCipher::generate();

    let empty_enc = cipher.encrypt("", Some(COOKIE_AAD));
    assert_eq!(decode_canonical_b64url(&empty_enc).unwrap().len(), 28);
    assert_eq!(
        cipher.decrypt(&empty_enc, Some(COOKIE_AAD)).as_deref(),
        Some("")
    );

    let jp = "船長 が大爆裂魔法・チャージ 120 %を召喚しました🚀";
    let jp_enc = cipher.encrypt(jp, Some("RES:/dh-console/send_thrall_msg"));
    assert_eq!(
        cipher
            .decrypt(&jp_enc, Some("RES:/dh-console/send_thrall_msg"))
            .as_deref(),
        Some(jp)
    );
}

#[test]
fn rejects_tampered_and_non_canonical_inputs() {
    let cipher = AesGcmSessionCipher::from_raw_key([0x42u8; 32]);
    let enc = cipher.encrypt("hello", Some("AAD"));
    let mut raw = decode_canonical_b64url(&enc).unwrap();

    // Tamper IV
    raw[0] ^= 0x01;
    assert_eq!(
        cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
        None
    );
    raw[0] ^= 0x01;

    // Tamper AuthTag (bytes 12..28)
    raw[15] ^= 0x80;
    assert_eq!(
        cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
        None
    );
    raw[15] ^= 0x80;

    // Tamper Ciphertext (bytes 28..)
    raw[28] ^= 0x01;
    assert_eq!(
        cipher.decrypt(&encode_b64url_nopad(&raw), Some("AAD")),
        None
    );

    // Truncated (< 28 bytes)
    assert_eq!(
        cipher.decrypt(&encode_b64url_nopad(&raw[..27]), Some("AAD")),
        None
    );

    // Empty / padded / dirty base64url
    assert_eq!(cipher.decrypt("", Some("AAD")), None);
    assert_eq!(cipher.decrypt(&format!("{enc}=="), Some("AAD")), None);
    assert_eq!(cipher.decrypt(&format!("{enc} "), Some("AAD")), None);

    // Non-canonical trailing bits (e.g. 1 byte encoded with non-zero unused lower bits)
    assert_eq!(decode_canonical_b64url("AB"), None);
}

#[test]
fn wire_layout_matches_iv_tag_ciphertext() {
    let key = [0x11u8; 32];
    let iv = [0x22u8; 12];
    let cipher = AesGcmSessionCipher::from_raw_key(key);
    let encoded = cipher.encrypt_with_iv("test", Some("REQ:/x"), iv);
    let raw = decode_canonical_b64url(&encoded).unwrap();
    assert_eq!(&raw[..12], &iv);
    assert_eq!(raw.len(), 12 + 16 + 4);
}
