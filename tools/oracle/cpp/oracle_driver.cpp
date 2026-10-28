// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999  Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/JSBSim.cpp (script execution loop).
//
// Runs a JSBSim script exactly as the JSBSim command-line program does in
// batch mode, and samples a list of properties at full double precision
// (%.17g). The stock CSV output is limited to 10 significant digits, which
// would set the floor of every parity tolerance.
//
// Usage: oracle_driver script <root> <script> <rate_hz> <end_time|-> <out.csv>
//                             <property>...
//        oracle_driver properties|units|xml|math <input.txt> <out.txt>
//
// The script mode is implemented here; the probe modes are in probe_*.cpp.
//
// Rows are written after RunIC (and trim, if requested) and then after every
// N-th call to FGFDMExec::Run(), where N is derived from <rate_hz> with the
// formula of FGOutputType::SetRateHz. The time step and N are printed to
// stdout as "dt=<dt> every_n_frames=<N>".

#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include "FGFDMExec.h"
#include "initialization/FGInitialCondition.h"
#include "initialization/FGTrim.h"
#include "input_output/FGPropertyManager.h"
#include "probes.h"

namespace {

bool WriteRow(FILE* out, JSBSim::FGFDMExec& exec,
              const std::vector<SGPropertyNode*>& nodes)
{
  std::fprintf(out, "%.17g", exec.GetSimTime());
  for (auto node : nodes) std::fprintf(out, ",%.17g", node->getDoubleValue());
  return std::fputc('\n', out) != EOF;
}

}  // namespace

int RunScript(int argc, char** argv)
{
  if (argc < 7) {
    std::fprintf(stderr, "usage: %s <root> <script> <rate_hz> "
                 "<end_time|-> <out.csv> <property>...\n", argv[0]);
    return 2;
  }
  const std::string root = argv[1];
  const std::string script = argv[2];
  const double rate_hz = std::strtod(argv[3], nullptr);
  const std::string end_arg = argv[4];
  const double end_time = end_arg == "-" ? 1e99 : std::strtod(argv[4], nullptr);
  if (!(rate_hz > 0.0 && rate_hz <= 1000.0)) {
    std::fprintf(stderr, "rate_hz must be in (0, 1000]\n");
    return 2;
  }

  JSBSim::FGFDMExec exec;
  exec.SetRootDir(SGPath::fromLocal8Bit(root.c_str()));
  exec.SetAircraftPath(SGPath("aircraft"));
  exec.SetEnginePath(SGPath("engine"));
  exec.SetSystemsPath(SGPath("systems"));

  if (!exec.LoadScript(SGPath::fromLocal8Bit(script.c_str()), 0.0, SGPath())) {
    std::fprintf(stderr, "script %s was not loaded\n", script.c_str());
    return 1;
  }

  const double dt = exec.GetDeltaT();
  const long every = (unsigned int)(0.5 + 1.0/(dt*rate_hz));
  if (every < 1) {
    std::fprintf(stderr, "rate_hz %g is faster than the time step %g\n", rate_hz, dt);
    return 2;
  }
  std::printf("dt=%.17g every_n_frames=%ld\n", dt, every);

  std::vector<SGPropertyNode*> nodes;
  for (int i = 6; i < argc; ++i) {
    SGPropertyNode* node = exec.GetPropertyManager()->GetNode(argv[i]);
    if (!node) {
      std::fprintf(stderr, "no property named %s\n", argv[i]);
      return 1;
    }
    nodes.push_back(node);
  }

  exec.RunIC();

  auto trim_mode = (JSBSim::TrimMode)exec.GetIC()->TrimRequested();
  if (trim_mode != JSBSim::TrimMode::tNone) {
    JSBSim::FGTrim trimmer(&exec, trim_mode);
    try {
      trimmer.DoTrim();
    } catch (std::string& msg) {
      std::fprintf(stderr, "trim failed: %s\n", msg.c_str());
      return 1;
    }
  }

  FILE* out = std::fopen(argv[5], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[5]);
    return 1;
  }
  std::fputs("Time", out);
  for (int i = 6; i < argc; ++i) std::fprintf(out, ",%s", argv[i]);
  std::fputc('\n', out);

  bool ok = WriteRow(out, exec, nodes);
  long frames = 0;
  // The command-line program makes one Run() before entering its loop.
  bool result = exec.Run();
  ++frames;
  if (frames % every == 0) ok = ok && WriteRow(out, exec, nodes);
  while (ok && result && exec.GetSimTime() <= end_time) {
    result = exec.Run();
    ++frames;
    if (frames % every == 0) ok = WriteRow(out, exec, nodes);
  }

  if (std::fclose(out) != 0 || !ok) {
    std::fprintf(stderr, "error writing %s\n", argv[5]);
    return 1;
  }
  return 0;
}

int main(int argc, char** argv)
{
  const std::string mode = argc > 1 ? argv[1] : "";
  if (mode == "script") return RunScript(argc - 1, argv + 1);
  if (argc == 4) {
    if (mode == "properties") return RunPropertiesProbe(argv[2], argv[3]);
    if (mode == "units") return RunUnitsProbe(argv[2], argv[3]);
    if (mode == "math") return RunMathProbe(argv[2], argv[3]);
    if (mode == "xml") return RunXmlProbe(argv[2], argv[3]);
  }
  std::fprintf(stderr, "usage: %s script|properties|units|xml|math ...\n", argv[0]);
  return 2;
}
