package email

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"sync"
	"time"
)

type StoredCredential struct {
	Email     string    `json:"email"`
	Password  string    `json:"password"`
	APIKey    string    `json:"apiKey"`
	CreatedAt time.Time `json:"createdAt"`
	Source    string    `json:"source"`
}

type CredStore struct {
	mu       sync.Mutex
	filePath string
	key      []byte
	creds    []StoredCredential
}

var globalCredStore *CredStore
var credStoreOnce sync.Once

func getCredStore() *CredStore {
	credStoreOnce.Do(func() {
		globalCredStore = &CredStore{
			filePath: "data/credentials.enc",
			creds:    make([]StoredCredential, 0),
		}
		globalCredStore.initKey()
		globalCredStore.load()
	})
	return globalCredStore
}

func (cs *CredStore) initKey() {
	keyHex := os.Getenv("CRED_STORE_KEY")
	if keyHex == "" {
		hostname, _ := os.Hostname()
		h := []byte(hostname + "personal-pilot-cred-v1")
		key := make([]byte, 32)
		for i := range key {
			key[i] = h[i%len(h)] ^ byte(i*13+37)
		}
		cs.key = key
		return
	}
	key, _ := hex.DecodeString(keyHex)
	if len(key) == 32 {
		cs.key = key
	} else {
		hostname, _ := os.Hostname()
		h := []byte(hostname + "personal-pilot-cred-v1")
		cs.key = make([]byte, 32)
		for i := range cs.key {
			cs.key[i] = h[i%len(h)] ^ byte(i*13+37)
		}
	}
}

func (cs *CredStore) load() {
	data, err := os.ReadFile(cs.filePath)
	if err != nil {
		return
	}
	decrypted, err := cs.decrypt(data)
	if err != nil {
		return
	}
	json.Unmarshal(decrypted, &cs.creds)
}

func (cs *CredStore) save() error {
	cs.mu.Lock()
	defer cs.mu.Unlock()

	raw, _ := json.Marshal(cs.creds)
	encrypted, err := cs.encrypt(raw)
	if err != nil {
		return err
	}
	return os.WriteFile(cs.filePath, encrypted, 0600)
}

func (cs *CredStore) encrypt(plaintext []byte) ([]byte, error) {
	block, _ := aes.NewCipher(cs.key)
	aesGCM, _ := cipher.NewGCM(block)
	nonce := make([]byte, aesGCM.NonceSize())
	io.ReadFull(rand.Reader, nonce)
	return aesGCM.Seal(nonce, nonce, plaintext, nil), nil
}

func (cs *CredStore) decrypt(ciphertext []byte) ([]byte, error) {
	block, _ := aes.NewCipher(cs.key)
	aesGCM, _ := cipher.NewGCM(block)
	nonceSize := aesGCM.NonceSize()
	if len(ciphertext) < nonceSize {
		return nil, fmt.Errorf("too short")
	}
	return aesGCM.Open(nil, ciphertext[:nonceSize], ciphertext[nonceSize:], nil)
}

func SaveCredential(emailAddr, password, apiKey, source string) error {
	cs := getCredStore()
	cs.mu.Lock()
	cs.creds = append(cs.creds, StoredCredential{
		Email:     emailAddr,
		Password:  password,
		APIKey:    apiKey,
		CreatedAt: time.Now(),
		Source:    source,
	})
	cs.mu.Unlock()
	return cs.save()
}

func ListCredentials() []StoredCredential {
	cs := getCredStore()
	cs.mu.Lock()
	defer cs.mu.Unlock()
	out := make([]StoredCredential, len(cs.creds))
	copy(out, cs.creds)
	return out
}
