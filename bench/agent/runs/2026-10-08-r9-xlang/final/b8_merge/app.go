package main

import (
	"fmt"
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

func quote(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, `"`, `\"`)
	return `"` + s + `"`
}

func getField(j J, key string) (J, bool) {
	switch v := j.(type) {
	case Obj:
		return v.get(key)
	case []J:
		if key == "" {
			return nil, false
		}
		for _, c := range key {
			if c < '0' || c > '9' {
				return nil, false
			}
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
		idx := -1
		for i, o := range out {
			if o.key == kv.key {
				idx = i
				break
			}
		}
		if kv.val == nil {
			if idx >= 0 {
				out = append(out[:idx], out[idx+1:]...)
			}
			continue
		}
		if idx >= 0 {
			out[idx].val = merge(out[idx].val, kv.val)
		} else {
			out = append(out, KV{kv.key, merge(nil, kv.val)})
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

// ConfigErr is a configuration error.
type ConfigErr interface {
	error
	configErr()
}

// MissingKey reports a required path that is absent.
type MissingKey struct{ path string }

func (e *MissingKey) Error() string { return "missing key: " + e.path }
func (e *MissingKey) configErr()    {}

func require(j J, paths []string) error {
	for _, p := range paths {
		if _, ok := getPath(j, p); !ok {
			return &MissingKey{p}
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
