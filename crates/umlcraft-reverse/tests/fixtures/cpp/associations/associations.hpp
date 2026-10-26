#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <vector>

namespace shop {
class Line {};
class Customer {};

class Order {
  Customer customer;
  Customer* payer;
  const Customer* reader;
  Customer& owner;
  std::vector<Line> lines;
  std::vector<Line*> pointers;
  std::set<Line> sorted;
  std::unordered_set<Line> tags;
  std::map<std::string, Line> byName;
  std::unique_ptr<Customer> account;
  std::shared_ptr<Customer> shared;
  std::optional<Customer> maybe;
  Line extra[3];
  std::string note;
  int count;
};
}
