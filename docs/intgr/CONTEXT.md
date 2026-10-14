# Sunucunun mevcut durumu — 14 Ekim 2026

[Dizin](README.md) · [Bitiş raporu](FINAL_REPORT.md) · [İlerleme](PHASE_PROGRESS.md)

| Alan | Mevcut değer |
|---|---|
| URL | https://tdrive.novastrum.dev |
| Aktif paket |20261014-87e5c3e; dört installed hash manifest ile doğrulandı|
| Backend |127.0.0.1:13100; direct TLS; plaintext reddi|
| Servis |tealdrive.service enabled/active; Linux UID995/GID986, locked/nologin|
| Binary/PATH |/home/tealdrive/bin dört binary; /usr/local/bin symlinks; yalnız helper4755|
| Root config/key |policy/server.toml644, helper.toml600/helper.key600; private TLS/channel key Root:service640|
| Metadata |/home/tealdrive/auth/accounts.db Root600, parent700; schema2|
| Panel overlay |/home/tealdrive/config/settings.toml servis600|
| Web admin |tealdrive;virtual/global/UID200000; eski web parola hash'i korunur|
| Admin home |/home/tealdrive/virtual-homes/tdv_200000 owner200000:200000 mode700|
| Eski admin |tealdrive-admin LinuxUID1001;web suspended;orijinal home/dosyalar korunur|
| Test user |tealdrive-test LinuxUID1000 RoleUser active;home/legacy dosyalar korunur|
| Virtual storage |1GiBpreallocated ext4/usrquota;netpool512MiB;nodev,nosuid,noexec|
| Admin quota |budget512MiB;own128MiB;childremaining384MiB|
| Transfer |Root/server/helper1GiB ceiling;chunk<=256KiB;same-session resume1800s|
| Ek kanal |NoiseNK WS etkin;REST/nativeHTTP TLS-only;default trust TLS-discovery|
| Certificate |policy/origin-cert.pem;expiry2027-10-13T21:06:35UTC;60s live refresh|
| TLS timer |tealdrive-tls-renewal.timer enabled/active;daily30day threshold|
| Retention |history504h/undoRootmax24h/trash504h/staging24h/deletedvirtual36h;hourlyGC|
| Audit |logs/server/audit.jsonl;10MiB/5archive/504h;Root protected canonical records ayrı|
| Caddy |private /run/caddy-admin/admin.sock;backend pinned cert+SNI;skipverify yok|
| Disk |118GiB total/19GiBused/95GiBfree;build peakte92GiBfree|

Cloudflare IP yalnız resmi trusted CIDR ve CF-Connecting-IP overwrite üzerinden alınır; backend yalnız loopback XRealIp'ye güvenir. Public/direct health200 ve gerçek Chromium admin login/logout geçti. Public17MiB upload/download/Range, direct CRUD/undo/audit+systemd restart/shutdownpopup/relogin geçti.

Kullanıcının `test/hello gpt.md`93byte dosyası source+newhome SHA/mode/owner ile doğrulandı. Source toplam5entry/4891regularbyte korundu. Dedicated browser test UUID dosyaları temizlendi. `/srv/tealdrive-test` orijinal kullanıcı verisidir; backup diye silinmez. Eski explicit hidden/sensitive home politikası korunur; fresh install `.env` açık, sistem sır dizinleri kapalı varsayılanını kullanır.

Kullanıcı bütün yedekleri silmeyi istedi: önceki ve bu successful update'in otomatik metadata/binary/unit yedekleri yoktur. Kaynak Linux home'ları gerçek kullanıcı verisidir; korunur. Root admin-migration.json recovery/idempotence kaydıdır, rollback data backup değildir; korunur. Sonraki Wizard update tekrar safety backup üretebilir. Aktif config/DB/key/audit silinmez veya rapora dump edilmez.

Completed development timer/service ve Root scheduled state kaldırıldı; başka Codex süreci kurulmadı. Tekrarlanabilir araçlar repoda, Node Playwright bağımlılıkları tools/dev altında korunur. Rusttarget/Pythoncache/tempfixtures/releasebundle temizlendi. Parola/anahtarlar Git'e veya rapora yazılmaz. Tarihsel bağlam [arşivde](history/CONTEXT-before-20261014-completion.md); eski backup yolları artık geçerli değildir.
