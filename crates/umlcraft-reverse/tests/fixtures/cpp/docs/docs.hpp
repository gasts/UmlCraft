/// A document.
/// With two lines.
class Document {
public:
  /** The title. */
  int title;

  //! Doxygen with an exclamation mark.
  void save();

  /*! Qt style. */
  void load();

  int pages; ///< Trailing, belongs to pages: left out.

  // A plain comment is no documentation.
  void print();
};

/**
 * A template, documented above its template line.
 */
template <typename T>
class Box {
  T value;
};
