// SPDX-License-Identifier: LGPL-2.1-or-later
// Keeps the driver's listening sockets on the loopback interface.
//
// FGfdmSocket binds the <input> sockets of an aircraft (the 737's TCP 5137
// and QTJSBSIM UDP 5139, tripod_input's UDP 5139) to INADDR_ANY, and the
// TCP one greets any client that connects. The oracle must listen on
// loopback only, and JSBSim's sources are not to be changed, so the driver
// supplies its own bind(): libJSBSim.a is linked statically into the
// driver, so its calls resolve to this definition at link time, ahead of
// the C library's, which it then calls through dlsym(RTLD_NEXT). Like the
// `#define private public` of probe_trim.cpp, the wrapper exists in the
// driver only and changes nothing the probes observe.
//
// Every AF_INET address becomes 127.0.0.1 and every AF_INET6 address ::1,
// keeping the port. JSBSim's sockets are all AF_INET; IPv6 is forced too so
// that a socket made by any other path cannot listen beyond loopback.
//
// When ORACLE_SOCKET_REPORT names a file, each call appends one line
//   bind <inet|inet6> sent=<address>:<port> result=<0|-1> bound=<address>:<port>|-
// where bound is getsockname's address after a successful bind, so that
// `oracle check-sockets` can tell what every listener of a run was bound to.

#include <dlfcn.h>
#include <errno.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <sys/types.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>

#ifndef __THROW
#define __THROW
#endif

namespace {

using BindFn = int (*)(int, const struct sockaddr*, socklen_t);

BindFn RealBind()
{
  static const BindFn real = reinterpret_cast<BindFn>(dlsym(RTLD_NEXT, "bind"));
  return real;
}

/// Rewrites an AF_INET or AF_INET6 address in place to loopback.
void ForceLoopback(struct sockaddr_storage& addr, socklen_t len)
{
  if (addr.ss_family == AF_INET && len >= sizeof(struct sockaddr_in)) {
    reinterpret_cast<struct sockaddr_in&>(addr).sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  } else if (addr.ss_family == AF_INET6 && len >= sizeof(struct sockaddr_in6)) {
    reinterpret_cast<struct sockaddr_in6&>(addr).sin6_addr = in6addr_loopback;
  }
}

/// "<address>:<port>" of an AF_INET or AF_INET6 address, or "?".
void Format(const struct sockaddr_storage& addr, char* out, size_t size)
{
  char text[INET6_ADDRSTRLEN] = "?";
  unsigned port = 0;
  if (addr.ss_family == AF_INET) {
    const auto& in = reinterpret_cast<const struct sockaddr_in&>(addr);
    inet_ntop(AF_INET, &in.sin_addr, text, sizeof(text));
    port = ntohs(in.sin_port);
  } else if (addr.ss_family == AF_INET6) {
    const auto& in6 = reinterpret_cast<const struct sockaddr_in6&>(addr);
    inet_ntop(AF_INET6, &in6.sin6_addr, text, sizeof(text));
    port = ntohs(in6.sin6_port);
  }
  std::snprintf(out, size, "%s:%u", text, port);
}

void Record(int fd, const struct sockaddr_storage& sent, int result)
{
  const char* path = std::getenv("ORACLE_SOCKET_REPORT");
  if (!path || !*path) return;
  const char* family = sent.ss_family == AF_INET ? "inet"
                       : sent.ss_family == AF_INET6 ? "inet6" : "other";
  char sent_text[INET6_ADDRSTRLEN + 8];
  Format(sent, sent_text, sizeof(sent_text));
  char bound_text[INET6_ADDRSTRLEN + 8] = "-";
  struct sockaddr_storage bound;
  socklen_t bound_len = sizeof(bound);
  std::memset(&bound, 0, sizeof(bound));
  if (result == 0 &&
      getsockname(fd, reinterpret_cast<struct sockaddr*>(&bound), &bound_len) == 0) {
    Format(bound, bound_text, sizeof(bound_text));
  }
  FILE* out = std::fopen(path, "a");
  if (!out) return;
  std::fprintf(out, "bind %s sent=%s result=%d bound=%s\n", family, sent_text, result,
               bound_text);
  std::fclose(out);
}

}  // namespace

extern "C" int bind(int fd, const struct sockaddr* addr, socklen_t len) __THROW
{
  const BindFn real = RealBind();
  if (!real) {
    errno = ENOSYS;
    return -1;
  }
  if (!addr || len > sizeof(struct sockaddr_storage)) return real(fd, addr, len);
  struct sockaddr_storage copy;
  std::memset(&copy, 0, sizeof(copy));
  std::memcpy(&copy, addr, len);
  ForceLoopback(copy, len);
  const int result = real(fd, reinterpret_cast<const struct sockaddr*>(&copy), len);
  // JSBSim reports errno after a failed bind; the report must not change it.
  const int saved = errno;
  Record(fd, copy, result);
  errno = saved;
  return result;
}
