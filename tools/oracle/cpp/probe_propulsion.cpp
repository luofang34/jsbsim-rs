// SPDX-License-Identifier: LGPL-2.1-or-later
// Propulsion probe: loads an aircraft with FGFDMExec::LoadModel, drives its
// tank properties and the fuel transfers of FGPropulsion, and prints the
// tanks, the propulsion totals and the mass balance, the way JSBSim's
// tests/TestFuelTanksInertia.py and tests/TestPointMassInertia.py read
// them. tools/oracle/src/probe/propulsion.rs is the Rust counterpart.
//
// Input lines:
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
//   tanks                   every tank's state, then GetTanksWeight,
//                           GetTanksMoment and CalculateTankInertias
//   refuel <s> | dump <s>   DoRefuel(s) | DumpFuel(s), then tanks
//   transfer <from> <to> <lbs>  Transfer (-1 for outside), its overage
//   init                    Propulsion InitModel, then tanks
//
// A print lists each watched property and its getDoubleValue, or "absent"
// when the node does not exist; an exception prints "error".
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "models/FGPropulsion.h"
#include "models/propulsion/FGTank.h"
#include "probes.h"

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

std::string Tanks(JSBSim::FGFDMExec& exec)
{
  auto propulsion = exec.GetPropulsion();
  std::string out;
  for (unsigned int i = 0; i < propulsion->GetNumTanks(); ++i) {
    auto t = propulsion->GetTank(i);
    out += " tank" + std::to_string(i) + " " + std::to_string(t->GetType()) +
           " " + std::to_string(t->GetGrainType());
    for (double v : {t->GetContents(), t->GetPctFull(), t->GetCapacity(),
                     t->GetUnusable(), t->GetDensity(), t->GetXYZ(1),
                     t->GetXYZ(2), t->GetXYZ(3), t->GetStandpipe(),
                     double(t->GetPriority()), double(t->GetSelected()),
                     t->GetTemperature_degC(), t->GetIxx(), t->GetIyy(),
                     t->GetIzz(), t->GetExternalFlow()})
      out += " " + Number(v);
  }
  out += " weight " + Number(propulsion->GetTanksWeight()) + " moment";
  const JSBSim::FGColumnVector3& moment = propulsion->GetTanksMoment();
  for (int i = 1; i <= 3; ++i) out += " " + Number(moment(i));
  out += " J";
  const JSBSim::FGMatrix33& j = propulsion->CalculateTankInertias();
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 3; ++c) out += " " + Number(j(r, c));
  return out;
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
  auto propulsion = exec->GetPropulsion();
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
    if (op == "tanks") return Tanks(*exec);
    if (op == "refuel" || op == "dump") {
      double s = 0.0;
      args >> s;
      if (op == "refuel") propulsion->DoRefuel(s);
      else propulsion->DumpFuel(s);
      return Tanks(*exec);
    }
    if (op == "transfer") {
      int from = 0, to = 0;
      double lbs = 0.0;
      args >> from >> to >> lbs;
      return " " + Number(propulsion->Transfer(from, to, lbs));
    }
    if (op == "init") {
      propulsion->InitModel();
      return Tanks(*exec);
    }
  } catch (...) {
    return op == "load" ? " load-error" : " error";
  }
  return " unknown";
}

}  // namespace

int RunPropulsionProbe(const char* ops_file, const char* out_file)
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
