package main

import (
	"errors"
	"reflect"
	"testing"
)

func hCfg() []Entry {
	return parse(" host = example.com ; port=8080;# port=9;url=http://x?a=b;empty=;=nokey;junk;port = 9090;debug=Yes;n=-5;bad=12a;off=NO")
}

func hLookup(t *testing.T, key, want string) {
	t.Helper()
	if v, ok := lookup(hCfg(), key); !ok || v != want {
		t.Fatalf("lookup(%q) = %q, %v; want %q", key, v, ok, want)
	}
}

func hInt(t *testing.T, key string, want int) {
	t.Helper()
	if n, err := getInt(hCfg(), key); err != nil || n != want {
		t.Fatalf("getInt(%q) = %d, %v; want %d", key, n, err, want)
	}
}

func hBool(t *testing.T, key string, def, want bool) {
	t.Helper()
	if b, err := getBool(hCfg(), key, def); err != nil || b != want {
		t.Fatalf("getBool(%q, %v) = %v, %v; want %v", key, def, b, err, want)
	}
}

func TestHidden_len(t *testing.T) {
	if got := len(hCfg()); got != 9 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_keys(t *testing.T) {
	want := []string{"host", "port", "url", "empty", "debug", "n", "bad", "off"}
	if got := keys(hCfg()); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_trim(t *testing.T) {
	hLookup(t, "host", "example.com")
}

func TestHidden_first_eq(t *testing.T) {
	hLookup(t, "url", "http://x?a=b")
}

func TestHidden_empty_value(t *testing.T) {
	hLookup(t, "empty", "")
}

func TestHidden_last_wins(t *testing.T) {
	hInt(t, "port", 9090)
}

func TestHidden_neg(t *testing.T) {
	hInt(t, "n", -5)
}

func TestHidden_not_int(t *testing.T) {
	_, err := getInt(hCfg(), "bad")
	var e *NotInt
	if !errors.As(err, &e) || e.key != "bad" || e.value != "12a" {
		t.Fatalf("err = %v", err)
	}
	var ce CfgErr
	if !errors.As(err, &ce) {
		t.Fatalf("not a CfgErr: %v", err)
	}
}

func TestHidden_int_missing(t *testing.T) {
	_, err := getInt(hCfg(), "zz")
	var e *Missing
	if !errors.As(err, &e) || e.key != "zz" {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_bool(t *testing.T) {
	hBool(t, "debug", false, true)
	hBool(t, "off", true, false)
}

func TestHidden_bool_default(t *testing.T) {
	hBool(t, "zz", true, true)
	hBool(t, "zz", false, false)
}

func TestHidden_not_bool(t *testing.T) {
	_, err := getBool(hCfg(), "host", false)
	var e *NotBool
	if !errors.As(err, &e) || e.key != "host" || e.value != "example.com" {
		t.Fatalf("err = %v", err)
	}
	var ce CfgErr
	if !errors.As(err, &ce) {
		t.Fatalf("not a CfgErr: %v", err)
	}
}

func TestHidden_blank(t *testing.T) {
	got := []string{}
	for _, e := range parse("a=1;;b=2; ;#c=3") {
		got = append(got, e.key)
	}
	if !reflect.DeepEqual(got, []string{"a", "b"}) {
		t.Fatalf("got %v", got)
	}
}
