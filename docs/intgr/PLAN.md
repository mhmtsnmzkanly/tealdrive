# TealDrive uygulama sırası ve kabul planı

[Dizin](README.md) · [Kararlar](DECISIONS.md) · [Testler](TESTS.md)

Bütün maddeler plan aşamasındadır; her mantıksal değişiklik ilgili test/ayrı commit ile yapılır. Küçükten büyüğe sıralama yaklaşık boyuttur; güvenlik bağımlılığı farklı olabilir.

| No | İş | Sözleşme |
|---|---|---|
| 1 | Kullanıcı uyarıları ve parola alanı temizliği — küçük | [UI.md](UI.md) |
| 2 | Gömülü UI kaynaklarını ayırma — küçük/orta | [UI.md](UI.md) |
| 3 | CSP ve parola belleği güçlendirmesi — orta | [SECURITY.md](SECURITY.md) |
| 4 | i18n ve hata sözleşmesi — orta | [SDK.md](SDK.md) |
| 5 | Audit eski/yeni değerleri ve kayıt ayrımı — orta/büyük | [AUDIT.md](AUDIT.md) |
| 6 | Kalıcı giriş deneme sınırları — orta | [AUTH.md](AUTH.md) |
| 7 | Hash ile editör çakışma kontrolü — orta/büyük | [HELPER.md](HELPER.md) |
| 8 | Merkezi dizin ve kritik/çalışma ayarlarının ayrılması — orta/büyük | [CONFIGURATION.md](CONFIGURATION.md) |
| 9 | Kararlı JS SDK ve sürümlenmiş API — büyük | [SDK.md](SDK.md) |
| 10 | Güvenli custom UI — büyük | [UI.md](UI.md) |
| 11 | Kullanıcı politikaları ve home kökleri — büyük | [HELPER.md](HELPER.md) |
| 12 | Proxy'siz ve internetsiz çalışma — büyük | [SERVER.md](SERVER.md) |
| 13 | Seçilebilir kimlik doğrulama ve öz kayıt — çok büyük | [AUTH.md](AUTH.md) |
| 14 | Sanal hesaplar, kota ve kullanıcı yaşam döngüsü — çok büyük | [QUOTA.md](QUOTA.md) |
| 15 | TealDrive Wizard — çok büyük | [WIZARD.md](WIZARD.md) |
| 16 | Son kabul ve dokümantasyon — büyük ve bütünleşik | [TESTS.md](TESTS.md) |
| 17 | Otomatik config reload / Wizard reload | [SERVER.md](SERVER.md), [WIZARD.md](WIZARD.md) |
| 18 | Beklenen duruma bağlı geri alma | [AUDIT.md](AUDIT.md) |
| 19 | İsteğe bağlı stateful SDK, navigation ve kalıcı bildirim | [STATE.md](STATE.md), [SDK.md](SDK.md) |
| 20 | Config tabanlı helper listeleme filtresi ve ortak erişim politikası | [HELPER.md](HELPER.md), [SECURITY.md](SECURITY.md) |
| 21 | Geniş SDK kapsam matrisi, ortak tipler ve geriye uyum contract testleri | [SDK.md](SDK.md), [TESTS.md](TESTS.md) |

## 6. Uygulama bağımlılıkları

1. Kimlik/yetki şeması ve aşağıdaki açık kararları kesinleştir.
2. UI kaynak ayrımı, hata/i18n sözleşmesi ve küçük korumalar.
3. Merkezi dizin + config ayrımı + helper yönetim doğrulaması.
4. Kullanıcı politikaları; sonra gerçek home erişimi.
5. Kimlik deposu/öz kayıt; sonra sanal hesap sağlama ve kota.
6. SDK; sonra custom UI paketi. UI ayrımı erken, güvenli custom UI açılması sonra.
7. Wizard ve migration; proxy'siz TLS ve offline kabul işleri bağımsız hazırlanabilir.
8. Temiz kurulum ve güvenlik kabulü; kullanıcı dosyalarını koruyan canlı geçiş.


## 9. Hâlâ açık ayrıntılar

- Geri alma/trash için 30 gün başlangıç önerisi; kesin süre ve log/journal saklama sınırları henüz seçilmedi.
- Admin başına sanal hesap sayısı sınırı ve root bütçeyi azaltınca mevcut tahsisler için davranış. Öneri: Kullanım/tahsis altına sessiz indirme yapılmaz; açık yeniden tahsis gerekir.
- Desteklenecek Debian/Ubuntu sürümleri ve kernel/filesystem matrisi kurulum testleriyle belirlenecek.
- `.ssh` dışındaki sistem düzeyi öğelerin kesin listesi belgelenmeli; tüm dotfile'lar toptan yasaklanmayacak.
- Geri alma temel kabulde mi, ana sürümden sonraki feature mı? Varsayılan plan: Temel altyapısı audit'e hazırlanır, uygulaması ayrı commit/aşama olur.
- MFA, SSH ayar değişikliği, uygulama süreç yönetimi ve çalışan uygulama tespiti kapsam dışıdır.


## 8. Henüz kesinleşmeyen değerler

- Trash/undo retention: Örnek 30 gün, kesin kullanıcı kararı bekleniyor.
- Admin başına sanal hesap sayısı ve bütçe azaltılırken mevcut tahsislerin davranışı.
- UID aralığı, Argon2 maliyet parametreleri, kayıt/transfer üst sınırları: Destek matrisi ve benchmark sonrasında.
- `.ssh` dışındaki varsayılan sistem sır dosyaları listesi.
- Undo'nun ilk ana sürüme dahil edilmesi veya ayrı sonraki feature olması.
- Sertifika yenileme/reload ve kritik config restart davranışının nihai CLI ayrıntıları.

Bu açık değerler yapılandırma ve komut yüzeyinin hazırlanmasına engel değildir; canlı kurulumda örnekler otomatik kesin tercih sayılmaz.

## Uygulama kapıları

- Home erişimi kullanıcı politikaları tamamlanmadan genişletilmez.
- Auth/kota depoları root kontrollü; server doğrudan erişmez.
- SDK başlangıcı handshake; connect eski taslak ifadesidir.
- Deployment taşıması config şemasını değiştirmez.
- Uygulama sonrası ilgili belgede kanıt/durum ve mevcut kod karşılaştırması güncellenir.

Home erişimi devreye girmeden helper filtre/erişim politikaları tamamlanır; SDK implementasyonundan önce kapsam matrisi ve ortak sözleşmeler tamamlanır.

## Son retention/lifecycle/state kararı

Önceki açık 30 gün örnekleri yerine genel 504 saat; deleted sanal hesap verisi 36 saat; kullanıcı undo 1..server-max; server-max varsayılanı 24 saat. Admin delete default'u kesinleşmedi. Gizleme direct access dahil, ek crypto temel özelliklerden sonra; SDK kullanıcı bazlı browser storage. Ayrıntı DECISIONS/CONFIGURATION/AUDIT/STATE. Sanal user log byte'ları kotaya dahil, Linux user merkezi logları sistem bütçesinde.

## 24 saat açıklaması — kullanıcı düzeltmesi

24 saat kullanıcı tercihi varsayılanı değil, server geri alma üst sınırının varsayılanıdır. Kullanıcı 1..server-max saat seçer; server-max root/admin yetki tavanı içinde yapılandırılır.
