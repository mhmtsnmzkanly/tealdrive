# Çekirdek aşamalar: kaynak tamamlandı, yayın kabulü sürüyor

14 Ekim 2026. Kullanıcı bütün işleri yetkilendirdi; tekrar onay beklenmez. Üretime geçiş ve nihai rapor tamamlanmadan bu belge bitiş raporu sayılmaz.

## Kaynakta tamamlananlar

- Önceki cookie bridge/no-store/UI cancel/readonly FD büyük aktarım ve Root virtual global scope çalışmaları korunur.
- `b01b461`: sabit admin own-file quota, çocuk/pending tahsisleriyle atomik bütçe muhasebesi. Legacy remainder hesaplar explicit Root geçişine kadar korunur.
- `991a7b4`, `bebfda4`: Root TLS prepare/activate/renew/auto; Caddy optional dual trust, strict live identity, unrelated active config/environment değişimi reddi. Generic proxy akışı vardır.
- `49ee5dc`: standart Noise NK/Curve25519/ChaChaPoly/SHA256; shared Rust native/browserWASM, optional independent key pins, replay/tamper/downgrade rejection. Root TLS hâlâ zorunlu. **HTTP/REST/cookie/native downloads Noise kapsamında değildir.**
- `354475b`: staging dosya ledger/flock; süreçler arası aktif declared bytes, crash/legacy orphan actual bytes ve transfer-count muhasebesi. Farklı TMPDIR ayrı alandır; OS UID hardquota iddiası yoktur.
- `e4a38bb`: aynı HttpOnly session ile cross-connection upload resume, incremental prefix SHA256, Root1800s window, policy/identity recheck, explicit cancel. Restart yetkisi service metadata'dan yeniden kurulmaz; eski stage dosyaları sayılır ve GC temizler.
- `0657ddd`: fresh install pool/own quota + random ilk virtual global admin, Root-private credential file, restricted first-login session, zorunlu farklı parola değişimi. Linux servis hesabı nologin/locked kalır.
- `afe120c`: existing Linux admin→virtual UID/home, descriptor copy/manifest/no-overwrite, protected password-hash transfer, Root resume journal. Source files/account retained; old web login yeni giriş doğrulandıktan sonra Root deny ile kapanır.
- Son SDK/UX sertleştirmeleri çalışma ağacındaysa `git diff` kontrol edilir: retained abort API, commit-unconfirmed semantiği, bundler/custom noise loader + concurrency dedup, typed transport adapters, SDK1.1.0, migration device/FIFO open öncesi O_PATH kontrolü, fresh-install test aracı.

## Test kanıtı

Son normal workspace:609library/607passed/2explicit ignored; channel unit+Cacophony vector ve normal integration passed. Rust1.85 workspace/all-target check passed; Clippy workspace/alltargets-Dwarnings passed. Strict TypeScript consumer passed. Yeni source değişirse uygun testler yeniden çalıştırılır.

Gerçek private mount/PID/NSS/TLS namespace + native quota image içinde: random bootstrap/restricted UI+SDK+server/password change/relogin; existing Linux hash/content/owner-preserving migration ve idempotent retry; pinned ChromiumWASM↔Rust17MiB upload/download/Range, same-session resume+wrongprefix rejection, cancel, unsafe source/truncate, wrongkey/plaintext/replay/tamper/malformedfragment rejection; global→tenant revoke; image expansion+SHA256 preservation+shrink rejection; gerçek private Caddy dual trust/live refresh/oldtrust retirement/30day no-op passed.

Fresh four-binary install --pool-size64MiB --admin-own-quota32MiB: native preallocated image, net pool, random private credential, virtual/global/ownquota/first-change/nologin/idempotence passed. Fixture başlangıçta host'un kopyalanan mount unit'ini taşıyordu; yalnız private fixture units temizlenerek gerçek fresh-host durumu sağlandı. Temporary unsupported useradd mail flag yayınlanmadan kaldırıldı. Existing Wizard namespace install/update/rollback/metadata backup/restore/repair/uninstall/remount kabulü passed.

## Kalan yayın işleri

1. Son SDK loader concurrency + retained abort/UI finalizing davranışının gerçek kabulü; Root ignored tests ve son full checks.
2. Release dört binary build/paket; kontrollü üretim update. Yeni config eski binary'ye verilmez.
3. Production existing tealdrive-admin dosya/hash koruyan virtual tealdrive migration; yeni hesabı doğrula, eski web login'i kapat, Linux service identity kilitli kalır.
4. Production upload ceiling, transport key/TLS renewal, direct Caddy/public browser kabulü.
5. Dev çıktı temizliği, güncel dokümanlar ve detaylı bitiş raporu; ayrı commitler.

## Hâlâ geçerli sınırlar

Hosting/domain/app process/DB yönetimi kapsam dışı. Tutarlı tam user-data+metadata backup/restore ile bağımsız güvenlik incelemesi bu iki aşamanın dışında bırakılmıştır. Mevcut metadata backup testi full native-volume recovery garantisi değildir. Root SQLite page cap512MiB, WAL journal-size16MiB checkpoint best effort; system storage sınırları ele geçirilmiş UID'ye OS-wide hardquota değildir. Noise library upstream formal audit almış diye tanıtılmaz. HTTP/REST encryption ve browser kaynak dağıtımı tehdit modeli TRANSPORT_SECURITY.md içindedir.

## Üretim (henüz değişmedi)

Installed old release; helper upload ceiling1MiB. Linux panel admintealdrive-admin UID1001/home/home/tealdrive-admin, kullanıcının seçtiği web parolası ve test/hello gpt.md korunur. Sanal tealdrive admin productionda henüz oluşturulmadı. Native image1GiB/netpool512MiB korunur. Public Caddy/TLS/servis aktif. Rootdisk118GiB/used21GiB/free93GiB. Debug/incremental kapalı.
