# TealDrive yeni geliştirme belgeleri

Bu dizin hedef mimarinin yetkili geliştirme kaynağıdır. Kökteki plan.md/TASARIM.md kaldırıldı; aynı gereksinimi iki yerde güncellemeyin. Mevcut kullanım belgeleri [docs](../), örnekler [deployment](../deployment/) altında; yeni şema ve komutlar henüz uygulanmış değildir.

| Belge | İçerik |
|---|---|
| [IMPLEMENTATION_DECISIONS.md](IMPLEMENTATION_DECISIONS.md) | Uygulamada seçilen kararlar ve gerekçeler |
| [TEST_RESULTS.md](TEST_RESULTS.md) | Yeni geliştirme test kanıtları |
| [PLAN.md](PLAN.md) | İş sırası ve bağımlılıklar |
| [DECISIONS.md](DECISIONS.md) | Kesinleşen kararlar ve kullanıcı notları |
| [HELPER.md](HELPER.md) | Root sınırı, home, kimlik, politika, dosya revision |
| [SERVER.md](SERVER.md) | Servis, TLS/proxy/HTTP, otomatik reload |
| [WIZARD.md](WIZARD.md) | Kurulum/yönetim/kullanıcı CLI ve örnek komutlar |
| [STATE.md](STATE.md) | Server yönetimli gezinti/state, event ve kalıcı bildirimler |
| [SDK.md](SDK.md) | Handshake, JS API, i18n ve uyum |
| [UI.md](UI.md) | Gömülü/custom UI, manifest, uyarılar |
| [CONFIGURATION.md](CONFIGURATION.md) | Kritik/web ayarları ve örnekler |
| [STORAGE.md](STORAGE.md) | Dizin/izinler, SQLite ve kullanıcı verisi |
| [AUTH.md](AUTH.md) | PAM/web parola, enrollment ve öz kayıt |
| [QUOTA.md](QUOTA.md) | Havuz/admin bütçesi, helper kullanım ve uyarı |
| [AUDIT.md](AUDIT.md) | Kullanıcı geçmişi, eski/yeni farklar, trash/undo |
| [SECURITY.md](SECURITY.md) | Ortak güven sınırları |
| [TESTS.md](TESTS.md) | Temiz kurulum ve kabul testleri |
| [CONTEXT.md](CONTEXT.md) | Mevcut canlı kurulum, geçmiş testler ve devam notları |
| [sdk-api.js](../../sdk-api.js) | 56 kullanım/JSDoc örneği; implementasyon değil |

## Okuma ve güncelleme düzeni

1. Dizin, kararlar, güvenlik ve planı okuyun; sonra ilgili bileşen sözleşmesi.
2. Yeni kullanıcı notunu ilgili belgeye Karar bekliyor olarak ekleyin; soruyu config/kod içine koymayın.
3. Kesin kararı DECISIONS, ayrıntısını tek ilgili belgeye yazın; bağımlı API/CLI/config örneklerini birlikte güncelleyin.
4. Özellik yalnızca kod/test/commit kanıtıyla tamamlandı sayılır.

## 1. Bileşenler ve kimlik modeli

```text
Tarayıcı
  ├── Gömülü UI / root onaylı custom UI
  └── TealDrive JS SDK
           │ aynı origin HTTPS + HTTP API / binary WebSocket
           ▼
     tealdrive-server                 girişe kapalı tealdrive UID
           │ sınırlı yerel IPC
           ▼
     tealdrive-helper                 işlem başına; root başlangıcı
           ├── kimlik / politika / yetki kontrolü
           ├── sınırlı yönetim işlemi (yetki varsa)
           └── kalıcı UID/GID düşürme → sandbox → kullanıcı dosya işlemi

SSH oturumundaki kullanıcı
  └── tealdrive-wizard init/revoke/policy
           └── helper → gerçek çağıran UID ile kendi hesabı

Root
  └── tealdrive-wizard install/config/admin/pool/update/uninstall
```

- Helper dosya motoru root olarak içerik/arşiv işleyemez. Yönetim komutları ayrı komut sınıfıdır; keyfi shell, dosya yazma veya UID seçme arayüzü değildir.
- Sistem kullanıcısı: Mevcut Linux hesabı, gerçek NSS home/UID/GID; OS kotası gösterilir.
- Sanal kullanıcı: TealDrive tarafından sağlanan ayrı Linux UID/GID, nologin shell ve kilitli OS parolası; web parolasıyla giriş.
- Sanal admin: Root'un oluşturduğu yönetim hesabı; sanal alt hesap yönetebilir, admin oluşturamaz. Admin kendi dosyalarına da erişecekse bunların kotası ayrıca atanır; yönetim bütçesi otomatik kişisel disk kotası değildir.
- Panel admini Linux root olmaz. Kimlikler sabit TealDrive hesap ID'siyle ilişkilendirilir; username/UID tek başına eski hesabın kaydını devralmaya yetmez.


## 1. Kesinleşen hedefler

