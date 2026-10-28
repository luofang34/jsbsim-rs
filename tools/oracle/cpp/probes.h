// SPDX-License-Identifier: LGPL-2.1-or-later
// Entry points of the oracle driver's modes, one per probe source file.
#ifndef ORACLE_PROBES_H
#define ORACLE_PROBES_H

int RunScript(int argc, char** argv);
int RunPropertiesProbe(const char* ops_file, const char* out_file);
int RunMathProbe(const char* ops_file, const char* out_file);
int RunUnitsProbe(const char* ops_file, const char* out_file);
int RunXmlProbe(const char* list_file, const char* out_file);

#endif
