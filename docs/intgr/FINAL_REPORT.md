# TealDrive bitiş raporu — 14 Ekim 2026

## Sonuç

Yetkilendirilen iki çekirdek aşaması tamamlandı, ayrı commitlerle kaydedildi ve bu sunucuda canlıya alındı. Aktif paket `20261014-87e5c3e`; dört binary [manifest SHA256](RELEASE_MANIFEST.json) değerleri kurulu dosyalarla eşleşti. Sonraki doküman/test aracı commitleri çalışan binary kodunu değiştirmez. Önceki teslim raporu [tarihsel arşivde](history/FINAL_REPORT-20261013.md) korunur.

Panel: **https://tdrive.novastrum.dev**. Yeni genel panel admini **tealdrive**; mevcut adminin seçilmiş web parolası değiştirilmeden korundu. Parola bu rapora yazılmadı. Eski `tealdrive-admin` web hesabı yeni giriş doğrulandıktan sonra suspended yapıldı. Linux hesabı ve `/home/tealdrive-admin` dosyaları silinmedi.

## 1. UI, SDK ve aktarım

- UI login/cookie bridge başarısızlığında eski bağlantıyı kapatır; yanıltıcı açık oturum bırakmaz. Aktarım ilerleme/iptal/yeniden bağlanma ve anlaşılır hata durumu vardır. Commit sonucu ağda kaybolursa `UPLOAD_COMMIT_UNCONFIRMED` döner; otomatik yeniden yayınlama yapılmaz.
- SDK1.1.0; `auth/files/admin` ayrımı, typed public sözleşme, managed/client state, filter/sort/pagination, event/condition observer, custom transport ve bundler Noise loader desteği korunur. `files.abortUpload` retained aktarımı da aynı session yetkisiyle iptal eder. Public transfer kimliği standart UUID metnidir; ikili wire UUID dış API'ye sızmaz. `UploadResult` byte sayısı/hash doğrulaması/transferId ve operation/requestId alanlarını açık tiplerle sunar.
- WASM modülünün eşzamanlı async initialization yarışı düzeltildi. Bir modülde tek runtime/memory; custom loader için shared promise; stable Rust handles/SHA256 regression ve ES2022 TypeScript tüketici kontrolü vardır.
- Upload Root varsayılanı1GiB, hard ceiling64GiB; production tavanı1GiB. Native kullanıcı kotası, staging bütçesi ve disk alanı ayrıca sınırlar. Server/helper tam dosyayı belleğe almaz. HTTP indirme/Range yapılandırmaya göre en fazla256KiB parçada çalışır ve her parçada yetki kontrolü sürer.
- Aynı HttpOnly session ile kopan yükleme varsayılan1800s içinde devam edebilir. Prefix SHA256, dosya kimliği/boyutu/izin/policy yeniden doğrulanır. Yanlış dosya prefix'i reddedilir. Server restart sonrasında pending upload authority yeniden kurulmaz; kalan staging muhasebeleştirilip GC ile temizlenir.

## 2. Hesaplar ve veri korunumu

| Kimlik | Tür / yetki | Dosya kimliği ve alanı |
|---|---|---|
| Linux servis `tealdrive` | locked/nologin; panel hesabı değildir | UID995/GID986, `/home/tealdrive` kontrol alanı |
| Panel `tealdrive` | virtual, Root tarafından global panel scope | UID/GID200000, `/home/tealdrive/virtual-homes/tdv_200000` |
| Eski `tealdrive-admin` | Linux; web erişimi suspended | UID1001, orijinal home korunur |
| `tealdrive-test` | Linux, normal kullanıcı | UID1000; mevcut kullanıcı dosyaları korunur |

Global admin Root olmaz. Korumalı DB rolü/global flag ve Root admin roster birlikte gereklidir; tenant virtual admin yalnız kendi hesaplarını yönetir. Root kritik tavan/policy/TLS/key/service işlemleri web yetkisi değildir.

Offline migration descriptor/openat2 kopya, kaynak UID sakinliği, digest/owner/mode kontrolü ve no-overwrite yayınlama kullanır. Symlink/hardlink/FIFO/device/mount geçişi reddedilir. Eski ve yeni home'da5entry/4891regular byte doğrulandı; kullanıcının `test/hello gpt.md`93byte dosyası içerik özeti ve izinleriyle aynıdır. Web parola hash'i değişmedi. Yeni home0700; dosyalarda0777 izin kısmı korunur, ACL/special-bit taşıma garantisi verilmez.

