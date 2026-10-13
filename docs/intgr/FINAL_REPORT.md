# TealDrive bitiş raporu

**Tarih:**2026-10-13. **Sonuç:** Planlanan çekirdek özellikler uygulandı, gerekli kabul testleri çalıştırıldı ve yeni sürüm bu sunucuda kalıcı servise geçirildi. Aktif kod release'i `b42a8e5`; dört kurulu binary son release build ile SHA256 eşleşiyor. Son belge/test commitleri aşağıdaki Git geçmişinde ayrıca yer alır.

## 1. Canlı kullanım

**Adres:** https://tdrive.novastrum.dev

- `tealdrive.service` enabled/active, UID995/GID986 `tealdrive`; NSS home `/home/tealdrive`, shell nologin.
- Backend127.0.0.1:13100 **TLS**; Caddy backend sertifikasını/SNI'yi doğrular. Plaintext backend bağlantısı reddedilir. Tarayıcı HTTPS/WSS güven zinciri doğrulaması kapatılmadı.
- Caddy'nin admin API'si private Unix socket; TealDrive UID TCP2019 veya private socket'e erişemiyor. Caddy config Root:caddy0640; gerçek systemd reload geçti.
- Native ext41GiB image ve Root pool512MiB; mount ve hourly GC timer enabled/active. Sanal hesaplar desteklenir; otomatik admin oluşturulmadı.
- Mevcut `tealdrive-test` hesabı Linux UID/GID1000, DB RoleUser ve active. Kimlik, rol ve mevcut web parola özeti migration öncesi yedekle eşleşti. Linux/web parolaları değiştirilmedi.
- Web parolası Linux parolasından ayrıdır. Özel test web parola dosyası `/root/tealdrive-candidate-web-password`; değer Git veya rapora alınmadı. İlk admin Root Wizard ile atanır; mevcut test hesabı sessizce admin yapılmadı.

Public Cloudflare HTTPS200 ve gerçek Chromium login/logout geçti. Önceki403 son kabulde tekrarlanmadı; Cloudflare koruması kapatılmadı. Caddy güvenilir Cloudflare CIDR'lerinden gerçek IP'yi alır, backend header'ını üzerine yazar; audit IP ile gerçek socket peer'ini ayrı kaydeder.

## 2. Uygulanan özellikler

**Kimlik ve yönetim:** Root-owned SQLite metadata, Argon2id web parola, ilk PAM kanıtı/tek kullanımlık enrollment, caller PID/start-time/boot-ID bağlı grantler, nonce logout ve generation iptali. Root/system/service/nologin filtreleri; own-UID Wizard öz kayıt; Root deny/suspend üstünlüğü. Linux ve virtual yönetim ayrımı, tenant sınırı ve sanal self-revoke reddi.

**Helper ve dosyalar:** Root supervisor ile fresh User worker ayrımı. İçerik/arşiv işleme kalıcı UID/GID düşüşü ve zorunlu Landlock sonrasında; cancellation thread de bu sınırı miras alır. Root ceiling server snapshot ile genişletilemez. Descriptor tabanlı path/chmod, symlink/hardlink/recursive/alias/trash denetimleri, no-replace move/rename/restore, revision-aware text save ve kontrollü staging.

**Sanal model:** Her hesaba locked/nologin ayrı OS UID/GID/home. Root havuzu→admin bütçesi→kendi/çocuk tahsisi, atomik pending rezervasyon/CAS ve usage guard. Native UID quota ve Root-only parent altında gerçek kullanıcı kotasına ücretlenen audit ledger. Başka OS kullanıcılarına delegated subuid/subgid aralıkları izolasyon için kullanılmaz; Root rezervasyonu ve UID retirement fence uygulanır.

**SDK:** Public `auth / files / admin`; admin altında `systemUsers / virtualUsers`. WS-first handshake ve aynı bağlantıda auth, cookie bridge, typed methods/options/results/events, client/managed state, account/origin/generation browser namespace, path restore revalidation, filter/type/category/extensions/sort/pagination, search, revision editing, upload progress/abort, streaming/native download/preview, archives/trash, policy/history/undo, quota condition+event observers. SDK DOM üretmez; UI aynı public SDK'yı kullanır. Hata/capability ve unknown option davranışı test edildi. [SDK rehberi](SDK.md), [tipler](../../src/ui/sdk/client.d.ts), [kullanım örnekleri](../../sdk-api.js).

