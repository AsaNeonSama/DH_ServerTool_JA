import type { CipherGCM, DecipherGCM } from "crypto";
import {
  createCipheriv,
  createDecipheriv,
  randomBytes,
  timingSafeEqual,
} from "crypto";
import {
  AES_AUTH_TAG_BYTES,
  AES_GCM_ALGORITHM,
  AES_IV_BYTES,
  AES_KEY_BYTES,
} from "./constants";
import type { AesGcmSessionCipher } from "./types";

export function timingSafeStringEquals(a: string, b: string): boolean {
  const bufA: Buffer = Buffer.from(a, "utf8");
  const bufB: Buffer = Buffer.from(b, "utf8");
  if (bufA.byteLength !== bufB.byteLength) {
    return false;
  }
  return timingSafeEqual(bufA, bufB);
}

export function createAesGcmCipher(rawKey?: Buffer): AesGcmSessionCipher {
  const key: Buffer =
    rawKey?.byteLength === AES_KEY_BYTES
      ? Buffer.from(rawKey)
      : randomBytes(AES_KEY_BYTES);
  const psk: string = key.toString("base64url");

  const encrypt = (plaintext: string, aad?: string): string => {
    const iv: Buffer = randomBytes(AES_IV_BYTES);
    const cipher: CipherGCM = createCipheriv(AES_GCM_ALGORITHM, key, iv);
    if (aad) {
      cipher.setAAD(Buffer.from(aad, "utf8"));
    }
    const encrypted: Buffer = Buffer.concat([
      cipher.update(plaintext, "utf8"),
      cipher.final(),
    ]);
    const authTag: Buffer = cipher.getAuthTag();
    return Buffer.concat([iv, authTag, encrypted]).toString("base64url");
  };

  const decrypt = (encoded: string, aad?: string): string | null => {
    try {
      if (encoded.length === 0) {
        return null;
      }
      const raw: Buffer = Buffer.from(encoded, "base64url");
      if (raw.toString("base64url") !== encoded) {
        return null;
      }
      const minLength: number = AES_IV_BYTES + AES_AUTH_TAG_BYTES;
      if (raw.byteLength < minLength) {
        return null;
      }
      const iv: Buffer = raw.subarray(0, AES_IV_BYTES);
      const authTag: Buffer = raw.subarray(
        AES_IV_BYTES,
        AES_IV_BYTES + AES_AUTH_TAG_BYTES
      );
      const ciphertext: Buffer = raw.subarray(AES_IV_BYTES + AES_AUTH_TAG_BYTES);

      const decipher: DecipherGCM = createDecipheriv(AES_GCM_ALGORITHM, key, iv);
      if (aad) {
        decipher.setAAD(Buffer.from(aad, "utf8"));
      }
      decipher.setAuthTag(authTag);
      const decrypted: Buffer = Buffer.concat([
        decipher.update(ciphertext),
        decipher.final(),
      ]);
      return decrypted.toString("utf8");
    } catch {
      return null;
    }
  };

  const verify = (
    encoded: string,
    expectedPlaintext: string,
    aad?: string
  ): boolean => {
    const decrypted: string | null = decrypt(encoded, aad);
    if (decrypted === null) {
      return false;
    }
    return timingSafeStringEquals(decrypted, expectedPlaintext);
  };

  return { psk, encrypt, decrypt, verify };
}
