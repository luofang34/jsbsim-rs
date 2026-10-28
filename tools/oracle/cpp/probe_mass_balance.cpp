// SPDX-License-Identifier: LGPL-2.1-or-later
// Mass balance probe: loads a <mass_balance> element with JSBSim's
// FGMassBalance in a fresh FGFDMExec, runs it, and prints its properties.
// tools/oracle/src/probe/mass_balance.rs is the Rust counterpart.
//
// Input lines:
//   load file|local <path>   new FGFDMExec; Load the document's root if it is
//                            a <mass_balance>, else its first <mass_balance>
//                            child; Run(false); print
//   set <property> <value>   setDoubleValue
//   run                      Run(false); print
//   init                     InitModel
//
// A print lists the tied inertia/ properties (point masses while their
// nodes exist), the CG shift of the last run, then J and Jinv row by row.
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "input_output/FGXMLFileRead.h"
#include "models/FGMassBalance.h"
#include "probes.h"

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

const char* const kProperties[] = {
  "mass-slugs", "weight-lbs", "empty-weight-lbs", "cg-x-in", "cg-y-in",
  "cg-z-in", "ixx-slugs_ft2", "iyy-slugs_ft2", "izz-slugs_ft2",
  "ixy-slugs_ft2", "ixz-slugs_ft2", "iyz-slugs_ft2",
};

const char* const kPointMass[] = {
  "pointmass-weight-lbs", "pointmass-location-X-inches",
  "pointmass-location-Y-inches", "pointmass-location-Z-inches",
};

std::string Print(JSBSim::FGFDMExec& exec)
{
  auto pm = exec.GetPropertyManager();
  auto mb = exec.GetMassBalance();
  std::string out;
  for (const char* name : kProperties)
    out += std::string(" ") + name + "=" +
           Number(pm->GetNode(std::string("inertia/") + name)->getDoubleValue());
  for (int i = 0;; ++i) {
    const std::string index = "[" + std::to_string(i) + "]";
    if (!pm->HasNode(std::string("inertia/") + kPointMass[0] + index)) break;
    for (const char* name : kPointMass)
      out += std::string(" ") + name + index + "=" +
             Number(pm->GetNode(std::string("inertia/") + name + index)->getDoubleValue());
  }
  out += " dcg";
  for (int i = 1; i <= 3; ++i) out += " " + Number(mb->GetDeltaXYZcg(i));
  out += " J";
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 3; ++c) out += " " + Number(mb->GetJ()(r, c));
  out += " Jinv";
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 3; ++c) out += " " + Number(mb->GetJinv()(r, c));
  return out;
}

class Probe {
public:
  explicit Probe(std::string dir) : list_dir(std::move(dir)) {}
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Load(const std::string& kind, const std::string& rel);

  std::string list_dir;
  JSBSim::FGFDMExec* exec = nullptr;
};

std::string Probe::Load(const std::string& kind, const std::string& rel)
{
  const char* root = std::getenv("JSBSIM_ROOT");
  std::string path = (kind == "file" ? std::string(root ? root : "") + "/" : list_dir) + rel;
  exec = new JSBSim::FGFDMExec();
  try {
    JSBSim::FGXMLFileRead reader;
    auto* doc = new JSBSim::Element_ptr(reader.LoadXMLDocument(SGPath(path), false));
    if (!*doc) return " missing";
    JSBSim::Element* el = (*doc)->GetName() == "mass_balance"
                              ? doc->ptr()
                              : (*doc)->FindElement("mass_balance");
    if (!el) return " no-mass-balance";
    if (!exec->GetMassBalance()->Load(el)) return " load-false";
    exec->GetMassBalance()->Run(false);
    return Print(*exec);
  } catch (...) {
    return " load-error";
  }
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  std::string a, b;
  if (op == "load") {
    args >> a >> b;
    return Load(a, b);
  }
  if (!exec) return " no-exec";
  if (op == "set") {
    double v;
    args >> a >> v;
    exec->GetPropertyManager()->GetNode(a, true)->setDoubleValue(v);
    return " ok";
  }
  if (op == "run") {
    exec->GetMassBalance()->Run(false);
    return Print(*exec);
  }
  if (op == "init") {
    exec->GetMassBalance()->InitModel();
    return " ok";
  }
  return " unknown";
}

}  // namespace

int RunMassBalanceProbe(const char* ops_file, const char* out_file)
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
