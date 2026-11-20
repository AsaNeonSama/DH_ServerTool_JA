import type { CipherGCMTypes } from "crypto";

export const AES_GCM_ALGORITHM: CipherGCMTypes = "aes-256-gcm";
export const AES_KEY_BYTES: number = 32;
export const AES_IV_BYTES: number = 12;
export const AES_AUTH_TAG_BYTES: number = 16;
export const AUTH_TOKEN_SEPARATOR: string = ".";
