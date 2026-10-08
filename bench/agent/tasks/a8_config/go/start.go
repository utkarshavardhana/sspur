package main

import (
	"fmt"
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

func parseLine(line string) (Entry, bool) {
	parts := strings.Split(line, "=")
	if len(parts) == 2 {
		return Entry{parts[0], parts[1]}, true
	}
	return Entry{}, false
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
	for _, e := range cfg {
		if e.key == key {
			return e.value, true
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
