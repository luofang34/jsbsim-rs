// SPDX-License-Identifier: LGPL-2.1-or-later
// XML probe: parses each listed file with JSBSim's FGXMLFileRead/FGXMLParse
// and dumps the Element tree (names, start lines, attributes, data lines),
// or ERROR when parsing fails. tools/oracle/src/probe/xml.rs is the Rust
// counterpart; both must print identical dumps.
//
// Input lines: "file <path>" relative to $JSBSIM_ROOT, or "local <path>"
// relative to the directory of the input file.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <set>
#include <string>

#include "input_output/FGXMLFileRead.h"
#include "input_output/FGXMLParse.h"
#include "probes.h"

namespace {

// Element keeps its attributes private; remember every name seen so the dump
// can ask HasAttribute for each.
class DumpParse : public JSBSim::FGXMLParse {
public:
  std::set<std::string> names;
  void startElement(const char* name, const XMLAttributes& atts) override
  {
    JSBSim::FGXMLParse::startElement(name, atts);
    for (int i = 0; i < atts.size(); ++i) names.insert(atts.getName(i));
  }
};

std::string Escape(const std::string& s)
{
  std::string out;
  for (char c : s) {
    if (c == '\\') out += "\\\\";
    else if (c == '\n') out += "\\n";
    else out += c;
  }
  return out;
}

void Dump(FILE* out, JSBSim::Element* el, const std::set<std::string>& names,
          int depth)
{
  std::string indent(2 * depth, ' ');
  std::fprintf(out, "%sE %s line=%d\n", indent.c_str(), el->GetName().c_str(),
               el->GetLineNumber());
  for (const auto& name : names)
    if (el->HasAttribute(name))
      std::fprintf(out, "%s  @%s=%s\n", indent.c_str(), name.c_str(),
                   Escape(el->GetAttributeValue(name)).c_str());
  for (unsigned i = 0; i < el->GetNumDataLines(); ++i)
    std::fprintf(out, "%s  |%s\n", indent.c_str(),
                 Escape(el->GetDataLine(i)).c_str());
  for (unsigned i = 0; i < el->GetNumElements(); ++i)
    Dump(out, el->GetElement(i), names, depth + 1);
}

}  // namespace

int RunXmlProbe(const char* list_file, const char* out_file)
{
  std::ifstream in(list_file);
  FILE* out = std::fopen(out_file, "w");
  const char* root = std::getenv("JSBSIM_ROOT");
  if (!in || !out || !root) {
    std::fprintf(stderr, "cannot open %s or %s, or JSBSIM_ROOT unset\n",
                 list_file, out_file);
    return 1;
  }
  std::string list_dir = list_file;
  list_dir = list_dir.substr(0, list_dir.rfind('/') + 1);
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::string kind = line.substr(0, line.find(' '));
    std::string rel = line.substr(line.find(' ') + 1);
    std::string path = (kind == "file" ? std::string(root) + "/" : list_dir) + rel;
    std::fprintf(out, "== %s\n", line.c_str());
    DumpParse parser;
    JSBSim::FGXMLFileRead reader;
    try {
      JSBSim::Element* doc = reader.LoadXMLDocument(SGPath(path), parser, false);
      if (doc) Dump(out, doc, parser.names, 0);
      else std::fprintf(out, "MISSING\n");
    } catch (...) {
      std::fprintf(out, "ERROR\n");
    }
  }
  return std::fclose(out) == 0 ? 0 : 1;
}
