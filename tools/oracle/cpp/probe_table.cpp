// SPDX-License-Identifier: LGPL-2.1-or-later
// Table probe: loads the first <table> under a document's root with
// FGTable(pm, el, prefix) into a fresh property manager, then evaluates it
// at explicit keys. tools/oracle/src/probe/table.rs is the Rust counterpart.
//
// Input lines:
//   table <prefix|-> <path>     path relative to the input file
//   lookup <key>...             GetValue(keys)
//   element <row> <col>         GetElement(row, col)
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "input_output/FGPropertyManager.h"
#include "input_output/FGXMLFileRead.h"
#include "math/FGTable.h"
#include "probes.h"

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

}  // namespace

int RunTableProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  std::string dir = ops_file;
  dir = dir.substr(0, dir.rfind('/') + 1);
  JSBSim::FGTable* table = nullptr;
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op, result;
    args >> op;
    try {
      if (op == "table") {
        std::string prefix, path;
        args >> prefix >> path;
        if (prefix == "-") prefix.clear();
        table = nullptr;
        auto* pm = new std::shared_ptr<JSBSim::FGPropertyManager>(
            std::make_shared<JSBSim::FGPropertyManager>());
        JSBSim::FGXMLFileRead reader;
        JSBSim::Element* doc = reader.LoadXMLDocument(SGPath(dir + path), false);
        JSBSim::Element* el = doc ? doc->FindElement("table") : nullptr;
        if (!el) {
          result = "missing";
        } else {
          table = new JSBSim::FGTable(*pm, el, prefix);
          result = "ok";
        }
      } else if (op == "lookup") {
        std::vector<double> keys;
        double k;
        while (args >> k) keys.push_back(k);
        result = table ? Number(table->GetValue(keys)) : "no-table";
      } else if (op == "element") {
        unsigned r, c;
        args >> r >> c;
        result = table ? Number(table->GetElement(r, c)) : "no-table";
      } else {
        result = "unknown";
      }
    } catch (...) {
      result = op == "table" ? "load-error" : "error";
      if (op == "table") table = nullptr;
    }
    std::fprintf(out, "%s => %s\n", line.c_str(), result.c_str());
  }
  std::_Exit(std::fclose(out) == 0 ? 0 : 1);
}
