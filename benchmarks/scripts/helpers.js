import path from "path";
import http from "http";
import fs from "fs";
import { fileURLToPath } from "url";
import { lookup } from "mrmime";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const ROOT_DIR = path.resolve(__dirname, "..");
export const DATA_DIR = path.join(ROOT_DIR, "data");

/**
 * Map of project names to entrypoint paths relative to src directory
 */
export const PROJECTS = {
  cheerpj: "/cheerpj/index.html",
  gwt: "/gwt/war/gwt.html",
  handwritten: "/handwritten/index.html",
  javascript: "/javascript/index.html",
  jvm: "/jvm/index.html",
  jwebassembly: "/jwebassembly/index.html",
  montera: "/montera/index.html",
  monteraopt: "/montera/index.opt.html",
  teavm: "/teavm/target/teavm-1.0-SNAPSHOT/index.html",
};

/**
 * @param {import("puppeteer").Page} page
 */
export async function waitForScriptReady(page) {
  // Each project will assign global functions for benchmarking.
  // When these are available, we know the page is ready to test.
  await page.waitForFunction(
    () =>
      window.fib !== undefined &&
      window.gcd !== undefined &&
      window.sum !== undefined
  );
}

/**
 * @param {string} hostname
 * @param {number} port
 * @param {string} rootPath
 * @returns {Promise<http.Server>}
 */
export function startFileServer(hostname, port, rootPath) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      // Sanitise path to prevent path traversal attacks: https://security.stackexchange.com/a/123723
      // Not too important for local benchmarking, but doesn't hurt
      const safePath = path.normalize(req.url).replace(/^(\.\.(\/|\\|$))+/, "");
      const filePath = path.join(rootPath, safePath);

      // Check the path exists and is a file
      let stat;
      try {
        stat = fs.statSync(filePath);
      } catch {}
      if (!stat || stat.isDirectory()) {
        // console.log(`${req.method} ${req.url} -> 404`);
        res.writeHead(404);
        return res.end("Not Found");
      }

      // Determine the Content-Type based on the file extension
      const type = lookup(filePath);

      // Send file response
      // console.log(`${req.method} ${req.url} -> 200 ${type ?? ""}`);
      res.writeHead(200, type && { "Content-Type": type });
      fs.createReadStream(filePath).pipe(res);
    });
    server.listen(port, hostname, () => resolve(server));
  });
}

/**
 * @param {number} n
 * @returns {string} n padded with 0
 */
function pad(n) {
  return n.toString().padStart(2, "0");
}

/**
 * @param {string} name
 * @returns {string}
 */
export function getOutputPath(name) {
  const now = new Date();
  const date = [
    now.getFullYear(),
    pad(now.getMonth() + 1),
    pad(now.getDate()),
    pad(now.getHours()),
    pad(now.getMinutes()),
    pad(now.getSeconds()),
  ].join("-");
  return path.join(DATA_DIR, `${name}-${date}.csv`);
}
