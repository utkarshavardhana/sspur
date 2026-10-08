package main

import (
	"fmt"
	"strconv"
	"strings"
)

type Entry struct {
	key   string
	value string
}

// CfgErr is implemented by every config error.
type CfgErr interface {
	error
	cfgErr()
}

type Missing struct{ key string }

func (e *Missing) Error() string { return "missing key " + e.key }
func (*Missing) cfgErr()         {}

type NotInt struct{ key, value string }

func (e *NotInt) Error() string { return "not an int: " + e.key + "=" + e.value }
func (*NotInt) cfgErr()         {}

type NotBool struct{ key, value string }

func (e *NotBool) Error() string { return "not a bool: " + e.key + "=" + e.value }
func (*NotBool) cfgErr()         {}

func parseLine(line string) (Entry, bool) {
	line = strings.TrimSpace(line)
	if strings.HasPrefix(line, "#") {
		return Entry{}, false
	}
	k, v, ok := strings.Cut(line, "=")
	if !ok {
		return Entry{}, false
	}
	k = strings.TrimSpace(k)
	if k == "" {
		return Entry{}, false
	}
	return Entry{k, strings.TrimSpace(v)}, true
}

func parse(text string) []Entry {
	out := []Entry{}
	for _, l := range strings.Split(text, ";") {
		if e, ok := parseLine(l); ok {
			out = append(out, e)
		}
	}
	return out
}

func lookup(cfg []Entry, key string) (string, bool) {
	val, found := "", false
	for _, e := range cfg {
		if e.key == key {
			val, found = e.value, true
		}
	}
	return val, found
}

func getInt(cfg []Entry, key string) (int, error) {
	v, ok := lookup(cfg, key)
	if !ok {
		return 0, &Missing{key}
	}
	d := strings.TrimPrefix(v, "-")
	if d == "" {
		return 0, &NotInt{key, v}
	}
	for _, c := range d {
		if c < '0' || c > '9' {
			return 0, &NotInt{key, v}
		}
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return 0, &NotInt{key, v}
	}
	return n, nil
}

func getBool(cfg []Entry, key string, def bool) (bool, error) {
	v, ok := lookup(cfg, key)
	if !ok {
		return def, nil
	}
	switch strings.ToLower(v) {
	case "true", "yes", "1":
		return true, nil
	case "false", "no", "0":
		return false, nil
	}
	return false, &NotBool{key, v}
}

func getStr(cfg []Entry, key string) (string, error) {
	v, ok := lookup(cfg, key)
	if !ok {
		return "", &Missing{key}
	}
	return v, nil
}

func keys(cfg []Entry) []string {
	seen := map[string]bool{}
	out := []string{}
	for _, e := range cfg {
		if !seen[e.key] {
			seen[e.key] = true
			out = append(out, e.key)
		}
	}
	return out
}

func sampleText() string {
	return "host=localhost;port=8080;debug=true"
}

func main() {
	fmt.Println(strings.Join(keys(parse(sampleText())), ","))
}
