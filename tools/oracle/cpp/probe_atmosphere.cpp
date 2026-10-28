// SPDX-License-Identifier: LGPL-2.1-or-later
// Atmosphere probe: drives the FGStandardAtmosphere of an FGFDMExec directly
// and prints its properties and altitude functions.
// tools/oracle/src/probe/atmosphere.rs is the Rust counterpart.
//
// Input lines (units: F C R K, psf mbar pa inhg):
//   alt <h>                in.altitudeASL = h, Run(false), print properties
//   hold <h>               in.altitudeASL = h, Run(true), print properties
//   init                   InitModel(), print properties
//   set <property> <v>     setDoubleValue on the (created) node, print
//   at <h>                 the altitude-taking functions at h
//   units                  unit-converting getters and viscosities
//   settemp <t> <h> <u> | settempsl <t> <u> | bias <u> <t> | graded <u> <t>
//   gradedh <t> <h> <u> | psl <u> <p> | resett | resetp | dew <u> <v>
//   vapor <u> <v> | rh <v> | ppm <v>
//                          call the setter, print properties
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "models/FGAtmosphere.h"
#include "models/atmosphere/FGStandardAtmosphere.h"
#include "probes.h"

using JSBSim::FGAtmosphere;
using JSBSim::FGStandardAtmosphere;

namespace {

const char* const kProperties[] = {
  "atmosphere/T-R", "atmosphere/rho-slugs_ft3", "atmosphere/P-psf",
  "atmosphere/a-fps", "atmosphere/T-sl-R", "atmosphere/rho-sl-slugs_ft3",
  "atmosphere/a-sl-fps", "atmosphere/theta", "atmosphere/sigma",
  "atmosphere/delta", "atmosphere/a-ratio", "atmosphere/density-altitude",
  "atmosphere/pressure-altitude", "atmosphere/delta-T",
  "atmosphere/SL-graded-delta-T", "atmosphere/P-sl-psf",
  "atmosphere/dew-point-R", "atmosphere/vapor-pressure-psf",
  "atmosphere/saturated-vapor-pressure-psf", "atmosphere/RH",
  "atmosphere/vapor-fraction-ppm"};

const char* const kTempUnits[] = {"F", "C", "R", "K"};
const FGAtmosphere::eTemperature kTempEnums[] = {
  FGAtmosphere::eFahrenheit, FGAtmosphere::eCelsius, FGAtmosphere::eRankine,
  FGAtmosphere::eKelvin};
const char* const kPressUnits[] = {"psf", "mbar", "pa", "inhg"};
const FGAtmosphere::ePressure kPressEnums[] = {
  FGAtmosphere::ePSF, FGAtmosphere::eMillibars, FGAtmosphere::ePascals,
  FGAtmosphere::eInchesHg};

FGAtmosphere::eTemperature TempUnit(const std::string& s)
{
  for (int i = 0; i < 4; ++i)
    if (s == kTempUnits[i]) return kTempEnums[i];
  return FGAtmosphere::eRankine;
}

FGAtmosphere::ePressure PressUnit(const std::string& s)
{
  for (int i = 0; i < 4; ++i)
    if (s == kPressUnits[i]) return kPressEnums[i];
  return FGAtmosphere::ePSF;
}

class Out {
public:
  void Num(const std::string& tag, double v)
  {
    char buf[64];
    std::snprintf(buf, sizeof(buf), " %.17g", v);
    text += " " + tag + buf;
  }
  std::string text;
};

class Probe {
public:
  Probe()
  {
    exec = new JSBSim::FGFDMExec();
    atm = exec->GetAtmosphere().get();
    sa = static_cast<FGStandardAtmosphere*>(atm);
  }
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Properties()
  {
    Out o;
    for (const char* p : kProperties)
      o.Num(p, exec->GetPropertyManager()->GetNode(p)->getDoubleValue());
    return o.text;
  }
  std::string At(double h)
  {
    Out o;
    o.Num("T", atm->GetTemperature(h));
    o.Num("P", atm->GetPressure(h));
    o.Num("rho", atm->GetDensity(h));
    o.Num("a", atm->GetSoundSpeed(h));
    o.Num("theta", atm->GetTemperatureRatio(h));
    o.Num("stdT", sa->GetStdTemperature(h));
    o.Num("stdP", sa->GetStdPressure(h));
    o.Num("stdrho", sa->GetStdDensity(h));
    o.Num("stdtheta", sa->GetStdTemperatureRatio(h));
    return o.text;
  }
  std::string Units()
  {
    Out o;
    for (int i = 0; i < 4; ++i) {
      std::string u = kTempUnits[i];
      o.Num("bias-" + u, sa->GetTemperatureBias(kTempEnums[i]));
      o.Num("gradient-" + u, sa->GetTemperatureDeltaGradient(kTempEnums[i]));
      o.Num("dew-" + u, sa->GetDewPoint(kTempEnums[i]));
    }
    for (int i = 0; i < 4; ++i) {
      std::string u = kPressUnits[i];
      o.Num("psl-" + u, atm->GetPressureSL(kPressEnums[i]));
      o.Num("vapor-" + u, sa->GetVaporPressure(kPressEnums[i]));
      o.Num("saturated-" + u, sa->GetSaturatedVaporPressure(kPressEnums[i]));
    }
    o.Num("mu", atm->GetAbsoluteViscosity());
    o.Num("nu", atm->GetKinematicViscosity());
    o.Num("stdTsl", sa->GetStdTemperatureSL());
    o.Num("Tsl", atm->GetTemperatureSL());
    return o.text;
  }
  bool Setter(const std::string& op, std::istringstream& args);

