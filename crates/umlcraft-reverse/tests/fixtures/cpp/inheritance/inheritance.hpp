class Base {};
class Mixin {};
template <typename T> class Holder {};

class Derived : public Base, protected Mixin {};
class Private : Base {};
struct Open : Base {};
class Diamond : public virtual Base, private Holder<int> {};
