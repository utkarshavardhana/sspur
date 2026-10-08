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
		return `"` + v + `"`
	case []J:
		parts := []string{}
		for _, x := range v {
			parts = append(parts, render(x))
		}
		return "[" + strings.Join(parts, ",") + "]"
	case Obj:
		parts := []string{}
		for _, kv := range v {
			parts = append(parts, `"`+kv.key+`":`+render(kv.val))
		}
		return "{" + strings.Join(parts, ",") + "}"
	}
	panic(fmt.Sprintf("not a JSON value: %#v", j))
}

func getField(j J, key string) (J, bool) {
	if o, ok := j.(Obj); ok {
		return o.get(key)
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
	t, ok1 := target.(Obj)
	p, ok2 := patch.(Obj)
	if ok1 && ok2 {
		out := Obj{}
		for _, kv := range t {
			if _, in := p.get(kv.key); !in {
				out = append(out, kv)
			}
		}
		return append(out, p...)
	}
	return patch
}

func defaults() J {
	return Obj{{"name", "app"}, {"port", 8080}, {"db", Obj{{"host", "localhost"}, {"pool", 5}}}, {"tags", []J{"a"}}}
}

func main() {
	fmt.Println(render(merge(defaults(), Obj{{"port", 9090}})))
}
