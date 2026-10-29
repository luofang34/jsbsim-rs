// SPDX-License-Identifier: LGPL-2.1-or-later
// Random probe: draws from JSBSim's RandomNumberGenerator, on its own or as
// the executive's shared generator, and prints the numbers.
// tools/oracle/src/probe/random.rs is the Rust counterpart.
//
// Input lines:
//   new <seed>             a RandomNumberGenerator(seed) of its own
//   exec                   a new FGFDMExec; draws come from its generator
//   set <property> <v>     setDoubleValue on the exec's (created) node
//   get <property>         getDoubleValue of the exec's node
//   seed <int>             seed((unsigned int)int) of the current generator
//   uniform <n>            n GetUniformRandomNumber()
//   normal <n>             n GetNormalRandomNumber()
//   draws <pattern>        one draw per character: u uniform, n normal
//
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "FGJSBBase.h"
#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

void Num(std::string& out, double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  out += " ";
  out += buf;
}

class Probe {
public:
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Draw(char kind)
  {
    std::string out;
    if (kind == 'u') Num(out, generator->GetUniformRandomNumber());
    else if (kind == 'n') Num(out, generator->GetNormalRandomNumber());
    else out = " bad";
    return out;
  }

  JSBSim::FGFDMExec* exec = nullptr;
  std::shared_ptr<JSBSim::RandomNumberGenerator> generator;
};

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  if (op == "new") {
    int seed = 0;
    args >> seed;
    generator = std::make_shared<JSBSim::RandomNumberGenerator>(seed);
    return " ok";
  }
  if (op == "exec") {
    exec = new JSBSim::FGFDMExec();
    generator = exec->GetRandomGenerator();
    return " ok";
  }
  if (op == "set" || op == "get") {
    if (!exec) return " no-exec";
    std::string name;
    double v = 0.0;
    args >> name >> v;
    auto node = exec->GetPropertyManager()->GetNode(name, true);
    if (op == "set") node->setDoubleValue(v);
    std::string out;
    Num(out, node->getDoubleValue());
    return out;
  }
  if (!generator) return " no-generator";
  if (op == "seed") {
    int seed = 0;
    args >> seed;
    generator->seed(seed);
    return " ok";
  }
  if (op == "uniform" || op == "normal") {
    int n = 0;
    args >> n;
    std::string out;
    for (int i = 0; i < n; ++i) out += Draw(op[0]);
    return out;
  }
  if (op == "draws") {
    std::string pattern;
    args >> pattern;
    std::string out;
    for (char c : pattern) out += Draw(c);
    return out;
  }
  return " unknown";
}

}  // namespace

int RunRandomProbe(const char* ops_file, const char* out_file)
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