**UI:** TR/EN ve kullanıcı bazlı dil; chmod checkbox+octal seçici ve uyarı; aktif proje/çakışma notu; kayıt/onay/erişim/politika/sanal yönetim; own history ve koşullu undo dialogları; CAS tercih kaydı; cancel/ESC/focus/mobile ve geç kalan cevabın görünümü ezmemesi. Kapanış popup'ı kaydedilmemiş editör içeriğini korur.

**Audit/undo/lifecycle:** Root intent/result file journal ve bounded auth/management/control audit. Kullanıcı sadece kendi, güncel politikaya göre görülebilir geçmişi okur. Move/rename/trash/chmod undo Root kayıt, epoch, süre ve gerçek beklenen dosya durumuyla uygulanır. Offline restore eski undo/grantleri geçersiz kılar. Genel retention504h, trash504h, staging24h, deleted virtual36h; user undo1..Rootmax(default24h). GC User worker altında yalnız Root-authorized exact tokenları temizler; değişmiş içerik korunur. HTTP range parçaları tek kayıt/ledgerde birikir; response request ID helper geçmişine bağlanır.

**Kurulum:** Dört binary, Wizard install/update/repair/rollback/backup/restore/uninstall ve bağımsız ctl. Trusted PATH linkleri otomatik; unrelated executable üzerine yazılmaz. Direct TLS/custom Root-approved immutable UI package ve embedded rescue route uygulanmıştır. Caddy zorunlu değildir.

## 3. Test kanıtları

| Test grubu | Sonuç |
|---|---|
| Genel `cargo test --locked` |588library test passed; normal binary/integration testleri passed;1PAM testi normal koşuda ignored, ayrıca explicit Root koşuda passed |
| fmt, Clippy all-targets/Dwarnings |Passed |
| Rust1.85 all-targets check / release dört binary |Passed |
| SDK transport/contracts/strict TypeScript |Passed; state/filter/transfer/errors/events/boundaries |
| Explicit Root integration |SQLite migration/WAL/restart/concurrency/login counters, chmod000, worker forgery/privilege, descendants confinement, ticket same-UID replay, settings separation ve2real helper lifecycle/cancellation testleri passed |
| Gerçek PAM/TTY |Enrollment/web login/revoke; password echo kapalı, own-UID role sınırı passed |
| Birleşik Root/PAM/Chromium/native quota |Tenant/OS identity/subids, CRUD/upload/revision/chmod/state/i18n, Root virtual admin, budget/quota/session, log charge/retention passed |
| Journal/undo/lifecycle |100000bayt HTTP download tek kayıt; own/foreign/current-policy history, undo mode000/move/rename/trash/tree, stale/duplicate/target preservation/CAS,504htrash/log,24hstaging,36hpurge/resume passed |
| İzole Wizard namespace |Install/reinstall denial/PATH collision/repair/uninstall, update/rollback/backup/restore/tamper, native image/remount vectl audit passed |
| Direct TLS/custom UI |HTTPS/WSS/secure cookies, invalid/unsafe key denial, trusted-proxy bounds, immutable package/SDK override denial/default rescue passed |
| Gerçek canlı sistemd + Caddy |Final release update/start/reload/restart, WSS, protected undo/download/request ID audit, shutdown popup/unsaved text/relogin passed |
| Backend kimlik doğrulaması |Doğru dedicated sertifika200; aynı isimli başka sertifika502, fake backend'e0HTTP request; verification kapatılmadı |
| Public Cloudflare + Caddy admin |Public Chromium login/logout passed; service TCP/private Unix deny, Root yönetim erişimi passed |

İzole systemctl stub testi gerçek restart diye sunulmadı; gerçek restart ayrıca canlı serviste yapıldı. Özel kabul testleri üretimden önce ayrılmış alanlarda çalıştı. Artık aynı Root helper policy'yi değiştiren driver, canlı home-installed server çalışıyorsa fail-closed durur. Son koşularda production fixture dosyaları ve Wizard temp dizinleri0; gerçek kullanıcı audit kayıtları test çıktısı diye silinmedi. Ayrıntı: [TEST_RESULTS.md](TEST_RESULTS.md).

## 4. Verilerin korunması ve temizlik

`/srv/tealdrive-test`ten gerçek NSS home'a27entry, kalıcı User UID ile no-replace kopyalandı.26regular file SHA256 eşleşti; kullanıcının oluşturduğu `Web test.md`60bayt korundu. Eski kaynak ve Root-private rollback tutulur. Sonraki home değişikliklerinin üzerine eski yedek körlemesine restore edilmez.

