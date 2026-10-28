// SPDX-License-Identifier: LGPL-2.1-or-later
// Times FGFDMExec::Run() on a script for `oracle bench`.
//
// Usage: oracle_driver bench <root> <script> <warm_up> <frames> <repeats>
//
// Each repeat is a fresh simulation: LoadScript, RunIC, <warm_up> calls to
// Run(), then <frames> calls to Run() timed with std::chrono::steady_clock.
// Nothing is printed and no file is written inside the timed loop: output
// directives are disabled (the Rust port has none) and log records, such as
// event notifications, which JSBSim prints whatever JSBSIM_DEBUG says, go
// to a logger that drops them. Each repeat prints one line on stdout:
//   bench frames=<frames> ns=<elapsed nanoseconds>
// A run that ends before its frames are done fails the mode, since the
// window would then not be the same work on both sides.

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <string>

#include "FGFDMExec.h"
#include "input_output/FGLog.h"
#include "probes.h"

namespace {

class DroppingLogger : public JSBSim::FGLogger
{
public:
  void Message(const std::string&) override {}
};

bool ParseCount(const char* text, long& value)
{
  char* end = nullptr;
  value = std::strtol(text, &end, 10);
  return end != text && *end == '\0' && value >= 0;
}

// One repeat; returns the elapsed time in ns, or -1 on failure.
long long TimeOnce(const std::string& root, const std::string& script,
                   long warm_up, long frames)
{
  JSBSim::FGFDMExec exec;
  exec.SetRootDir(SGPath::fromLocal8Bit(root.c_str()));
  const char* aircraft_path = std::getenv("ORACLE_AIRCRAFT_PATH");
  exec.SetAircraftPath(SGPath(aircraft_path ? aircraft_path : "aircraft"));
  exec.SetEnginePath(SGPath("engine"));
  exec.SetSystemsPath(SGPath("systems"));
  if (!exec.LoadScript(SGPath::fromLocal8Bit(script.c_str()), 0.0, SGPath())) {
    std::fprintf(stderr, "script %s was not loaded\n", script.c_str());
    return -1;
  }
  exec.DisableOutput();
  if (!exec.RunIC()) {
    std::fprintf(stderr, "RunIC failed for %s\n", script.c_str());
    return -1;
  }
  auto previous = JSBSim::GetLogger();
  JSBSim::SetLogger(std::make_shared<DroppingLogger>());
  bool ok = true;
  for (long i = 0; ok && i < warm_up; ++i) ok = exec.Run();
  const auto start = std::chrono::steady_clock::now();
  for (long i = 0; ok && i < frames; ++i) ok = exec.Run();
  const auto stop = std::chrono::steady_clock::now();
  JSBSim::SetLogger(previous);
  if (!ok) {
    std::fprintf(stderr, "%s ended before %ld warm-up and %ld timed frames\n",
                 script.c_str(), warm_up, frames);
    return -1;
  }
  return std::chrono::duration_cast<std::chrono::nanoseconds>(stop - start).count();
}

}  // namespace

int RunBench(int argc, char** argv)
{
  long warm_up = 0, frames = 0, repeats = 0;
  if (argc != 6 || !ParseCount(argv[3], warm_up) || !ParseCount(argv[4], frames)
      || !ParseCount(argv[5], repeats) || frames < 1 || repeats < 1) {
    std::fprintf(stderr, "usage: %s <root> <script> <warm_up> <frames> <repeats>\n",
                 argv[0]);
    return 2;
  }
  for (long r = 0; r < repeats; ++r) {
    const long long ns = TimeOnce(argv[1], argv[2], warm_up, frames);
    if (ns < 0) return 1;
    std::printf("bench frames=%ld ns=%lld\n", frames, ns);
  }
  return 0;
}
