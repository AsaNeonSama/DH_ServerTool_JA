export interface AesGcmSessionCipher {
  readonly encrypt: (plaintext: string) => string;
  readonly decrypt: (encoded: string) => string | null;
  readonly verify: (encoded: string, expectedPlaintext: string) => boolean;
}
