# Kararlar ve kullanıcı notları

[Dizin](README.md) · [İş planı](PLAN.md)

Örnek değer/öneriler otomatik kullanıcı onayı değildir.

## 7. Son kullanıcı cevaplarıyla kesinleşen kararlar

1. **Öz kayıt ve allowed_users:** Hesaplar başlangıçta web erişimine kapalıdır. Öz kayıt ayarı açıkken uygun sistem kullanıcısı Wizard ile yalnızca kendini etkin `allowed_users` listesine ekleyebilir. Root/admin engeli, servis/root hesap filtreleri ve uygunluk tavanı aşılamaz. Listede olmak admin yetkisi sağlamaz. Bu uygulanabilir; kimlik/politika temeli tamamlandıktan sonra açılır.
2. **Admin/sanal kota:** Admini yalnızca root oluşturur. Admin sanal alt hesap oluşturabilir; başka admin oluşturamaz. Root ortak sanal depolama havuzunu belirler; admin bütçeleri bu havuzdan, alt hesap kotaları ilgili admin bütçesinden ayrılır. Gerçek Linux kullanıcılarının kotası OS/root tarafından yönetilir.
3. **TLS:** Zorunluluk ve sertifika/anahtar yolları ayardır; mevcut dosyalar program tarafından okunur. İnternetsiz ortamda yerel CA desteklenir. Özel browser şifrelemesi TLS'nin yerini almaz. HTTP kullanımına ilişkin açık seçim ve loopback varsayılanı korunur.
4. **Audit arızası:** Kritik hesap/parola/yetki işlemlerinde audit zorunlu; yazılamıyorsa işlem reddedilir. Normal dosya işlemleri için yapılandırılabilir zorunluluk. Bir işlem sonrası audit hatası, işlemin hiç yapılmadığı şeklinde raporlanmaz.
5. **Trash/geri alma:** Dosya erişimi, hash/sürüm doğrulaması ve geri alma helper'ın sorumluluğudur; server arayüz ve istek akışını yönetir. Beklenen durum geçerli değilse işlem geçmişte görünür ama geri alınamaz. Kullanıcı panelden kendi audit geçmişini görebilir.
6. **Hassas dosyalar:** `.ssh` ve sistem düzeyindeki kimlik/anahtar öğeleri varsayılan kapalı; `.env` gibi proje dosyaları açık. Root/admin ve kişisel kurallarla sonradan daraltılabilir.
7. **Destek matrisi:** İlk sürüm Debian/Ubuntu yerel hesapları + TealDrive sanal hesapları. SFTP-only/nologin gerçek hesaplar açık root izniyle; LDAP entegrasyonu sonraki aşama.
8. **Revoke:** Kullanıcı kendi oturum/biletlerini iptal edebilir; uygunluk sürüyorsa yeniden `init` yapabilir. Root/admin engeli aşılamaz; yeniden kayıt eski parolayı/biletleri otomatik canlandırmaz.
9. **API:** Mevcut API kritik sorun dışında korunur; yeni özellikler eklenebilir. Custom UI'de eksik yeni özellik tema geliştiricisinin sorumluluğudur.


## TASARIM içine eklenen kullanıcı notları — korunmuş metin

- Birde reload ekleyelim, config dosyası değişince otomatik ayarlar güncellensin, mimariyi buna göre güncellersin/değiştirirsin
- istediğime yakın, benim istediğim daha çok "pam_enroll_then_web = true|false" tarzında ama aşağıdaki yöntem ileride geliştirmeyi kolaylaştırıyorsa kalabilir
- güzel fikir ama gerek var mı? custom yok -> fallback (binary gömülü olan sayfa)
- güzel fikir, versiyonlama için her seferind 1 artırma mı kullanacaksın?
- güzel fikir, ama ben loglar o kullanıcının belleğinden yesin istiyorum. bu yöntem daha güvenliyse kalabilir

## Notların durumu

