namespace lib {
class Str {};
}

namespace shop {
class Order {
public:
  Order(int);
  void label(lib::Str);
  void ship(int, bool);
  int total() const;
  static int count;
};
}
