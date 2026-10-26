#include <vector>
#include <memory>

class Item {};

typedef std::vector<Item> Items;
using ItemPtr = std::unique_ptr<Item>;

typedef struct Point {
  int x;
} Point;

typedef struct {
  int w, h;
} Size;

class Store {
public:
  using Id = unsigned long;
  typedef Item* Raw;
  Items items;
  ItemPtr best;
  Id id;
  Raw raw;
  Point origin;
  Size size;
};
