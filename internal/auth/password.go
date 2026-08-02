package auth

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"fmt"

	"golang.org/x/crypto/argon2"
)

const (
	argonTime    = 3
	argonMemory  = 64 * 1024
	argonThreads = 1
	argonKeyLen  = 32
)

// HashPassword returns an encoded Argon2id hash (PHC string format).
func HashPassword(password string) (string, error) {
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return "", err
	}
	key := argon2.IDKey([]byte(password), salt, argonTime, argonMemory, argonThreads, argonKeyLen)
	return fmt.Sprintf("$argon2id$v=19$m=%d,t=%d,p=%d$%s$%s",
		argonMemory, argonTime, argonThreads,
		base64.RawStdEncoding.EncodeToString(salt),
		base64.RawStdEncoding.EncodeToString(key)), nil
}

// VerifyPassword checks a password against an encoded Argon2id hash.
func VerifyPassword(password, encoded string) (bool, error) {
	parts := splitHash(encoded)
	if parts == nil {
		return false, errors.New("invalid password hash format")
	}
	if _, err := parseVersion(parts[2]); err != nil {
		return false, err
	}
	m, t, p, err := parseParams(parts[3])
	if err != nil {
		return false, err
	}
	salt, err := base64.RawStdEncoding.DecodeString(parts[4])
	if err != nil {
		return false, err
	}
	expected, err := base64.RawStdEncoding.DecodeString(parts[5])
	if err != nil {
		return false, err
	}
	actual := argon2.IDKey([]byte(password), salt, t, m, p, uint32(len(expected)))
	return subtle.ConstantTimeCompare(actual, expected) == 1, nil
}

func splitHash(encoded string) []string {
	var parts []string
	start := 0
	for i := 0; i < len(encoded); i++ {
		if encoded[i] == '$' {
			parts = append(parts, encoded[start:i])
			start = i + 1
		}
	}
	parts = append(parts, encoded[start:])
	if len(parts) != 6 || parts[0] != "" || parts[1] != "argon2id" {
		return nil
	}
	return parts
}

func parseVersion(s string) (int, error) {
	var v int
	if _, err := fmt.Sscanf(s, "v=%d", &v); err != nil {
		return 0, err
	}
	if v != 19 {
		return 0, errors.New("unsupported argon2 version")
	}
	return v, nil
}

func parseParams(s string) (uint32, uint32, uint8, error) {
	var m, t, p int
	if _, err := fmt.Sscanf(s, "m=%d,t=%d,p=%d", &m, &t, &p); err != nil {
		return 0, 0, 0, err
	}
	return uint32(m), uint32(t), uint8(p), nil
}
