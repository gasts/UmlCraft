namespace model {
enum class Kind { A, B, C };

class Order {
public:
  int number;
  int added;
  void ship(int days) const;
  void ship(int days);
};
}
