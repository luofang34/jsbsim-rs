// SPDX-License-Identifier: LGPL-2.1-or-later
// Entry points of the oracle driver's modes, one per probe source file.
#ifndef ORACLE_PROBES_H
#define ORACLE_PROBES_H

int RunScript(int argc, char** argv);
int RunBench(int argc, char** argv);
int RunBenchAircraft(int argc, char** argv);
int RunPropertiesProbe(const char* ops_file, const char* out_file);
int RunFunctionsProbe(const char* ops_file, const char* out_file);
int RunMassBalanceProbe(const char* ops_file, const char* out_file);
int RunModelsProbe(const char* ops_file, const char* out_file);
int RunMathProbe(const char* ops_file, const char* out_file);
int RunTableProbe(const char* ops_file, const char* out_file);
int RunUnitsProbe(const char* ops_file, const char* out_file);
int RunAtmosphereProbe(const char* ops_file, const char* out_file);
int RunWindsProbe(const char* ops_file, const char* out_file);
int RunFcsProbe(const char* ops_file, const char* out_file);
int RunPropulsionProbe(const char* ops_file, const char* out_file);
int RunAuxiliaryProbe(const char* ops_file, const char* out_file);
int RunGeodesicProbe(const char* ops_file, const char* out_file);
int RunInitialConditionProbe(const char* ops_file, const char* out_file);
int RunTrimProbe(const char* ops_file, const char* out_file);
int RunRandomProbe(const char* ops_file, const char* out_file);
int RunXmlProbe(const char* list_file, const char* out_file);

#endif
