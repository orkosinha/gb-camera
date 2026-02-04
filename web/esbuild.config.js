import esbuild from "esbuild";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Directories
const distDir = path.join(__dirname, "dist");
const distPkgDir = path.join(distDir, "pkg");
const wasmPkgDir = path.join(__dirname, "pkg");

// Ensure output directories exist
if (!fs.existsSync(distDir)) {
  fs.mkdirSync(distDir, { recursive: true });
}
if (!fs.existsSync(distPkgDir)) {
  fs.mkdirSync(distPkgDir, { recursive: true });
}

// Copy WASM package files to dist/pkg
const pkgFiles = [
  "gb_emu_bg.wasm",
  "gb_emu.js",
  "gb_emu.d.ts",
  "gb_emu_bg.wasm.d.ts",
];
pkgFiles.forEach((file) => {
  const src = path.join(wasmPkgDir, file);
  const dest = path.join(distPkgDir, file);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, dest);
    console.log(`Copied ${file} to dist/pkg/`);
  }
});

// Copy static files to dist
const staticFiles = ["index.html"];
staticFiles.forEach((file) => {
  const src = path.join(__dirname, file);
  const dest = path.join(distDir, file);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, dest);
    console.log(`Copied ${file} to dist/`);
  }
});

// Bundle debug application (single entry point)
esbuild
  .build({
    entryPoints: [path.join(__dirname, "src", "main.js")],
    bundle: true,
    outfile: path.join(distDir, "debug-bundle.js"),
    format: "esm",
    platform: "browser",
    target: ["es2020"],
    minify: false,
    sourcemap: true,
    external: ["./pkg/gb_emu.js"],
  })
  .then(() => {
    console.log("Built debug-bundle.js");
  })
  .catch((err) => {
    console.error("Build failed:", err);
    process.exit(1);
  });