| Not | Durum | Sözleşme |
|---|---|---|
| Otomatik config/reload | Kullanıcı gereksinimi; canlı/restart alanları ayrılacak | [SERVER.md](SERVER.md) |
| Boolean/enum auth modu | Enum genişletilebilir öneri; kullanıcı kalabileceğini belirtti | [CONFIGURATION.md](CONFIGURATION.md) |
| fallback_ui alanı | Tek fallback gömülü; gereksiz alanı kaldırma önerisi henüz ayrı kesinleşmedi | [UI.md](UI.md) |
| SDK major artışı | Uyumsuz değişimde major, uyumlu özellikte minor | [SDK.md](SDK.md) |
| Kullanıcı logu kendi belleğinden yesin | Disk/kota olarak yorumlandı; merkezi korumalı log + sanal muhasebe mümkün. Gerçek OS kotası desteği ve nihai karar bekliyor | [AUDIT.md](AUDIT.md), [QUOTA.md](QUOTA.md) |

## Son kesinleşen ayrıntılar

- Hibrit: Yönetim/kota SQLite, kullanıcı içeriği filesystem.
- Kullanım helper'dan, kota DB'den; örnek ayarlanabilir warning %80. Ölçüm hatası sıfır değildir.
- SDK API.handshake başlangıcı; TLS taşıma, login kimlik. Kullanım dosyası sdk-api.js.
- Plan/tasarım docs/intgr bileşenlere bölündü; root plan.md/TASARIM.md kullanıcı talebiyle kaldırıldı.

## Açık değerler

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

## 2026-10-12 SDK/state kullanıcı kararı

- SDK üç ana iş alanı: auth, user, admin. Ortak dosya/audit/quota alt grupları user altında.
- Sanal kullanıcı kendi hesabını revoke edemez; admin yapar. Logout ile revoke ayrıdır.
- State server yönetir, helper gerçek home/path/dosya sürümünü doğrular; SDK salt okunur yansıma/event/observer sunar. State varsayılan kullanım önerisi, özel UI kendi state'ini de kullanabilir.
- Etki-tepki event ile kalıcı notification condition ayrı; %80 event adında sabit değil parametredir.
- API.handshake başlangıç, HTTPS/WSS tarayıcı taşıması; Origin/TLS/policy sunucu tarafından denetlenir.
- Açık soru: Sekme başına state mi, kullanıcı başına ortak mı? Yeni girişte son konum da hatırlansın mı? Yanıt gelmeden default kesinleşmez.
- Ayrıntı ve sınırlar [STATE.md](STATE.md). Yeni feature uygulanmış değildir.

## 2026-10-12 handshake düzeltmesi — kesin kullanıcı akışı

`handshake → websocket bağlı/protokol hazır → login denemesi → authenticated`. Önceki HTTP-discovery/login-sonrası-WS önerisi geçersizdir. Login öncesi WS sınırlı handshake/auth mesajlarına açıktır; dosya/admin yetkisi vermez. İleride anahtar anlaşması eklemek için sürümlü handshake aşamaları tasarlanır; PGP/protokol/kütüphane seçimi henüz yapılmadı. [STATE.md](STATE.md), [SDK.md](SDK.md).

## 2026-10-12 ek kararlar

- Yasaklı dosyalar helper dosya motoru tarafından sadece atlanmasın; işletim sistemi erişim sınırı ve görünüm izolasyonu hedeflensin. Prototip/test sonrası sağlanabilen garanti açıkça belirtilsin. [HELPER.md](HELPER.md).
- SDK baştan geniş kapsamlı tasarlansın; sonraki eklemeler eski UI/istemci kodunu bozmasın. Üç namespace, ortak tip/hata/state/event sözleşmeleri ve contract testleri. Gelecekte hiç özellik eklenmeyeceği garantisi değil, geriye uyum gereksinimidir. [SDK.md](SDK.md).

## Admin yönetim alanları ayrımı — 2026-10-12

Sistem ve sanal hesaplar ayrı yönetim yüzeyi: admin.systemUsers / admin.virtualUsers. Birincisi mevcut Linux hesabının web izinleri; ikincisi yöneticinin bütçe/kapsamındaki sanal alt hesap yaşam döngüsü. Root admin yaratır; panel admini başka admin veya keyfi OS hesap yetkisi yaratamaz. Account kind/owner kontrolü helper'da bağımsızdır. [SDK.md](SDK.md).

## SDK genişletilebilir seçenekler — 2026-10-12

