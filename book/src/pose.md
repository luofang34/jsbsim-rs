# Position and attitude

JSBSim publishes attitude as Euler angles (`attitude/phi-deg`,
`attitude/theta-deg`, `attitude/psi-deg`), which are singular at ±90° of
pitch. For rendering, networking or another simulation, read the pose
directly:

| | |
|---|---|
| [`location`](api/jsbsim/struct.Fdm.html#method.location) | position, as a [`Location`](api/jsbsim/math/struct.Location.html) |
| [`quaternion`](api/jsbsim/struct.Fdm.html#method.quaternion) | attitude from the local north-east-down frame to body axes |
| [`quaternion_ecef`](api/jsbsim/struct.Fdm.html#method.quaternion_ecef) | attitude from Earth-centred, Earth-fixed axes to body axes |
| [`quaternion_eci`](api/jsbsim/struct.Fdm.html#method.quaternion_eci) | attitude from Earth-centred inertial axes to body axes |

Body axes are JSBSim's: x forward, y right, z down. Lengths are in feet and
angles in radians unless a name says otherwise.

## The `math` module

[`jsbsim::math`](api/jsbsim/math/index.html) holds the types the models use,
ported from JSBSim's `FGLocation`, `FGQuaternion`, `FGColumnVector3` and
`FGMatrix33`:

- [`Location`](api/jsbsim/math/struct.Location.html): a point by its ECEF
  coordinates, with geocentric
  ([`latitude`](api/jsbsim/math/struct.Location.html#method.latitude),
  [`longitude`](api/jsbsim/math/struct.Location.html#method.longitude),
  [`radius`](api/jsbsim/math/struct.Location.html#method.radius)) and, once
  an ellipse is set, geodetic
  ([`geod_latitude_deg`](api/jsbsim/math/struct.Location.html#method.geod_latitude_deg),
  [`geod_altitude`](api/jsbsim/math/struct.Location.html#method.geod_altitude))
  views; conversions to and from the local frame; and great-circle
  [`distance_to`](api/jsbsim/math/struct.Location.html#method.distance_to)
  and [`heading_to`](api/jsbsim/math/struct.Location.html#method.heading_to).
- [`Quaternion`](api/jsbsim/math/struct.Quaternion.html): a rotation, with
  [`euler`](api/jsbsim/math/struct.Quaternion.html#method.euler) angles and
  the transformation matrix
  [`t`](api/jsbsim/math/struct.Quaternion.html#method.t) and its inverse.
- [`ColumnVector3`](api/jsbsim/math/struct.ColumnVector3.html) and
  [`Matrix33`](api/jsbsim/math/struct.Matrix33.html), indexed by
  [`Axis`](api/jsbsim/math/enum.Axis.html) so that no index can be out of
  range.

`Location` and `Quaternion` cache derived values (the geodetic
coordinates, the rotation matrices) the way JSBSim does. A copy you modify
recomputes them on demand, and
[`cache_derived`](api/jsbsim/math/struct.Location.html#method.cache_derived)
computes them up front, which a [ground callback](terrain.md) should do on
the point it returns.
