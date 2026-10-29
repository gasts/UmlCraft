#include "order.hpp"

namespace shop {
/// Makes an order.
Order::Order(int number) {}

/// Ships it.
void Order::ship(int days, bool express) {}
}

int shop::Order::total() const { return 0; }

int shop::Order::count = 0;

// Not declared in the class: added.
void shop::Order::extra() {}

// Through the definitions' own using: the declared one, and one added.
using namespace lib;
void shop::Order::label(Str text) {}
void shop::Order::note(Str text) {}

// Of a class nobody declares.
void Gone::f() {}

// A free function of a namespace.
void shop::helper() {}

int main() { return 0; }
