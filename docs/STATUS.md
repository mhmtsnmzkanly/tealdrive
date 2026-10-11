# Düzeltme ve mevcut durum raporu

Tarih: 2026-10-11. Proje: TealDrive 0.1.0.

İstenen sekiz mimari düzeltme ayrı commit'lerle uygulandı. Küçük kusurlar ayrı
commit'te toplandı; doküman ve kurulum örnekleri ayrıca commit edildi. Kod, Caddy
üzerinden hedef sunucuda kabul testi yapılabilecek durumdadır. İlk kısıtlı oturumdan sonra bu sunucuda gerçek PAM, setuid helper ve Caddy HTTPS/WSS
akışı da doğrulandı. Tarayıcıda görsel kullanım ve üretim yük testi ayrıca yapılmalı.

## Sekiz mimari düzeltme

| No | Commit | Değişiklik ve sonucu |
|---:|---|---|
| 1 | `3f196b8` | PAM stub kaldırıldı; gerçek PAM/NSS, imzalı kimlik bileti, çağıran UID bağı ve kalıcı UID/GID düşürme getirildi. İstemcinin kimlik beyanı yetki sağlamıyor. |
| 2 | `a482d7a` | HTTP dosya okumaları ve kullanıcı dosyası kontrolleri helper'a taşındı. Servis hesabı yönetilen dosyaları kendi yetkisiyle açmıyor. |
| 3 | `b1b9197` | Landlock zorunlu kernel sınırı, `openat2` kök-altı/symlink yasağı ve descriptor üzerinden chmod eklendi. Kernel sınırı kurulamazsa işlem reddediliyor. |
| 4 | `2f2d91d` | Her işlem için sürümlü, değişmez politika snapshot'ı ve root-owned helper izin tavanı getirildi. Server reload helper yetkilerini genişletemiyor. |
| 5 | `dec6a4f` | Aktif WebSocket/indirme işlemlerine iptal edilebilir oturum lease'i eklendi. Çıkış/süre dolumu sonraki işlemleri engelliyor ve devam eden helper'ı iptal ediyor. |
| 6 | `6491c29` | Bloklayan işlemler sınırlı işçilere taşındı; IPC iki yönlü eşzamanlı okunup yazılıyor. Timeout/iptal child process'i sonlandırıp topluyor. |
| 7 | `ebbbd60` | Kök bazlı process'ler arası kota kilidi, sınırlı yazıcılar ve geçici çıktının atomik yayınlanması getirildi. Başarısız büyüyen işlemler kısmi hedef yayınlamıyor. |
| 8 | `8e9991b` | Rust kaynaklarından browser protokol sabitleri üretiliyor. Header, mesaj kodları, MessagePack, 64-bit offset, UUID ve yükleme ACK akışı uyumlu hale getirildi. |

## Küçük kusurlar — `32d78ba`

- Boş admin listesi herkese yetki vermiyor; üyelik açık ve büyük/küçük harfe duyarlı.
- CSRF token oturumda saklanıyor ve logout/admin PATCH/reload sırasında doğrulanıyor.
- Origin kontrolü tam eşleşme yapıyor; wildcard ayarlar reddediliyor.
- İç içe gizli dizin/dosyalar filtreleniyor. ZIP ve TAR klasör dolaşımı hassas/gizli
  dosya ve symlink politikalarını uyguluyor; extraction hatası hedefi yayınlamıyor.
- Gzip kapanış hataları başarı olarak geçmiyor.
- Yükleme başlangıcındaki SHA-256 bitişte atlanamıyor; başarısız End geçici dosyayı siliyor.
- Upload ve metin düzenleme izinleri ayrıldı. Helper da yazma boyutlarını denetliyor.
- UI ayar alanları gerçek backend adlarıyla eşleşiyor; CSRF başlıkları gönderiliyor;
  admin olmayan kullanıcı için ayarlar kapalı. Sayfalama ve arama türleri düzeltildi.
- İç içe config patch'leri belirtilmeyen değerleri koruyor, null temizleme ve bilinmeyen
  alan reddi var. Kaydetme TOML/JSON uyumlu, atomik ve 0600; hata görünür ve live
  config değişmiyor. Reload ve patch mutasyonları aynı kilitle sıralanıyor.
- Seçilen config dosyası izleniyor; atomik değiştirme sonrasında watcher çalışmaya
  devam ediyor. SIGHUP oturum/limiter/helper timeout ayarlarını da senkronize ediyor.
