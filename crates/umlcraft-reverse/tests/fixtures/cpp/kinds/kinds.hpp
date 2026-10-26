class Point {
  int x;
};

struct Pair {
  int first;
  int second;
};

union Value {
  int i;
  float f;
};

enum Color { Red, Green = 2, Blue };

enum class Shape : unsigned char { Circle, Square };

// Only declared here, defined below.
class Later;

class Later {
  int n;
};

// Only declared, never defined: an empty class.
class Opaque;

struct {
  int unnamed;
} instance;
