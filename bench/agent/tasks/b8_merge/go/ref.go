package main

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

// J is a JSON value: nil, bool, int, string, []J or Obj.
type J = any

// KV is one key of a JSON object.
type KV struct {
	key string
	val J
}

// Obj is a JSON object, its keys in insertion order.
type Obj []KV

func (o Obj) get(key string) (J, bool) {
	for _, kv := range o {
		if kv.key == key {
			return kv.val, true
		}
	}
	return nil, false
}

// ConfigErr is implemented by every config error.
type ConfigErr interface {
	error
	configErr()
}

type MissingKey struct{ path string }

func (e *MissingKey) Error() string { return "missing key " + e.path }
func (*MissingKey) configErr()      {}

func quote(s string) string {
	return `"` + strings.ReplaceAll(strings.ReplaceAll(s, `\`, `\\`), `"`, `\"`) + `"`
}

func render(j J) string {
	switch v := j.(type) {
	case nil:
		return "null"
	case bool:
		if v {
			return "true"
		}
		return "false"
	case int:
		return strconv.Itoa(v)
	case string:
		return quote(v)
	case []J:
		parts := []string{}
		for _, x := range v {
			parts = append(parts, render(x))
		}
		return "[" + strings.Join(parts, ",") + "]"
	case Obj:
		parts := []string{}
		for _, kv := range v {
			parts = append(parts, quote(kv.key)+":"+render(kv.val))
		}
		return "{" + strings.Join(parts, ",") + "}"
	}
	panic(fmt.Sprintf("not a JSON value: %#v", j))
}

var digits = regexp.MustCompile(`^[0-9]+$`)

func getField(j J, key string) (J, bool) {
	switch v := j.(type) {
	case Obj:
		return v.get(key)
	case []J:
		if !digits.MatchString(key) {
			return nil, false
		}
		i, err := strconv.Atoi(key)
		if err != nil || i >= len(v) {
			return nil, false
		}
		return v[i], true
	}
	return nil, false
}

func getPath(j J, path string) (J, bool) {
	cur := j
	for _, k := range strings.Split(path, ".") {
		v, ok := getField(cur, k)
		if !ok {
			return nil, false
		}
		cur = v
	}
	return cur, true
}

func merge(target J, patch J) J {
	p, ok := patch.(Obj)
	if !ok {
		return patch
	}
	out := Obj{}
	if t, ok := target.(Obj); ok {
		out = append(out, t...)
	}
	for _, kv := range p {
		if kv.val == nil {
			kept := Obj{}
			for _, x := range out {
				if x.key != kv.key {
					kept = append(kept, x)
				}
			}
			out = kept
			continue
		}
		cur, _ := out.get(kv.key)
		val := merge(cur, kv.val)
		found := false
		for i := range out {
			if out[i].key == kv.key {
				out[i].val = val
				found = true
			}
		}
		if !found {
			out = append(out, KV{kv.key, val})
		}
	}
	return out
}

func mergeAll(base J, patches []J) J {
	for _, p := range patches {
		base = merge(base, p)
	}
	return base
}

func require(j J, paths []string) error {
	for _, path := range paths {
		if _, ok := getPath(j, path); !ok {
			return &MissingKey{path}
		}
	}
	return nil
}

func defaults() J {
	return Obj{{"name", "app"}, {"port", 8080}, {"db", Obj{{"host", "localhost"}, {"pool", 5}}}, {"tags", []J{"a"}}}
}

func main() {
	fmt.Println(render(merge(defaults(), Obj{{"port", 9090}})))
}
