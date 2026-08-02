package session

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
)

type RuntimeKind string

const (
	RuntimeChromium   RuntimeKind = "chromium"
	RuntimeCamoufox   RuntimeKind = "camoufox"
	RuntimeLightpanda RuntimeKind = "lightpanda"
)

type Bundle struct {
	ProfileID string
	Runtime   RuntimeKind
	Cookies   json.RawMessage
	Storage   json.RawMessage
	Params    map[string]string
}

type Mapping struct {
	From RuntimeKind
	To   RuntimeKind
	Keys map[string]string
}

type EncryptedBundle struct {
	Version string `json:"version"`
	Nonce   []byte `json:"nonce"`
	Data    []byte `json:"data"`
}

func Serialize(bundle Bundle) ([]byte, error) { return json.Marshal(bundle) }

func Deserialize(raw []byte) (Bundle, error) {
	var bundle Bundle
	err := json.Unmarshal(raw, &bundle)
	return bundle, err
}

func Encrypt(bundle Bundle, passphrase string) ([]byte, error) {
	plain, err := Serialize(bundle)
	if err != nil {
		return nil, err
	}
	block, err := aes.NewCipher(keyFromPassphrase(passphrase))
	if err != nil {
		return nil, err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}
	nonce := make([]byte, gcm.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return nil, err
	}
	payload := EncryptedBundle{Version: "aes-gcm-sha256-v1", Nonce: nonce, Data: gcm.Seal(nil, nonce, plain, nil)}
	return json.Marshal(payload)
}

func Decrypt(raw []byte, passphrase string) (Bundle, error) {
	var payload EncryptedBundle
	if err := json.Unmarshal(raw, &payload); err != nil {
		return Bundle{}, err
	}
	if payload.Version != "aes-gcm-sha256-v1" {
		return Bundle{}, fmt.Errorf("unsupported encrypted bundle version: %s", payload.Version)
	}
	block, err := aes.NewCipher(keyFromPassphrase(passphrase))
	if err != nil {
		return Bundle{}, err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return Bundle{}, err
	}
	plain, err := gcm.Open(nil, payload.Nonce, payload.Data, nil)
	if err != nil {
		return Bundle{}, err
	}
	return Deserialize(plain)
}

func Migrate(bundle Bundle, mapping Mapping) Bundle {
	out := bundle
	out.Runtime = mapping.To
	out.Params = map[string]string{}
	for key, value := range bundle.Params {
		if target, ok := mapping.Keys[key]; ok {
			out.Params[target] = value
		} else {
			out.Params[key] = value
		}
	}
	return out
}

func DefaultRuntimeMapping(from, to RuntimeKind) Mapping {
	keys := map[string]string{"userAgent": "userAgent", "timezone": "timezone", "locale": "locale", "webglVendor": "webglVendor", "webglRenderer": "webglRenderer"}
	if from == RuntimeChromium && to == RuntimeCamoufox {
		keys["userAgent"] = "general.useragent.override"
	}
	return Mapping{From: from, To: to, Keys: keys}
}

func keyFromPassphrase(passphrase string) []byte {
	sum := sha256.Sum256([]byte(passphrase))
	return sum[:]
}