Fresh install `--pool-size` ve `--admin-own-quota` ile ayrı sanal global admini kurabilir. İlk random parola Root-private bootstrap.json içindedir; ilk farklı parola değişimi tamamlanmadan dosya/yönetim erişimi verilmez. İdempotent bootstrap eski parolayı sıfırlamaz.

## 3. İmaj, kota ve sistem alanı

| Mevcut kurulum | Değer |
|---|---:|
| Shared ext4 image, fiziksel ayrılmış |1GiB|
| Net dağıtılabilir Root havuzu |512MiB|
| `tealdrive` admin delegated budget |512MiB|
| Bu bütçeden sabit admin own quota |128MiB|
| Çocuk hesaplara ayrılabilecek kalan |384MiB|
| Root upload tavanı |1GiB|
| Resume penceresi |1800s|

1GiB upload tavanı128MiB admin kotasını aşma hakkı vermez. Kendi dosyaları, çöp kutusu ve kullanıcı geçmişi kendi kotasındadır. Çocuk tahsisleri adminin sabit own quota'sını daraltmaz. DB tahsis/pending işlemleri atomiktir; mevcut kullanım ve tahsis altına indirme reddedilir. Root kendi quota/bütçe değerlerini ayrıca büyütebilir.

Wizard native storage init/status/check/mount/sync/expand ve net havuz yayınlamasını destekler. Fiziksel kapasite, filesystem metadata ve işletim payı net dağıtılabilir miktardan ayrılır. Grow-only genişletme, fsck/remount/recovery ve digest korunumu kabul edilmiştir; otomatik shrink yoktur. Kernel UID quota, nodev/nosuid/noexec mount aktiftir.

Root DB/control audit/system staging kullanıcı havuzundan ayrı bütçedir. Aynı TMPDIR kullanan cooperating süreçler flock/lease ledger ile aktif declared bytes, crash/orphan actual bytes ve transfer sayısını ortak hesaplar. Bu, ele geçirilmiş servis UID'sine karşı OS-wide hard quota değildir. DB page cap512MiB; WAL16MiB checkpoint hedefi toplam DB+WAL disk partition garantisi değildir.

## 4. TLS ve ek kanal

Backend127.0.0.1:13100 TLS; Caddy pinned public certificate/SNI doğrulaması yapar, skip-verify yoktur. PEM çiftleri60s aralıkla doğrulanıp canlı yenilenir; hatalı çift son geçerli sertifikayı bozmaz. Root TLS prepare/activate/renew/auto ile dual trust→publish→exact identity/health→old trust retirement sırası uygulanır. Caddy zorunlu değildir; generic proxy akışı belgelenmiştir.

Günlük `tealdrive-tls-renewal.timer` etkin;30gün eşikte yenileme yapar. Aktif backend sertifikası2027-10-13T21:06:35UTC sonuna kadar geçerli, son kontrol no-op. Gerçek dual-trust rotasyon private Caddy kabulünde geçti. Production active Caddy ile clean-environment adapted config eşitliği doğrulandı. Adaptör başka sitenin bekleyen ayar değişimini yüklemeyi reddeder.

Noise NK/Curve25519/ChaChaPoly/SHA256 standart protokolü native Rust ve browser WASM ile uygulanmıştır; TLS yanında production WS login/dosya/upload/sonuç/event payload'larını şifreler. Replay, tamper, yanlış pin ve plaintext downgrade reddedilir. Bağımsız Root key pin verilmezse güven `tls-discovery` seviyesidir. REST/parola değişimi/cookie bridge/native HTTP download **TLS-only** kalır. Proxy'nin bütün oturum yetkilerinden dışlandığı veya bütün trafiğin uçtan uca şifreli olduğu iddia edilmez. [Tehdit modeli](TRANSPORT_SECURITY.md).

## 5. Testler

