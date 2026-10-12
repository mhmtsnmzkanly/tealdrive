# TealDrive UI geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Kullanıcı notları ve durum

Custom yok/uygunsuz ise gömülü UI'ye dönüş kesin davranıştır. Tek fallback için fallback_ui alanını kaldırma önerildi, henüz ayrıca kesinleşmedi. required_sdk_major her özellikte artmaz; [SDK.md](SDK.md).

Login ve file-manager kaynakları ayrılır; ayrı auth binary zorunlu değildir. CSP/parola temizliği [SECURITY.md](SECURITY.md). Custom UI root onaylı güvenilen koddur; sabit SDK oturumla işlem yetkisini sandbox'lamaz.


## 6. Custom UI paketi

```text
ui/custom/
├── manifest.json
├── index.html
└── assets/
    ├── main.js
    ├── style.css
    └── locales/
        ├── tr.json
        └── en.json
```

```json
{
  "manifestVersion": 1,
  "name": "community-theme",
  "version": "1.0.0",
  "sdkMajor": 1,
  "entry": "index.html",
  "assets": [
    "assets/main.js",
    "assets/style.css",
    "assets/locales/tr.json",
    "assets/locales/en.json"
  ]
}
```

Custom HTML kendi JS'sini paket route'undan yükler; bu JS `/sdk/v1/tealdrive.js` import eder. `/sdk/` yolu paketten override edilemez. Paket sürümü atomik aktive edilir; eksik/bozuk/uyumsuz pakette tamamı gömülü UI'ye döner. Manifest validation, scriptin iyi niyetli olduğunu kanıtlamaz: Custom UI aynı origin'de aktif oturumu kullanabilen güvenilen uygulama kodudur. Bu nedenle sıradan kullanıcıdan paket upload/activation yoktur.

Default UI yolu custom JS runtime hatasında da açılabilir. CSP script inline/eval kullanımını engeller; dış CDN veya internet bağlantısı zorunlu olmaz. Default UI bile ayrı static kaynaklardan derleme sırasında gömülür.



### 1. Kullanıcı uyarıları ve parola alanı temizliği — küçük

- Çalışan uygulama/dosya çakışması bilgi notu; chmod için kalıcı açıklama ve riskli seçime onay.
- Dosya ve klasörde izin bitlerinin anlamını doğru açıkla; SUID/SGID/sticky yasağını koru.
- Login başarı/hatasında parola alanını temizle.
- Kabul: Mobil/masaüstü, checkbox/kod eşdeğer davranış; iptalde işlem yok.



### 2. Gömülü UI kaynaklarını ayırma — küçük/orta

- `assets.rs` içeriğini `ui/static/` benzeri HTML/CSS/JS/SVG dosyalarına taşı; `include_str!`/`include_bytes!` ile gömülü dağıtımı koru.
- Login ve dosya yöneticisini `auth` ve `file-manager` modülleri olarak ayır; ayrı sunucu/binary gerekmez.
- Mevcut cache sürümlemesi ve browser testlerini güncelle.
- Kabul: Varsayılan UI harici asset gerektirmeden çalışır; görünüm/işlev korunur.



### 10. Güvenli custom UI — büyük

- `custom_ui=false` varsayılanı ve `custom_ui_path`; mevcut `static_dir` için migration/deprecation.
- Paket manifest'inde UI/SDK uyumu, başlangıç dosyası ve asset listesi; boyut/type sınırları, güvenilir sahiplik, symlink/path escape denetimi.
- HTML/CSS ve sunum JS'si değiştirilebilir; sunum JS'sinin SDK çağrısı yapması normaldir. Sabit SDK bir güvenlik izolasyonu değildir: Aynı origin'de custom JS oturumla işlem yapabilir.
- Custom UI yükleme root/kurulum güven kararıdır; sıradan kullanıcı yazılabilir home veya kullanıcı upload dizininden çalıştırılmaz.
- Uygun olmayan pakette gömülü UI'ye bütün paket olarak dön; dosya bazında iki sürümü karıştırma. Hata teknik log ve yetkili durum ekranında görünür.
- Statik denetim her kötü niyetli veya bozuk scripti anlayamaz; custom UI güvenilen uygulama kodu sayılır. Paket runtime çöküşü için manuel default UI yolu sağlanır.
- İnternetsiz çalışmak için CDN/font/çeviri/SDK dış bağımlılığı şart koşma.
- Kabul: Geçerli paket; eksik/uyumsuz/kaçış girişimli paket; SDK override reddi; default UI'ye dönüş.


## SDK gezinti/state güncellemesi

2026-10-12 yeni server state, helper path doğrulaması, üç SDK alanı ve event/kalıcı bildirim sözleşmesi [STATE.md](STATE.md) altında. Bu hedef henüz uygulanmış değildir; ilgili kabul testleri orada tanımlanır.

## Uygulandı — kaynak dosyaları

HTML/CSS/JS/SVG src/ui/static altında ayrı dosyalardır; assets.rs include_str ile gömer. Browser contract ve shutdown testleri geçti (TEST_RESULTS). Auth/file-manager modül ayrımı ve SDK entegrasyonu sonraki işlerdir.
