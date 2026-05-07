package ratelimit

import (
	"sync"

	"golang.org/x/time/rate"
)

type Store struct {
	mu       sync.Mutex
	limiters map[string]*rate.Limiter
	limit    rate.Limit
	burst    int
}

func NewStore(limit rate.Limit, burst int) *Store {
	return &Store{
		limiters: make(map[string]*rate.Limiter),
		limit:    limit,
		burst:    burst,
	}
}

func (s *Store) Get(userID string) *rate.Limiter {
	s.mu.Lock()
	defer s.mu.Unlock()
	if l, ok := s.limiters[userID]; ok {
		return l
	}
	l := rate.NewLimiter(s.limit, s.burst)
	s.limiters[userID] = l
	return l
}
