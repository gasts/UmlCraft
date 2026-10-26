namespace shop {
class Order {};

namespace billing {
class Invoice {};
}
}

// C++17 nested namespace.
namespace shop::billing::tax {
class Rate {};
}

// Anonymous: its declarations belong to the namespace around it.
namespace {
class Hidden {};
}

extern "C" {
struct CHandle {
  int fd;
};
}

class Global {};
