# RelAudio ikon seti (1a — "Köprü")

Aksan rengi: `#F5A524` · koyu mürekkep: `#12141A`

## Yerleşim

`app/src-tauri/icons/` içine kopyala:

| Dosya | Kullanım |
|---|---|
| `icon.svg` | Ana master (1024, amber, saydam zemin) |
| `16x16 … 1024x1024.svg` | Boyuta göre optimize edilmiş amber ikon |
| `tile-*.svg` | Amber zeminli uygulama karosu (kurulum sihirbazı, mağaza, README) |
| `tray/tray-{16,20,24,32}-{light,dark,mono}.svg` | Sistem tepsisi. `light` = koyu görev çubuğu, `dark` = aydınlık görev çubuğu, `mono` = `currentColor` |
| `icon-mono.svg`, `icon-mono-16.svg` | `currentColor` master — CSS ile temaya bağlanır |
| `favicon.svg` | `app/static/` içine |

## İki grid var

- **≥ 24px** → 64 birimlik grid (`icon.svg`, `icon-mono.svg`)
- **≤ 20px** → 16 birimlik grid, piksele hizalı sadeleştirme (`icon-mono-16.svg`, `16x16.svg`, `tray/tray-16-*`)

Küçük boyutta ayrı çizim kullanmak zorunlu: 64 gridin yarım-piksel değerleri 16px'e bölündüğünde bulanıklaşıyor.

## Tauri PNG/ICO üretimi

`tauri.conf.json` PNG ve ICO bekliyor. SVG master'dan üret:

```bash
cd app
npx @tauri-apps/cli icon ../icons/tile-1024x1024.svg
```

Bu komut `src-tauri/icons/` altındaki tüm PNG/ICO/ICNS dosyalarını yeniden yazar.
Saydam zeminli sürüm isteniyorsa `icons/1024x1024.svg` kullan.
