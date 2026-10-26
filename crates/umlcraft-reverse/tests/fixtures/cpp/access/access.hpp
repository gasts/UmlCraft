class Account {
  int id;  // private by default
public:
  Account();
  int balance() const;
protected:
  void audit();
private:
  int secret;
public:
  struct Entry {
    int amount;
  };
};

struct Record {
  int open;  // public by default
private:
  int hidden;
};
