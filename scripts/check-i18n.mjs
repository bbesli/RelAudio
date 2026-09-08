#!/usr/bin/env node
/**
 * Çeviri bütünlüğü denetimi.
 *
 * Bir dilde eksik anahtar kalırsa arayüz sessizce İngilizce'ye düşer ve
 * kimse fark etmez. Bu betik onu derlemeden önce yakalar.
 *
 *   node scripts/check-i18n.mjs
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = readFileSync(join(root, "app/src/lib/i18n.ts"), "utf8");

// `const xx: Dict = { ... };` bloklarını ayıkla
const dicts = {};
const re = /const (\w+): Dict = \{([\s\S]*?)\n\};/g;
let m;
while ((m = re.exec(src))) {
  const keys = [...m[2].matchAll(/"([\w.]+)":/g)].map((k) => k[1]);
  dicts[m[1]] = new Set(keys);
}

const names = Object.keys(dicts);
if (names.length === 0) {
  console.error("HATA: hiç sözlük bulunamadı — i18n.ts biçimi değişmiş olabilir");
  process.exit(1);
}

const reference = dicts.en;
let failed = false;

for (const name of names) {
  const missing = [...reference].filter((k) => !dicts[name].has(k));
  const extra = [...dicts[name]].filter((k) => !reference.has(k));
  if (missing.length || extra.length) {
    failed = true;
    console.error(`\n${name}:`);
    if (missing.length) console.error(`  eksik (${missing.length}): ${missing.join(", ")}`);
    if (extra.length) console.error(`  fazladan (${extra.length}): ${extra.join(", ")}`);
  }
}

if (failed) {
  console.error("\ni18n denetimi BAŞARISIZ");
  process.exit(1);
}
console.log(`i18n tamam — ${names.length} dil, her birinde ${reference.size} anahtar`);
