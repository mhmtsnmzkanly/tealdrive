> Tarihsel tasarım/önceki durum arşivi. Güncel kullanım için ../CONTEXT.md dosyasını okuyun.

# Mevcut kurulum ve geliştirmeye devam bilgileri

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md) · [Mevcut sürüm raporu](../STATUS.md)

Bu belge konuşma bağlamına ihtiyaç duymadan devam edebilmek içindir. Gizli değerler, parola, anahtar ve kullanıcı dosyalarının içerikleri yazılmaz. Canlı snapshot 2026-10-11 tarihinde salt okunur kontrol edildi; daha sonraki çalışmalarda yeniden doğrulanmalıdır.

## 1. Repository ve işin durumu

- Repository: `/root/tealdrive`. Sunucu saat dilimi UTC.
- Son mevcut uygulama commit'i: `a68b0a3` (yerel kullanıcı seçicisi belgeleri). Sonraki konuşma kapsamında yeni mimari uygulanmadı.
- Yeni hedefin yetkili belgeleri `docs/intgr/` altında. `sdk-api.js` yalnızca 50 örnek/JSDoc referansıdır; SDK implementasyonu değildir.
- Kökteki `plan.md` ve `TASARIM.md` kullanıcı talebiyle kaldırıldı. Kullanıcı notları DECISIONS ve ilgili bileşenlerde korundu.
- `deployment/` içerikleri değiştirilmeden `docs/deployment/` altına taşındı. README, mevcut deployment/configuration/status belgeleri ve config örnek testinin yolları güncellendi.
- Bu dokümantasyon değişiklikleri henüz commit edilmedi; kod çalışması başlamadan mevcut `git status` incelenmeli. Kullanıcı tarafından eklenen notlar silinmemeli veya öneriler onaylanmış karar sayılmamalı.
- Kullanıcı geliştirme sırasında mantıksal değişikliklerin ayrı commit, uygun test ve güncel dokümantasyonla ayrılmasını istiyor. Canlı kullanıcı verileri korunmalı.
- Mevcut çalışan sürüm ile hedef tasarım farklıdır; tasarım örnekleri mevcut AppConfig/CLI'ye verilmez.

## 2. Canlı test kurulumu — doğrulanan snapshot

| Alan | Değer |
|---|---|
| URL | `https://tdrive.novastrum.dev` |
| Backend | `127.0.0.1:13000` |
| Servis | `tealdrive.service`, kontrol tarihinde active |
| Servis UID/GID hesabı | `tealdrive:tealdrive` |
| Working directory | `/var/lib/tealdrive-test` |
| Server binary | `/usr/local/bin/tealdrive-test-server`, root:root 0755 |
| Helper binary | `/usr/local/libexec/tealdrive-test-helper`, root:root 4755 |
| Server config | `/var/lib/tealdrive-test/config.toml`, tealdrive:tealdrive 0600 |
| Root helper config | `/etc/tealdrive/helper.toml`, root:root 0600 |
| Helper anahtarı | `/etc/tealdrive/helper.key`, root:root 0600; içerik yazılmaz |
| İzinli kullanıcı | Her iki config'te yalnızca `tealdrive-test` |
| Panel admini | Server config'te yalnızca `tealdrive-test` |
| Dosya kökü | `/srv/tealdrive-test`; gerçek home kökü henüz uygulanmadı |
| Helper timeout | 10 saniye |
| Kapanış | KillMode=mixed, systemd TimeoutStopSec=45 saniye |

Loopback health kontrolü `ok` döndü. Bu PAM, browser ve bütün dosya işlemlerinin tekrar test edildiği anlamına gelmez.

Önceki kurulum/testlerde doğrulanan ayarlar: HTTPS proxy/secure cookie, exact domain Origin, X-Real-IP için yalnızca loopback trusted proxy; audit JSONL 10 MiB / 5 arşiv / 30 gün; düzenleme/chmod/extract açık, kalıcı silme/chown kapalı, gizli dosyalar kapalı, kök kotası 10 GiB. Bu değerler yeni mimarinin varsayılanı değildir.

Test Linux kullanıcısının sudo yetkisi olmadığı güvenlik incelemesinde kontrol edildi. SSH parola girişi inceleme sırasında açıktı; bu çalışma SSH ayarını değiştirmedi. Test parolası özel root otomasyon dosyasında bulunur; değeri belgelere/git'e yazılmaz, üretim temizliği ayrı yapılmalıdır.

`/srv/tealdrive-test/Web test.md` kullanıcı tarafından oluşturulmuş dosyadır; testlerde içerik üzerine yazılmamalı. Yeni test dosyaları ayrı benzersiz dizinde oluşturulup temizlenir.

## 3. Caddy/Cloudflare ve sistem müdahalesi notları

- Bu sunucuda Caddy ve Cloudflare kullanılıyor; yeni kurulum için Caddy zorunlu olmayacak.
- Önceki kabulde Caddy gerçek client IP'yi doğrulayıp backend X-Real-IP üzerine yazdı; uygulama yalnızca loopback proxy'ye güveniyor. Spoofing kontrolü geçmişte yapıldı.
- Caddy config DNS sağlayıcı gizli anahtarları içerebilir; tam Caddyfile/adapt edilmiş JSON loga veya belgeye basılmamalı.
- Mevcut diğer Caddy siteleri korunacak; config dosyasının tamamı üzerine yazılmayacak.
- `/usr/local/bin` kısa yol betikleri vardı. Root işlemde içerikleri/sahiplikleri doğrulanmadan çalıştırılmamalı; önceki çalışmada doğrudan systemctl/Caddy binary tercih edildi. Önceki dizin izinleri yeni çalışmada varsayılmamalı.
- Yeni `/home/tealdrive` düzeni, SQLite ve sanal hesaplar henüz canlıya kurulmadı. Bu sunucudaki eski test yolları hâlâ geçerli.

