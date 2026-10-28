// SPDX-License-Identifier: LGPL-2.1-or-later
// Math probe: exercises FGColumnVector3, FGMatrix33, FGQuaternion and
// FGLocation on the inputs of each line and prints every derived quantity.
// tools/oracle/src/probe/math.rs is the Rust counterpart.
//
// Input lines (numbers in %.17g):
//   vec ax ay az bx by bz
//   mat m11 m12 m13 m21 m22 m23 m31 m32 m33
//   quat phi tht psi
//   qaxis idx angle | qangleaxis angle x y z | qexp x y z
//   loc x y z [semimajor semiminor]
//   geod lon lat height semimajor semiminor
//   locset x y z lon lat radius

#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "math/FGColumnVector3.h"
#include "math/FGLocation.h"
#include "math/FGMatrix33.h"
#include "math/FGQuaternion.h"
#include "probes.h"

using JSBSim::FGColumnVector3;
using JSBSim::FGLocation;
using JSBSim::FGMatrix33;
using JSBSim::FGQuaternion;

namespace {

class Out {
public:
  void Num(double v)
  {
    char buf[64];
    std::snprintf(buf, sizeof(buf), " %.17g", v);
    text += buf;
  }
  void Tag(const char* t) { text += std::string(" ") + t; }
  void Vec(const char* t, const FGColumnVector3& v)
  {
    Tag(t);
    for (int i = 1; i <= 3; ++i) Num(v(i));
  }
  void Mat(const char* t, const FGMatrix33& m)
  {
    Tag(t);
    for (int r = 1; r <= 3; ++r)
      for (int c = 1; c <= 3; ++c) Num(m(r, c));
  }
  void Quat(const char* t, const FGQuaternion& q)
  {
    Tag(t);
    for (int i = 1; i <= 4; ++i) Num(q(i));
  }
  std::string text;
};

void Vector(Out& o, const std::vector<double>& a)
{
  FGColumnVector3 u(a[0], a[1], a[2]), v(a[3], a[4], a[5]);
  o.Vec("sum", u + v);
  o.Vec("diff", u - v);
  o.Vec("cross", u * v);
  o.Tag("dot"); o.Num(DotProduct(u, v));
  o.Tag("mag"); o.Num(u.Magnitude());
  o.Tag("mag12"); o.Num(u.Magnitude(1, 2));
  o.Tag("mag23"); o.Num(u.Magnitude(2, 3));
  FGColumnVector3 n = u;
  o.Vec("norm", n.Normalize());
  o.Vec("scaled", 2.5 * u);
  o.Vec("div3", u / 3.0);
  o.Vec("div0", u / 0.0);
  FGColumnVector3 w = u;
  w /= v(1);
  o.Vec("divassign", w);
}

void Matrix(Out& o, const std::vector<double>& a)
{
  FGMatrix33 m(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]);
  FGColumnVector3 v(1.25, -2.5, 3.75);
  o.Tag("det"); o.Num(m.Determinant());
  o.Mat("inv", m.Inverse());
  o.Mat("tr", m.Transposed());
  o.Mat("sq", m * m);
  FGMatrix33 p = m;
  p *= m.Transposed();
  o.Mat("mulassign", p);
  o.Mat("sum", m + m.Transposed());
  o.Mat("diff", m - m.Transposed());
  o.Mat("scaled", 0.3 * m);
  o.Mat("div7", m / 7.0);
  o.Vec("mv", m * v);
  o.Vec("euler", m.GetEuler());
  o.Quat("quat", m.GetQuaternion());
}

void Quaternion(Out& o, const FGQuaternion& q)
{
  FGColumnVector3 pqr(0.1, -0.2, 0.3);
  o.Quat("q", q);
  o.Mat("T", q.GetT());
  o.Mat("Tinv", q.GetTInv());
  o.Vec("euler", q.GetEuler());
  o.Vec("eulerdeg", q.GetEulerDeg());
  o.Tag("sin");
  for (int i = 1; i <= 3; ++i) o.Num(q.GetSinEuler(i));
  o.Tag("cos");
  for (int i = 1; i <= 3; ++i) o.Num(q.GetCosEuler(i));
  o.Quat("qdot", q.GetQDot(pqr));
  o.Quat("inv", q.Inverse());
  o.Quat("conj", q.Conjugate());
  o.Quat("sq", q * q);
  o.Quat("sum", q + q.Conjugate());
  o.Quat("fromT", FGQuaternion(q.GetT()));
  FGQuaternion n = 3.0 * q;
  n.Normalize();
  o.Quat("renorm", n);
  o.Tag("mag"); o.Num(q.Magnitude());
}

