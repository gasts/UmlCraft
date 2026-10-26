template <typename T, int N = 4, typename... Rest>
class Array {
  T items[N];
public:
  template <class U> U convert(U u) const;
};

template <class K, class V = int>
class Table {
  K key;
  V value;
};

template <template <class> class C, class T>
class Adapter {
  C<T> inner;
};

// Specializations are left out.
template <> class Table<int, int> {};
template <class T> class Array<T*, 1> {};
