// SPDX-License-Identifier: LGPL-2.1-or-later
// Auxiliary probe: sets the inputs of the FGAuxiliary of an FGFDMExec
// directly, runs it and prints its properties and derived vectors.
// tools/oracle/src/probe/auxiliary.rs is the Rust counterpart.
//
// Input lines:
//   zero                    every input 0 (matrices too), but the planet
//                           constants StandardGravity and StdDaySLsoundspeed
//   s <name> <v>            scalar input (see kScalars)
//   v <name> <x> <y> <z>    vector input (see kVectors)
//   m <name> <9 values>     matrix input, row by row (tl2b, tb2l)
//   loc <lon> <lat> <h>     in.vLocation on the planet's ellipse at geodetic
//                           longitude, latitude (rad) and height (ft)
//   ic                      SetInitialState from the default initial
//                           conditions (longitude and latitude 0)
//   run | hold              Run(false) | Run(true), print
//   init                    InitModel, print
//   pitot <M> <p> | machqc <qc> <p> | vcas <M> <p> | machvc <vc> <p>
//                           PitotTotalPressure, MachFromImpactPressure,
//                           VcalibratedFromMach, MachFromVcalibrated
//   heading <lon> <lat>     in.vLocation.GetHeadingTo
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "input_output/FGPropertyManager.h"
#include "models/FGAuxiliary.h"
#include "models/FGInertial.h"
#include "probes.h"

using JSBSim::FGAuxiliary;
using JSBSim::FGColumnVector3;
using JSBSim::FGMatrix33;

namespace {

const char* const kProperties[] = {
  "propulsion/tat-r", "propulsion/tat-c", "propulsion/pt-lbs_sqft",
  "velocities/vc-fps", "velocities/vc-kts", "velocities/ve-fps",
  "velocities/ve-kts", "velocities/vtrue-fps", "velocities/vtrue-kts",
  "velocities/machU", "velocities/p-aero-rad_sec", "velocities/q-aero-rad_sec",
  "velocities/r-aero-rad_sec", "velocities/phidot-rad_sec",
  "velocities/thetadot-rad_sec", "velocities/psidot-rad_sec",
  "velocities/u-aero-fps", "velocities/v-aero-fps", "velocities/w-aero-fps",
  "velocities/vt-fps", "velocities/mach", "velocities/vg-fps",
  "accelerations/a-pilot-x-ft_sec2", "accelerations/a-pilot-y-ft_sec2",
  "accelerations/a-pilot-z-ft_sec2", "accelerations/n-pilot-x-norm",
  "accelerations/n-pilot-y-norm", "accelerations/n-pilot-z-norm",
  "accelerations/Nx", "accelerations/Ny", "accelerations/Nz",
  "forces/load-factor", "aero/alpha-rad", "aero/beta-rad", "aero/mag-beta-rad",
  "aero/alpha-deg", "aero/beta-deg", "aero/mag-beta-deg", "aero/Re",
  "aero/qbar-psf", "aero/qbarUW-psf", "aero/qbarUV-psf",
  "aero/alphadot-rad_sec", "aero/betadot-rad_sec", "aero/alphadot-deg_sec",
  "aero/betadot-deg_sec", "aero/h_b-cg-ft", "aero/h_b-mac-ft",
  "flight-path/gamma-rad", "flight-path/gamma-deg", "flight-path/psi-gt-rad",
  "position/distance-from-start-lon-mt", "position/distance-from-start-lat-mt",
  "position/distance-from-start-mag-mt", "position/vrp-gc-latitude_deg",
  "position/vrp-longitude_deg", "position/vrp-radius-ft",
  "position/from-start-neu-n-ft", "position/from-start-neu-e-ft",
  "position/from-start-neu-u-ft",
};

class Out {
public:
  void Num(const std::string& tag, double v)
  {
    char buf[64];
    std::snprintf(buf, sizeof(buf), " %.17g", v);
    text += " " + tag + buf;
  }
  void Vec(const std::string& tag, const FGColumnVector3& v)
  {
    for (int i = 1; i <= 3; ++i) Num(tag + std::to_string(i), v(i));
  }
  void Mat(const std::string& tag, const FGMatrix33& m)
  {
    for (int r = 1; r <= 3; ++r)
      for (int c = 1; c <= 3; ++c)
        Num(tag + std::to_string(r) + std::to_string(c), m(r, c));
  }
  std::string text;
};

double* Scalar(FGAuxiliary::Inputs& in, const std::string& name)
{
  if (name == "pressure") return &in.Pressure;
  if (name == "density") return &in.Density;
  if (name == "temperature") return &in.Temperature;
  if (name == "soundspeed") return &in.SoundSpeed;
  if (name == "nu") return &in.KinematicViscosity;
  if (name == "agl") return &in.DistanceAGL;
  if (name == "span") return &in.Wingspan;
  if (name == "chord") return &in.Wingchord;
  if (name == "mass") return &in.Mass;
  if (name == "costht") return &in.CosTht;
  if (name == "sintht") return &in.SinTht;
  if (name == "cosphi") return &in.CosPhi;
  if (name == "sinphi") return &in.SinPhi;
  return nullptr;
}

FGColumnVector3* Vector(FGAuxiliary::Inputs& in, const std::string& name)
{
  if (name == "pqr") return &in.vPQR;
  if (name == "pqri") return &in.vPQRi;
  if (name == "pqridot") return &in.vPQRidot;
  if (name == "uvw") return &in.vUVW;
  if (name == "uvwdot") return &in.vUVWdot;
  if (name == "vel") return &in.vVel;
  if (name == "accel") return &in.vBodyAccel;
  if (name == "eye") return &in.ToEyePt;
  if (name == "rp") return &in.RPBody;
  if (name == "vrp") return &in.VRPBody;
  if (name == "fw") return &in.vFw;
  if (name == "wind") return &in.TotalWindNED;
  if (name == "turb") return &in.TurbPQR;
  return nullptr;
}

class Probe {
public:
  Probe()
  {
    exec = new JSBSim::FGFDMExec();
    aux = exec->GetAuxiliary().get();
  }
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Print()
  {
    Out o;
    auto pm = exec->GetPropertyManager();
    for (const char* p : kProperties) o.Num(p, pm->GetNode(p)->getDoubleValue());
    o.Mat("tw2b", aux->GetTw2b());
    o.Mat("tb2w", aux->GetTb2w());
    o.Vec("ncg", aux->GetNcg());
    o.Vec("nwcg", aux->GetNwcg());
    return o.text;
  }
  void Zero();
  JSBSim::FGLocation Geodetic(double lon, double lat, double h)
  {
    auto inertial = exec->GetInertial();
    JSBSim::FGLocation l;
    l.SetEllipse(inertial->GetSemimajor(), inertial->GetSemiminor());
    l.SetPositionGeodetic(lon, lat, h);
    return l;
  }