- Audit timestamp artık sabit 1970 yerine gerçek UTC zamanıdır.
- Gerçek olmayan UI özellikleri kaldırıldı; kullanılmayan otomatik trash temizliği
  kontrolü kapatıldı. Bilinmeyen kota kullanımı 0 olarak gösterilmiyor.
- İlan edilen Rust minimumu kilitli bağımlılıkların gerektirdiği 1.85'e düzeltildi.
- Rustfmt uygulandı ve HTTP entegrasyon testleri soket açmadan router üzerinde
  çalışacak şekilde yenilendi. Gerçek yetki düşürme testi açıkça ayrı tutuldu.

## Doğrulama

İlk kısıtlı ortamda kaynaklar üzerinde yapılan kontroller:

| Kontrol | Sonuç |
|---|---|
| `cargo test --locked --offline` | 497 test başarılı, 0 başarısız, 1 bilinçli ignored |
| Dağılım | 484 library + 5 server CLI + 1 browser + 1 config örneği + 5 entegrasyon + 1 kernel |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | Başarılı |
| `cargo fmt --check` | Başarılı |
| `cargo build --release --locked --offline` | Başarılı; server/helper binary üretildi |
| Server ve helper örneği `--check-config` | İkisi de geçerli |
| Caddyfile `adapt` ve `validate` | Caddy 2.11.7 ile başarılı |
| Landlock escape testi `--nocapture` | Linux 6.8 üzerinde gerçekten çalıştı, skip olmadı |

Browser testi gerçek shipped codec'i Node'da çalıştırır; Rust ServerHello/UUID
cevaplarını ve JavaScript isteklerini karşılıklı çözer. Ham chunk offset'i 4 GiB'den
büyük değerle sınanır. HTTP testleri oturum/CSRF, admin yetkisi, hatalı persist,
statik UI, >1 MiB dosya indirme, byte range, gizli dosya ve güvenli preview'ı kapsar.
Kota testi eşzamanlı sekiz yazarda iki işlemin sınır içinde başarılı olmasını ve
başarısız işlemlerin kısmi çıktı bırakmamasını kontrol eder.

Ignored test gerçek helper subprocess yaşam döngüsünde UID/GID değiştirir. Bu
çalışma ortamında root process'e `CAP_SETUID`/`CAP_SETGID` verilmediği ve
`NoNewPrivs` etkin olduğu için burada çalıştırılamıyor. TCP dinleme de kısıtlı.
Bu ilk kısıt daha sonra kaldırıldı; aşağıdaki sunucu testi bu eksikleri kapatıyor.

## Kullanım sınırları ve kalan geliştirmeler

- Linux/Landlock ABI 3+ zorunlu; eski kernel veya setuid'i engelleyen servis ayarıyla
  helper işlemleri reddedilir. Sistem kurulumu repo dışında yapılmadı.
- Upload varsayılan 1 MiB, en fazla 3 MiB; resume ve sınırsız büyük yükleme yok.
  Metin düzenleme varsayılan 128 KiB. HTTP büyük dosya indirebilir; eski WebSocket
  download yolu 768 KiB ile sınırlı.
- Kota yalnızca helper üzerinden işbirliği yapan yazıcıları sıralar; doğrudan OS
  yazmaları için filesystem quota gerekir. Crash sonrası orphan staging temizliği
  ve tüm işlem tipleri için crash-durable journal yok.
- HTTP download birden fazla helper range okuması kullanır; değişen dosyanın tek
  snapshot'ından okuma ve yüksek throughput optimizasyonu yok.
- Oturumlar bellekte; restart girişleri siler, çoklu instance paylaşımı yok.
- `chown`, otomatik trash retention purge, tam per-file-operation audit ve async
  audit sink sağlanmıyor. MFA/şifre yenileme PAM sohbetleri desteklenmiyor.
- Ayrı Rust 1.85 toolchain, tarayıcıda görsel kullanım ve üretim yük testi çalıştırılmadı.

Kurulum dosyaları [deployment/](../deployment/) içinde. Alan adı, kullanıcı,
PAM stack'i ve dosya izinleriyle birlikte yapılacak kabul testi
[DEPLOYMENT.md](DEPLOYMENT.md) içinde adım adım anlatıldı.

