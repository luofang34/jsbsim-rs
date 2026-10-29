// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/FGFDMExec.cpp (DoTrim).
//
// Trim probe: drives scripts and aircraft the way JSBSim's Python trim
// tests do, and prints what each trim did. tools/oracle/src/probe/trim.rs
// is the Rust counterpart.
//
// simulation/do_simple_trim is re-tied to Recorder::DoTrim, which is
// FGFDMExec::DoTrim with the FGTrim kept long enough to read its
// statistics. FGTrim keeps them private (TrimStats prints sub-iterations
// at three significant digits), so FGTrim.h is included with its members
// made public; access does not change the class layout.
//
// Input lines (root relative to this file's directory; aircraft and
// systems paths relative to the root):
//   paths <root> <aircraft> <systems>  for the next new
//   new                     new FGFDMExec with those paths
//   script <path>           LoadScript(<root>/<path>): ok or load-error
//   model <name>            LoadModel(<name>): ok or load-error
//   ic <name>               IC->Load(<name>, true): its result
//   watch <property>...     the properties a print lists, in order
//   set <property> <v>      setDoubleValue on the (created) node
//   runic                   RunIC(), then the trim the IC requests if any
//   run <n>                 Run() n times, or until it returns false
//   until <t>               Run() while sim-time-sec < t (ExecuteUntil)
//   print                   print
//
// Every op but paths, new and watch is followed by what each trim it ran
// did ("trim t <time> mode <m> result <0|1> its <cycles>", then per axis
// "axis <state> <control> control <value> state <value> tolerance <tol>
// sub <runs> successful <cycles> stability <frames> runs <frames>",
// then "after" and the watched properties when the frame ends), then
// "trim-failed <message>" if one threw, then the op's own result. A
// throw ends the op, as it ends the Python call.
// Nothing is destroyed (see probe_functions.cpp); the probe leaves with _Exit.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "FGFDMExec.h"
#include "FGJSBBase.h"
#include "initialization/FGInitialCondition.h"
#include "initialization/FGTrimAxis.h"
#include "input_output/FGPropertyManager.h"
#define class struct
#define private public
#include "initialization/FGTrim.h"
#undef private
#undef class
#include "probes.h"

namespace {

std::string Number(double v)
{
  char buf[64];
  std::snprintf(buf, sizeof(buf), "%.17g", v);
  return buf;
}

/// FGFDMExec::DoTrim, recording each trim.
class Recorder {
public:
  JSBSim::FGFDMExec* exec = nullptr;
  std::string records;

  void DoTrim(int mode)
  {
    if (mode < 0 || mode > JSBSim::tNone)
      throw JSBSim::TrimFailureException("Illegal trimming mode!");
    JSBSim::FGTrim trim(exec, (JSBSim::TrimMode)mode);
    bool success = trim.DoTrim();
    Record(trim, success);
    if (!success) throw JSBSim::TrimFailureException("Trim Failed");
    exec->GetPropertyManager()->GetNode("simulation/trim-completed")->setIntValue(1);
  }

  void Record(JSBSim::FGTrim& trim, bool success)
  {
    records += " trim t " + Number(exec->GetSimTime()) + " mode " +
               std::to_string(int(trim.mode)) + " result " +
               std::to_string(int(success)) + " its " +
               std::to_string(trim.total_its);
    for (unsigned int i = 0; i < trim.TrimAxes.size(); ++i) {
      JSBSim::FGTrimAxis& a = trim.TrimAxes[i];
      records += " axis " + a.GetStateName() + " " +
                 std::to_string(int(a.GetControlType())) + " control " +
                 Number(a.GetControl()) + " state " +
                 Number(a.GetState() + a.GetStateTarget()) + " tolerance " +
                 Number(a.GetTolerance()) + " sub " +
                 Number(trim.sub_iterations[i]) + " successful " +
                 Number(trim.successful[i]) + " stability " +
                 std::to_string(a.GetStability()) + " runs " +
                 std::to_string(a.GetRunCount());
    }
    pending = true;
  }

