// SPDX-License-Identifier: LGPL-2.1-or-later
//
// Ground callbacks the script mode installs when ORACLE_GROUND_CALLBACK
// names one; tools/oracle/src/ground.rs is their Rust twin, operation for
// operation. Each is a plane through the ECEF point (a, 0, 0), where the
// planet's surface crosses latitude 0 and longitude 0 (local north is +z,
// east +y, up +x), so that no trigonometry enters the terrain itself:
//
//   slope           tilted down towards the north by atan(5/12), at rest;
//   deck-moving     level, moving at a constant ECEF velocity (east and up);
//   deck-rotating   level, spinning about its normal through a point 30 ft
//                   south of (a, 0, 0);
//   deck-heaving    level, rising and falling 1 ft in a 4 s smoothstep
//                   cycle with the time it is given.

#include <cmath>
#include <cstring>

#include "FGFDMExec.h"
#include "input_output/FGGroundCallback.h"
#include "math/FGColumnVector3.h"
#include "math/FGLocation.h"
#include "models/FGInertial.h"
#include "probes.h"

namespace {

using JSBSim::FGColumnVector3;
using JSBSim::FGLocation;

enum class Kind { Slope, DeckMoving, DeckRotating, DeckHeaving };

class OracleGround : public JSBSim::FGGroundCallback
{
public:
  OracleGround(Kind kind, double a, double b) : kind(kind), a(a), b(b) {}

  double GetAGLevel(double t, const FGLocation& location, FGLocation& contact,
                    FGColumnVector3& normal, FGColumnVector3& v,
                    FGColumnVector3& w) const override
  {
    FGColumnVector3 origin(a, 0.0, 0.0);
    FGColumnVector3 n(1.0, 0.0, 0.0);
    FGColumnVector3 vel(0.0, 0.0, 0.0);
    FGColumnVector3 omega(0.0, 0.0, 0.0);
    double height = 0.0;
    switch (kind) {
    case Kind::Slope:
      n = FGColumnVector3(12.0/13.0, 0.0, 5.0/13.0);
      break;
    case Kind::DeckMoving:
      vel = FGColumnVector3(0.5, 20.0, 0.0);
      origin = origin + vel*t;
      break;
    case Kind::DeckRotating:
      omega = FGColumnVector3(0.3, 0.0, 0.0);
      break;
    case Kind::DeckHeaving: {
      double phase = std::fmod(t, 4.0)/4.0;
      double s = phase < 0.5 ? 2.0*phase : 2.0 - 2.0*phase;
      double ds = phase < 0.5 ? 0.5 : -0.5;
      height = s*s*(3.0 - 2.0*s);
      vel = n*(6.0*s*(1.0 - s)*ds);
      origin = origin + n*height;
      break;
    }
    }
    FGColumnVector3 r(location(1), location(2), location(3));
    double agl = DotProduct(r - origin, n);
    FGColumnVector3 point = r - agl*n;
    if (kind == Kind::DeckRotating) {
      FGColumnVector3 centre(a, 0.0, -30.0);
      vel = omega*(point - centre);
    }
    contact = FGLocation(point);
    contact.SetEllipse(a, b);
    normal = n;
    v = vel;
    w = omega;
    return agl;
  }

private:
  Kind kind;
  double a, b;
};

}  // namespace

bool InstallGroundCallback(JSBSim::FGFDMExec& exec, const char* name)
{
  Kind kind;
  if (std::strcmp(name, "slope") == 0) kind = Kind::Slope;
  else if (std::strcmp(name, "deck-moving") == 0) kind = Kind::DeckMoving;
  else if (std::strcmp(name, "deck-rotating") == 0) kind = Kind::DeckRotating;
  else if (std::strcmp(name, "deck-heaving") == 0) kind = Kind::DeckHeaving;
  else return false;
  auto inertial = exec.GetInertial();
  inertial->SetGroundCallback(new OracleGround(kind, inertial->GetSemimajor(),
                                               inertial->GetSemiminor()));
  return true;
}