## Güncelleme: bu sunucuda gerçek kullanıcı ve Caddy testi

2026-10-11 tarihinde kısıtsız oturumda `tealdrive-test` (UID/GID 1000) normal hesabı
ve `tealdrive` servis hesabı oluşturuldu. Rastgele test parolası yalnızca root'un
okuyabildiği `/root/tealdrive-test-password` dosyasında; repo veya loglara yazılmadı.

Test kurulumu: `/srv/tealdrive-test`, `/var/lib/tealdrive-test/config.toml`,
`/etc/tealdrive/helper.toml`, 32-byte root-owned helper anahtarı, gerçek PAM servisi
ve `/usr/local/libexec/tealdrive-test-helper` (root-owned 4755). Servis hesabının
0700 kullanıcı veri dizinindeki dosyayı doğrudan okuyamadığı doğrulandı.

Gerçek PAM girişi 200, yanlış parola 401 verdi. Kullanıcının 0600 dosyaları helper
üzerinden okundu; 1.1 MB HTTP indirme ve Range 206 geçti. `.env` ve `/etc/passwd`'a
symlink erişimi 403 ile reddedildi. Gerçek WebSocket bağlantısı ve shipped browser
codec'i kullanılarak handshake, listeleme, klasör oluşturma, upload ACK/End,
metin kaydı, rename, ZIP oluşturma/açma çalıştırıldı. Sonuç içeriği ve dosya UID'si
1000 olarak doğrulandı. CSRF'siz logout 403; geçerli logout 200 ve açık WS kapanışı
başarılıydı.

Aynı akış bağımsız Caddy instance'ı üzerinden `https://localhost:13443` ve WSS ile
tekrarlandı. Yerel CA sertifikası istemcide açıkça doğrulandı (TLS doğrulaması
kapatılmadı), Secure cookie ve büyük dosya streaming kontrol edildi. Mevcut Caddy
servisi, siteleri ve global trust store değiştirilmedi. Test Caddy ve backend
process'leri tamamlanınca kapatıldı; hesaplar ve test kurulum dosyaları sonraki
manuel kullanım için bırakıldı. Bu endpoint şu anda dinlemiyor.

`cargo test --locked --offline -- --include-ignored` ile **498 test geçti;
0 başarısız, 0 atlanan**. Daha önce atlanan gerçek helper subprocess yaşam döngüsü
testi de dahil edildi. Çıktı `/root/tealdrive-real-test-results.log` içinde.

Bu test gerçek HTTPS/WSS istemcisiyle gerçekleştirilmiştir; tarayıcının DOM/görsel
etkileşim testi değildir. Üretim alan adı ve gerçek tarayıcı testi kullanıcıya kalır.

## Yayın: tdrive.novastrum.dev

Kullanıcının talebiyle 2026-10-11 tarihinde `https://tdrive.novastrum.dev` açıldı.
Mevcut Caddyfile'a ayrı site bloğu eklendi; mevcut TLS snippet'i kullanıldı ve
backend `127.0.0.1:13000` adresine yönlendirildi. Caddyfile validate sonrasında
reload edildi. Önceki dosyalar `/root/tealdrive-deploy-backup-20261011-145127`
altında root-only yedeklendi.

`tealdrive.service` oluşturuldu, enabled/active durumda. Unprivileged `tealdrive`
hesabıyla `/usr/local/bin/tealdrive-test-server` ve 4755 helper çalıştırıyor.
Yapılandırma `/var/lib/tealdrive-test/config.toml`, veri `/srv/tealdrive-test`.
Cookie Secure, TLS proxy modu açık ve allowed origin yalnızca yayın alan adı.
Giriş hesabı `tealdrive-test`; parola repo dışında root-only dosyada tutuluyor.

Dış alan adı üzerinden geçerli TLS sertifikasıyla ana sayfa, JavaScript, health,
gerçek PAM girişi, oturum, HTTP Range ve logout doğrulandı. Aynı alan adının WSS
bağlantısıyla handshake, listeleme, klasör oluşturma, upload, save, rename, ZIP,
extraction, UID sahipliği ve çıkışta socket kapanışı tekrar başarılı oldu.
Servisler test sonunda açık bırakıldı. Tarayıcıdaki görsel kullanım testi kullanıcıya kalır.

## İzin seçici arayüzü

