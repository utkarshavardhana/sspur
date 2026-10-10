package client

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"
)

type User struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

var httpClient = &http.Client{Timeout: 2 * time.Second}

func FetchUser(ctx context.Context, url string, attempts int) (User, error) {
	for n := 0; n < attempts; n++ {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
		if err != nil {
			return User{}, err
		}
		resp, err := httpClient.Do(req)
		if err == nil {
			if resp.StatusCode < 500 {
				defer resp.Body.Close()
				if resp.StatusCode >= 400 {
					return User{}, fmt.Errorf("http %d", resp.StatusCode)
				}
				var u User
				if err := json.NewDecoder(resp.Body).Decode(&u); err != nil {
					return User{}, err
				}
				return u, nil
			}
			resp.Body.Close()
		}
		select {
		case <-ctx.Done():
			return User{}, ctx.Err()
		case <-time.After(100 * time.Millisecond << n):
		}
	}
	return User{}, fmt.Errorf("gave up after %d attempts", attempts)
}
