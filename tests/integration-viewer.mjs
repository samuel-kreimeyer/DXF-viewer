import path from "node:path";
import { pathToFileURL } from "node:url";

const pageUrl = pathToFileURL(path.resolve("web/index.html")).href;
const fixturePath = path.resolve("fixtures/simple_line.dxf");
const textFixturePath = path.resolve("fixtures/text_entities.dxf");
const ellipseFixturePath = path.resolve("fixtures/ellipse_entities.dxf");
const splineFixturePath = path.resolve("fixtures/spline_entities.dxf");
const hatchFixturePath = path.resolve("fixtures/hatch_entities.dxf");
const insertFixturePath = path.resolve("fixtures/insert_entities.dxf");
const dimensionFixturePath = path.resolve("fixtures/dimension_entities.dxf");
const leaderFixturePath = path.resolve("fixtures/leader_entities.dxf");
const xlineRayFixturePath = path.resolve("fixtures/xline_ray_entities.dxf");

let chromium;
try {
  ({ chromium } = await import("playwright"));
} catch (error) {
  console.error("Playwright is required for integration tests. Install dependencies first.");
  console.error(error.message);
  process.exit(1);
}

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

try {
  await page.goto(pageUrl, { waitUntil: "domcontentloaded" });
  await page.waitForSelector("#viewerCanvas");

  await page.setInputFiles("#fileInput", fixturePath);

  await page.hover("#viewerCanvas");
  await page.mouse.wheel(0, -240);

  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    return (
      window.__viewerDebug &&
      window.__viewerDebug.renderCount > 2 &&
      window.__viewerDebug.entityCount > 0 &&
      window.__viewerDebug.drawnSegments > 0
    );
  });

  await page.setInputFiles("#fileInput", textFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 3 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", ellipseFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 6 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", splineFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 10 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", hatchFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 12 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", insertFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 16 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", dimensionFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 19 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", leaderFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 24 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  await page.setInputFiles("#fileInput", xlineRayFixturePath);
  await page.click("#focusBtn");

  await page.waitForFunction(() => {
    const warnings = Array.from(document.querySelectorAll("#warningList li")).map((el) => el.textContent);
    return (
      window.__viewerDebug &&
      window.__viewerDebug.entityCount >= 27 &&
      window.__viewerDebug.drawnSegments > 0 &&
      warnings.length === 0
    );
  });

  console.log("Integration test passed.");
} finally {
  await browser.close();
}
