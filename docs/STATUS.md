# Düzeltme ve mevcut durum raporu

Tarih: 2026-10-11. Proje: TealDrive 0.1.0.

İstenen sekiz mimari düzeltme ayrı commit'lerle uygulandı. Küçük kusurlar ayrı
commit'te toplandı; doküman ve kurulum örnekleri ayrıca commit edildi. Kod, Caddy
üzerinden hedef sunucuda kabul testi yapılabilecek durumdadır. Bu ortamda başarılı
bir gerçek PAM girişi ve setuid servis akışı doğrulanamadığı için üretim kabulünün
geçtiği iddia edilmiyor.

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

Son kaynaklar üzerinde:

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
PAM/setuid/Caddy/browser uçtan uca kabul testi hedef host'ta yapılmalıdır.

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
- Ayrı Rust 1.85 toolchain, gerçek PAM parolası ve üretim yük testi çalıştırılmadı.

Kurulum dosyaları [deployment/](../deployment/) içinde. Alan adı, kullanıcı,
PAM stack'i ve dosya izinleriyle birlikte yapılacak kabul testi
[DEPLOYMENT.md](DEPLOYMENT.md) içinde adım adım anlatıldı.
