(function (globalScope) {
  "use strict";

  var SBOX = new Uint8Array([
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16
  ]);

  var RCON = new Uint8Array([0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40]);
  var textEncoder = new TextEncoder();
  var textDecoder = new TextDecoder("utf-8", { fatal: true });

  function b64uEncode(bytes) {
    var bin = "";
    for (var i = 0; i < bytes.length; i++) {
      bin += String.fromCharCode(bytes[i]);
    }
    return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  function b64uDecode(str) {
    if (typeof str !== "string" || str.length === 0 || !/^[A-Za-z0-9_-]+$/.test(str)) {
      return null;
    }
    var pad = (4 - (str.length % 4)) % 4;
    if (pad === 3) {
      return null;
    }
    var std = str.replace(/-/g, "+").replace(/_/g, "/") + "===".slice(0, pad);
    try {
      var bin = atob(std);
      var out = new Uint8Array(bin.length);
      for (var i = 0; i < bin.length; i++) {
        out[i] = bin.charCodeAt(i);
      }
      if (b64uEncode(out) !== str) {
        return null;
      }
      return out;
    } catch (_) {
      return null;
    }
  }

  function subWord(w) {
    return (
      ((SBOX[(w >>> 24) & 0xff] << 24) |
        (SBOX[(w >>> 16) & 0xff] << 16) |
        (SBOX[(w >>> 8) & 0xff] << 8) |
        SBOX[w & 0xff]) >>>
      0
    );
  }

  function rotWord(w) {
    return ((w << 8) | (w >>> 24)) >>> 0;
  }

  function expandKey256(key32) {
    var w = new Uint32Array(60);
    for (var i = 0; i < 8; i++) {
      var o = i * 4;
      w[i] =
        ((key32[o] << 24) |
          (key32[o + 1] << 16) |
          (key32[o + 2] << 8) |
          key32[o + 3]) >>>
        0;
    }
    for (var j = 8; j < 60; j++) {
      var temp = w[j - 1];
      if ((j & 7) === 0) {
        temp = (subWord(rotWord(temp)) ^ (RCON[(j >>> 3) - 1] << 24)) >>> 0;
      } else if ((j & 7) === 4) {
        temp = subWord(temp);
      }
      w[j] = (w[j - 8] ^ temp) >>> 0;
    }
    return w;
  }

  function xtime(a) {
    return ((a << 1) ^ (a & 0x80 ? 0x1b : 0)) & 0xff;
  }

  function addRoundKey(s, w, offset) {
    for (var c = 0; c < 4; c++) {
      var rk = w[offset + c];
      var i = c * 4;
      s[i] ^= (rk >>> 24) & 0xff;
      s[i + 1] ^= (rk >>> 16) & 0xff;
      s[i + 2] ^= (rk >>> 8) & 0xff;
      s[i + 3] ^= rk & 0xff;
    }
  }

  function aes256EncryptBlock(w, input16) {
    var s = new Uint8Array(16);
    s.set(input16);
    addRoundKey(s, w, 0);

    for (var round = 1; round <= 14; round++) {
      // SubBytes
      for (var i = 0; i < 16; i++) {
        s[i] = SBOX[s[i]];
      }
      // ShiftRows
      var t1 = s[1];
      s[1] = s[5];
      s[5] = s[9];
      s[9] = s[13];
      s[13] = t1;

      var t2 = s[2];
      s[2] = s[10];
      s[10] = t2;
      t2 = s[6];
      s[6] = s[14];
      s[14] = t2;

      var t3 = s[15];
      s[15] = s[11];
      s[11] = s[7];
      s[7] = s[3];
      s[3] = t3;

      // MixColumns (rounds 1..13)
      if (round < 14) {
        for (var c = 0; c < 4; c++) {
          var idx = c * 4;
          var a0 = s[idx];
          var a1 = s[idx + 1];
          var a2 = s[idx + 2];
          var a3 = s[idx + 3];
          var tx = a0 ^ a1 ^ a2 ^ a3;
          s[idx] = a0 ^ tx ^ xtime(a0 ^ a1);
          s[idx + 1] = a1 ^ tx ^ xtime(a1 ^ a2);
          s[idx + 2] = a2 ^ tx ^ xtime(a2 ^ a3);
          s[idx + 3] = a3 ^ tx ^ xtime(a3 ^ a0);
        }
      }

      addRoundKey(s, w, round * 4);
    }

    return s;
  }

  function inc32(counter) {
    var out = new Uint8Array(counter);
    var c =
      (((out[12] << 24) | (out[13] << 16) | (out[14] << 8) | out[15]) + 1) >>>
      0;
    out[12] = (c >>> 24) & 0xff;
    out[13] = (c >>> 16) & 0xff;
    out[14] = (c >>> 8) & 0xff;
    out[15] = c & 0xff;
    return out;
  }

  function gctr(w, icb, data) {
    var out = new Uint8Array(data.length);
    var cb = new Uint8Array(icb);
    for (var offset = 0; offset < data.length; offset += 16) {
      var ks = aes256EncryptBlock(w, cb);
      var n = Math.min(16, data.length - offset);
      for (var i = 0; i < n; i++) {
        out[offset + i] = data[offset + i] ^ ks[i];
      }
      cb = inc32(cb);
    }
    return out;
  }

  function gfMul(X, Y) {
    var Z = new Uint8Array(16);
    var V = new Uint8Array(Y);
    for (var i = 0; i < 128; i++) {
      if ((X[i >>> 3] & (0x80 >>> (i & 7))) !== 0) {
        for (var k = 0; k < 16; k++) {
          Z[k] ^= V[k];
        }
      }
      var lsb = V[15] & 1;
      for (var m = 15; m > 0; m--) {
        V[m] = (V[m] >>> 1) | ((V[m - 1] & 1) << 7);
      }
      V[0] = V[0] >>> 1;
      if (lsb !== 0) {
        V[0] ^= 0xe1;
      }
    }
    return Z;
  }

  function ghashFeed(Y, H, bytes) {
    for (var offset = 0; offset < bytes.length; offset += 16) {
      var n = Math.min(16, bytes.length - offset);
      for (var i = 0; i < n; i++) {
        Y[i] ^= bytes[offset + i];
      }
      Y = gfMul(Y, H);
    }
    return Y;
  }

  function writeUint64BE(buf, offset, val) {
    var hi = Math.floor(val / 4294967296) >>> 0;
    var lo = val >>> 0;
    buf[offset] = (hi >>> 24) & 0xff;
    buf[offset + 1] = (hi >>> 16) & 0xff;
    buf[offset + 2] = (hi >>> 8) & 0xff;
    buf[offset + 3] = hi & 0xff;
    buf[offset + 4] = (lo >>> 24) & 0xff;
    buf[offset + 5] = (lo >>> 16) & 0xff;
    buf[offset + 6] = (lo >>> 8) & 0xff;
    buf[offset + 7] = lo & 0xff;
  }

  function ghash(H, aadBytes, ctBytes) {
    var Y = new Uint8Array(16);
    Y = ghashFeed(Y, H, aadBytes);
    Y = ghashFeed(Y, H, ctBytes);
    var lenBlk = new Uint8Array(16);
    writeUint64BE(lenBlk, 0, aadBytes.length * 8);
    writeUint64BE(lenBlk, 8, ctBytes.length * 8);
    for (var i = 0; i < 16; i++) {
      Y[i] ^= lenBlk[i];
    }
    return gfMul(Y, H);
  }

  function isValidPsk(psk) {
    var raw = b64uDecode(psk);
    return raw !== null && raw.length === 32;
  }

  function encrypt(plaintext, psk, aad) {
    var key32 = b64uDecode(psk);
    if (!key32 || key32.length !== 32) {
      throw new Error("Invalid PSK");
    }
    var iv = new Uint8Array(12);
    crypto.getRandomValues(iv);

    var w = expandKey256(key32);
    var H = aes256EncryptBlock(w, new Uint8Array(16));
    var J0 = new Uint8Array(16);
    J0.set(iv, 0);
    J0[15] = 1;

    var S0 = aes256EncryptBlock(w, J0);
    var ptBytes = textEncoder.encode(plaintext);
    var ctBytes = gctr(w, inc32(J0), ptBytes);
    var aadBytes = aad ? textEncoder.encode(aad) : new Uint8Array(0);
    var tag = ghash(H, aadBytes, ctBytes);
    for (var i = 0; i < 16; i++) {
      tag[i] ^= S0[i];
    }

    var packed = new Uint8Array(12 + 16 + ctBytes.length);
    packed.set(iv, 0);
    packed.set(tag, 12);
    packed.set(ctBytes, 28);
    return b64uEncode(packed);
  }

  function decrypt(encoded, psk, aad) {
    var key32 = b64uDecode(psk);
    if (!key32 || key32.length !== 32) {
      return null;
    }
    var raw = b64uDecode(encoded);
    if (!raw || raw.length < 28) {
      return null;
    }
    var iv = raw.subarray(0, 12);
    var tag = raw.subarray(12, 28);
    var ctBytes = raw.subarray(28);

    var w = expandKey256(key32);
    var H = aes256EncryptBlock(w, new Uint8Array(16));
    var J0 = new Uint8Array(16);
    J0.set(iv, 0);
    J0[15] = 1;

    var S0 = aes256EncryptBlock(w, J0);
    var aadBytes = aad ? textEncoder.encode(aad) : new Uint8Array(0);
    var expectedTag = ghash(H, aadBytes, ctBytes);
    var diff = 0;
    for (var i = 0; i < 16; i++) {
      diff |= (expectedTag[i] ^ S0[i]) ^ tag[i];
    }
    if (diff !== 0) {
      return null;
    }

    var ptBytes = gctr(w, inc32(J0), ctBytes);
    try {
      return textDecoder.decode(ptBytes);
    } catch (_) {
      return null;
    }
  }

  globalScope.__dhCipher = {
    isValidPsk: isValidPsk,
    encrypt: encrypt,
    decrypt: decrypt
  };
})(typeof window !== "undefined" ? window : globalThis);
