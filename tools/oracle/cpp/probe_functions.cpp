// SPDX-License-Identifier: LGPL-2.1-or-later
// Function probe: loads <function> elements with JSBSim's FGFunction and
// evaluates them after property changes. tools/oracle/src/probe/functions.rs
// is the Rust counterpart.
//
// Input lines:
//   skip <name>                    do not load functions with this name
//   load file|local <path>         declare the root's <property> elements,
//                                  register type="template" functions, and
//                                  load every other <function> not nested in
//                                  another one (an unnamed one takes its
//                                  parent's name, else #<n>)
//   set <property> <value>
//   eval [<name>...]               print the named (or all) function values
//   get <property>                 print a property's value and flags
//   cache <name> 0|1
//   reset                          start over with a new FGFDMExec
//
// JSBSim re-reads every bound function while unbinding it, and a throwing
// operation (e.g. an undefined property) then terminates the process. So
// nothing is ever destroyed: reset abandons the old state and the probe
// leaves with _Exit once its output is flushed.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <set>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "FGFDMExec.h"
#include "input_output/FGXMLFileRead.h"
#include "math/FGFunction.h"
#include "probes.h"

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
  void DeclareProperties(JSBSim::Element* root);
  void Walk(JSBSim::Element* el);
  std::string Eval(std::istringstream& args);

  using Functions = std::vector<std::pair<std::string, SGSharedPtr<JSBSim::FGFunction>>>;
  void Reset()
  {
    exec_ptr = new JSBSim::FGFDMExec();
    functions_ptr = new Functions();
    documents_ptr = new std::vector<JSBSim::Element_ptr>();
    skipped.clear();
  }

  std::string list_dir;
  JSBSim::FGFDMExec* exec_ptr = new JSBSim::FGFDMExec();
  Functions* functions_ptr = new Functions();
  std::vector<JSBSim::Element_ptr>* documents_ptr = new std::vector<JSBSim::Element_ptr>();
  std::set<std::string> skipped;
};

void Probe::DeclareProperties(JSBSim::Element* root)
{
  auto pm = exec_ptr->GetPropertyManager();
  for (JSBSim::Element* p = root->FindElement("property"); p;
       p = root->FindNextElement("property")) {
    double value = 0.0;
    if (!p->GetAttributeValue("value").empty())
      value = p->GetAttributeValueAsNumber("value");
    std::string name = p->GetDataLine();
    if (pm->HasNode(name)) continue;
    pm->GetNode(name, true)->setDoubleValue(value);
  }
}

void Probe::Walk(JSBSim::Element* el)
{
  for (unsigned i = 0; i < el->GetNumElements(); ++i) {
    JSBSim::Element* child = el->GetElement(i);
    if (child->GetName() != "function") {
      Walk(child);
      continue;
    }
    std::string name = child->GetAttributeValue("name");
    if (child->GetAttributeValue("type") == "template") {
      exec_ptr->AddTemplateFunc(name, child);
      continue;
    }
    // An unnamed function is known by its owner's name (an <fcs_function>).
    if (name.empty()) name = el->GetAttributeValue("name");
    if (!skipped.count(name)) {
      if (name.empty()) name = "#" + std::to_string(functions_ptr->size());
      functions_ptr->emplace_back(name, new JSBSim::FGFunction(exec_ptr, child, ""));
    }
  }
}

std::string Probe::Eval(std::istringstream& args)
{
  std::set<std::string> only;
  std::string name;
  while (args >> name) only.insert(name);
  std::string out;
  for (auto& [fname, f] : *functions_ptr) {
    if (!only.empty() && !only.count(fname)) continue;
    std::string value;
    try {
      value = Number(f->GetValue());
    } catch (...) {
      value = "error";
    }
    out += " " + fname + " " + value;
  }
  return out;
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  std::string a, b;
  if (op == "skip") {
    args >> a;
    skipped.insert(a);
    return " ok";
  }
  if (op == "load") {
    args >> a >> b;
    const char* root = std::getenv("JSBSIM_ROOT");
    std::string path = (a == "file" ? std::string(root ? root : "") + "/" : list_dir) + b;
    try {
      JSBSim::FGXMLFileRead reader;
      JSBSim::Element_ptr doc = reader.LoadXMLDocument(SGPath(path), false);
      if (!doc) return " missing";
      documents_ptr->push_back(doc);
      DeclareProperties(doc);
      Walk(doc);
      return " " + std::to_string(functions_ptr->size()) + " functions";
    } catch (...) {
      return " load-error";
    }
  }
  if (op == "set") {
    double v;
    args >> a >> v;
    exec_ptr->GetPropertyManager()->GetNode(a, true)->setDoubleValue(v);
    return " ok";
  }
  if (op == "eval") return Eval(args);
  if (op == "reset") {
    Reset();
    return " ok";
  }
  if (op == "get") {
    args >> a;
    SGPropertyNode* node = exec_ptr->GetPropertyManager()->GetNode(a);
    if (!node) return " null";
    std::string value;
    try {
      value = Number(node->getDoubleValue());
    } catch (...) {
      value = "error";
    }
    return " value " + value + " tied " + (node->isTied() ? "1" : "0") +
           " write " + (node->getAttribute(SGPropertyNode::WRITE) ? "1" : "0");
  }
  if (op == "cache") {
    int on;
    args >> a >> on;
    for (auto& [fname, f] : *functions_ptr)
      if (fname == a) f->cacheValue(on != 0);
    return " ok";
  }
  return " unknown";
}

}  // namespace

int RunFunctionsProbe(const char* ops_file, const char* out_file)
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
