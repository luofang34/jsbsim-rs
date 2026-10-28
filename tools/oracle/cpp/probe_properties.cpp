// SPDX-License-Identifier: LGPL-2.1-or-later
// Property-tree probe: executes one operation per input line against
// JSBSim's FGPropertyManager / SGPropertyNode and prints one result line per
// operation, so the Rust property tree can be compared transcript to
// transcript. tools/oracle/src/probe/properties.rs is the Rust counterpart;
// the two must agree on the operation language.

#include <cstdio>
#include <fstream>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

struct ReadOnly {
  double value = 42.0;
  double Get() const { return value; }
};

std::string FormatDouble(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

int AttributeBit(const std::string& name)
{
  static const std::map<std::string, int> bits = {
    {"READ", SGPropertyNode::READ}, {"WRITE", SGPropertyNode::WRITE},
    {"ARCHIVE", SGPropertyNode::ARCHIVE}, {"REMOVED", SGPropertyNode::REMOVED},
    {"TRACE_READ", SGPropertyNode::TRACE_READ},
    {"TRACE_WRITE", SGPropertyNode::TRACE_WRITE},
    {"USERARCHIVE", SGPropertyNode::USERARCHIVE},
    {"PRESERVE", SGPropertyNode::PRESERVE}};
  auto it = bits.find(name);
  return it == bits.end() ? 0 : it->second;
}

const char* TypeName(simgear::props::Type t)
{
  using namespace simgear::props;
  switch (t) {
  case NONE: return "NONE";
  case ALIAS: return "ALIAS";
  case BOOL: return "BOOL";
  case INT: return "INT";
  case LONG: return "LONG";
  case FLOAT: return "FLOAT";
  case DOUBLE: return "DOUBLE";
  case STRING: return "STRING";
  case UNSPECIFIED: return "UNSPECIFIED";
  default: return "OTHER";
  }
}

std::string Get(SGPropertyNode* node, const std::string& type)
{
  if (type == "bool") return node->getBoolValue() ? "1" : "0";
  if (type == "int") return std::to_string(node->getIntValue());
  if (type == "long") return std::to_string(node->getLongValue());
  if (type == "float") return FormatDouble(node->getFloatValue());
  if (type == "double") return FormatDouble(node->getDoubleValue());
  return std::string("\"") + node->getStringValue() + "\"";
}

bool Set(SGPropertyNode* node, const std::string& type, const std::string& text)
{
  if (type == "bool") return node->setBoolValue(text == "1");
  if (type == "int") return node->setIntValue(std::stoi(text));
  if (type == "long") return node->setLongValue(std::stol(text));
  if (type == "float") return node->setFloatValue(std::stof(text));
  if (type == "double") return node->setDoubleValue(std::stod(text));
  if (type == "unspecified") return node->setUnspecifiedValue(text.c_str());
  return node->setStringValue(text.c_str());
}

class Probe {
public:
  std::string Run(const std::string& op, std::istringstream& args);

private:
  SGPropertyNode* Require(const std::string& path)
  {
    SGPropertyNode* node = pm.GetNode(path, true);
    if (!node) throw std::string("cannot create ") + path;
    return node;
  }

  JSBSim::FGPropertyManager pm;
  std::vector<std::unique_ptr<double>> tied_doubles;
  std::vector<std::unique_ptr<ReadOnly>> tied_objects;
};

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  std::string a, b, c;
  args >> a;
  if (op == "find" || op == "create") {
    SGPropertyNode* node = pm.GetNode(a, op == "create");
    return node ? JSBSim::GetFullyQualifiedName(node) : "null";
  }
  if (op == "findi" || op == "createi") {
    int index;
    args >> index;
    SGPropertyNode* node = pm.GetNode(a, index, op == "createi");
    return node ? JSBSim::GetFullyQualifiedName(node) : "null";
  }
  if (op == "has") return pm.HasNode(a) ? "1" : "0";
  if (op == "get") {
    args >> b;
    SGPropertyNode* node = pm.GetNode(b);
    return node ? Get(node, a) : "null";
  }
  if (op == "set") {
    args >> b;
    std::getline(args >> std::ws, c);
    return Set(Require(b), a, c) ? "1" : "0";
  }
  if (op == "type") return TypeName(Require(a)->getType());
  if (op == "path") return Require(a)->getPath(false);
  if (op == "alias") {
    args >> b;
    return Require(a)->alias(pm.GetNode(b, true)) ? "1" : "0";
  }
  if (op == "alias-rel") {
    args >> b;
    return Require(a)->alias(b.c_str()) ? "1" : "0";
  }
  if (op == "unalias") return Require(a)->unalias() ? "1" : "0";
  if (op == "attr") {
    int on;
    args >> b >> on;
    Require(a)->setAttribute(SGPropertyNode::Attribute(AttributeBit(b)), on != 0);
    return "ok";
  }
  if (op == "getattr") {
    args >> b;
    return Require(a)->getAttribute(SGPropertyNode::Attribute(AttributeBit(b))) ? "1" : "0";
  }
  if (op == "tie-rw") {
    tied_doubles.push_back(std::make_unique<double>(0.0));
    pm.Tie(a, tied_doubles.back().get());
    return pm.GetNode(a)->isTied() ? "1" : "0";
  }
  if (op == "tie-ro") {
    tied_objects.push_back(std::make_unique<ReadOnly>());
    pm.Tie(a, tied_objects.back().get(), &ReadOnly::Get);
    return pm.GetNode(a)->isTied() ? "1" : "0";
  }
  if (op == "untie") {
    pm.Untie(a);
    return Require(a)->isTied() ? "1" : "0";
  }
  if (op == "mkname") {
    std::getline(args >> std::ws, b);
    return "\"" + pm.mkPropertyName(b, a == "1") + "\"";
  }
  if (op == "relname") {
    args >> b;
    return JSBSim::GetRelativeName(Require(a), b);
  }
  return "unknown op " + op;
}

}  // namespace

int RunPropertiesProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) {
    std::fprintf(stderr, "cannot open %s or %s\n", ops_file, out_file);
    return 1;
  }
  Probe probe;
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op;
    args >> op;
    std::string result;
    try {
      result = probe.Run(op, args);
    } catch (const std::string& msg) {
      result = "error: " + msg;
    }
    std::fprintf(out, "%s => %s\n", line.c_str(), result.c_str());
  }
  return std::fclose(out) == 0 ? 0 : 1;
}
