#include <string>
#include "lib.hpp"

namespace fmt = acme::lib;
using acme::lib::Customer;

namespace acme {
namespace app {
using namespace lib;

class Shop {
  // From `using namespace`.
  Money price;
  // From a using declaration.
  Customer buyer;
  // Nested type of a type in scope.
  Customer::Address address;
  // From the namespace around: `acme::Root`.
  Root root;
  // Relative to the namespace around: `acme::lib::Money`.
  lib::Money second;
  // Fully qualified from the root.
  ::acme::lib::Money third;
  // Through a namespace alias.
  fmt::Money fourth;
  // The standard library.
  std::string name;
  // Unknown: a placeholder in the root.
  Missing missing;
  // Unknown qualified: under its namespace.
  other::Widget widget;
};
}
}
