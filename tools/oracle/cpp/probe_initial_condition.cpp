// SPDX-License-Identifier: LGPL-2.1-or-later
// Initial condition probe: loads initialization files with JSBSim's
// FGInitialCondition in an FGFDMExec without aircraft, or writes its ic/
// properties, and prints every IC getter.
// tools/oracle/src/probe/initial_condition.rs is the Rust counterpart.
//
// Input lines (paths relative to the input file's directory):
//   new                      new FGFDMExec; print
//   load <path>              new FGFDMExec; IC->Load(path, false); print
//   reload <path>            IC->Load(path, false) on the current state; print
//   set <property> <value>   setDoubleValue; print
//
// A print is the Load result (load and reload), then every ic/ property in
// bind order as "<name> <value>" (vw-dir-deg prints "below-noise-floor"
// instead of a value while the wind is round-off of airspeed minus ground
// velocity, so its direction is arbitrary; the wind components are still
// printed and compared), then the ECEF position, the orientation
// quaternion, the planet position angle, GetSpeedSet and TrimRequested.
// A Load that throws prints "load-error". Nothing is destroyed (see
// probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "initialization/FGInitialCondition.h"
#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

const char* const kProperties[] = {
  "vc-kts", "ve-kts", "vg-kts", "vt-kts", "mach", "roc-fpm", "gamma-deg",
  "alpha-deg", "beta-deg", "theta-deg", "phi-deg", "psi-true-deg",
  "lat-gc-deg", "long-gc-deg", "h-sl-ft", "h-agl-ft", "terrain-elevation-ft",
  "vg-fps", "vt-fps", "vw-bx-fps", "vw-by-fps", "vw-bz-fps", "vw-north-fps",
  "vw-east-fps", "vw-down-fps", "vw-mag-fps", "vw-dir-deg", "roc-fps",
  "u-fps", "v-fps", "w-fps", "vn-fps", "ve-fps", "vd-fps", "gamma-rad",
  "alpha-rad", "theta-rad", "beta-rad", "phi-rad", "psi-true-rad",
  "lat-gc-rad", "long-gc-rad", "p-rad_sec", "q-rad_sec", "r-rad_sec",
  "lat-geod-rad", "lat-geod-deg", "geod-alt-ft", "targetNlf",
};

// Below this horizontal wind speed (ft/s) the wind is round-off left by
// the airspeed minus the ground velocity, and its direction is arbitrary.
constexpr double kNoiseWind = 1e-9;

std::string Print(JSBSim::FGFDMExec& exec)
{
  auto pm = exec.GetPropertyManager();
  auto ic = exec.GetIC();
  std::string out;
  const bool noise_wind = ic->GetWindMagFpsIC() < kNoiseWind;
  for (const char* name : kProperties) {
    if (noise_wind && std::string(name) == "vw-dir-deg") {
      out += " vw-dir-deg below-noise-floor";
      continue;
    }
    out += " " + std::string(name) + " " +
           Number(pm->GetNode(std::string("ic/") + name)->getDoubleValue());
  }
  out += " ecef";
  for (int i = 1; i <= 3; ++i) out += " " + Number(ic->GetPosition()(i));
  out += " q";
  for (int i = 1; i <= 4; ++i) out += " " + Number(ic->GetOrientation()(i));
  out += " epa " + Number(ic->GetEarthPositionAngleIC());
  out += " speedset " + std::to_string(static_cast<int>(ic->GetSpeedSet()));
  out += " trim " + std::to_string(ic->TrimRequested());
  return out;
}

class Probe {
public:
  explicit Probe(std::string dir) : list_dir(std::move(dir)) {}
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Load(const std::string& rel);

  std::string list_dir;
  JSBSim::FGFDMExec* exec = nullptr;
};

std::string Probe::Load(const std::string& rel)
{
  try {
    bool result = exec->GetIC()->Load(SGPath(list_dir + rel), false);
    return std::string(result ? " true" : " false") + Print(*exec);
  } catch (...) {
    return " load-error";
  }
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  std::string a;
  if (op == "new" || op == "load") {
    exec = new JSBSim::FGFDMExec();
    if (op == "new") return Print(*exec);
    args >> a;
    return Load(a);
  }
  if (!exec) return " no-exec";
  if (op == "reload") {
    args >> a;
    return Load(a);
  }
  if (op == "set") {
    double v;
    args >> a >> v;
    exec->GetPropertyManager()->GetNode(a, true)->setDoubleValue(v);
    return Print(*exec);
  }
  return " unknown";
}

}  // namespace

int RunInitialConditionProbe(const char* ops_file, const char* out_file)
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