Dosya listesindeki Mode aksiyonu Permissions olarak adlandırıldı. İzin penceresine
Owner/Group/Others için Read/Write/Execute checkbox matrisi eklendi. Octal kod ve
checkbox'lar çift yönlü eşleşir; sembolik izin özeti gösterilir. 0000 doğru işlenir;
geçersiz kodlarda Apply kapalıdır. Backend hatası pencereyi kapatmaz; başarı yalnızca
sunucu onayından sonra gösterilir. Özel setuid/setgid/sticky bitleri desteklenmez.
Mevcut backend chmod feature gate'i korunmuştur.

JavaScript davranış kontrolleri (0644, 0755, checkbox değişikliği, 0000, hatalı
kodlar ve hatadan toparlanma), browser contract testi, Clippy ve release build
başarılıdır. Yeni server binary canlı servise yüklenmiş ve HTTPS üzerinden yeni
HTML/JavaScript'in sunulduğu doğrulanmıştır.

Canlı kontrolde eski CDN JavaScript cache'i görüldüğü için gömülü HTML artık CSS
ve JavaScript URL'lerine içerik hash'i ekler. Yeni build yeni asset adresleri üretir;
canlı HTTPS üzerinden HTML'in referans verdiği sürümlü CSS/JS içinde izin seçici
kodunun bulunduğu doğrulanmıştır. Browser contract/HTTP entegrasyon testleri,
Clippy ve release build tekrar başarılıdır.

## Yapılandırılabilir audit log saklama

Admin Settings içindeki Audit Logs bölümüne dosya konumu, MiB cinsinden boyut,
arşiv sayısı ve gün cinsinden yaş sınırı eklendi. TOML/API/UI aynı ayarları kullanır;
PATCH, reload ve dosya izleyici değişiklikleri yeniden başlatmadan uygular. Yeni
hedef açılamaz veya config kaydedilemezse mevcut log ve ayarlar korunur. Dosyalar
0600 ile oluşturulur; JSON satırları bölünmeden boyuta göre döndürülür. Yaş temizliği
log yazılırken çalışır; boşta çalışan bağımsız bir temizleme zamanlayıcısı yoktur.
Konum değişince önceki konumdaki dosyalar taşınmaz veya silinmez. Audit olay kapsamı
bu değişiklikle genişletilmemiştir.

504 test (ignored kernel/helper testleri dahil), Clippy ve release build başarılı.
Altı yeni test eşzamanlı rotasyonu, arşiv sayısı/yaşını, konum değişikliğini, güvenli
hedef açmayı, başarısız config kaydını ve kapalı yaş sınırının TOML roundtrip'ini
kapsar. Server ve helper binary'leri birlikte canlı servise kuruldu. HTTPS/PAM ve
gerçek helper üzerinden mevcut dosyanın indirilmesi doğrulandı. Canlı admin API
ile 1 KiB/iki arşiv sınırı, eski arşiv silme, null yaş sınırının save/reload sonrası
korunması ve geçersiz hedefin reddedilmesi test edildi. Test sonunda canlı ayarlar
`/var/lib/tealdrive-test/audit.jsonl`, 10 MiB, beş arşiv, 30 gün olarak geri yüklendi.

## Kontrollü servis kapanışı

SIGTERM ve SIGINT/Ctrl+C kapanış akışı eklendi: admission kapatılır, aktif işlemler
30 saniyeye kadar tamamlanabilir; kalan işler iptal edilir ve beş saniye temizlik
beklenir. HTTP kapanışına son iki saniye tanınır. WebSocket/upload sahipliği erken
hata/panic dahil geçici dosyaları temizler. HTTP indirmeleri ve blocking helper
işleri ayrıca takip edilir. Helper stdin EOF iptali UID geçişinden sonra da çalışır;
zorunlu çıkış ve SIGKILL durumundaki sınırlar DEPLOYMENT.md'de açıklanmıştır.

Systemd örneği ve canlı unit `KillMode=mixed`, `TimeoutStopSec=45s` kullanır.
Server/helper birlikte canlıya kuruldu. Gerçek HTTPS/WSS üzerinden SIGTERM ve
SIGINT testlerinde bağlantı kapanışı, yarım upload temizliği ve sağlıklı restart
doğrulandı. Kapanış başladıktan sonra kilidi bırakılan gerçek helper işlemi
başarıyla tamamlandı, kullanıcıya sonucu iletti ve doğru UID ile yayınlandı.
Ayrı testte helper timeout'u yalnızca bellekte 60 saniyeye yükseltildi; kilitli
helper 30 saniyelik kapanış süresinden sonra iptal/reap edildi, servis yaklaşık
31 saniyede temiz durdu. Restart sonrası normal helper timeout'u geri yüklendi.
`ServerShutdown` / `Success` kayıtları aktif audit dosyasında doğrulandı.