## 4. Geçmiş uygulama ve test kanıtları

Mevcut uygulamanın tarihsel kanıtı [STATUS.md](../STATUS.md) içindedir. Son tam çalışma: 528 test (gerçek-helper/ignored testler dahil), Clippy ve release build başarılı; mevcut kullanıcı checkbox kaydetme ve altı Chromium viewport'u doğrulandı. Bu rapor yeni hedef mimarinin doğrulandığı anlamına gelmez.

Önceki tamamlanan başlıklar: PAM/helper kimlik sınırı, Landlock/openat2, snapshot ve lease iptali, bounded IPC/transfers, quota reservation ve staged publication; audit rotation; kontrollü kapanış ve kullanıcı bildirimi; audit bağlamı/proxy IP; mobil UI; çift login allowlist; servis hesabı filtreli kullanıcı seçicisi.

İlgili son commitler:

| Başlık | Commitler |
|---|---|
| Audit rotation | `4174b30`, `aef074e` |
| Kontrollü kapanış | `edabae0`, `ad8ae47` |
| Kapanış bildirimi | `c7f257e`, `e84fbf2` |
| Audit/proxy IP | `d931b78`, `faccf7e` |
| Mobil UI | `07bca67` |
| Login allowlist | `df88ac0`, `37c9561` |
| Kullanıcı seçicisi | `8cac8db`, `a68b0a3` |

Dokümantasyon taşımasında yeniden çalıştırılan kontroller: `cargo test --locked --test config_examples`, `cargo fmt --check`, local Markdown linkleri, TOML/JSON/JS/shell örnek sözdizimi, `sdk-api.js` parse, `git diff --check`; beş deployment dosyasının eski git içeriğiyle aynı olması. Tam test suite/live PAM/browser bu taşıma nedeniyle yeniden çalıştırılmadı.

## 5. Repo dışı test araçları — sunucuya özel

Bu yollar taşınabilir ürün gereksinimi değildir; mevcut sunucuda önce dosyanın varlığını ve içeriğini kontrol edin. Parola/token/screenshot çıktıları git'e eklenmez.

- Browser: `/root/tdrv-ui-check/check.cjs`; Playwright/Chromium repo dışında. `TDRV_LIVE_ASSETS=1` eski candidate asset override'ını kapatır, `TDRV_CHECK_PICKER=1` kullanıcı seçici testini açar. Altı viewport: 320×568, 375×667, 390×844, 667×375, 768×1024, 1280×720.
- `/root/tealdrive-audit-proxy-live-test.py`: Gerçek HTTPS/WSS/PAM dosya/transfer/audit/proxy negatif kabul akışı; kendi test dizinini kullanır.
- `/root/tealdrive-domain-ws-test.py`: Ham domain WS ve codec test yardımcıları.
- `/root/tealdrive-shutdown-live-test.py`: SIGTERM/SIGINT, servis restart ve canlı shutdown akışı.
- `/root/tealdrive-login-access-live-test.py`: Geçici allowlist değiştirip eski HTTP/WS/ticket ve yarım upload iptalini sınar; canlı hizmeti etkiler, restoration/finally incelenmeden çalıştırılmaz.
- `/root/tealdrive-popup-live-test.py`: Geçmiş forced timeout senaryosu helper timeout'unu değiştirebiliyordu; tam script config restore doğrulanmadan yeniden çalıştırılmaz.
- Geçmiş test/clippy/build logları `/tmp/tdrv-user-picker-*.log` altında bulunabiliyordu; geçici dosyaların hâlâ varlığı garanti değildir.

## 6. Başlamadan önce okunacak kararlar

- [DECISIONS.md](DECISIONS.md): Kullanıcı kararları ve açık değerler. Sanal UID ayrı; havuz/admin/alt hesap kotaları; kendi init/revoke; `.ssh` kapalı, `.env` açık; seçilebilir PAM/web auth; kritik ayarlar root, çalışma ayarları web.
- [QUOTA.md](QUOTA.md): Kota DB'de, gerçek kullanım helper sorgusu; ayarlanabilir örnek %80; cache/ölçüm yaşı ve parallel write rezervasyonu ayrı.
- [SERVER.md](SERVER.md): Otomatik reload kullanıcının isteği. Mevcut watcher/SIGHUP var; yeni config yaşam döngüsüne uyarlanacak.
- [SDK.md](SDK.md): `API.handshake()` başlangıcı; TLS şifrelemenin kendisidir, handshake ayrı kripto sağlamaz. SDK/tema geriye uyumlu kalacak.
- [SECURITY.md](SECURITY.md): Helper ve root sınırlarını UI/SDK için gevşetmeyin.
- Açık konular: Retention süreleri, admin hesap sayısı/bütçe azaltma, logların kullanıcı kotasına muhasebesi, `.ssh` dışındaki sır listesi, undo'nun ilk sürüme dahil edilmesi. UI fallback config ve auth boolean/enum kullanıcı notları öneri/karar durumuyla ayrı tutulmuştur.

## 7. Belgeleme kapsamı

Geliştirmeyi sürdürmek için gerekli mevcut durum, karar, taslak sözleşme, sınırlar ve test geçmişi dosyalara aktarılmıştır. Bütün konuşmanın kelimesi kelimesine arşivi veya tüm sunucu verilerinin kopyası değildir. Gizli değerler, kullanıcı içerikleri ve geçici araçların tam kaynakları bilinçli olarak bu belgelerde yoktur. Yeni kullanıcı kararları ilgili bileşen ve karar dosyasına; uygulama/test kanıtı durum belgesine eklenmelidir.
