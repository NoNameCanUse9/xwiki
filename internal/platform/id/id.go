package id

import "github.com/oklog/ulid/v2"

// New returns a prefixed ULID, e.g. New("usr") -> "usr_01KABC...".
func New(prefix string) string {
	return prefix + "_" + ulid.Make().String()
}