  bool pending = false;
};

class Probe {
public:
  explicit Probe(std::string dir) : list_dir(std::move(dir)) {}
  std::string Run(const std::string& op, std::istringstream& args);

private:
  std::string Print()
  {
    std::string out;
    auto pm = exec->GetPropertyManager();
    for (const auto& p : watched) {
      SGPropertyNode* node = pm->GetNode(p);
      out += " " + p + " " + (node ? Number(node->getDoubleValue()) : std::string("absent"));
    }
    return out;
  }

  /// The trims recorded since the last call, each with the watched
  /// properties as the frame ended.
  std::string Trims()
  {
    if (!recorder.pending) return "";
    std::string out = recorder.records + " after" + Print();
    recorder.records.clear();
    recorder.pending = false;
    return out;
  }

  bool Step()
  {
    bool result = exec->Run();
    trims += Trims();
    return result;
  }

  std::string Op(const std::string& op, std::istringstream& args);

  std::string list_dir, root, aircraft, systems, trims;
  std::vector<std::string> watched;
  JSBSim::FGFDMExec* exec = nullptr;
  Recorder recorder;
};

std::string Probe::Op(const std::string& op, std::istringstream& args)
{
  if (op == "script") {
    std::string path;
    args >> path;
    return exec->LoadScript(SGPath(list_dir + root + "/" + path)) ? " ok" : " load-error";
  }
  if (op == "model") {
    std::string name;
    args >> name;
    return exec->LoadModel(name) ? " ok" : " load-error";
  }
  if (op == "ic") {
    std::string name;
    args >> name;
    return exec->GetIC()->Load(SGPath(name), true) ? " true" : " false";
  }
  if (op == "set") {
    std::string p;
    double v = 0.0;
    args >> p >> v;
    exec->GetPropertyManager()->GetNode(p, true)->setDoubleValue(v);
    trims += Trims();
    return " ok";
  }
  if (op == "runic") {
    exec->RunIC();
    auto mode = (JSBSim::TrimMode)exec->GetIC()->TrimRequested();
    if (mode != JSBSim::tNone) {
      // As JSBSim.cpp: the trim's result is not checked.
      JSBSim::FGTrim trim(exec, mode);
      recorder.Record(trim, trim.DoTrim());
      trims += Trims();
    }
    return Print();
  }
  if (op == "run") {
    int n = 0;
    args >> n;
    for (int i = 0; i < n && Step(); ++i) {}
    return Print();
  }
  if (op == "until") {
    double t = 0.0;
    args >> t;
    while (exec->GetSimTime() < t) Step();
    return Print();
  }
  if (op == "print") return Print();
  return " unknown";
}

std::string Probe::Run(const std::string& op, std::istringstream& args)
{
  if (op == "paths") {
    args >> root >> aircraft >> systems;
    return " ok";
  }
  if (op == "new") {
    exec = new JSBSim::FGFDMExec();
    exec->SetRootDir(SGPath(list_dir + root));
    exec->SetAircraftPath(SGPath(aircraft));
    exec->SetEnginePath(SGPath("engine"));
    exec->SetSystemsPath(SGPath(systems));
    recorder = Recorder();
    recorder.exec = exec;
    auto pm = exec->GetPropertyManager();
    pm->Untie("simulation/do_simple_trim");
    pm->Tie<Recorder, int>("simulation/do_simple_trim", &recorder, nullptr,
                           &Recorder::DoTrim);
    return " ok";
  }
  if (op == "watch") {
    watched.clear();
    std::string p;
    while (args >> p) watched.push_back(p);
    return " ok";
  }
  if (!exec) return " no-exec";
  trims.clear();
  std::string result;
  try {
    result = Op(op, args);
  } catch (JSBSim::TrimFailureException& e) {
    trims += Trims();
    result = std::string(" trim-failed ") + e.what();
  } catch (...) {
    trims += Trims();
    result = " error";
  }
  return trims + result;
}

}  // namespace

int RunTrimProbe(const char* ops_file, const char* out_file)
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
