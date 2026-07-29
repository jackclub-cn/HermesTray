import { readFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

// Read version from package.json
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf-8"));
const version = pkg.version;
if (!version) {
  console.error("No version found in package.json");
  process.exit(1);
}

// ── Update Cargo.toml ──
const cargoPath = join(root, "src-tauri", "Cargo.toml");
let cargo = readFileSync(cargoPath, "utf-8");
cargo = cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
writeFileSync(cargoPath, cargo);
console.log(`  ✓ src-tauri/Cargo.toml → ${version}`);

// ── Update tauri.conf.json ──
const tauriConfPath = join(root, "src-tauri", "tauri.conf.json");
let tauriConf = readFileSync(tauriConfPath, "utf-8");
const conf = JSON.parse(tauriConf);
conf.version = version;
writeFileSync(tauriConfPath, JSON.stringify(conf, null, 2) + "\n");
console.log(`  ✓ src-tauri/tauri.conf.json → ${version}`);

console.log(`\nAll version references synced to ${version}`);
