class Shape {
public:
  virtual ~Shape();
  virtual double area() const = 0;
  static int count();
  static const int sides = 0;
  static constexpr double pi = 3.14159;
  mutable int cache;
  const int id;
  volatile int flag;
  Shape(const Shape&) = delete;
  Shape& operator=(const Shape&) = default;
  bool operator==(const Shape& other) const;
  explicit operator bool() const;
  virtual void draw() final;
  friend class Canvas;
  int (*callback)(int);
};

class Square final : public Shape {
public:
  double area() const override { return side * side; }
private:
  double side = 1.0;
};
