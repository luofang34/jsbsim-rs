// SPDX-License-Identifier: LGPL-2.1-or-later
// Model-loading probe: loads whole aircraft with FGFDMExec::LoadModel and
// prints the property subtrees of the models the port covers.
// tools/oracle/src/probe/models.rs is the Rust counterpart.
//
// Input lines:
//   model <name>   new FGFDMExec rooted at JSBSIM_ROOT; LoadModel(<name>);
//                  print "ok" and one line per valued leaf node under the
//                  covered prefixes, sorted, or "load-error"
//
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>
#include <algorithm>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

// Keep in step with PREFIXES in tools/oracle/src/probe/models.rs.
const char* const kPrefixes[] = {
  "inertia/", "metrics/", "position/", "velocities/", "attitude/",
  "accelerations/", "forces/", "moments/", "aero/", "atmosphere/",
  "external_reactions/", "fcs/", "systems/", "simulation/",
};

// Output types made by <output> I/O directives, which the port ignores.
const char* const kExcluded = "simulation/output";

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

bool Covered(const std::string& path)
{
  for (const char* p : kPrefixes)
    if (path.compare(0, std::string(p).size(), p) == 0) return true;
  return false;
}

void Walk(SGPropertyNode* node, std::vector<std::string>& lines)
{
  for (int i = 0; i < node->nChildren(); ++i) {
    SGPropertyNode* child = node->getChild(i);
    if (child->nChildren() > 0) {
      Walk(child, lines);
      continue;
    }
    std::string path = JSBSim::GetRelativeName(child, "/fdm/jsbsim/");
    if (!Covered(path) || path.compare(0, std::string(kExcluded).size(), kExcluded) == 0)
      continue;
    switch (child->getType()) {
    case simgear::props::BOOL:
    case simgear::props::INT:
    case simgear::props::LONG:
    case simgear::props::FLOAT:
    case simgear::props::DOUBLE:
      lines.push_back(path + " = " + Number(child->getDoubleValue()));
      break;
    case simgear::props::STRING:
    case simgear::props::UNSPECIFIED:
      lines.push_back(path + " = \"" + child->getStringValue() + "\"");
      break;
    default:
      break;
    }
  }
}

std::string Load(const std::string& name)
{
  const char* root = std::getenv("JSBSIM_ROOT");
  auto* exec = new JSBSim::FGFDMExec();
  exec->SetRootDir(SGPath(root ? root : "."));
  exec->SetAircraftPath(SGPath("aircraft"));
  exec->SetEnginePath(SGPath("engine"));
  exec->SetSystemsPath(SGPath("systems"));
  try {
    if (!exec->LoadModel(name)) return " load-error";
  } catch (...) {
    return " load-error";
  }
  std::vector<std::string> lines;
  Walk(exec->GetPropertyManager()->GetNode(), lines);
  std::sort(lines.begin(), lines.end());
  std::string out = " ok";
  for (const auto& l : lines) out += "\n  " + l;
  return out;
}

}  // namespace

int RunModelsProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op, name;
    args >> op >> name;
    std::string result = op == "model" ? Load(name) : " unknown";
    std::fprintf(out, "%s =>%s\n", line.c_str(), result.c_str());
  }
  std::_Exit(std::fclose(out) == 0 ? 0 : 1);
}
