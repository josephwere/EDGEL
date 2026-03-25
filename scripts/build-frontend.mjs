import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(".");
const sourceDir = resolve(root, "frontend");
const outputDir = resolve(root, "dist/vercel");
const apiBase = (process.env.EDGEL_API_BASE || "").replace(/\/$/, "");

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });
await cp(sourceDir, outputDir, { recursive: true });

const configSource = await readFile(resolve(sourceDir, "config.js"), "utf8");
const releaseVersion = process.env.EDGEL_VERSION || "v0.1.0";
const deployConfig = `
window.EDGESTUDIO_CONFIG = Object.assign(
  {
    apiBase: ${JSON.stringify(apiBase || "")},
    releaseVersion: ${JSON.stringify(releaseVersion)},
    deployTarget: "vercel"
  },
  window.EDGESTUDIO_CONFIG || {}
);
`;

await writeFile(resolve(outputDir, "config.js"), deployConfig || configSource, "utf8");
await writeFile(
  resolve(outputDir, "deployment.json"),
  JSON.stringify(
    {
      apiBase: apiBase || null,
      releaseVersion,
      generatedAt: new Date().toISOString()
    },
    null,
    2
  ) + "\n",
  "utf8"
);

console.log(`EdgeStudio frontend prepared in ${outputDir}`);
