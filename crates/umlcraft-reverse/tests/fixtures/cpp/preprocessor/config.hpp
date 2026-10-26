#ifndef CONFIG_HPP
#define CONFIG_HPP

#define VERSION 3
#define MAX(a, b) ((a) > (b) ? (a) : (b))

#ifdef _WIN32
class WinOnly {};
#else
class Posix {};
#endif

class Service
#if VERSION > 2
    : public Base
#endif
{
public:
#if defined(FAST)
  void fast();
#elif defined(SLOW)
  void slow();
#else
  void normal();
#endif
};

#endif
