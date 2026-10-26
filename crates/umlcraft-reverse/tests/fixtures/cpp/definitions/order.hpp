namespace shop {
class Order {
public:
  Order(int);
  void ship(int, bool);
  int total() const;
  static int count;
};
}
