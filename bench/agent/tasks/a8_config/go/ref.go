package main

import (
	"fmt"
	"regexp"
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

func (e *NotInt) Error() string { return e.key + ": not an int: " + e.value }
func (*NotInt) cfgErr()         {}

type NotBool struct{ key, value string }

func (e *NotBool) Error() string { return e.key + ": not a bool: " + e.value }
func (*NotBool) cfgErr()         {}

func parseLine(line string) (Entry, bool) {
	t := strings.TrimSpace(line)
	key, value, eq := strings.Cut(t, "=")
	if strings.HasPrefix(t, "#") || !eq || strings.TrimSpace(key) == "" {
		return Entry{}, false
	}
	return Entry{strings.TrimSpace(key), strings.TrimSpace(value)}, true
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
	for i := len(cfg) - 1; i >= 0; i-- {
		if cfg[i].key == key {
			return cfg[i].value, true
		}
	}
	return "", false
}

func getStr(cfg []Entry, key string) (string, error) {
	v, ok := lookup(cfg, key)
	if !ok {
		return "", &Missing{key}
	}
	return v, nil
}

var intRe = regexp.MustCompile(`^-?[0-9]+$`)

func getInt(cfg []Entry, key string) (int, error) {
	v, err := getStr(cfg, key)
	if err != nil {
		return 0, err
	}
	if !intRe.MatchString(v) {
		return 0, &NotInt{key, v}
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
