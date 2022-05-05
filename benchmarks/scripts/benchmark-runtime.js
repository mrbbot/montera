import assert from "assert";
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";
import puppeteer from "puppeteer";
import {
  PROJECTS,
  ROOT_DIR,
  startFileServer,
  waitForScriptReady,
  getOutputPath,
} from "./helpers.js";

const BENCHMARK_ITERATION_COUNT = 15;

// Start an HTTP server serving files in the src directory
const PORT = 8080;
const serverRoot = path.join(ROOT_DIR, "src");
const server = await startFileServer("127.0.0.1", PORT, serverRoot);

// Start a Chromium browser
const browser = await puppeteer.launch({
  headless: false,
  // Disable WebAssembly baseline compiler, require optimising compiler:
  // https://v8.dev/blog/liftoff#conclusion
  args: ["--js-flags=--no-liftoff"],
});
const page = await browser.newPage();

// Disable caching
await page.setCacheEnabled(false);

// Benchmark all projects, writing results to a .csv file
const csv = fs.createWriteStream(getOutputPath("runtime"));
csv.write("name,iteration,fib_time_ms,gcd_time_ms,sum_time_ms\n");
const BASE_URL = `http://localhost:${PORT}`;

/**
 * @param {string} path
 */
async function runBrowserBenchmarks(path) {
  // Visit project in browser and wait for scripts to load
  await page.goto(`${BASE_URL}${path}`);
  await waitForScriptReady(page);

  // Run benchmarks
  return await page.evaluate(() => {
    // Calculate the first 40 fibonacci numbers
    const fibStart = performance.now();
    for (let i = 1; i <= 40; i++) window.fib(i);
    const fibTime = performance.now() - fibStart;

    // Calculate the gcd's of all combinations of naturals up to 2^11
    const max = 2 ** 11;
    const gcdStart = performance.now();
    for (let a = 1; a < max; a++) {
      for (let b = 1; b < max; b++) window.gcd(a, b);
    }
    const gcdTime = performance.now() - gcdStart;

    // Calculate the sum of all {i, i+1, i+2}s for natural i's up to 4000
    const sumStart = performance.now();
    for (let i = 1; i < 4000; i++) window.sum(i, i + 1, i + 2);
    const sumTime = performance.now() - sumStart;

    return { fibTime, gcdTime, sumTime };
  });
}

function runJVMBenchmarks() {
  // Run Main class using JVM
  const cwd = path.join(serverRoot, "jvm", "out-class");
  const bench = spawnSync("java", ["Main"], { cwd });
  assert.strictEqual(bench.status, 0);
  // Results will be printed to stdout as JSON
  return JSON.parse(bench.stdout.toString().trim());
}

for (const [name, path] of Object.entries(PROJECTS)) {
  for (let i = 0; i < BENCHMARK_ITERATION_COUNT; i++) {
    // Java doesn't run in the browser :(
    const times = name === "jvm" ? runJVMBenchmarks() : await runBrowserBenchmarks(path);
    // Record times
    csv.write(
      `${name},${i},${times.fibTime},${times.gcdTime},${times.sumTime}\n`
    );
    console.log(`${name} #${i + 1}: ${JSON.stringify(times)}`);
  }
}
csv.end();

await browser.close();
await server.close();
