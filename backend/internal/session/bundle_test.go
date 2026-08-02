package session

import "testing"

func TestBundleMigrateAndEncrypt(t *testing.T) {
	bundle := Bundle{ProfileID: "p1", Runtime: RuntimeChromium, Params: map[string]string{"ua": "x"}}
	raw, err := Serialize(bundle)
	if err != nil {
		t.Fatal(err)
	}
	loaded, err := Deserialize(raw)
	if err != nil {
		t.Fatal(err)
	}
	encrypted, err := Encrypt(loaded, "secret")
	if err != nil {
		t.Fatal(err)
	}
	decrypted, err := Decrypt(encrypted, "secret")
	if err != nil {
		t.Fatal(err)
	}
	if decrypted.ProfileID != "p1" {
		t.Fatalf("decrypted = %+v", decrypted)
	}
	migrated := Migrate(loaded, Mapping{From: RuntimeChromium, To: RuntimeCamoufox, Keys: map[string]string{"ua": "userAgent"}})
	if DefaultRuntimeMapping(RuntimeChromium, RuntimeCamoufox).Keys["userAgent"] == "" {
		t.Fatal("default mapping missing")
	}
	if migrated.Runtime != RuntimeCamoufox || migrated.Params["userAgent"] != "x" {
		t.Fatalf("migrated = %+v", migrated)
	}
}