  JSBSim::FGFDMExec* exec;
  FGAtmosphere* atm;
  FGStandardAtmosphere* sa;
};

bool Probe::Setter(const std::string& op, std::istringstream& args)
{
  std::string u;
  double t = 0.0, h = 0.0;
  if (op == "settemp") {
    args >> t >> h >> u;
    sa->SetTemperature(t, h, TempUnit(u));
  } else if (op == "settempsl") {
    args >> t >> u;
    sa->SetTemperatureSL(t, TempUnit(u));
  } else if (op == "bias") {
    args >> u >> t;
    sa->SetTemperatureBias(TempUnit(u), t);
  } else if (op == "graded") {
    args >> u >> t;
    sa->SetSLTemperatureGradedDelta(TempUnit(u), t);
  } else if (op == "gradedh") {
    args >> t >> h >> u;
    sa->SetTemperatureGradedDelta(t, h, TempUnit(u));
  } else if (op == "psl") {
    args >> u >> t;
    sa->SetPressureSL(PressUnit(u), t);
  } else if (op == "resett") {
    sa->ResetSLTemperature();
  } else if (op == "resetp") {
    sa->ResetSLPressure();
  } else if (op == "dew") {
    args >> u >> t;
    sa->SetDewPoint(TempUnit(u), t);
  } else if (op == "vapor") {
    args >> u >> t;
    sa->SetVaporPressure(PressUnit(u), t);
  } else if (op == "rh") {
    args >> t;
    sa->SetRelativeHumidity(t);
  } else if (op == "ppm") {
    args >> t;
    sa->SetVaporMassFractionPPM(t);
  } else {
    return false;
  }
  return true;
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  double h = 0.0;
  if (op == "alt" || op == "hold") {
    args >> h;
    atm->in.altitudeASL = h;
    atm->Run(op == "hold");
    return Properties();
  }
  if (op == "init") {
    atm->InitModel();
    return Properties();
  }
  if (op == "set") {
    std::string path;
    args >> path >> h;
    exec->GetPropertyManager()->GetNode(path, true)->setDoubleValue(h);
    return Properties();
  }
  if (op == "at") {
    args >> h;
    return At(h);
  }
  if (op == "units") return Units();
  if (Setter(op, args)) return Properties();
  return " unknown";
}

}  // namespace

int RunAtmosphereProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  Probe* probe = new Probe();
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op;
    args >> op;
    std::fprintf(out, "%s =>%s\n", line.c_str(), probe->Run(op, args).c_str());
  }
  std::_Exit(std::fclose(out) == 0 ? 0 : 1);
}