512 test (ignored helper/kernel testleri dahil), Clippy ve release build başarılı.
Sekiz yeni regression testi admission kapanışını, grace sonrası iptal/üst sınırı,
HTTP stream tamamlama/iptalini, gerçek TCP listener kapanışını, upload unwind
temizliğini ve gerçek helper'ın kilitte beklerken iptalini kapsar.

## Kapanış popup bildirimi

ConnectionStatus'a geriye uyumlu `shutting_down` ve `grace_period_seconds` alanları
eklendi. Bildirim boşta veya helper işi çalışırken gönderilir; aktif işin cevabı
korunur. Kontrollü kapanış close code 1001 / `Server shutting down` kullanır.
Web arayüzü popup gösterir, yeni işleri engeller, editör metnini korur ve tekrar
bağlanma yerine kullanıcının Check availability işlemini bekler. Bildirim dismiss
sonrası tekrar açılmaz; klavye odağı ve eski WebSocket callback'leri denetlenir.
Ağ hatası kontrollü kapanış olarak gösterilmez.

514 test (ignored dahil), Clippy ve release build başarılıdır. Yeni browser
regression testi gerçek SPA kodunu deterministik DOM/WebSocket harness'i içinde
çalıştırır: açık editör, pending işlem cevabı, yeni istek engeli, geç handshake,
Escape/focus, tekrar bildirim, 1001 fallback, erişilebilirlik kontrolü ve eski
socket callback'lerini doğrular. Bu davranış testi gerçek tarayıcıda görsel test
olarak sunulmaz. Rust testi status payload'ının yönünü ve eski map'lerin decode
uyumluluğunu doğrular.

Server/helper canlıya birlikte kuruldu. Gerçek HTTPS/WSS ile boşta popup mesajı,
yarım upload temizliği ve close code/reason doğrulandı. Kilitli helper'a kapanış
bildirimi iki saniyeden önce ulaştı; kilidi bırakınca işlem başarıyla tamamlandı,
cevabı iletildi ve bildirim tekrarlanmadı. Ayrı 30 saniyelik zorunlu iptal testinde
bildirim önceden ulaştı, helper iptal/reap edildi ve normal timeout restart sonrası
geri yüklendi. Sürümlü HTTPS HTML/JavaScript'te popup kodu ve canlı health doğrulandı.

## Audit kapsamı ve güvenilen proxy IP ayarı

Kod/test commit: `d931b78`.

Dosya komutlarının tamamı merkezi başlangıç/sonuç audit kaydından geçiyor.
HTTP ve WebSocket indirmelerinde hazırlık, tamamlanma ve kesilme ayrılıyor.
Upload iptali/kapanış temizliği, malformed protokol mesajları, HTTP/CSRF/yönetici
reddi ve süresi dolan oturumlar da kaydediliyor. Audit yazma hataları journal'a
bildiriliyor; HTTP başlığı ve yönetici ayarları hata sayacını gösteriyor.
Ayrıntılar ve kapsam sınırları: [AUDIT.md](AUDIT.md).

Settings → Client IP / Trusted proxies altında IP/CIDR listesi ve başlık seçimi
eklendi. Varsayılan liste boş; doğrudan bağlantıda başlıkla IP taklidi yapılamıyor.
Canlı kurulum yalnızca loopback Caddy bağlantısını ve `X-Real-IP` başlığını kabul
ediyor. Caddy Cloudflare'dan doğruladığı istemci IP'sini bu başlığın üzerine yazıyor.
Başlıkları taklit ederek doğrudan Caddy'ye bağlanma testi gerçek bağlantı IP'sini
korudu. Sunucu/helper güncellendi; kullanıcı dosyaları korunarak kabul testi yapıldı.

