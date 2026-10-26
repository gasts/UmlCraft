namespace model {
enum class Kind { A, B };

class Order {
public:
  int number;
  int removed;
  void ship(int days);
  void drop();
};
}