void Location(Out& o, const FGLocation& l, bool ellipse)
{
  FGColumnVector3 lvec(100.0, -250.0, 30.0);
  o.Tag("lon"); o.Num(l.GetLongitude());
  o.Tag("londeg"); o.Num(l.GetLongitudeDeg());
  o.Tag("lat"); o.Num(l.GetLatitude());
  o.Tag("latdeg"); o.Num(l.GetLatitudeDeg());
  o.Tag("radius"); o.Num(l.GetRadius());
  o.Tag("sincoslon"); o.Num(l.GetSinLongitude()); o.Num(l.GetCosLongitude());
  if (ellipse) {
    o.Tag("geodlat"); o.Num(l.GetGeodLatitudeRad());
    o.Tag("geodlatdeg"); o.Num(l.GetGeodLatitudeDeg());
    o.Tag("geodalt"); o.Num(l.GetGeodAltitude());
  }
  o.Tag("sealevel"); o.Num(l.GetSeaLevelRadius());
  o.Mat("tec2l", l.GetTec2l());
  o.Mat("tl2ec", l.GetTl2ec());
  FGLocation moved = l.LocalToLocation(lvec);
  o.Vec("l2loc", FGColumnVector3(moved(1), moved(2), moved(3)));
  o.Vec("loc2l", l.LocationToLocal(FGColumnVector3(moved(1), moved(2), moved(3))));
  FGLocation sum = l + l;
  o.Tag("sumradius"); o.Num(sum.GetRadius());
  if (ellipse) { o.Tag("sumgeodalt"); o.Num(sum.GetGeodAltitude()); }
  FGLocation half = l * 0.5;
  if (ellipse) { o.Tag("halfgeodlat"); o.Num(half.GetGeodLatitudeRad()); }
}

}  // namespace

int RunMathProbe(const char* ops_file, const char* out_file)
{
  std::ifstream in(ops_file);
  FILE* out = std::fopen(out_file, "w");
  if (!in || !out) return 1;
  std::string line;
  while (std::getline(in, line)) {
    if (line.empty() || line[0] == '#') continue;
    std::istringstream args(line);
    std::string op;
    args >> op;
    std::vector<double> a;
    double x;
    while (args >> x) a.push_back(x);
    Out o;
    if (op == "vec") Vector(o, a);
    else if (op == "mat") Matrix(o, a);
    else if (op == "quat") Quaternion(o, FGQuaternion(a[0], a[1], a[2]));
    else if (op == "qaxis") Quaternion(o, FGQuaternion((int)a[0], a[1]));
    else if (op == "qangleaxis")
      Quaternion(o, FGQuaternion(a[0], FGColumnVector3(a[1], a[2], a[3])));
    else if (op == "qexp") Quaternion(o, QExp(FGColumnVector3(a[0], a[1], a[2])));
    else if (op == "loc") {
      FGLocation l(FGColumnVector3(a[0], a[1], a[2]));
      if (a.size() == 5) l.SetEllipse(a[3], a[4]);
      Location(o, l, a.size() == 5);
    } else if (op == "geod") {
      FGLocation l;
      l.SetEllipse(a[3], a[4]);
      l.SetPositionGeodetic(a[0], a[1], a[2]);
      o.Vec("ecef", FGColumnVector3(l(1), l(2), l(3)));
      Location(o, l, true);
    } else if (op == "locset") {
      FGLocation l(FGColumnVector3(a[0], a[1], a[2]));
      l.SetLongitude(a[3]);
      o.Vec("setlon", FGColumnVector3(l(1), l(2), l(3)));
      l.SetLatitude(a[4]);
      o.Vec("setlat", FGColumnVector3(l(1), l(2), l(3)));
      l.SetRadius(a[5]);
      o.Vec("setrad", FGColumnVector3(l(1), l(2), l(3)));
      FGLocation s(a[3], a[4], a[5]);
      o.Vec("spherical", FGColumnVector3(s(1), s(2), s(3)));
    } else o.Tag("unknown");
    std::fprintf(out, "%s =>%s\n", line.c_str(), o.text.c_str());
  }
  return std::fclose(out) == 0 ? 0 : 1;
}