- Mevcut Linux kullanıcıları kendi home dizinlerinden başlayarak dosyalarını webden yönetebilecek. PHP/Node.js gibi çalışan projeler desteklenen kullanım; çakışma bilgi notuyla açıklanacak.
- `tealdrive`, girişe kapalı servis hesabı olacak. Binary, ayar ve uygulama verileri `/home/tealdrive` altında toplanacak; güvenlik dosyalarının sahipliği root'ta kalacak.
- Tek ortak helper, işlem başına çalışma, bağımsız kimlik doğrulama ve UID/GID düşürme modeli korunacak. Kullanıcı başına binary oluşturulmayacak.
- Kullanıcıya özel görme/okuma/yazma/chmod vb. politikalar olacak. Web panel yöneticisi ve kullanıcı Wizard üzerinden kendi yetki sınırları içinde ayar yapabilecek.
- Kritik ayarlar server tarafından değiştirilemeyecek. Root kontrollü ana ayarlar ile webden değiştirilebilir ayarlar ayrılacak.
- Sanal hesaplar yalnızca TealDrive için kullanılacak; **her birine ayrı, girişe kapalı Linux UID/GID verilecek**. Ortak UID ile izolasyon yapılmayacak.
- Uygun mevcut hesapların Wizard ile kendi web erişimini açması **varsayılan açık** olacak; root bunu kapatabilecek. Root/servis hesapları otomatik uygun sayılmayacak.
- Linux kullanıcıları için PAM veya ilk PAM sonrası ayrı web parolası seçimi kurulum ayarı olacak. Sanal kullanıcılar web parolası kullanacak.
- Yönetim aracının adı **TealDrive Wizard**; kurulum, güncelleme, kaldırma, hesap ve öz kayıt işlemleri sunacak.
- Caddy veya herhangi bir proxy zorunlu olmayacak; internetsiz kullanım desteklenecek.
- Gömülü UI kalacak; doğrulanan custom UI seçeneği ve gömülü UI'ye geri dönüş olacak. İletişim SDK'sı özelleştirilebilir UI'den ayrılacak.
- UI i18n destekleyecek; server kullanıcıya gösterilecek çevrilmiş cümleler yerine kararlı kodlar ve yapılandırılmış parametreler döndürecek.
- Genel server, kullanıcı işlem ve güvenlik kayıtları ayrılacak; chmod gibi değişikliklerde eski/yeni değerler kaydedilecek.


## 2. Kodla karşılaştırma

| Alan | Şu an | Gerekli değişiklik |
|---|---|---|
| Binaryler | `Cargo.toml`: server ve helper | Wizard eklenmesi; API SDK ayrı JS varlığı, ayrı binary değil |
| Home | `auth` home bilgisini alıyor; `roots` sabit | Helper'ın gerçek hesap home'undan kök üretmesi |
| Politikalar | `helper/policy.rs`: genel root tavanı | Kullanıcı politikası, kişisel kısıtlar ve işlem bazında birleşim |
| Config yazma | `admin/api.rs` → `apply_patch_saved`; server config'i yazabiliyor | Kritik/çalışma ayarlarını ayırma; kritik alanları PATCH'ten çıkarma |
| Helper güvenilir yolları | `helper/security.rs`: `/etc/tealdrive` sabit | Yeni root korumalı ana dizin ve ebeveyn denetimleri |
| UI | `ui/assets.rs`: HTML/CSS/JS sabitleri | Ayrı kaynak dosyaları; yine derleme sırasında gömme |
| Özel UI | `server.static_dir`, `ui/mod.rs`: app.js dahil değiştirilebiliyor | `custom_ui`, `custom_ui_path`, manifest/sürüm denetimi; SDK override yasağı |
| Protokol | Rust kontratı ve `ui/protocol.js` var | Yüksek seviyeli kararlı SDK, yetenek/sürüm pazarlığı, i18n kodları |
| Login | Aynı UI'de login ve dosya yöneticisi; backend `src/auth`, `src/fs`, `src/ws` ayrı | `auth` ve `file-manager` UI modülleri; gerekirse ayrı sayfalar |
| Chmod | Checkbox/kod seçici var; özel bitler yasak | Uyarı/onay; helper'dan gerçek eski/yeni mode audit'i |
| Editör | Beklenen dosya sürümü koşulu yok | Okunan içeriğin hash'ı ve kaydetme sırasında sürüm doğrulaması |
| Audit | JSONL, rotation, işlem/HTTP/transfer bağlamı var; tek sink | Eski/yeni metadata; ayrılmış hedefler, bağımsız helper doğrulaması |
| Giriş sınırı | Bellekte kullanıcı+IP | Hesap/genel sınır, kalıcılık, sınırlı kayıt büyüklüğü |
| Hesaplar | Linux/PAM ve açık allowlist | Sanal hesap, kontrollü öz kayıt, kalıcı kayıt durumu |
| Kota | Config kökü bazında uygulama kotası | Sanal hesaba TealDrive kotası; Linux hesabında OS kotası gösterimi |
| Kurulum | Elle komutlar; Caddy odaklı | Wizard ve proxy'siz/proxy'li kabul testleri |

