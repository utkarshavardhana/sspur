package main

import "testing"

func TestRenders(t *testing.T) {
	if got := render(Obj{{"a", []J{1, true, nil}}}); got != `{"a":[1,true,null]}` {
		t.Fatalf("got %s", got)
	}
}

func TestPath(t *testing.T) {
	if v, ok := getPath(defaults(), "db.pool"); !ok || v != 5 {
		t.Fatalf("db.pool = %v, %v", v, ok)
	}
	if v, ok := getPath(defaults(), "db.user"); ok {
		t.Fatalf("db.user = %v", v)
	}
}

func TestMergesTopLevel(t *testing.T) {
	if v, ok := getPath(merge(defaults(), Obj{{"name", "web"}}), "name"); !ok || v != "web" {
		t.Fatalf("name = %v, %v", v, ok)
	}
}
