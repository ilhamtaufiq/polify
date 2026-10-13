// Salin paket WASM hasil `wasm-pack build` ke src/wasm agar bisa diimpor oleh Vite/Astro.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "..", "..", "crates", "polify-wasm", "pkg");
const dest = join(root, "src", "wasm");
const files = ["polify_wasm.js", "polify_wasm_bg.wasm"];

const missing = files.filter((f) => !existsSync(join(src, f)));
if (missing.length) {
  console.error(`File WASM tidak ditemukan di ${src}: ${missing.join(", ")}`);
  console.error("Jalankan dulu: cd crates/polify-wasm && wasm-pack build --target web --release");
  process.exit(1);
}

mkdirSync(dest, { recursive: true });
for (const f of files) {
  copyFileSync(join(src, f), join(dest, f));
}
console.log(`WASM disalin ke ${dest}`);