Kullanıcı örneği files.list için filter/search seçenekleridir; yöntemler options nesnesiyle genişletilir. Mevcut dizin filtreleme ile recursive search ayrı iş yükleri olarak tasarlanır. Gerçek filtre/yetki helper, input/API yüzeyi SDK, taşıma/state server. Ayrılmış görev listeleri HELPER.md ve SDK.md içindedir.

## Son açıklama / önceki yorumları değiştiren kararlar — 2026-10-12

1. State isteğe bağlı SDK kolaylık katmanıdır; kalıcı server view-state deposu istenmiyor. Stateless core + stateful adapter, helper doğrulamalı sonuçlar. STATE.md güncellendi.
2. Root belirler/admin belirler ayar grupları aktörlerdir; “server” ifadesi panel yöneticisi demekti. Critical OS/tavan yetkisi root, devredilmiş panel ayarları admin.
3. Helper config `.env` gibi öğeyi list dir fonksiyonunda atlar; OS düzeyinde görünüm izolasyonu isteği bu son açıklamayla kapsamdan çıkarıldı. Doğrudan erişim yasağı sorusu yanıt bekliyor; öneri gizli öğenin read/download/archive yolundan da erişilememesi.
4. Handshake karşı taraf anahtarıyla şifreleme isteği MITM/log plaintext kaygısıdır. Ek kripto uçları/doğrulanmış anahtar kaynağı henüz seçilmedi; TLS yerine geçmez. Sırları loglamama kuralı korunur.
5. Kota parallel-write koordinasyonu için mevcut mimariye uygun helper kilit/rezervasyon çözümü tercih edilir; kullanım helper'dan, tahsis DB'den.
6. Geniş SDK: Yaygın özellikleri baştan kapsa; UI geliştiricisi filter gibi ortak işlevleri kendisi yazmak zorunda kalmasın. Compatibility tek başına yeterli değil; kapsam matrisi ve ortak seçenekler gerekir.

Bu bölüm önceki server-owned state ve OS görünmezlik ifadeleri üzerinde önceliklidir; çelişkili eski metinler tarihsel hedef diye okunur, uygulanmaz.

## Kesin kararlar — son kullanıcı yanıtı, 2026-10-12

Bu bölüm önceki açık soru/öneriler üzerinde önceliklidir.

1. Gizlenen öğe web panel/API'de hiç yokmuş gibi davranır. Listeleme/arama/stat/read/download/editor/copy/archive ve undo/audit geçmişinde yasaklı içerik/metadata sızmaz; doğrudan path ile erişim de reddedilir. Bu helper kod/policy korumasıdır; OS görünmezlik iddiası değildir. .env örnek gizleme kuralıdır; global varsayılan .env açık, .ssh kapalı kalır.
2. Ek uygulama şifrelemesi temel istenen özelliklerden sonra ayrı aşama; şimdi TLS/WSS ve sürümlü handshake genişleme noktası. Standart mevcut çözümler araştırılacak; özel kripto icat edilmeyecek.
3. SDK browser storage kullanır, kullanıcı bazında ayrılır. Secret/password/content saklanmaz; restore path helper'da yeniden doğrulanır. Başka kullanıcının state'i gösterilmez. Sekme paylaşımı ve storage türü kullanıcı tarafından ayrıca seçilmedi.
4. Log kota kararı geliştiriciye bırakıldı. Tercih: Merkezi root korumalı kendi-görüntülenebilir işlem loglarını sanal hesabın uygulama kotasına dahil et; gerçek Linux hesaplarında merkezi loglar ayrı sistem bütçesinde kalır. Root güvenlik/server logları kullanıcının kotasına dahil değildir. OS UID kotasına root-owned logu yazmış gibi hesaplama iddiası yapılmaz.
5. Genel varsayılan saklama süresi 21 gün = 504 saat; yeni ayar dosyalarında saat birimi. Audit/trash saklama ve geri alma/silinmiş hesap saklama ayrı ayarlardır; aşağıdaki özel süreler geneli geçersiz kılar.
6. Hesap suspended olunca giriş ve aktif yetki kapatılır, veri silinmez. Adminin hesap silme yetkisi root ayarıyla belirlenir. Silinen hesap verisi varsayılan 36 saat tutulur; süre ayarlanabilir, süre sonrası doğrulanmış purge ayrı kontrollü iş. Gerçek Linux OS hesabı/home'u panel silmesiyle silinmez.
7. İlk undo kapsamı move/rename/delete-to-trash/chmod. Kullanıcı undo süresini 1 saat ile server üst sınırı arasında seçer; server üst sınırının varsayılanı 24 saattir. Kullanıcı 1 saat ile yapılandırılmış server üst sınırı arasında seçim yapar; kullanıcıya ayrıca sabit 24 saat default atanması kesinleşmiş karar değildir.