Doğrulama: ignored gerçek-helper testleri dahil 520 test, Clippy uyarısız,
release build; HTTPS/WSS gerçek PAM/setuid helper üzerinden dosya işlemleri,
aktarımlar, yetki reddi, CSRF, IP taklidi, yapılandırma doğrulaması ve logda
parola/cookie/CSRF/dosya içeriği bulunmaması. Tarayıcı görsel testi yapılmadı;
SPA shutdown davranışı Node DOM/socket testleriyle doğrulandı. Canlı sunucuda
kapanış bildirimi, yarım upload temizliği ve aktif helper işleminin tamamlanması
yeniden doğrulandı. Son sağlık kontrolünde audit yazma hata sayacı 0.


## Mobil arayüz düzeltmesi

Dosya listesi 700 piksel ve altında kart düzenine geçiyor; boyut, izin, tarih ve
bütün işlem düğmeleri görünür kalıyor. Masaüstünde tablo düzeni korunuyor; işlem
düğmeleri gerçek tablo hücresinin içindeki ayrı bir kapsayıcıda sarılıyor.
Uzun dosya/yol adları, üst menü, kök seçimi, arama ve footer ekranı genişletmiyor.
Mode alanı dosya türü bitleri yerine izin bitlerini gösteriyor (`0700` gibi).

Ayarlar küçük ekranlarda dikey yerleşiyor; metin/yol alanları tam genişlikte.
Login, izin, ayar, editör ve çöp kutusu pencereleri ekran yüksekliğine sığıyor;
uzun içerik içeriden kaydırılıyor, kapatma ve alt işlem düğmeleri erişilebilir.
Dinamik viewport yüksekliği (`dvh`) ve eski tarayıcılar için `vh` fallback var.

Gerçek headless Chromium ile canlı HTTPS/PAM/WSS bağlantısında 320×568, 375×667,
390×844, 667×375, 768×1024 ve 1280×720 ekranlar test edildi. Dosya listesi,
uzun dosya adı, ayarlar/uzun log yolu, izin seçicisi, editör ve çöp kutusunda yatay
taşma veya ekran dışında modal görülmedi. JavaScript hatası yok. Ayarlar ve dosya
izinleri test sırasında kaydedilmedi; kullanıcı dosyaları değiştirilmedi.
Browser codec/shutdown regresyon testleri, Clippy ve release build geçti.
Son sürüm Caddy üzerinden canlıda yeniden tarayıcı testinden geçirildi.


## Kullanıcı giriş izin listesi

Sunucu ve root-owned helper için `account_policy.allowed_users` zorunlu bir giriş
izin listesi olarak eklendi. Eksik/boş liste bütün girişleri reddediyor; `admin_users`
yönetici yetkisi veriyor, tek başına giriş izni vermiyor. Mevcut UID/root/nologin
kontrolleri listede olan kullanıcılara da uygulanıyor. Canlı iki yapılandırmada
şu an yalnızca `tealdrive-test` izinli; servis hesapları izinli değil.

Settings → Login access alanı sunucu listesini yönetiyor. Helper üst sınırını
sadece sunucudaki root-owned dosyayı düzenleyen sistem yöneticisi genişletebilir.
Listeden çıkarılma yeni HTTP/WS işlemlerini engelliyor; oturum bakım görevi eski
oturumları kaldırıyor, lease iptali yarım upload'ı temizliyor. Helper kendi listesini
eski kimlik biletlerinde de yeniden denetliyor. Audit `AccountNotAllowed` ve
`SessionRevoked` kayıtlarını üretiyor; dışarıya genel giriş hatası gösteriliyor.

525 test (ignored gerçek-helper testleri dahil), Clippy ve release build geçti.
Canlı testte www-data/wwwdata/caddy/tealdrive/root ve listede olmayan hesaplar
reddedildi. Doğru parolaya sahip kullanıcı helper veya sunucu listesinden geçici
çıkarıldığında da reddedildi; mevcut kimlik biletiyle helper kullanımı engellendi.
Mevcut HTTP oturumu, WebSocket kapanışı, yarım upload temizliği ve audit kayıtları
doğrulandı. Test sonunda kalıcı `tealdrive-test` listeleri korundu, servis yeniden
başlatıldı ve izinli kullanıcı girişi doğrulandı. Dosya işlemleri ve 6 ekran boyutunda
Chromium mobil arayüz testleri de geçti. Audit yazma hatası 0.

Eski kurulumlar güncellemeden önce iki yapılandırmaya da izin listesini eklemeli;
ayrıntılar CONFIGURATION.md ve DEPLOYMENT.md içinde.