- Son workspace koşusu:633passed,0failed,13explicitignored; library607passed/2ignored. Host/credential gerektiren testler normal koşuda sessiz başarı sayılmadı. Root guarded SQLite migration/concurrent budget/persistent login3/3 ve gerçek TLS refresh1/1 ayrıca geçti. Önceki PAM/kernel/worker kabul kanıtları TEST_RESULTS içinde korunur.
- fmt, Clippy workspace/all-targets-Dwarnings, Rust1.85workspace/all-targets, strict ES2022 TypeScript; generated WASM init singleton/handle/hash; resmi Cacophony Noise NK vector geçti.
- Namespace içinde gerçek helper/User worker/native image:256MiB disk-backed upload+stream download+Range;1024progress; same-session resume, wrongprefix, active/retained abort, permission/truncate staging reddi, wrongpin/plaintext/replay/tamper/malformedfragment geçti. Peak RSSserver32416KiB/helper33044KiB;192MiB eşik altında.
- Fresh four-binary install/random restricted admin/idempotence/nologin/ownquota; mevcut Linux hash/dosya migration; native sync/usage-altına-shrink reddi; imaj genişletme+SHA korunumu+shrink reddi geçti.
- Private gerçek Caddy dual-trust/live refresh/old-trust cleanup ve ilgisiz config değişimini reddetme geçti. Sahte aynı isimli backend certificate502 ile reddedildi; fake backend'e0HTTP request gitti.
- Canlı direct Caddy ve public domain Chromium login/logout:virtual/globaladmin, korunmuş93byte dosya, ownquota, encrypted WS geçti.
- Canlı CRUD/revision conflict/chmod undo/download audit korelasyonu, gerçek systemd restart, kapanış popup, unsaved editör koruma ve yeniden giriş geçti.
- Canlı public proxy üzerinden17MiB upload/download içerik ve byte eşitliği,68progress, Range206 ve encrypted WS geçti. UUID test dosyası kullanıcı UID'siyle temizlendi. Son servis NRestarts0/ExecMainStatus0; Caddy/server active ve health200.

Tekrarlanabilir komutlar/opt-in koşulları [tools/dev](../../tools/dev/README.md) ve ayrıntılı kanıt [TEST_RESULTS.md](TEST_RESULTS.md) içindedir. Ara kabul hatalarının nedenleri ve düzeltmeleri kayıtlarda korunur; başarısız koşular başarılı sayılmadı.

## 6. Temizlik ve işletim

Faydalı geliştirme/kabul araçları tools/dev ve tests/live; release/migration yardımcıları tools/ops altında korunur. Tamamlanmış tek kullanımlık development timer/service kaldırıldı. Geçici admin parola dosyası, eski plan başlangıç script/prompt/logları, namespace fixture'ları, Python cache ve Rust build cache temizlendi.4376tanınan fixture directory ve63geçici dosya kaldırıldı; `cargo clean` ilk7993file/2.6GiB ve son derlemeden1629file/523.6MiB kaldırdı.

Kullanıcının bütün yedekleri silme kararı gereği bu update'in otomatik binary/metadata yedeği ve previous service kopyası başarılı kabulden sonra silindi. Orijinal kullanıcı home'ları, aktif native image, Root migration recovery journal, DB/key/config/audit ve mevcut özel test parola dosyaları korunur. Eski release kopyası rollback için tutulmuyor; Git kaynakları ve kurulu aktif binaryler vardır. Disk son ölçümü CONTEXT.md ve PHASE_PROGRESS.md içindedir.

## 7. Kapsam dışında kalanlar ve güven sınırları

Bu iki aşamadan bekleyen zorunlu geliştirme kalmadı. Hosting/domain/process/DB provision, MFA/SSH politikası, server restart ötesi resumable upload, tutarlı tam user-data+metadata disaster recovery, uzun süreli soak ve bağımsız penetration/security review ayrı işlerdir. Metadata backup kabulü full native-volume recovery garantisi değildir; upstream Noise kütüphanesi formal bağımsız audit almış diye tanıtılmaz.

Tarayıcıya dağıtılan JS'nin değiştirilmesi, XSS/endpoint compromise, dış uygulamanın aynı dosyaya eşzamanlı müdahalesi ve Root compromise için mutlak koruma yoktur. Kullanıcı web kökü olarak aynı alanı Caddy/PHP/Node'a sunabilir; Unix erişim izinleri ve dış writer çakışmaları kendisinin işletim kararıdır. TealDrive bu aşamada hosting paneli değildir.