  JSBSim::FGFDMExec* exec;
  FGAuxiliary* aux;
};

void Probe::Zero()
{
  FGAuxiliary::Inputs& in = aux->in;
  const char* scalars[] = {"pressure", "density", "temperature", "soundspeed",
                           "nu", "agl", "span", "chord", "mass", "costht",
                           "sintht", "cosphi", "sinphi"};
  for (const char* s : scalars) *Scalar(in, s) = 0.0;
  const char* vectors[] = {"pqr", "pqri", "pqridot", "uvw", "uvwdot", "vel",
                           "accel", "eye", "rp", "vrp", "fw", "wind", "turb"};
  for (const char* v : vectors) Vector(in, v)->InitMatrix();
  in.Tl2b.InitMatrix();
  in.Tb2l.InitMatrix();
  in.vLocation = JSBSim::FGLocation();
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  FGAuxiliary::Inputs& in = aux->in;
  std::string name;
  double x = 0.0, y = 0.0, z = 0.0;
  if (op == "zero") {
    Zero();
    return " ok";
  }
  if (op == "s") {
    args >> name >> x;
    double* p = Scalar(in, name);
    if (!p) return " unknown";
    *p = x;
    return " ok";
  }
  if (op == "v") {
    args >> name >> x >> y >> z;
    FGColumnVector3* p = Vector(in, name);
    if (!p) return " unknown";
    *p = FGColumnVector3(x, y, z);
    return " ok";
  }
  if (op == "m") {
    args >> name;
    FGMatrix33* p = name == "tl2b" ? &in.Tl2b : name == "tb2l" ? &in.Tb2l : nullptr;
    if (!p) return " unknown";
    for (int r = 1; r <= 3; ++r)
      for (int c = 1; c <= 3; ++c) args >> (*p)(r, c);
    return " ok";
  }
  if (op == "loc") {
    args >> x >> y >> z;
    in.vLocation = Geodetic(x, y, z);
    return " ok";
  }
  if (op == "ic") {
    aux->SetInitialState(exec->GetIC().get());
    return " ok";
  }
  if (op == "run" || op == "hold") {
    aux->Run(op == "hold");
    return Print();
  }
  if (op == "init") {
    aux->InitModel();
    return Print();
  }
  Out o;
  args >> x >> y;
  if (op == "pitot") o.Num("pt", aux->PitotTotalPressure(x, y));
  else if (op == "machqc") o.Num("M", aux->MachFromImpactPressure(x, y));
  else if (op == "vcas") o.Num("vc", aux->VcalibratedFromMach(x, y));
  else if (op == "machvc") o.Num("M", aux->MachFromVcalibrated(x, y));
  else if (op == "heading") o.Num("psi", in.vLocation.GetHeadingTo(x, y));
  else return " unknown";
  return o.text;
}

}  // namespace

int RunAuxiliaryProbe(const char* ops_file, const char* out_file)
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
