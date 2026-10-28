// SPDX-License-Identifier: LGPL-2.1-or-later
// FCS probe: loads an aircraft with FGFDMExec::LoadModel, drives its
// properties and runs it frame by frame the way JSBSim's Python FCS tests
// (tests/TestGain.py and friends) do, printing the watched properties.
// tools/oracle/src/probe/fcs.rs is the Rust counterpart.
//
// Input lines:
//   clock <seconds>         pin the wall clock (time()) at <seconds> since
//                           the Unix epoch, for FGMagnetometer's date
//   paths <root> <aircraft> <systems>  root relative to this file's directory;
//                                      aircraft and systems paths relative
//                                      to the root, for the next new
//   new                     new FGFDMExec with those paths
//   dt <s>                  Setdt
//   load <model>            LoadModel(<model>): "ok" or "load-error"
//   watch <property>...     the properties a print lists, in order
//   set <property> <v>      setDoubleValue on the (created) node
//   runic | run             RunIC() | Run(), then print
//   print                   print
//
// A print lists each watched property and its getDoubleValue, or "absent"
// when the node does not exist; an exception prints "error".
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <ctime>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

bool clock_pinned = false;
std::time_t pinned_time = 0;

}  // namespace

// FGMagnetometer dates its field model by time(). The driver links
// libJSBSim statically, so this definition is the one it calls: the
// pinned time once a "clock" line sets it, the real time before.
#if defined(__GLIBC__)
extern "C" std::time_t time(std::time_t* t) noexcept
#else
extern "C" std::time_t time(std::time_t* t)
#endif
{
  std::time_t now = pinned_time;
  if (!clock_pinned) {
    timespec ts{};
    clock_gettime(CLOCK_REALTIME, &ts);
    now = ts.tv_sec;
  }
  if (t) *t = now;
  return now;
}

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

class Probe {
public:
  explicit Probe(std::string dir) : list_dir(std::move(dir)) {}
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Print()
  {
    std::string out;
    auto pm = exec->GetPropertyManager();
    for (const auto& p : watched) {
      SGPropertyNode* node = pm->GetNode(p);
      out += " " + p + " " + (node ? Number(node->getDoubleValue()) : std::string("absent"));
    }
    return out;
  }

  std::string list_dir, root, aircraft, systems;
  std::vector<std::string> watched;
  JSBSim::FGFDMExec* exec = nullptr;
};

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  if (op == "clock") {
    long long seconds = 0;
    args >> seconds;
    pinned_time = static_cast<std::time_t>(seconds);
    clock_pinned = true;
    return " ok";
  }
  if (op == "paths") {
    args >> root >> aircraft >> systems;
    return " ok";
  }
  if (op == "new") {
    exec = new JSBSim::FGFDMExec();
    exec->SetRootDir(SGPath(list_dir + root));
    exec->SetAircraftPath(SGPath(aircraft));
    exec->SetSystemsPath(SGPath(systems));
    return " ok";
  }
  if (op == "watch") {
    watched.clear();
    std::string p;
    while (args >> p) watched.push_back(p);
    return " ok";
  }
  if (!exec) return " no-exec";
  try {
    if (op == "dt") {
      double dt = 0.0;
      args >> dt;
      exec->Setdt(dt);
      return " ok";
    }
    if (op == "load") {
      std::string model;
      args >> model;
      return exec->LoadModel(model) ? " ok" : " load-error";
    }
    if (op == "set") {
      std::string p;
      double v = 0.0;
      args >> p >> v;
      exec->GetPropertyManager()->GetNode(p, true)->setDoubleValue(v);
      return " ok";
    }
    if (op == "runic") {
      exec->RunIC();
      return Print();
    }
    if (op == "run") {
      exec->Run();
      return Print();
    }
    if (op == "print") return Print();
  } catch (...) {
    return op == "load" ? " load-error" : " error";
  }
  return " unknown";
}

}  // namespace

int RunFcsProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  std::string list_dir = ops_file;
  list_dir = list_dir.substr(0, list_dir.rfind('/') + 1);
  Probe probe(list_dir);
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op;
    args >> op;
    std::fprintf(out, "%s =>%s\n", line.c_str(), probe.Run(op, args).c_str());
  }
  std::_Exit(std::fclose(out) == 0 ? 0 : 1);
}
