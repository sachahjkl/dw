package contract

// Pair is one entry in an OrderedMap.
type Pair[K comparable, V any] struct {
	Key   K
	Value V
}

// OrderedMap provides keyed lookup without exposing Go map iteration order.
// Updating an existing key preserves its original position.
type OrderedMap[K comparable, V any] struct {
	entries []Pair[K, V]
	index   map[K]int
}

// NewOrderedMap constructs an empty ordered map.
func NewOrderedMap[K comparable, V any](capacity int) *OrderedMap[K, V] {
	if capacity < 0 {
		capacity = 0
	}
	return &OrderedMap[K, V]{
		entries: make([]Pair[K, V], 0, capacity),
		index:   make(map[K]int, capacity),
	}
}

// Len returns the number of unique keys.
func (m *OrderedMap[K, V]) Len() int { return len(m.entries) }

// Get returns the current value for key.
func (m *OrderedMap[K, V]) Get(key K) (V, bool) {
	position, ok := m.index[key]
	if !ok {
		var zero V
		return zero, false
	}
	return m.entries[position].Value, true
}

// Set updates key in place or appends a new key.
func (m *OrderedMap[K, V]) Set(key K, value V) (replaced bool) {
	if m.index == nil {
		m.index = make(map[K]int)
	}
	if position, ok := m.index[key]; ok {
		m.entries[position].Value = value
		return true
	}
	m.index[key] = len(m.entries)
	m.entries = append(m.entries, Pair[K, V]{Key: key, Value: value})
	return false
}

// Delete removes key while retaining the relative order of every other key.
func (m *OrderedMap[K, V]) Delete(key K) bool {
	position, ok := m.index[key]
	if !ok {
		return false
	}
	copy(m.entries[position:], m.entries[position+1:])
	var zero Pair[K, V]
	m.entries[len(m.entries)-1] = zero
	m.entries = m.entries[:len(m.entries)-1]
	delete(m.index, key)
	for i := position; i < len(m.entries); i++ {
		m.index[m.entries[i].Key] = i
	}
	return true
}

// Entries returns a copy in insertion order.
func (m *OrderedMap[K, V]) Entries() []Pair[K, V] {
	return append([]Pair[K, V](nil), m.entries...)
}

// Keys returns keys in insertion order.
func (m *OrderedMap[K, V]) Keys() []K {
	keys := make([]K, len(m.entries))
	for i := range m.entries {
		keys[i] = m.entries[i].Key
	}
	return keys
}

// Values returns values in insertion order.
func (m *OrderedMap[K, V]) Values() []V {
	values := make([]V, len(m.entries))
	for i := range m.entries {
		values[i] = m.entries[i].Value
	}
	return values
}

// Clone creates an independent collection with the same order.
func (m *OrderedMap[K, V]) Clone() *OrderedMap[K, V] {
	clone := NewOrderedMap[K, V](len(m.entries))
	for _, entry := range m.entries {
		clone.Set(entry.Key, entry.Value)
	}
	return clone
}