User/files ana namespace adının kesin onayı hâlâ yok; son örnekler files kullanıyor ve öneri files. Naming ve diğer açık teknik değerler bu kararlarla karıştırılmaz.

## 24 saat açıklaması — kullanıcı düzeltmesi

24 saat kullanıcı tercihi varsayılanı değil, server geri alma üst sınırının varsayılanıdır. Kullanıcı 1..server-max saat seçer; server-max root/admin yetki tavanı içinde yapılandırılır.


## 14 Ekim 2026 — yayın kararları (tarihsel açık maddeleri günceller)

- Üretimde sanal genel panel admininin adı `tealdrive`; servis Linux UID995/nologin hesabından farklı UID/GID ve native virtual home kullanır. Mevcut `tealdrive-admin` parola hash'i ve dosyaları offline doğrulanarak kopyalanır; kaynak home silinmez. Yeni giriş doğrulanınca eski web hesabı suspended yapılır.
- Mevcut net512MiB havuz korunur. Genel admine512MiB bütçe ve bunun içinden sabit128MiB kendi dosya kotası ayrılır; kalan384MiB çocuk hesap tahsislerine açıktır. İmaj fiziksel1GiB kalır. Bu değerler ürünün gömülü varsayılanları değil mevcut kurulum tercihidir.
- Helper/server aktarım tavanı mevcut kurulumda1GiB yapılır; kullanıcının gerçek kotası daha küçükse o sınır da uygulanır. HTTP indirmede yapılandırılmış chunk sınırı ve hard256KiB aşılmaz; her parçada yetki kontrolü devam eder.
- SDK public transfer kimliği standart tireli UUID metnidir. MessagePack'in ikili UUID temsili dış API'ye sızdırılmaz. Gerçek ikili cevapla regression ve reconnect sonrası explicit abort kabul testi yapılır.
- WASM async init tek modül örneğinde eşzamanlı çağrıları birleştirir. TypeScript ES2022 uyumu için opsiyonel üretilmiş Symbol.dispose declaration'ları çıkarılır; free() ve koşullu runtime dispose korunur. Üretim yeniden derleme betiği aynı dönüşümü uygular.
- Önceki açık retention notları artık karar beklemiyor: history504saat, trash504saat, silinen virtual hesap36saat, Root undo üst sınırı varsayılan24saat; kullanıcı1..Root üst sınırı seçer.
- TLS uygulama şifrelemesinin ön koşuludur. Noise standart NK protokolüdür; REST ve native HTTP download TLS kapsamındadır. Bağımsız key pin olmadan güven TLS discovery'dir; bağımsız güvenlik denetimi veya endpoint compromise koruması iddiası yoktur.
- Geliştirme verileri namespace fixture'larında tutulur. Kalıcı faydalı araçlar tools/dev ve tools/ops altında kalır; geçici fixture/derleme çıktıları ve yayın doğrulaması sonrası otomatik güncelleme yedekleri temizlenir. Kaynak kullanıcı home'ları ve audit kayıtları yedek sayılıp silinmez.


### SDK tamamlanma sonucu düzeltmesi

Son public API karşılaştırmasında upload callback/resume dışında success-result transferId'nin de normalize edilmesi gerektiği görüldü. `UploadResult extends OperationResult` bytesWritten/transferId/sha256Verified alanlarını açık tanımlar; success cevabına gerçek wire requestId ve UPLOAD operation eklenir. Bu, önceki OperationResult tüketicisiyle tip uyumunu korur. Rust-generated contract ile binary UUID regression ve strict TypeScript kontrolü, ardından yeni release/public transfer kabulü yapılır. Kalıcı fonksiyon isimleri değişmez.
