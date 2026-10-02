package ring

type Buffer[T any] struct {
	buf  []T
	head int
	n    int
}

func New[T any](capacity int) *Buffer[T] {
	return &Buffer[T]{buf: make([]T, capacity)}
}

func (b *Buffer[T]) Len() int { return b.n }

func (b *Buffer[T]) Push(x T) bool {
	if b.n == len(b.buf) {
		return false
	}
	b.buf[(b.head+b.n)%len(b.buf)] = x
	b.n++
	return true
}

func (b *Buffer[T]) Pop() (T, bool) {
	var zero T
	if b.n == 0 {
		return zero, false
	}
	x := b.buf[b.head]
	b.buf[b.head] = zero
	b.head = (b.head + 1) % len(b.buf)
	b.n--
	return x, true
}
