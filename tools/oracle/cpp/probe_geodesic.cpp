// SPDX-License-Identifier: LGPL-2.1-or-later
// Geodesic probe: FGLocation::GetDistanceTo and GetHeadingTo, which solve
// the inverse geodesic problem with the GeographicLib bundled in JSBSim, on
// the ellipse of JSBSim's default planet (WGS-84) or of a planet loaded from
// XML. tools/oracle/src/probe/geodesic.rs is the Rust counterpart.
//
// Input lines:
//   wgs84                      new FGFDMExec; use its planet's ellipse; print
//                              the semimajor and semiminor axes (ft)
//   planet file|local <path>   new FGFDMExec; FGInertial::Load the document's
//                              root if it is a <planet>, else its first
//                              <planet> child; print the axes
//   pair <lon1> <lat1> <h1> <lon2> <lat2>
//                              FGLocation on the ellipse at geodetic
//                              longitude, latitude (rad) and height (ft);
//                              print its derived longitude and geodetic
//                              latitude, then GetDistanceTo and GetHeadingTo
//                              the target longitude and latitude (rad)
//   anti <lon1> <lat1> <h1> <lon2> <lat2>
//   tiny <lon1> <lat1> <h1> <lon2> <lat2>
//                              as pair, for nearly antipodal pairs and for
//                              pairs a few metres apart or less, whose
//                              results are ill-conditioned
//   meridian <lon1> <lat1> <h1> <dlon> <lat2>
//                              as pair, to the location's own derived
//                              longitude plus dlon (0 or pi): exactly
//                              meridional whatever the derived longitude
//   same <lon> <lat> <h>       as pair, to the location's own derived
//                              longitude and geodetic latitude
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

#include "FGFDMExec.h"
#include "input_output/FGXMLFileRead.h"
#include "math/FGLocation.h"
#include "models/FGInertial.h"
#include "probes.h"

namespace {

void Num(std::string& out, const char* tag, double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), " %s %.17g", tag, v);
  out += buf;
}

class Probe {
public:
  explicit Probe(std::string dir) : list_dir(std::move(dir)) {}
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Axes()
  {
    std::string out;
    Num(out, "a", a);
    Num(out, "b", b);
    return out;
  }
  std::string Planet(const std::string& kind, const std::string& rel);
  std::string Pair(const std::string& op, std::istringstream& args);

  std::string list_dir;
  bool ellipse_set = false;
  double a = 0.0, b = 0.0;
};

std::string Probe::Planet(const std::string& kind, const std::string& rel)
{
  ellipse_set = false;
  const char* root = std::getenv("JSBSIM_ROOT");
  std::string path = (kind == "file" ? std::string(root ? root : "") + "/" : list_dir) + rel;
  auto* exec = new JSBSim::FGFDMExec();
  try {
    JSBSim::FGXMLFileRead reader;
    auto* doc = new JSBSim::Element_ptr(reader.LoadXMLDocument(SGPath(path), false));
    if (!*doc) return " missing";
    JSBSim::Element* el =
        (*doc)->GetName() == "planet" ? doc->ptr() : (*doc)->FindElement("planet");
    if (!el) return " no-planet";
    auto inertial = exec->GetInertial();
    if (!inertial->Load(el)) return " load-false";
    a = inertial->GetSemimajor();
    b = inertial->GetSemiminor();
  } catch (...) {
    return " load-error";
  }
  ellipse_set = true;
  return Axes();
}

std::string Probe::Pair(const std::string& op, std::istringstream& args)
{
  if (!ellipse_set) return " no-ellipse";
  double lon1 = 0.0, lat1 = 0.0, h1 = 0.0, lon2 = 0.0, lat2 = 0.0;
  args >> lon1 >> lat1 >> h1;
  if (op != "same") args >> lon2 >> lat2;
  JSBSim::FGLocation l;
  l.SetEllipse(a, b);
  l.SetPositionGeodetic(lon1, lat1, h1);
  if (op == "meridian") lon2 += l.GetLongitude();
  if (op == "same") {
    lon2 = l.GetLongitude();
    lat2 = l.GetGeodLatitudeRad();
  }
  std::string out;
  Num(out, "lon1", l.GetLongitude());
  Num(out, "lat1", l.GetGeodLatitudeRad());
  Num(out, "s", l.GetDistanceTo(lon2, lat2));
  Num(out, "psi", l.GetHeadingTo(lon2, lat2));
  return out;
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  if (op == "wgs84") {
    auto* exec = new JSBSim::FGFDMExec();
    a = exec->GetInertial()->GetSemimajor();
    b = exec->GetInertial()->GetSemiminor();
    ellipse_set = true;
    return Axes();
  }
  if (op == "planet") {
    std::string kind, rel;
    args >> kind >> rel;
    return Planet(kind, rel);
  }
  if (op == "pair" || op == "anti" || op == "tiny" || op == "meridian" || op == "same")
    return Pair(op, args);
  return " unknown";
}

}  // namespace

int RunGeodesicProbe(const char* ops_file, const char* out_file)
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
