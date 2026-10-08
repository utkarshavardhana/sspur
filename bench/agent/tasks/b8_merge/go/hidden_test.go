package main

import (
	"errors"
	"reflect"
	"testing"
)

func hM(target, patch J) string {
	return render(merge(target, patch))
}

func hCfg() J {
	return Obj{{"name", "app"}, {"servers", []J{Obj{{"host", "a"}}, Obj{{"host", "b"}}}}, {"db", Obj{{"pool", 5}}}}
}

func hReq(j J, paths []string) string {
	err := require(j, paths)
	if err == nil {
		return "ok"
	}
	var mk *MissingKey
	var ce ConfigErr
	switch {
	case errors.As(err, &mk):
		return "missing " + mk.path
	case errors.As(err, &ce):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hEq(t *testing.T, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func hPath(t *testing.T, j J, path string, want J) {
	t.Helper()
	got, ok := getPath(j, path)
	if !ok || !reflect.DeepEqual(got, want) {
		t.Fatalf("getPath(%q) = %#v, %v; want %#v", path, got, ok, want)
	}
}

func hNoPath(t *testing.T, j J, path string) {
	t.Helper()
	if got, ok := getPath(j, path); ok {
		t.Fatalf("getPath(%q) = %#v, want missing", path, got)
	}
}

func TestHidden_rfc_replace(t *testing.T) {
	hEq(t, hM(Obj{{"a", "b"}}, Obj{{"a", "c"}}), `{"a":"c"}`)
}

func TestHidden_rfc_add(t *testing.T) {
	hEq(t, hM(Obj{{"a", "b"}}, Obj{{"b", "c"}}), `{"a":"b","b":"c"}`)
}

func TestHidden_rfc_remove(t *testing.T) {
	hEq(t, hM(Obj{{"a", "b"}}, Obj{{"a", nil}}), `{}`)
	hEq(t, hM(Obj{{"a", "b"}, {"b", "c"}}, Obj{{"a", nil}}), `{"b":"c"}`)
}

func TestHidden_rfc_array_replaced(t *testing.T) {
	hEq(t, hM(Obj{{"a", []J{"b"}}}, Obj{{"a", "c"}}), `{"a":"c"}`)
	hEq(t, hM(Obj{{"a", "c"}}, Obj{{"a", []J{"b"}}}), `{"a":["b"]}`)
}

func TestHidden_rfc_nested(t *testing.T) {
	hEq(t, hM(Obj{{"a", Obj{{"b", "c"}}}}, Obj{{"a", Obj{{"b", "d"}, {"c", nil}}}}), `{"a":{"b":"d"}}`)
}

func TestHidden_rfc_array_of_objects(t *testing.T) {
	hEq(t, hM(Obj{{"a", []J{Obj{{"b", "c"}}}}}, Obj{{"a", []J{1}}}), `{"a":[1]}`)
}

func TestHidden_rfc_non_object_patch(t *testing.T) {
	hEq(t, hM([]J{"a", "b"}, []J{"c", "d"}), `["c","d"]`)
	hEq(t, hM(Obj{{"a", "b"}}, []J{"c"}), `["c"]`)
	hEq(t, hM(Obj{{"a", "foo"}}, nil), `null`)
	hEq(t, hM(Obj{{"a", "foo"}}, "bar"), `"bar"`)
}

func TestHidden_rfc_keep_target_null(t *testing.T) {
	hEq(t, hM(Obj{{"e", nil}}, Obj{{"a", 1}}), `{"e":null,"a":1}`)
}

func TestHidden_rfc_non_object_target(t *testing.T) {
	hEq(t, hM([]J{1, 2}, Obj{{"a", "b"}, {"c", nil}}), `{"a":"b"}`)
}

func TestHidden_rfc_deep_new(t *testing.T) {
	hEq(t, hM(Obj{}, Obj{{"a", Obj{{"bb", Obj{{"ccc", nil}}}}}}), `{"a":{"bb":{}}}`)
}

func TestHidden_key_order(t *testing.T) {
	hEq(t, hM(Obj{{"a", 1}, {"b", 2}, {"c", 3}}, Obj{{"b", 9}, {"d", 4}}), `{"a":1,"b":9,"c":3,"d":4}`)
}

func TestHidden_defaults_merge(t *testing.T) {
	hEq(t, hM(defaults(), Obj{{"port", 9090}, {"db", Obj{{"pool", 10}, {"user", "u"}}}}),
		`{"name":"app","port":9090,"db":{"host":"localhost","pool":10,"user":"u"},"tags":["a"]}`)
}

func TestHidden_escape(t *testing.T) {
	hEq(t, render(`say "hi" \ bye`), `"say \"hi\" \\ bye"`)
	hEq(t, render(Obj{{`k"q`, 1}}), `{"k\"q":1}`)
}

func TestHidden_path_index(t *testing.T) {
	hPath(t, hCfg(), "servers.1.host", "b")
	hPath(t, hCfg(), "servers.0", Obj{{"host", "a"}})
}

func TestHidden_path_missing(t *testing.T) {
	hNoPath(t, hCfg(), "servers.5.host")
	hNoPath(t, hCfg(), "servers.x")
	hNoPath(t, hCfg(), "name.first")
	hPath(t, hCfg(), "db.pool", 5)
}

func TestHidden_merge_all(t *testing.T) {
	hEq(t, render(mergeAll(defaults(), []J{})), render(defaults()))
	hEq(t, render(mergeAll(Obj{{"a", 1}}, []J{Obj{{"a", 2}, {"b", 3}}, Obj{{"b", nil}}})), `{"a":2}`)
}

func TestHidden_require(t *testing.T) {
	hEq(t, hReq(hCfg(), []string{"name", "db.pool", "servers.1.host"}), "ok")
	hEq(t, hReq(hCfg(), []string{"name", "db.user", "x"}), "missing db.user")
	hEq(t, hReq(hCfg(), []string{}), "ok")
}
