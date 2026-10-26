class Node {
public:
  Node* next;
  Node& self;
  Node** list;
  int buffer[16];
  char name[32 + 1];
  const char* label;
  int *a, b, &c = b;
  Node&& moved(Node&& other);
  void fill(int values[], int count, ...);
};
