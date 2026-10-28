// SPDX-License-Identifier: LGPL-2.1-or-later
// Winds probe: drives the FGWinds of an FGFDMExec through its properties and
// inputs and prints its properties.
// tools/oracle/src/probe/winds.rs is the Rust counterpart.
//
// Input lines:
//   set <property> <v>     setDoubleValue on the (created) node, print
//   uvw <u> <v> <w>        in.vUVW
//   tl2b <9 entries>       in.Tl2b, row by row
//   tw2b <9 entries>       in.Tw2b, row by row
//   dt <s>                 in.totalDeltaT
//   alt <asl> <agl> <span> in.AltitudeASL, in.DistanceAGL, in.wingspan
//   rate <n>               SetRate(n)
//   windned <n> <e> <d>    SetWindNED(n, e, d), the initial-condition path;
//                          print
//   run | hold             Run(false) | Run(true), print
//   init                   InitModel(), print
//   print                  print
//
// A print lists the readable properties FGWinds ties, in bind order, then
// GetTurbMagnitude and GetTurbDirection.
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "models/atmosphere/FGWinds.h"
#include "probes.h"

namespace {

const char* const kProperties[] = {
  "atmosphere/psiw-rad", "atmosphere/wind-north-fps",
  "atmosphere/wind-east-fps", "atmosphere/wind-down-fps",
  "atmosphere/wind-mag-fps", "atmosphere/gust-north-fps",
  "atmosphere/gust-east-fps", "atmosphere/gust-down-fps",
  "atmosphere/turb-north-fps", "atmosphere/turb-east-fps",
  "atmosphere/turb-down-fps", "atmosphere/p-turb-rad_sec",
  "atmosphere/q-turb-rad_sec", "atmosphere/r-turb-rad_sec",
  "atmosphere/turb-type", "atmosphere/turb-rate", "atmosphere/turb-gain",
  "atmosphere/turb-rhythmicity",
  "atmosphere/turbulence/milspec/windspeed_at_20ft_AGL-fps",
  "atmosphere/turbulence/milspec/severity",
  "atmosphere/total-wind-north-fps", "atmosphere/total-wind-east-fps",
  "atmosphere/total-wind-down-fps", "atmosphere/randomseed"};

void Num(std::string& out, const std::string& tag, double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  out += " " + tag + " " + buf;
}

JSBSim::FGMatrix33 ReadMatrix(std::istringstream& args)
{
  double m[9] = {0.0};
  for (double& v : m) args >> v;
  return JSBSim::FGMatrix33(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]);
}

class Probe {
public:
  Probe()
  {
    exec = new JSBSim::FGFDMExec();
    winds = exec->GetWinds().get();
  }
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Print()
  {
    std::string out;
    auto pm = exec->GetPropertyManager();
    for (const char* p : kProperties) Num(out, p, pm->GetNode(p)->getDoubleValue());
    Num(out, "turb-magnitude", winds->GetTurbMagnitude());
    Num(out, "turb-direction", winds->GetTurbDirection());
    return out;
  }

  JSBSim::FGFDMExec* exec;
  JSBSim::FGWinds* winds;
};

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  auto& in = winds->in;
  if (op == "set") {
    std::string name;
    double v = 0.0;
    args >> name >> v;
    exec->GetPropertyManager()->GetNode(name, true)->setDoubleValue(v);
    return Print();
  }
  if (op == "uvw") {
    double u = 0.0, v = 0.0, w = 0.0;
    args >> u >> v >> w;
    in.vUVW = JSBSim::FGColumnVector3(u, v, w);
    return " ok";
  }
  if (op == "tl2b") {
    in.Tl2b = ReadMatrix(args);
    return " ok";
  }
  if (op == "tw2b") {
    in.Tw2b = ReadMatrix(args);
    return " ok";
  }
  if (op == "dt") {
    args >> in.totalDeltaT;
    return " ok";
  }
  if (op == "alt") {
    args >> in.AltitudeASL >> in.DistanceAGL >> in.wingspan;
    return " ok";
  }
  if (op == "rate") {
    unsigned int n = 1;
    args >> n;
    winds->SetRate(n);
    return " ok";
  }
  if (op == "windned") {
    double n = 0.0, e = 0.0, d = 0.0;
    args >> n >> e >> d;
    winds->SetWindNED(n, e, d);
    return Print();
  }
  if (op == "run" || op == "hold") {
    winds->Run(op == "hold");
    return Print();
  }
  if (op == "init") {
    winds->InitModel();
    return Print();
  }
  if (op == "print") return Print();
  return " unknown";
}

}  // namespace

int RunWindsProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  Probe probe;
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
