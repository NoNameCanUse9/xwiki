package id

import "testing"

func TestNew(t *testing.T) {
	got := New("usr")
	// ULID 为 26 字符，前缀加下划线共 30 字符
	if len(got) != 30 {
		t.Fatalf("unexpected id %q (len %d)", got, len(got))
	}
	if got[:4] != "usr_" {
		t.Fatalf("prefix missing: %q", got)
	}
}

func TestNewUnique(t *testing.T) {
	a, b := New("prj"), New("prj")
	if a == b {
		t.Fatalf("ids must differ: %q", a)
	}
}
