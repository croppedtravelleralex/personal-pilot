package backup

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"io"
	"os"
)

const encryptKeyEnv = "BACKUP_ENCRYPT_KEY"

// EncryptFile encrypts src path to dst path using AES-256-GCM.
// The key is derived from the BACKUP_ENCRYPT_KEY env var.
// If the env var is empty, the file is copied as-is (no encryption).
func EncryptFile(src, dst string) error {
	keyHex := os.Getenv(encryptKeyEnv)
	if keyHex == "" {
		return copyFile(src, dst)
	}

	key, err := hex.DecodeString(keyHex)
	if err != nil {
		return fmt.Errorf("backup encrypt: invalid BACKUP_ENCRYPT_KEY hex: %w", err)
	}

	if len(key) != 32 {
		return fmt.Errorf("backup encrypt: BACKUP_ENCRYPT_KEY must be 64 hex chars (32 bytes), got %d bytes", len(key))
	}

	plaintext, err := os.ReadFile(src)
	if err != nil {
		return fmt.Errorf("backup encrypt read: %w", err)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return fmt.Errorf("backup encrypt cipher: %w", err)
	}

	aesGCM, err := cipher.NewGCM(block)
	if err != nil {
		return fmt.Errorf("backup encrypt GCM: %w", err)
	}

	nonce := make([]byte, aesGCM.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return fmt.Errorf("backup encrypt nonce: %w", err)
	}

	ciphertext := aesGCM.Seal(nonce, nonce, plaintext, nil)
	if err := os.WriteFile(dst, ciphertext, 0600); err != nil {
		return fmt.Errorf("backup encrypt write: %w", err)
	}

	return nil
}

// DecryptFile decrypts src to dst using AES-256-GCM with BACKUP_ENCRYPT_KEY.
// If BACKUP_ENCRYPT_KEY is empty, copies src to dst.
func DecryptFile(src, dst string) error {
	keyHex := os.Getenv(encryptKeyEnv)
	if keyHex == "" {
		return copyFile(src, dst)
	}

	key, err := hex.DecodeString(keyHex)
	if err != nil {
		return fmt.Errorf("backup decrypt: invalid BACKUP_ENCRYPT_KEY hex: %w", err)
	}

	if len(key) != 32 {
		return fmt.Errorf("backup decrypt: BACKUP_ENCRYPT_KEY must be 64 hex chars (32 bytes), got %d bytes", len(key))
	}

	ciphertext, err := os.ReadFile(src)
	if err != nil {
		return fmt.Errorf("backup decrypt read: %w", err)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return fmt.Errorf("backup decrypt cipher: %w", err)
	}

	aesGCM, err := cipher.NewGCM(block)
	if err != nil {
		return fmt.Errorf("backup decrypt GCM: %w", err)
	}

	nonceSize := aesGCM.NonceSize()
	if len(ciphertext) < nonceSize {
		return fmt.Errorf("backup decrypt: ciphertext too short")
	}

	nonce, ciphertext := ciphertext[:nonceSize], ciphertext[nonceSize:]
	plaintext, err := aesGCM.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return fmt.Errorf("backup decrypt: %w (wrong key or corrupted data)", err)
	}

	if err := os.WriteFile(dst, plaintext, 0600); err != nil {
		return fmt.Errorf("backup decrypt write: %w", err)
	}

	return nil
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, in)
	return err
}
