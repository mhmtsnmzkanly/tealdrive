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

## Uygulanan auth UI (2026-10-12)

Login artık SDK handshake/WS login kullanır; HTTP'ye parola body göndermez. ServerHello.authentication enrollment/mode'a göre ilk kayıt seçeneği görünür ve password label değişir. Linux verification sonrası username kilitlenir, yeni web password + confirmation istenir. Input'lar ağdan önce ve finally'de temizlenir; mismatch/weak password/proof-expiry/backend hata mesajları gösterilir. Disconnect olursa pending kayıt tekrar başlamalıdır; token storage'a konmaz. Global settings düğmesi Root-verified Linux admin role + root roster ister. Logout503'ü başarı saymaz, yeniden deneme mesajı verir. File-manager UI mevcut transport'tadır; tam SDK dönüşümü/TR-EN işi devam eder.


## Uygulandı — hesap erişim yönetimi

Admin rolünde ve accountManagement capability açıkken **Accounts** düğmesi görünür. UI ayrı embedded ES module `src/ui/static/accounts.js`; bütün hesap işlemleri gerçek SDK kullanır. Linux/global admin system ve virtual gruplarını; tenant admin yalnız kendi virtual grubunu görür. Root gerektiren admin rol atama veya OS UID/home seçimi sunulmaz. Admin hesap satırları Root managed, deleted hesaplar değiştirilemez.

System grubunda uygun yerel NSS adayları kayıtlı hesaplardan ayrılır; dropdown'dan **Approve registration** mevcut Linux hesabı için password'süz User kaydı açar. Bekleyen kullanıcı **Awaiting registration**; PAM + web password tamamlanınca **Active** görünür. Öz kayıt varsayılan açıkken onay zorunlu değildir; kapalıyken Root whitelist/admin onayı geçerlidir. Root deny son karardır. Profile/home otomatik oluşturma bu ekranın tamamlanmış özelliği değildir.

**Suspend/Enable** confirmation içerir; cancel hiçbir mutation çağırmaz. Suspend aktif web bağlantılarını keser; dosyalar/OS hesabı korunur. Yeniden açma eski cookie/grant/proof'u canlandırmaz. Loading/error sırasında stale işlem kontrolleri temizlenir; aynı modal içindeki talepler epoch ile izlenir, close sonrası eski cevap render edilmez. Busy sırasında double-submit ve tab değiştirme kapalıdır. Escape/focus return, Tab focus containment ve mobil ayrı tablo düzeni uygulanmıştır. Failed refresh503 gerçek Chromium'da verify edilmiştir (browser intercept; backend outage kabulü sayılmaz).

Başlangıç file-connection göstergesi **Not connected**: gerçek ProtocolReady alınmadan connected işareti gösterilmez. Native UI testleri login modal kapanışını ve gerçek file socket'i bekler; initial HTML sınıfına dayalı false-positive kaldırılmıştır. SDK/full file-manager dönüşümü ve TR/EN henüz ayrı PLAN işidir.
