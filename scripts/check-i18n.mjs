#!/usr/bin/env node
/**
 * Çeviri bütünlüğü denetimi.
 *
 * İki şeyi denetliyor:
 *
 * 1. **Diller arası tutarlılık.** Bir dilde eksik anahtar kalırsa arayüz
 *    sessizce İngilizce'ye düşer ve kimse fark etmez.
 * 2. **Kod ile sözlük arası tutarlılık.** Kodda `t("x.y")` olarak kullanılan
 *    ama sözlükte hiç olmayan bir anahtar ekranda ham `x.y` olarak görünür.
 *    İlk denetim bunu yakalamıyordu: anahtar hiçbir dilde yoksa diller kendi
 *    aralarında tutarlıdır.
 *
 * Kullanılmayan anahtarlar uyarı olarak listeleniyor (hata değil): dinamik
 * kullanımlar (`remote.${kod}`, `nav.*`) statik olarak görülemiyor.
 *
 *   node scripts/check-i18n.mjs
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = join(root, "app/src");
const src = readFileSync(join(srcDir, "lib/i18n.ts"), "utf8");

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

// — Kodda kullanılan anahtarlar —
function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(svelte|ts)$/.test(name) && name !== "i18n.ts") out.push(p);
  }
  return out;
}
const used = new Set();
for (const file of walk(srcDir)) {
  const text = readFileSync(file, "utf8");
  // t("a.b"), t('a.b'), t(`a.b`) — değişken içermeyen dizeler
  for (const k of text.matchAll(/\bt\(\s*["'`]([\w]+\.[\w.]+)["'`]/g)) used.add(k[1]);
  // { key: "a.b" } — gezinme listesi gibi tablolar
  for (const k of text.matchAll(/\bkey:\s*["']([\w]+\.[\w.]+)["']/g)) used.add(k[1]);
}

const undefinedKeys = [...used].filter((k) => !reference.has(k)).sort();
if (undefinedKeys.length) {
  failed = true;
  console.error(`\nkodda kullanılıp sözlükte olmayan (${undefinedKeys.length}): ${undefinedKeys.join(", ")}`);
}

const DYNAMIC = [/^remote\./, /^nav\./, /^tray\./];
const unused = [...reference]
  .filter((k) => !used.has(k) && !DYNAMIC.some((r) => r.test(k)))
  .sort();
if (unused.length) {
  console.warn(`\nuyarı — kullanılmayan anahtarlar (${unused.length}): ${unused.join(", ")}`);
}

if (failed) {
  console.error("\ni18n denetimi BAŞARISIZ");
  process.exit(1);
}
console.log(`i18n tamam — ${names.length} dil, her birinde ${reference.size} anahtar; kodda ${used.size} anahtar kullanılıyor`);
