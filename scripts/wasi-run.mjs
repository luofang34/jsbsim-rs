// SPDX-License-Identifier: LGPL-2.1-or-later
// Runs a wasm32-wasip1 program under Node's built-in WASI, with the
// repository root preopened at its own absolute host path, so that paths
// compiled in from CARGO_MANIFEST_DIR resolve inside the sandbox as they
// do natively. Exits with the program's status. Usage, also as a Cargo
// runner (CARGO_TARGET_WASM32_WASIP1_RUNNER):
//   node scripts/wasi-run.mjs program.wasm [args...]
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { WASI } from "node:wasi";

const [program, ...args] = process.argv.slice(2);
if (!program) {
  process.stderr.write("usage: wasi-run.mjs program.wasm [args...]\n");
  process.exit(2);
}
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const wasi = new WASI({
  version: "preview1",
  args: [program, ...args],
  env: { RUST_LOG: process.env.RUST_LOG ?? "info", NO_COLOR: "1" },
  preopens: { [repoRoot]: repoRoot },
  returnOnExit: true,
});
const module = await WebAssembly.compile(await readFile(program));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
