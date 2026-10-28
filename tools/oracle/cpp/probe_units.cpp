// SPDX-License-Identifier: LGPL-2.1-or-later
// Units probe: evaluates JSBSim's unit conversions and FGJSBBase constants.
// tools/oracle/src/probe/units.rs is the Rust counterpart.
//
// Input lines: "convert <FROM> <TO>" converts <v unit="FROM">1</v> to TO
// with FindElementValueAsNumberConvertTo; "const <name>" prints an
// FGJSBBase constant; "temp <function> <value>" applies a conversion helper.

#include <cstdio>
#include <fstream>
#include <map>
#include <sstream>
#include <string>

#include "FGJSBBase.h"
#include "input_output/FGXMLElement.h"
#include "probes.h"

namespace {

struct Base : public JSBSim::FGJSBBase {
  static double Constant(const std::string& name)
  {
    static const std::map<std::string, double> constants = {
      {"radtodeg", radtodeg}, {"degtorad", degtorad},
      {"hptoftlbssec", hptoftlbssec}, {"psftoinhg", psftoinhg},
      {"psftopa", psftopa}, {"fttom", fttom}, {"ktstofps", ktstofps},
      {"fpstokts", fpstokts}, {"inchtoft", inchtoft}, {"m3toft3", m3toft3},
      {"in3tom3", in3tom3}, {"inhgtopa", inhgtopa}, {"slugtolb", slugtolb},
      {"lbtoslug", lbtoslug}, {"kgtolb", kgtolb}, {"kgtoslug", kgtoslug}};
    return constants.at(name);
  }

  static double Temperature(const std::string& f, double v)
  {
    if (f == "KelvinToFahrenheit") return KelvinToFahrenheit(v);
    if (f == "CelsiusToRankine") return CelsiusToRankine(v);
    if (f == "RankineToCelsius") return RankineToCelsius(v);
    if (f == "KelvinToRankine") return KelvinToRankine(v);
    if (f == "RankineToKelvin") return RankineToKelvin(v);
    if (f == "FahrenheitToCelsius") return FahrenheitToCelsius(v);
    if (f == "CelsiusToFahrenheit") return CelsiusToFahrenheit(v);
    if (f == "CelsiusToKelvin") return CelsiusToKelvin(v);
    if (f == "KelvinToCelsius") return KelvinToCelsius(v);
    return FeetToMeters(v);
  }
};

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

std::string Convert(const std::string& from, const std::string& to)
{
  JSBSim::Element_ptr parent = new JSBSim::Element("p");
  JSBSim::Element* v = new JSBSim::Element("v");
  v->AddAttribute("unit", from);
  v->AddData("1");
  v->SetParent(parent);
  parent->AddChildElement(v);
  try {
    return Number(parent->FindElementValueAsNumberConvertTo("v", to));
  } catch (JSBSim::BaseException& e) {
    std::string what = e.what();
    return what.find("does not exist") != std::string::npos ? "unknown"
                                                             : "inconvertible";
  }
}

}  // namespace

int RunUnitsProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op, a, b;
    args >> op >> a >> b;
    std::string result;
    if (op == "convert") result = Convert(a, b);
    else if (op == "const") result = Number(Base::Constant(a));
    else if (op == "temp") result = Number(Base::Temperature(a, std::stod(b)));
    else result = "unknown op";
    std::fprintf(out, "%s => %s\n", line.c_str(), result.c_str());
  }
  return std::fclose(out) == 0 ? 0 : 1;
}
