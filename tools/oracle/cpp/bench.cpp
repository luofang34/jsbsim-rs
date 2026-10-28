// SPDX-License-Identifier: LGPL-2.1-or-later
// Times FGFDMExec::Run() on a script or an aircraft for `oracle bench`.
//
// Usage: oracle_driver bench <root> <script> <warm_up> <frames> <repeats>
//        oracle_driver bench-aircraft <root> <aircraft> <warm_up> <frames>
//                      <repeats> [<property>=<value>...]
//
// Each repeat is a fresh simulation: LoadScript, or LoadModel and the
// initial-condition properties given, RunIC, <warm_up> calls to Run(),
// then <frames> calls to Run() timed with std::chrono::steady_clock.
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
#include <functional>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "FGFDMExec.h"
#include "input_output/FGLog.h"
#include "input_output/FGPropertyManager.h"
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

// Loads what one repeat runs; false when it was not loaded.
using Loader = std::function<bool(JSBSim::FGFDMExec&)>;

// One repeat of `what`; returns the elapsed time in ns, or -1 on failure.
long long TimeOnce(const std::string& root, const std::string& what,
                   const Loader& load, long warm_up, long frames)
{
  JSBSim::FGFDMExec exec;
  exec.SetRootDir(SGPath::fromLocal8Bit(root.c_str()));
  const char* aircraft_path = std::getenv("ORACLE_AIRCRAFT_PATH");
  exec.SetAircraftPath(SGPath(aircraft_path ? aircraft_path : "aircraft"));
  exec.SetEnginePath(SGPath("engine"));
  exec.SetSystemsPath(SGPath("systems"));
  if (!load(exec)) {
    std::fprintf(stderr, "%s was not loaded\n", what.c_str());
    return -1;
  }
  exec.DisableOutput();
  if (!exec.RunIC()) {
    std::fprintf(stderr, "RunIC failed for %s\n", what.c_str());
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
                 what.c_str(), warm_up, frames);
    return -1;
  }
  return std::chrono::duration_cast<std::chrono::nanoseconds>(stop - start).count();
}

// Times `load` <repeats> times, printing one line per repeat.
int Repeat(const std::string& root, const std::string& what, const Loader& load,
           long warm_up, long frames, long repeats)
{
  for (long r = 0; r < repeats; ++r) {
    const long long ns = TimeOnce(root, what, load, warm_up, frames);
    if (ns < 0) return 1;
    std::printf("bench frames=%ld ns=%lld\n", frames, ns);
  }
  return 0;
}

bool ParseCounts(char** argv, long& warm_up, long& frames, long& repeats)
{
  return ParseCount(argv[0], warm_up) && ParseCount(argv[1], frames)
      && ParseCount(argv[2], repeats) && frames >= 1 && repeats >= 1;
}

}  // namespace

int RunBench(int argc, char** argv)
{
  long warm_up = 0, frames = 0, repeats = 0;
  if (argc != 6 || !ParseCounts(argv + 3, warm_up, frames, repeats)) {
    std::fprintf(stderr, "usage: %s <root> <script> <warm_up> <frames> <repeats>\n",
                 argv[0]);
    return 2;
  }
  const std::string script = argv[2];
  const Loader load = [&script](JSBSim::FGFDMExec& exec) {
    return exec.LoadScript(SGPath::fromLocal8Bit(script.c_str()), 0.0, SGPath());
  };
  return Repeat(argv[1], "script " + script, load, warm_up, frames, repeats);
}

int RunBenchAircraft(int argc, char** argv)
{
  long warm_up = 0, frames = 0, repeats = 0;
  if (argc < 6 || !ParseCounts(argv + 3, warm_up, frames, repeats)) {
    std::fprintf(stderr, "usage: %s <root> <aircraft> <warm_up> <frames> <repeats> "
                 "[<property>=<value>...]\n", argv[0]);
    return 2;
  }
  std::vector<std::pair<std::string, double>> ic;
  for (int i = 6; i < argc; ++i) {
    const std::string arg = argv[i];
    const auto eq = arg.find('=');
    char* end = nullptr;
    const double value = eq == std::string::npos ? 0.0 : std::strtod(arg.c_str() + eq + 1, &end);
    if (eq == std::string::npos || end == arg.c_str() + eq + 1 || *end != '\0') {
      std::fprintf(stderr, "malformed initial condition %s\n", arg.c_str());
      return 2;
    }
    ic.emplace_back(arg.substr(0, eq), value);
  }
  const std::string aircraft = argv[2];
  const Loader load = [&aircraft, &ic](JSBSim::FGFDMExec& exec) {
    if (!exec.LoadModel(aircraft)) return false;
    for (const auto& [property, value] : ic)
      exec.GetPropertyManager()->GetNode(property, true)->setDoubleValue(value);
    return true;
  };
  return Repeat(argv[1], "aircraft " + aircraft, load, warm_up, frames, repeats);
}
