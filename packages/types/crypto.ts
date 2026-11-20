export interface AesGcmSessionCipher {
  readonly psk: string;
  readonly encrypt: (plaintext: string, aad?: string) => string;
  readonly decrypt: (encoded: string, aad?: string) => string | null;
  readonly verify: (
    encoded: string,
    expectedPlaintext: string,
    aad?: string
  ) => boolean;
}
