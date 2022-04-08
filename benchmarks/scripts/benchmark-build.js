import assert from "assert";
import fs from "fs";
import { ROOT_DIR, getOutputPath, PROJECTS } from "./helpers.js";
import { spawnSync } from "child_process";

const BENCHMARK_ITERATION_COUNT = 15;

/**
 * @typedef {"cheerpj" | "gwt" | "handwritten" | "javascript" | "jwebassembly" | "montera" | "teavm"} Project
 */

/**
 * @param {Project} name
 * @param {import("child_process").StdioOptions} [stdio]
 * @returns {number | [total: number, wasm: number]} build time in ms
 */
function buildProject(name, stdio) {
  // javascript doesn't require a build
  if (name === "javascript") return 0;

  // Clean build outputs and caches
  const clean = spawnSync("make", [`${name}_clean`], { cwd: ROOT_DIR, stdio });
  assert.strictEqual(clean.status, 0);

  // Rebuild project, recording build time in milliseconds
  const start = performance.now();
  const build = spawnSync("make", [`${name}_build`], { cwd: ROOT_DIR, stdio });
  const buildTime = performance.now() - start;
  assert.strictEqual(build.status, 0);

  // If we're building using montera (our project), and have captured stdio,
  // extract WebAssembly build time excluding javac
  if (name === "montera" && !stdio) {
    const stderr = build.stderr.toString();
    const match = stderr.match(/Finished in ([\d.]+)ms!/);
    assert(match);
    return [buildTime, parseFloat(match[1])];
  }

  return buildTime;
}

/**
 * @param {Project} name
 */
function* benchmarkProjectBuild(name) {
  for (let i = 0; i < BENCHMARK_ITERATION_COUNT; i++) {
    const result = buildProject(name);
    console.log(`${name} #${i + 1}: ${result}`);
    yield result;
  }
}

// Benchmark all project builds, writing results to a .csv file
const csv = fs.createWriteStream(getOutputPath("build"));
csv.write("name,iteration,build_time_ms,wasm_build_time_ms\n");
for (const name in PROJECTS) {
  // Skip optimised projects, they're for size/runtime benchmarks
  if (name.endsWith("opt")) continue;
  let i = 0;
  for (let result of benchmarkProjectBuild(name)) {
    if (!Array.isArray(result)) result = [result, ""];
    csv.write(`${name},${i},${result[0]},${result[1]}\n`);
    i++;
  }
}
csv.end();
