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
  AUTH_TOKEN_SEPARATOR,
} from "./constants";
import type { AesGcmSessionCipher } from "./types";

function timingSafeStringEquals(a: string, b: string): boolean {
  const bufA: Buffer = Buffer.from(a, "utf8");
  const bufB: Buffer = Buffer.from(b, "utf8");
  if (bufA.byteLength !== bufB.byteLength) {
    return false;
  }
  return timingSafeEqual(bufA, bufB);
}

export function createAesGcmCipher(): AesGcmSessionCipher {
  const key: Buffer = randomBytes(AES_KEY_BYTES);

  const encrypt = (plaintext: string): string => {
    const iv: Buffer = randomBytes(AES_IV_BYTES);
    const cipher: CipherGCM = createCipheriv(AES_GCM_ALGORITHM, key, iv);
    const encrypted: Buffer = Buffer.concat([
      cipher.update(plaintext, "utf8"),
      cipher.final(),
    ]);
    const authTag: Buffer = cipher.getAuthTag();
    return Buffer.concat([iv, authTag, encrypted]).toString("base64url");
  };

  const decrypt = (encoded: string): string | null => {
    try {
      const raw: Buffer = Buffer.from(encoded, "base64url");
      if (raw.toString("base64url") !== encoded) {
        return null;
      }
      const minLength: number = AES_IV_BYTES + AES_AUTH_TAG_BYTES + 1;
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

  const verify = (encoded: string, expectedPlaintext: string): boolean => {
    const decrypted: string | null = decrypt(encoded);
    if (decrypted === null) {
      return false;
    }
    return timingSafeStringEquals(decrypted, expectedPlaintext);
  };

  return { encrypt, decrypt, verify };
}

export function createEncryptedAuthToken(
  baseAuthId: string,
  cipher: AesGcmSessionCipher
): string {
  const encryptedSuffix: string = cipher.encrypt(baseAuthId);
  return `${baseAuthId}${AUTH_TOKEN_SEPARATOR}${encryptedSuffix}`;
}

export function verifyEncryptedAuthToken(
  fullToken: string,
  expectedBaseId: string,
  cipher: AesGcmSessionCipher
): boolean {
  const sepIdx: number = fullToken.indexOf(AUTH_TOKEN_SEPARATOR);
  if (sepIdx === -1) {
    return false;
  }
  const basePart: string = fullToken.slice(0, sepIdx);
  const encryptedPart: string = fullToken.slice(sepIdx + 1);
  if (!timingSafeStringEquals(basePart, expectedBaseId)) {
    return false;
  }
  return cipher.verify(encryptedPart, expectedBaseId);
}
