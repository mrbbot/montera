import assert from "assert";
import fs from "fs";
import path from "path";
import http from "http";
import puppeteer from "puppeteer";
import {
  getOutputPath,
  PROJECTS,
  ROOT_DIR,
  startFileServer,
  waitForScriptReady,
} from "./helpers.js";

// Start an HTTP server serving files in the src directory
const PORT = 8080;
const serverRoot = path.join(ROOT_DIR, "src");
const server = await startFileServer("127.0.0.1", PORT, serverRoot);

// Start a Chromium browser
const browser = await puppeteer.launch({ headless: false });
const page = await browser.newPage();

// Disable caching, ensuring we intercept all network requests
await page.setCacheEnabled(false);

// Intercept network requests, and record the total number of bytes received
let bytesReceived = 0;
const client = await page.target().createCDPSession();
await client.send("Network.enable");
await client.send("Network.setRequestInterception", {
  // The HeadersReceived stage allows us to intercept the response body
  patterns: [{ urlPattern: "*", interceptionStage: "HeadersReceived" }],
});
client.on(
  "Network.requestIntercepted",
  async ({ interceptionId, request, responseStatusCode, responseHeaders }) => {
    // Record the size of the response body
    const response = await client.send(
      "Network.getResponseBodyForInterception",
      { interceptionId }
    );
    const body = response.base64Encoded ? atob(response.body) : response.body;
    const bytes = Buffer.byteLength(body);
    bytesReceived += bytes;
    console.log(`${request.url} = ${bytes} bytes`);

    // Continue the response without modification
    const status = `${responseStatusCode} ${http.STATUS_CODES[responseStatusCode]}`;
    const headers = Object.entries(responseHeaders)
      .map(([key, value]) => `${key}: ${value}`)
      .join("\r\n");
    const rawResponse = `HTTP/1.1 ${status}\r\n${headers}\r\n\r\n${body}`;
    client.send("Network.continueInterceptedRequest", {
      interceptionId,
      rawResponse: btoa(rawResponse),
    });
  }
);

// Benchmark all project built sizes, writing results to a .csv file
const csv = fs.createWriteStream(getOutputPath("size"));
csv.write("name,bytes\n");
const BASE_URL = `http://localhost:${PORT}`;
for (const [name, path] of Object.entries(PROJECTS)) {
  // Reset total bytes intercepted
  bytesReceived = 0;
  // Navigate to URL, waiting until no more network requests for > 500ms
  await page.goto(`${BASE_URL}${path}`, { waitUntil: "networkidle0" });
  await waitForScriptReady(page);
  // Record total bytes intercepted, no need to repeat this as size constant
  csv.write(`${name},${bytesReceived}\n`);
  console.log(`${name}: ${bytesReceived}`);
}
csv.end();

await browser.close();
await server.close();