Eski public test/candidate server/helper kopyaları kaldırıldı; setuid bitleri önce indirildi. Dört yeni `/usr/local/bin` symlinki tek aktif `/home/tealdrive/bin` sürümüne gider. Reusable kabul araçları `tools/dev`, canlı browser kaynakları `tests/live`, release/migration operasyon araçları `tools/ops` altında. Tek kullanımlık geçici edit scripti ve test fixture çıktıları temizlendi; dependencies/build cache, operational logs/private backup ve gerçek kullanıcı verisi korunmuştur. Eski plan/tasarım notları `docs/intgr/history` altında etiketlenerek saklandı.

Aktif ayarlar ve metadata `/home/tealdrive` altında; systemd/PAM/PATH entegrasyonları OS dizinlerinde kalır. Rollback merkezi `/root/tealdrive-runtime-backups/final-migration-20261013` Root700. İçinde config/unit/Caddyfile, DB/key metadata ve kullanıcı/binary yedekleri bulunur; sır değerleri repoya alınmadı.

## 5. Açıkça belirtilen sınırlar

- Server ele geçirilirse aktif kullanıcı grantleriyle **izinli dosyalar** etkilenebilir. Root role/ceiling/key/DB ayrımı bu kapsamı daraltır; mutlak compromise-proof iddiası yoktur. Helper içindeki arbitrary code execution, home içi logical hidden-path kurallarına OS inode secrecy sağlamaz.
- Dış SSH/Node/PHP writerları ile mutlak dosya snapshot/transaction garantisi yoktur. API hidden policy dış programı yönetmez; çalışan projeye dosya/chmod müdahalesi uygulamayı etkileyebilir.
- Protected Root history selected operations/security/control intents içindir; bütün OS syscalls/Root shell işlemlerini kapsayan sistem audit'i değildir. Merkezi JSONL server-writable; canonical Root kayıtları ayrı korunur. Root emergency stop, bozuk/dolu DB'de audit olmadan çalışabilir; web fallback değildir.
- Range `success/bytesRead` helper→server teslimidir; bütün indirme veya tarayıcı diske kaydetme kanıtı değildir. Undo content backup değildir; değişmiş durum, pending/spent claim veya restore epoch durumunda güvenli biçimde reddedilir.
- Root-tokenı olmayan legacy trash/staging otomatik tarih taramasıyla silinmez. OS/filesystem türlerinin bütün soft/hard quota formatlarına taşınabilir introspection API sağlanmaz; Linux OS kotası kernelde uygulanır, virtual native kota ayrı tam desteklidir.
- Upload default1MiB/max3MiB, text edit default128KiB; resumable/unbounded upload yoktur. MFA/SSH kapatma, kullanıcı uygulaması süreç yönetimi, ek application encryption ve uzun süreli production load/penetration testi bu teslim kapsamına alınmadı.
- Dedicated backend sertifikası **2027-10-13 21:06:35UTC** sonunda biter; Root cert/key ve Caddy trust pool süresi dolmadan yenilenmelidir. Public Caddy/Cloudflare sertifikasıyla ayrı yaşam döngüsüdür.

## 6. Commit ayrımı ve kullanım

Son aşamada bağımsız commitler: `a919810`protected control audit, `6a80991`range aggregation, `eaa96da`settings/fresh journal, `74cd895`PATH links, `ae0d002`real deployment acceptance, `8a8f00b`ctl audit, `4e76024`request correlation, `f936710`test/production isolation, `b42a8e5`release packager. Önceki undo/GC/subids/SDK/TLS/Wizard aşamaları da ayrı commitlerdir; tam kayıt `git log` içindedir. Son dokümantasyon/teslim kanıtları ayrıca commit edilir.

```sh
tealdrive-ctl status
sudo tealdrive-ctl reload
sudo tealdrive-ctl restart
sudo tealdrive-wizard diagnose
sudo tealdrive-wizard config validate
sudo tealdrive-wizard audit list 50
sudo tealdrive-wizard storage status
sudo tealdrive-wizard gc
```

Root ilk admin/sanal admin adımları [WIZARD.md](WIZARD.md), sıfırdan kurulum [DEPLOYMENT.md](../DEPLOYMENT.md), güncel ayarlar [CONFIGURATION.md](CONFIGURATION.md). Mevcut kullanıcı normal dosya yöneticisini domain üzerinden test edebilir.
