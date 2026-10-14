# Uygulama test sonuçları

Bu dosya yeni geliştirme testlerini tutar; tarihsel 528-test raporu docs/STATUS.md içindedir. Henüz yapılmamış kontroller başarılı olarak yazılmaz.

## 2026-10-12 — Dokümantasyon ve deployment taşıması

- cargo test --locked --test config_examples: 1 passed.
- cargo fmt --check: passed.
- Deployment beş dosyası eski git blob'larıyla byte-for-byte eşit.
- Local Markdown link ve fenced TOML/JSON/JS/shell syntax doğrulaması: passed.
- sdk-api.js syntax: passed; kullanım taslağıdır, SDK implementasyonu değildir.
- git diff --check: passed.
- Canlı backend read-only health: ok. Tam PAM/browser suite bu belge değişimi için yeniden çalıştırılmadı.

## 2026-10-12 — Gömülü UI kaynak ayrımı

HTML/CSS/JS/SVG ayrı src/ui/static dosyalarına byte içeriği korunarak taşındı; assets.rs include_str üzerinden aynı sabitleri yayımlar.

- cargo test --locked --test browser_contract --test browser_shutdown: 2 passed.
- Binary içinde gömme korunuyor; dış asset dizini zorunlu değil.

## 2026-10-12 — UI güvenlik uyarıları ve parola alanı

Aktif program/dosya çakışması notu, chmod izin kaldırma/special-bit açıklaması ve yeni Others-write için iptal edilebilir onay eklendi. Login input ağ başlamadan temizlenir; hata/success/finally sonrası da boş kalır.

- cargo test --locked --test browser_permissions --test browser_shutdown --test browser_contract: 3 passed.
- Deterministic UI testi: cancel mutation göndermiyor, onaylanan chmod doğru mode gönderiyor, invalid octal reddediliyor, special bit uyarısı, existing world-write yeniden onay istemiyor, success/denial/network login'de parola alanı temiz.
- node --check src/ui/static/app.js: passed.

## 2026-10-12 — Script CSP

script-src 'unsafe-inline' kaldırıldı; programatik event listener/property handler çalışması korunur. Style inline henüz kaldırılmadı.

- cargo test --locked ui::tests --lib: 3 passed.
- Gerçek Chromium: mevcut dış app script'i çalıştı; eklenen inline script çalışmadı; operasyon notu ve 6 viewport taşma kontrolü başarılı.
- Browser script: /root/tdrv-ui-check/csp-current.cjs (sunucuya özel test aracı, repo dışı).

## 2026-10-12 — Backend parola ömrü

AuthRequest/LoginPayload drop sırasında zeroize; PAM password CString yerine zeroizing nul-terminated byte buffer; helper stdin/serialization/auth receive tamponlarında zeroize. LoginPayload Debug artık plaintext parola basamaz. PAM/tarayıcı/kütüphane iç kopyaları için mutlak temizleme garantisi verilmez.

- cargo test --locked --lib auth::: 14 passed.
- cargo test --locked --lib helper::ipc::tests: 10 passed.
- cargo clippy --locked --all-targets -- -D warnings: passed.

## 2026-10-12 — WS-first auth

- cargo test --locked --test ws_auth --test browser_contract: 2 passed.
- ws::handshake::tests: 7 passed.
- Gerçek ephemeral TCP WS: kötü Origin reddi, login öncesi list reddi, binary hello, yanlış parola, aynı WS'de doğru parola, HttpOnly bind ve replay reddi; audit içinde parola/bind token yok.
- Legacy /ws korunur; SDK /ws/auth ayrı sınırlı pre-auth durum makinesi.

## 2026-10-12 — Listeleme extension filtresi

FilterMode optional extensions eklenmesi eski MessagePack alanlarını korur. Server/helper filter boyut/extension syntax denetler; helper pagination öncesi filtreler.

- extension_filter_precedes_pagination_and_handles_case: passed; gerçek dosyalarla case-insensitive PDF, folder exclusion, iki sayfa, total count, traversal-benzeri extension reddi.
- listing:: kapsamı: 14 mevcut test passed.
- SDK/WS/browser contract testleri: 3 passed.

## 2026-10-12 — Editör revision ve metadata

- helper::revision::tests: 1 passed; stale token original'i değiştirmiyor, fresh token yazıyor, 0640 mode korunuyor, missing sentinel mevcut dosyayı overwrite etmiyor, staging artığı yok.
- helper::text::: 7 passed.
- SDK/browser permission/shutdown/contract: 4 passed.
- UI ek testi: conflict editor'ü kapatmıyor, unsaved text kalıyor, root seçimi değişse bile opened-file root'u kullanılıyor; başarılı ack sonrası kapanıyor.

## 2026-10-12 — Helper kullanım sorgusu

- helper::usage::tests: passed; hardlink dedup, dış symlink ignore, iç/trash byte sayımı, missing scan error.
- quota_api_measures_on_helper_and_validates_warning_threshold: passed; anonymous401, valid helper result/no-store, unknown root404, warning0 rejected, warning75 reflected.
- SDK transport/state/filter/quota/transfer testi: passed.
- Clippy all targets: passed.

## 2026-10-12 — SDK v1 çekirdeği ve tip sözleşmesi

- sdk_transport_state_filters_quota_and_transfers: passed. Rust-generated contract + gerçek codec kullanarak handshake/auth gate, wrong/right login, list filter/sort/extensions, traversal/unknown option, observer unsubscribe, warning entered/clear dedup, abort/timeout cleanup, raw-chunk ACK/progress, missing path, kullanıcı storage izolasyonu, out-of-order navigation ve deleted-current-dir fallback.
- WS auth ve browser contract: passed.
- npx --package typescript tsc --noEmit --strict --lib es2022,dom src/ui/sdk/client.d.ts: passed; tool dev-only, üründe dış dependency yok.
- node module syntax: passed.
- Clippy all targets: passed.
- Çekirdek entegre edildi; virtual/enrollment/policy/audit-history/undo backendleri henüz bu aşamada bitmiş değildir. SDK doc destek tablosunda ayrılır.

## 2026-10-12 — Gerçek Chromium/PAM/setuid SDK kabulü

Canlı eski servis değiştirilmeden loopback aday13101, ayrı audit/config ve root-owned aday binary kullanıldı. İlk test indirme query field uyuşmazlığını buldu; düzeltme sonrası aynı test tekrar başarılı.

- Gerçek browser: WS handshake/login, HttpOnly cookie bridge, create folder/text, read revision, stale-write conflict, chmod0640, extension filter, chunk upload progress, HTTP streaming download ve helper quota ölçümü PASS.
- WS işlem audit session_id_hash 64 karakter correlation PASS; password/bind token loglanmaz.
- SDK download path/root_id mapping için deterministic regression PASS; ws_auth PASS.
- Test sadece kendi sdk-test-UUID dizinlerini kullanır ve trash'e taşır; kullanıcı dosyaları üzerine yazılmaz. Test aracı /root/tdrv-ui-check/sdk-live.cjs repo dışında.

## 2026-10-12 — Audit retention saat birimi

- audit::sink::tests: 9 passed. 1 saat sınırında 2 saatlik archive silinir, 30 dakikalık archive korunur; gün uyumluluğu, 504 saat default, nullable PATCH/TOML restart roundtrip, üst sınır ve concurrent rotation doğrulandı.
- SDK/browser permission regression sonuçları bu değişikliğin commit'i öncesinde doğrulandı.

## 2026-10-12 — Root/runtime config ayrımı

- runtime_settings::tests: 2 passed; projected TOML roundtrip, feature/limit/cookie/admin/retention/path genişletme reddi, eksik/empty/unknown/symlink settings.
- cargo test --test runtime_settings -- --ignored: root gerektiren gerçek dosya testi passed; root dosyası byte-identical, runtime0600, restart roundtrip, forbidden PATCH disk/state değiştirmiyor, bozuk reload mevcut admin/state'i koruyor, root admin değişikliği yükleniyor, bind değişikliği restart gerektiriyor, disabled retention TOML'de 0 ile kalıcı.
- Mevcut config unit: 6 passed; SDK/browser permission/WS auth regression: 3 passed.
- cargo clippy --locked --all-targets -- -D warnings: passed.

## 2026-10-12 — SDK HTTP hata sözleşmesi

- SDK deterministic regression: passed; indirme query uyumluluğuna ek olarak 401/403/404/429/503 yanıtlarında ayrı TealDriveError.code/status kontrol edildi.
- Bu düzeltme gerçek browser'da gizli path'in indirme testinden elde edildi; genel DOWNLOAD_FAILED geliştirici için yetersizdi.

## 2026-10-12 — Ortak dosya erişim politikası

- policy::access: 2 passed; katman yasakları, kullanıcı ayrımı, component sınırları, malformed path, helper ceiling union ve nonrecursive Discover ancestor bypass reddi.
- helper::access: 6 passed; doğrudan gizli işlem NotFound, list/pagination filtresi, .env izinli explicit profile, chmod kullanıcısı ayrımı, hardlink/symlink alias, parent move/rename, ZIP exclusion, final extraction/staging kontrolü, search/trash/restore/delete current policy ve includeHidden ceiling.
- Tam cargo test --locked --quiet: 522 unit + diğer aktif integration/CLI/browser/SDK testleri geçti. Root gerektiren 3 test varsayılan suite'de ignored; ayrı komutla sonuç ayrıca kaydedilir.
- Gerçek Chromium + PAM + setuid candidate: tests/live/sdk-access.cjs PASS; .ssh/alias list/read/stat/download yok, parent move reddi, normal dosya okunuyor. Fixture gerçek UID/GID ile oluşturulup yalnızca kendi UUID dizinleri temizlendi. Eski Caddy servisi değiştirilmedi.
- Gerçek SDK CRUD/revision/chmod/filter/upload/download/quota kabulü tekrar PASS.
- Clippy all targets: passed.

- Ayrı privileged kabul: cargo test --test e2e_integration_test -- --ignored --nocapture: 2 passed (gerçek helper lifecycle, quota-lock cancellation). Kernel confinement açık çıktıyla: 1 passed, skip olmadı.

## 2026-10-12 — Protected hesap metadata deposu

- accounts:: 5 passed; random salted Argon2id, wrong password, PHC resource bounds, lifecycle/generation, no-secret public metadata, admin isolation, virtual self-revoke reddi, root pool/budget/child tahsis ve virtual-admin own available quota.
- cargo test --test accounts_store -- --ignored: root gerçek dosya testi passed; DB/WAL/SHM private root izinleri, nobody UID okuyamaz, restart, concurrent iki400-byte tahsisten600 bütçeye yalnızca biri sığar, symlink/0644/unknown schema reddi, init/backup overwrite reddi, WAL-aware online backup doğrulaması.
- Tam aktif suite: 527 unit + diğer aktif integration/browser/SDK/CLI passed (root gerektirenler ayrıca çalıştırıldı).
- Clippy all targets: passed.
- Gerçek Rust1.85 toolchain check --locked --all-targets: passed; ilk MSRV denemesi ctutils gereksinimini yakaladı, compatible lock seçimi sonrası tekrar geçti.
- Bu sonuç DB temelini doğrular; enrollment/virtual management uçları henüz canlı auth testine dahil değildir.

## 2026-10-12 — Merkezi yol seçimi

- layout::tests passed: absent primary legacy seçer; corrupt ve dangling symlink primary legacy'ye düşmez.
- cargo clippy --locked --all-targets -- -D warnings: passed.
- Existing deployment taşınmadı; bu commit yol seçimini hazırlar, kurulum kabulü ayrıca yapılacak.

## 2026-10-12 — Helper MAC standard library

- helper::security::tests: 2 passed; RFC4231 HMAC output aynı, UID/expiry değişmiş ticket reddi.
- Clippy all targets: passed; root helper key buffer artık Zeroizing.

## 2026-10-12 — Grant/enrollment ve schema migration

- accounts::grants::tests: 6 passed. Single-nonce logout/other session alive, all-session suspend, UID/PID/start/boot/expiry/tamper/PW-generation denial, hashed one-time enrollment, identity change/replay/expired token denial, administrative deny between steps, own vs administrative revoke recovery, bounded grant reclaim, real kernel caller capture.
- accounts:: tüm 11 test passed.
- cargo test --test accounts_store -- --ignored: 2 root tests passed; existing privacy/concurrency/backup + explicit schema1 migration preserves roles/password/home/budget, old proof cancellation, pre-migration private backup, generation bump, idempotence.
- Clippy all targets passed (grant primitives); normal auth uçları henüz yeni modele geçirilmedi.

### PAM biletinin kernel süreç bağı — 2026-10-12

- Değişiklik öncesi tam suite: 534 birim testi ve normal integration tests geçti; root gerektiren testler ayrıca çalıştırılır.
- HMAC/RFC4231 ve bilet UID/expiry/UID-PID-start-time-boot-ID tamper vektörleri geçti.
- `tests/helper_ticket.rs`: dedicated root-installed candidate setuid helper + gerçek PAM ile giriş; aynı driver süreçte gerçek quota operation başarılı; aynı UID altında ayrı süreçten aynı bilet replay reddedildi. Kimlik bilgileri yalnız pipe üzerinden taşındı. Opt-in test root ve test fixture environment ister.
- Candidate 127.0.0.1:13101 üzerinde yeni server/helper birlikte kuruldu. `tests/live/sdk-access.cjs` gerçek Chromium/SDK/PAM/setuid zincirinde gizli path, hardlink, download, parent move ve izinli read kontrollerinin tamamı geçti.
- Protected SQLite root testleri (ownership/isolation/concurrency/backup + v1→v2 migration): 2 geçti. Clippy all-targets `-D warnings` geçti (yeni acceptance test eklendikten sonra ayrıca tekrar kontrol edilir).
- Caddy üzerinden yayınlanan mevcut servis ve kullanıcı dosyaları değiştirilmedi. Yeni DB ledger henüz canlı auth'a bağlanmadı.

### Root account komut yüzeyi — 2026-10-12

- Altı gateway testi: explicit mode/nonroot server UID, yalnız subject registration, registration-off/approved/root deny, service/root/nologin/virtual-range reddi, config validation, secret wire roundtrip/redacted Debug geçti.
- Root ve explicit PAM fixture ile ignored `pam_enrollment_web_login_and_session_revocation` testi geçti: gerçek PAM ilk kanıtı; completion sırasında root deny; deny kaldırılınca ayrı Argon2id web parola; proof replay reddi; Linux parolası web login/reset olarak reddedilir; iki oturumdan bir logout diğerini korur; suspend ikinciyi iptal eder.
- Bu test root process içinde ayrı in-memory SQLite ve gerçek PAM kullanır; server/WS/SDK enrollment uçtan uca kabulü veya fiziksel sanal UID oluşturma testi değildir.
- Clippy all-targets `-D warnings` geçti. Tam suite ve Rust1.85 sonuçları aşağıda güncellenir.
- Gateway sonrası tam suite: 540 birim testi geçti, gerçek PAM fixture testi varsayılan ignored (yukarıda explicit çalıştırılıp geçti); normal browser/SDK/WS/CLI entegrasyonları geçti.
- `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --all-targets` geçti; MSRV korunuyor.

### Chmod mode-000 kurtarma — 2026-10-12

- `cargo test --locked helper::chmod --quiet`: mevcut chmod unit suite geçti.
- `cargo test --locked --test chmod_owner -- --ignored --nocapture`: root fixture sonrası supplementary groups temizlenmiş UID/GID65534 süreç + gerçek Landlock/openat2 altında file000→0600 başarılı, içerik korundu. Root-owned başka file ve symlink chmod reddedildi, root-owned file mode000 değişmedi.
- Çalışan kernel6.8; yeni unreadable inode yolu Linux6.6+ gerektirir. Eski kernel readable-FD fallback'i bu sunucuda kernel değiştirilerek test edilmedi.

### Server / WS / SDK / UI web-account bağlantısı — 2026-10-12

- Tam suite: 541 unit geçti; gerçek PAM fixture unit varsayılan ignored ve önce explicit geçti. Normal browser/SDK/WS/CLI integration tests geçti. Clippy all-targets ve Rust1.85 all-targets geçti; TS strict declarations geçti.
- Gerçek TCP `ws_auth` hem legacy PAM fixture hem web mode fixture ile file-before-auth denial, origin, pending enrollment, proof gizliliği, completion, cookie bridge one-use, audit secrets exclusion geçti. Gateway bu protocol testinde fixture; gerçek root gateway ayrı aşağıdaki Chromium/PAM testinde kullanıldı.
- SDK Node real binary codec/fake transport testi enrollment actions, proof-ID olmaması, weak-password validation, completion session binding dahil geçti.
- Root lease testi grant browser JSON'da yok, nested cancellation scope token karışmıyor, root revoke aktif lease/local session'ı iptal ediyor.
- `/home/tealdrive` altında root policy/key/auth/bin ve service-writable config/log dizinleriyle gerçek candidate kuruldu. SQLite init overwrite etmez; schema2/WAL root0600. Caddy eski servis/root/data korunur.
- `tests/live/sdk-enrollment.cjs`: ilk çalışmada gerçek PAM→DB first-enrollment→web login tamamlandı; sonraki CRUD kontrolü eski `/srv` root'u NSS `/home/tealdrive-test` dışında olduğu için reddedildi. Candidate root gerçek home'a açıkça düzeltildi. Sonra Chromium→WS→setuid helper→SQLite zincirinde ayrı web password login, Linux password login reddi, file create/read/revision, chmod000→0600/content, helper quota, streaming download, DB nonce logout ve resumed socket denial geçti. Rerun output enrollment=false mevcut kaydı yeniden açmadığını gösterir; PAM reset yapmaz.
- `tests/live/ui-auth.cjs`: gerçek embedded UI/SDK, doğru password-mode label, login, legacy file socket grant, input temizliği, user/admin ayrımı, logout geçti.
- Aktif lease doğrulama normal1s aralığında; queued/helper latency eklenir. Native file tests yeni admission/nonce/logout; suspend watcher unit + root DAO suspend/generation testleri ayrı. Canlı TLS/Caddy migration, iki gerçek UID home profile ve virtual provisioning kabulü değildir.
- Son UI/SDK contract güncellemesinde handshake authentication capability, account ID/generation/kind/role metadata ve parola türü etiketi eklendi. TS strict declarations ve normal browser contract/permissions/shutdown tekrar geçti. İlk kez başarılı PAM/enroll yapan çalışma ile mevcut account rerun ayrımı native fixture output'ta açıktır.

### Bodyless audit düzeltmesi

Native testte204 cookie bridge başarıyla cookie bağladığı halde audit ConnectionClosed kaydediyordu. Status204/205/304 bodyless testinde guard end() çağrılmadan drop edildiğinde Success/0byte/no reason geçti; interrupted download ayrımı korundu. Web-mode root guard'ı exact NSS home olarak daraltıldı; child path home root'a relative olur, ayrı user-controlled parent-symlink sub-root sunulmaz.
- Virtual account view testinde initial budget0, admin budget600-child200=own400, child200 doğrulandı. Helper snapshot DB effective quota üstüne çıkamaz; gerçek virtual UID provisioning/enforcement native kabulü ayrı PLAN 5'tedir. Session metadata configured root'ları exact home'a filtreler.

### Son kabul / commit doğrulaması

- Son tam suite: 543 unit passed, 1 opt-in PAM unit ignored (ayrıca önce explicit passed); normal entegrasyonlar passed.
- Root acceptance ekleri: protected SQLite/backup/migration2 + split runtime1 passed.
- `tests/helper_ticket.rs` artık legacy ticket ve web grant'ı destekler. Gerçek home-layout setuid helper + web password ile same-UID başka process replay reddi passed; meşru driver quota operation passed. Denial typed response veya nonzero-exit olabilir; success hiçbir durumda kabul edilmez.
- Clippy `--all-targets -D warnings`, Rust1.85 `check --locked --all-targets`, fmt/diff ve SDK TS strict geçti.
- Caddy https health200; `/srv/tealdrive-test/Web test.md`60byte korunuyor. Candidate localhost13101; canlı server PID449282 ve eski konfigürasyon değişmedi.
- `tests/live/sdk-suspend.cjs` gerçek Chromium/WS/setuid/SQLite zincirinde passed: root test operator dedicated candidate Linux app account'ını suspended+generation artırarak durdurdu; aktif SDK WS5s timeout içinde kapandı ve file API erişimi reddedildi. Test finally'de hesabı active + yeni generation'a geri aldı; Linux shadow/home/data değişmez. Explicit root test-account mutation flag gerekir; private online test backup bırakır. Root/admin suspend endpoint'i testi değildir (bu yönetim uçları bekliyor).
- Son kurulu binary'de native SDK enrollment/login/CRUD ve UI auth tekrar; web grant üzerinden actual-home path policy/hardlink/direct download/parent move kabulü passed. Caddy eski binary'de kalır. Aşama raporu AUTH_INTEGRATION_REPORT.md.


## 2026-10-12 — Parola değiştirme ve öz erişim iptali

- Mevcut tam test paketi: 543 unit passed, 1 opt-in ignored; normal integration testleri passed.
- Opt-in gerçek PAM gateway testi genişletildi: yanlış mevcut parola reddi, parola generation değişimi, eski grant reddi, öz revoke sonrası grant reddi ve yeniden PAM kaydı kabulü passed. Dedicated account / root-private secret file kullanıldı; production DB değişmedi.
- SDK contract: eksik capability reddi, kısa parola reddi, başarılı password/revoke sonrası state/connection temizliği passed.
- Gerçek Chromium `tests/live/ui-credentials.cjs`: CSRF'siz POST403, yanlış mevcut parola401, UI submit sonrası logout, yeniden girişte generation artışı passed. Test parola değerini koruyup sadece yeni hash/generation oluşturur; açık mutation opt-in gerektirir.
- Aday localhost13101 kullanıldı. Caddy production13000 değişmedi. Sanal hesap provisioning ve public admin API kabulü bu testin kapsamı değildir.


## 2026-10-12 — Hesap erişim yönetimi gateway'i

- Root roster + DB role / tenant isolation unit testi passed: Linux DB admin root listesinde yoksa global yönetemez; virtual admin root listesine yazılsa bile global yetki alamaz; user role yönetemez.
- Gerçek PAM fixture'a user role ile ListAccounts/SetAccountAccess negatif testleri eklendi ve passed.
- Tam paket 544 unit passed, 1 opt-in ignored; normal integration testleri passed. Clippy all-targets -D warnings passed.
- Registered system account listesi ile NSS aday listesi farklı SDK method'larında; tamamlanmayan create/delete/quota capability kapalı tutuldu.
- Başarılı admin suspend/enable için public endpoint → gerçek OS hesabı kabul testi henüz yok; önceki root suspend testi yeni public admin endpoint testi sayılmaz. Sanal provisioning/multi-UID testi ayrıca bekler.


### Ortak development runner doğrulaması

`tools/dev/check.sh live ui-credentials` ve `live ui-auth` yeni `tools/dev/node_modules` bağımlılığıyla passed; eski `/root/tdrv-ui-check` kaldırıldıktan sonra ui-auth yeniden passed. Gerçek gömülü UI üzerinden normal user için `/api/admin/accounts/system` ve `/api/admin/virtual/list`403 doğrulandı. Strict TypeScript d.ts ve Rust1.85 all-target check passed. Eski production Caddy HTTP200 korundu. Runner source `bash -n` passed; `npm ci` lock ile2 packages kurdu. Cleanup ayrıntıları `tools/dev/CLEANUP.md`.


### Effective quota eşzamanlı snapshot düzeltmesi

Admin budget ve child allocation artık tek SELECT snapshot'ında okunur. Mevcut `virtual_admin_uses_root_pool_and_own_quota_excludes_child_reservations` passed; root-only `accounts_store --ignored`2 test passed (private ownership/WAL, concurrency, backup/restart ve migration). Clippy all-targets -D warnings passed. OS/provisioning/usage kabulü değildir.


## 2026-10-12 — Public admin gerçek Linux hesap kabulü

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 tools/dev/check.sh admin` passed. UUID fixture admin/user ayrı gerçek Linux UID/GID/home, Root metadata bootstrap, ayrı service-UID server13102 ve kurulu setuid helper kullanıldı. Chromium admin.systemUsers.deny public endpoint üzerinden suspended/generation; kullanıcı socket<=5s kapandı, dosya erişimi ve login reddedildi. Public allow sonrası eski cookie401 kaldı; yeni login/generation ve gerçek helper read/list çalıştı. Normal user yönetim403, CSRF'siz mutation403, Linux ID virtual endpoint403, admin self-suspend403 passed. Üç fixture OS account/home ve yalnız kendilerine ait DB/grant/proof satırları finally'de kaldırıldı; helper config byte-identical restored. Bu artık doğrudan public admin API kabulüdür; önceki doğrudan Root SQL suspend testiyle karıştırılmaz.


## 2026-10-12 — Kayıt öncesi admin onayı

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=approval tools/dev/check.sh admin` passed. Öz kayıt kapalı, henüz DB kaydı olmayan gerçek UUID Linux user ilk PAM enrollment'da reddedildi. Root deny operator tarafından fixture'a eklendiğinde global admin approve403; kaldırıldığında approveUser metadata/passwordEnrolledfalse. Root/service names403; client role/UID/home JSON fields422; normal user403; repeat approval409. Web login PAM öncesi reddedildi; gerçek PAM begin sonrası admin suspend completion'ı reddetti. Enable eski proof'u canlandırmadı; yeni transport/PAM proof ile web password kaydı ve gerçek helper list passed. Önceki public suspend/enable/native UID dosya testi aynı fixture'da tekrar geçti. Helper policy restored, fixture OS/DB/home temizlendi.

Fixture ilk denemesinde WS başına5 auth denemesi limitine ulaşıldı; negatif senaryolar sonrası yeni handshake başlatılarak test düzeltildi, güvenlik limiti değiştirilmedi. TCP TIME_WAIT yeniden çalıştırmayı engellediği için yalnız port preflight SO_REUSEADDR kullanır; aktif listener hâlâ reddedilir. Unit proof revocation after re-enable ve SDK approval capability/options/DTO testleri passed. Root reserved service names approved=true bile reddedilir.


## 2026-10-12 — Üç işin UI kabulü ve final checks

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` passed. Gerçek gömülü UI: user management düğmesi hidden; admin suspend/enable, cancel→generation unchanged, açık file socket kapanışı, yeniden login, servislerden filtrelenmiş dropdown approve, Awaiting registration→gerçek PAM/web-password enrollment→Active passed. System/virtual tab, mobile390x844 overflow yok, Escape/focus return passed. Browser intercepted503 refresh stale mutation düğmelerini temizledi, normal refresh recover passed. Browser pageerror yok.

Root operator driver audit'i ayrıca okudu: AccountApproved/AccountEnabled/AccountSuspended actor+target ID/username var; OS/web secret string'leri yok; public API Root helper config'i byte-identical tuttu. Fixture hesap/home/grant/proof/config/log/runtime dosyaları temizlendi, original helper policy restored. Bu gerçek Linux hesap testidir; virtual UID provisioning/OS quota kabulü değildir.

UI test önce başlangıç markup'ındaki connected sınıfını oturum tamamlandı gibi kabul edip registration sonrası erken refresh aldı (backend passwordEnrolledfalse). Başlangıç göstergesi Not connected olarak düzeltildi; test loginModal.hidden + gerçek file ProtocolReady bekler. Root/PAM/DB durumu değiştirilerek test yeşillendirilmedi.

Final normal Cargo package **544 unit passed,1 explicit PAM ignored**, normal integration tests passed; Clippy all-targets -D warnings/fmt passed. Rust **1.85.0** all-target check ve strict TypeScript d.ts passed. Source diff/format checks kaydedildi. Diğer opt-in root/kernel testleri bu normal paket sayısına dahil değildir.


## 2026-10-12 — Home çözümleyicisinin ilk güvenlik katmanı

Dört yeni test: doğrulanmış home kullanımı ve template kısıtlarının korunması; legacy profili sessiz genişletme ve başka kullanıcı kökü reddi; ayrılmış sistem home'ları/inaktif hesap reddi; sanal kota tavanı ve eksik allocation=0. `cargo test --locked --quiet`: **548 unit passed, 1 explicit PAM ignored**, normal integration tests passed. Bu aşamada root/PAM/browser opt-in kabulü çalıştırılmadı; yeni otomatik home akışı henüz uçtan uca bağlı değildir. Canlı ve aday binary değiştirilmedi.


## 2026-10-12 — Otomatik home public API ve Chromium/PAM kabulü

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` passed. Üç ayrı geçici gerçek Linux UID/home; Root template config'te static roots yok. SDK session tek `home` kökü, gerçek helper read fixture, açık proje .env, hidden .ssh metadata/read=NOT_FOUND ve olmayan dosyayla aynı kod; HTTP dosya indirme; cross-home traversal reddi passed. Public admin onay/suspend/enable, canlı socket ve eski cookie revoke, PAM→web password ve UI önceki kabulü aynı dynamic-home konfigürasyonunda tekrar geçti. Driver audit actor/target/no-secret + API Root config byte-identical + fixture OS/DB/home cleanup passed.

İlk .env kabul denemesi existing warn_read pattern nedeniyle POLICY_DENIED verdi; fixture migration iki legacy pattern listesinde açık değişiklik yapılarak düzeltildi. Güvenlik kontrolü test için kaldırılmadı. Sonuç eski hidden/sensitive kuralların sessiz değişmediğini de doğrular.

Aday 127.0.0.1:13101 güncel binary'lerle legacy config'te yeniden başlatıldı; `tools/dev/check.sh live ui-auth` passed. Rust1.85.0 all-target check passed. Root/PAM/Chromium bu kabulde gerçekten çalıştırıldı; OS virtual provisioning/native quota/undo bu testin kapsamında değildir.

Son normal paket: **550 unit passed,1 explicit PAM ignored**; normal integration tests passed. Clippy all-targets -D warnings, fmt, diff ve Rust1.85.0 all-target check passed. Yeni test automatic mode için web-account zorunluluğu/static root karıştırma/invalid root ID/runtime PATCH reddi/serde roundtrip'i ve iki ayrı home'un aynı root ID ile snapshot paylaşamamasını doğrular.


## 2026-10-12 — Protected policy SDK/UI/PAM kabulü

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_POLICY_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` passed. Public SDK personal add/remove/set, admin get/add/set, correct kind and owner scope, expectedGeneration/expectedRevision conflict, self-change clears SDK session, old live socket revoke, hidden file direct read NOT_FOUND, personal layer clear leaves admin chmod denial passed. Server snapshot'ta kayıtlı olmayan DB kısıtını helper bağımsız uyguladı. Native UI kişisel checkbox save→relogin→persisted restriction ve admin checkbox/remove save→persisted admin layer clear passed. Root settings/policy dosyası byte-identical; PolicyChanged audit both actors+target verified; temporary DB policy records/grants/OS users/home cleanup passed.

Ek store unit: personal/cross-account/Root layer authority, atomic generation/CAS, malformed JSON fail-closed/invalid path no commit, virtual tenant admin only-own-child + cannot replace personal layer. SDK deterministic capability default false, camelCase DTO, revision-bound rule ID, invalid path, Root rule removal rejection, CSRF payload and virtual default routing passed. Normal package553 unit passed,1 PAM opt-in ignored; normal integrations passed. Clippy/fmt/MSRV1.85.0/strict d.ts ve final diff checks kaydedildi.

UI native test ilk yeni denemede yanlış `#loginBtn` test selector'ında timeout verdi; gerçek `#loginForm button[type=submit]` selector'ı kullanılarak düzeltildi. Uygulama güvenlik kontrolü değişmedi.


## 2026-10-12 — Sanal UID/GID/native quota/SDK/UI kabulü

Application acceptance ve native acceptance passed:

```sh
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_VIRTUAL_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_VIRTUAL_ACCEPTANCE=1 TEALDRIVE_NATIVE_QUOTA_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Public SDK creates two distinct locked/nologin OS UID/GID identities, native helper owner reads/writes, idempotent replay same account/no password reset, parent budget/no overbooking, normal-user provisioning403, virtual self-revoke403, cross UID Unix cat denied. Quota limit/usage comes from helper; too large file DISK_FULL. Native probe writes directly as UID outside helper and kernel rejects. Quota shrink below actual usage409, stale generation409, increase invalidates target live socket and new session sees new limit. Delete invalidates sessions and keeps files for retention. UI creates a third virtual account, changes quota and deletes via SDK; cancel does not create.

Root actor/target audit and no credentials confirmed. Driver removes fresh virtual groups/users/home/policy/intents/sync/grants first, then its Linux fixtures; own admin budget/pool/cursor restored, temporary ext4 image unmounted/deleted. Original Caddy/data untouched. Native test first hit duplicate State/Actions Deleted locator; exact State cell fixed. Text save quota error exposed missing DiskFull translation; backend mapper fixed and regression unit added.

Normal package557 unit passed,1 explicit PAM ignored, normal integrations passed. Store tests pending reservation/finalization without double charge, expected generation/owner before callback, callback failure transaction rollback, delete CAS passed. Trash quota refusal preserves source/no published metadata. MSRV1.85.0/Clippy all-targets -D warnings/fmt/strict SDK d.ts checks passed.


## Wizard bağımsız kurulum kabulü — 12 Ekim 2026

`cargo test --locked --quiet`: 562 unit passed, 1 ignored; normal integration grupları geçti. Root/PAM opt-in testler bu komutla çalışmış sayılmaz. `cargo clippy --locked --all-targets -- -D warnings`, MSRV1.85 ve build kontrolü yapıldı.

`TEALDRIVE_WIZARD_ACCEPTANCE=1 python3 tools/dev/wizard-acceptance.py` geçti: namespace-isolated clean install, existing install rejection, helper4755/diğer755, DB/key600, Root/service approval rejection, update key/config preservation, separate binary rollback, metadata backup/restore, repair, checksum tamper rejection ve data-preserving uninstall. `/home/tealdrive`, `/etc/systemd/system`, `/etc/pam.d`, `/proc`, systemctl yalnız test namespace'inde bind edilir. Gerçek unit start/stop bu testin kapsamı değildir. PAM/local init ve native recovery ayrıca doğrulanacak.


`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` geçti: public admin/UI/home testlerine ek gerçek UID own status/policy, Root account/service management reddi, self-revoke, gerçek PAM re-enrollment, TTY ECHO kapalı, parola çıktıda yok, UID/rol sabit. Fixture hesapları/home/DB/policy/logları finally temizlenir; primary helper policy byte-identical geri gelir. Son normal test sayısı562 passed/1 ignored.


## Root sanal admin / native lifecycle kabulü — 12 Ekim 2026

Normal test563 passed/1 ignored; Clippy-Dwarnings ve MSRV1.85 kontrolü geçti. Root pending pool rezervasyonu/atomic finalization unit testi eklendi.

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_VIRTUAL_ACCEPTANCE=1 TEALDRIVE_NATIVE_QUOTA_ACCEPTANCE=1 TEALDRIVE_LIFECYCLE_ACCEPTANCE=1 TEALDRIVE_ROOT_VIRTUAL_ACCEPTANCE=1 TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` geçti. Gerçek Root Wizard secure-TTY sanal admin create, independent nologin UID/GID, browser tenant authority (Linux yönetimi403), own file usage budget azaltım reddi ve idempotent retry; native kernel quota direct UID write reddi ve UI quota/delete;36 saat önce purge0, fixture timestamp sonrası UID-dropped/Landlock worker purge1, mode000 directory, outside symlink marker byte-preserved, native UID removal, budget release/passwordNULL/UID fence. Completion marker test fixture'da kaldırılarak OS removal sonrası restart recovery tekrar geçti. Own UID PAM revoke/re-enroll da aynı koşuda geçti. Yalnız bu run fixture metadata/UID/home/config/pool/cursor temizlendi; primary Root policy byte-identical, Caddy production untouched.

Rootvirtual testinde ilk deneme yanlış HTTP route'u `/api/admin/system/accounts` kullandığı için başarısızdı; gerçek `/api/admin/users` ile403 doğrulandı. Yanlış route sonucu yetki açığı olarak raporlanmadı. Timer gerçek systemd etkinleştirme/boot kabulü final kurulum aşamasındadır.


## Shared SDK file-manager UI — 12 Ekim 2026

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_UI_FILES_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` geçti: browser UI yalnız tek/ws/auth socket; create directory/file, edit/read, upload, permissions selector, reload managed location, independent second SDK writer revision conflict ve unsaved text preservation, directory dışarıdan trash'e taşındığında refresh nearest-parent recovery. Test yalnız fixture UID/home'unda işlem yapar, cleanup primary policy/DB/home/logları geri alır.

İlk UI test hatası hidden modal locator'ın visible beklemesiydi; corrected state=attached. Sonra read tamamlanmadan editor input açıldığı eski yarış ortaya çıktı; UI artık read sonrası açılır ve eski editor request generation'ı atılır. Normal cargo test563 unit passed/1 ignored ve integration grupları geçti. Browser permissions/shutdown mocks yeni public SDK event sözleşmesine taşındı; config inspect/persist/reload/CSRF ve native URL invalid-path/root testleri geçti. d.ts TypeScript strict geçti.


## TR/EN UI kabulü — 12 Ekim 2026

Shared SDK UI fixture `TEALDRIVE_UI_FILES_ACCEPTANCE=1` koşusuna TR/EN testi eklendi ve geçti: language selector, Turkish labels, policy dialog input sayısı dil değişikliğinde sabit, “Cancel” filename untranslated, reload language restore, aynı context'te logout→diğer admin login ile yeni dil namespace'i, storage'da PAM/web password yok. Normal test564 unit passed/1 ignored ve integrations geçti; catalog same-key/interpolation-name testi var. Root/PAM opt-in koşular ayrı kanıtlanır. Script/locale JSON embedded route/digest'e eklenmiştir. CLippy-Dwarnings/MSRV1.85 kontrolü geçti.


## Direct TLS / bağımsız proxy kabulü — 12 Ekim 2026

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_TLS_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` geçti. Self-signed TEST sertifikasıyla directHTTPS13103 ve ayrı CaddyHTTPS13104→loopback backend13105: realChromium login/WSS, Secure+HttpOnly cookie, SDK helper UID file listing, signal shutdown popup ve unsaved editor text preserved. Selfsigned ignoreHTTPSErrors yalnız test context'te kullanılır. PlainHTTP directTLS portunda accepted200 yok; invalidPEM key, worldread key, symlink key startupfail/noHTTPfallback. Backend missingHTTPSheader426, spoofedHTTPSheader untrusted127.0.0.2peer426. Production Caddy file/token/process untouched; isolated Caddy adminoff, ownXDG dirs. Credential bytes audit/loga çıkmadı; private fixture/helperpolicy cleanup tamamlandı.

İlk port-unbind kontrolü TCP TIME_WAIT nedeniyle bind hatası verdi; testSO_REUSEADDR ile live-listener kontrolü düzeltildi (SO_REUSEPORT kullanılmaz). Axum-server0.7.2 currenthyperutil trait boundary error;0.7.3 fixedversion ile Clippy-Dwarnings/MSRV1.85 passed. Normal tests566 unit passed/1 ignored +integration grupları geçti; transport unsafe/ambiguous modes, critical restart equality tests var. Legacy implicit mode backward compatible; newproduction require_tls explicittrue.


## Root custom UI paketi kabulü — 12 Ekim 2026

`TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_UI_PACKAGE_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin` geçti: uniqueRoot-owned SHAmanifest package, CLIcheck, customHTML/script fixedSDK auth+UID file list; undeclaredasset404; activecustom sırasında `/ui/default` fullSDK/browserfilelist; incompatibleSDKmajor wholefallback; manifestSDKoverride reject/fixedSDK intact.13106 server/privatepackage/audit fixture cleanup; permanentUI/serverconfig/helperpolicy untouched. Normal567 unit passed/1 ignored +integrations, Clippy-Dwarnings/MSRV1.85 passed. Asset8MiB/total16MiB bound mevcut source şemasıdır; özel packagecode Roottrust gerektirir.

## 2026-10-13 — Hesap metadata sayfalama kabulü

Normal paket568 unit passed/1 PAM ignored; bütün normal integration grupları geçti. fmt ve Clippy all-targets -D warnings passed. Gerçek Root/PAM/Chromium/native quota kabulü: Linux kayıt yenileme, suspend/enable, ayrı UID/GID, nologin/locked hesap, sanal adminin kendi tenant'ı, filtreli limit1 devam sayfası ve eski array uyumu passed. Sanal admin UI bütçe dialog'u da test edildi; eski undefined ticket hatası giderildi. Fixture cleanup ve helper policy restore passed; production değiştirilmedi. Testte sayfalı yanıtın items alanı okunacak şekilde assertion düzeltildi. SDK UTF-8 byte sınırını backend ile aynı uygular.

## 2026-10-13 — Kalıcı giriş sınırları

Normal paket570 unit passed/1 PAM ignored ve normal integration grupları passed; fmt/Clippy -D warnings ve Rust1.85 all-target check passed. Root protected SQLite kabulü12 paralel connection'da max3 rezervasyon, yeni connection'da devam eden engel ve pencere sonunda yeniden izin doğruladı. Unit testler farklı hesapların ortak IP/global sınırları, IPv6 normalizasyonu, yalnız başarılı attempt refund'u ve secret-free hashed keys doğruladı. Gerçek UID/PAM, own-user Wizard, shared-SDK CRUD/state/EN-TR, custom package/fallback ve direct TLS + local proxy HTTPS/WSS/cookie/shutdown kabulü yeni binary'lerle geçti. Üretim proxy/config değiştirilmedi, fixture cleanup/helper policy restore passed.

## 2026-10-13 — Wizard dedicated native storage

Gerçek özel mount/PID namespace içinde clean install, ext4/usrquota128MiB preallocated image, Root600 file, kernel Q_GETFMT/Q_GETQUOTA, kapasite/headroom, nonempty mountpoint koruma, tekrar init reddi, mount idempotency, unmount/remount ve native pool kapasite üstü reddi passed. systemd-escape çıktısı mount unit adı ve service dependency ile eşleşti. Sonrasında update/rollback/metadata backup/restore/repair/tamper reject/data-preserving uninstall passed. systemctl yalnız namespace stub olduğundan gerçek boot/service başlatma son kabulde ayrıca yapılacak. İlk hata distro mkfs.ext4 symlink'inin Root utility kuralıyla reddedilmesiydi; gerçek mke2fs -t ext4 kullanılarak giderildi. Native test config değişikliği test sonunda önceki Root config'e geri alındı. Normal570 unit/1 ignored ve integration grupları, Rust1.85 all-target, fmt/Clippy -D warnings passed. Üretim mount veya proxy değiştirilmedi.

## 2026-10-13 — Root supervisor/User worker kabulü

Normal571 unit passed/1 ignored ve normal integration grupları passed; fmt/Clippy -D warnings passed. Root private delegation kabulü: doğru MAC ile user UID'deki worker Root600 file içeriğini okuyamaz; imzalandıktan sonra path/UID değiştirme, expired delegation ve Root-created authorization'ın unprivileged parent'tan replay'i reddedilir. Gerçek PAM/Chromium sharedSDK CRUD/upload/chmod/state, protected policy CAS/revoke/bağımsız helper denial, native ext4/kernel quota, Root virtual admin/tenant/own-usage guard ve36h purge/crash-resume geçer; helper policy restore/fixture cleanup passed. İlk normal testte identity failure malformed response'a dönüşmüştü; structured PermissionDenied uyumu geri getirildi. Private Worker MAC/grant signing RPC olarak sunulmaz.

## 2026-10-13 — Atomik move/trash no-overwrite

Helper140 testten139 passed/1 PAM ignored. Kernel primitive testinde validation sonrası external destination oluşturuldu; kaynak ve hedef içerikleri korunarak AlreadyExists döndü. Search/trash/restore/current-policy denial ve alias testleri geçti. Gerçek Chromium shared-SDK CRUD/upload/chmod/state/dir recovery ve Root admin/enrollment/suspend acceptance passed; production untouched/fixture cleanup. Clippy all-targets -D warnings passed.

## 2026-10-13 — Protected operasyon journal temeli

Normal574 unit passed/1 ignored ve normal integration grupları, fmt/Clippy -D warnings, Rust1.85 all-target check passed. Unit: intent→result, interrupted unknown, repeat finish conflict, iki kullanıcı/cursor scope isolation, action filtre, content exclusion, journal cap ve expiry reclamation. Özel Root/PAM/Chromium helper config'te journal açılarak sharedSDK CRUD/conflict/upload/chmod/state çalıştırıldı; RootDB gerçek success/failed sonuçları ve credential exclusion doğrulandı; tamamlanan fixture'da pending yok. Fixture temizliği yalnız kendi account ID operation kayıtlarını kaldırır; üretim helper config byte-identical restore. Kullanıcı history/undo/kota capability henüz açılmadı.

## 2026-10-13 — Own history gateway/API ve geniş SDK sorgusu

Normal575 unit passed/1 ignored, normal integration, fmt/Clippy -D warnings, Rust1.85 all-target ve strict TypeScript passed. Gerçek iki Chromium/UID oturumu kendi history/page/continuation, foreign cursor ile scope değişmeme, unknown accountId query400, current personal deny sonrası eski path'in de kayıtlardan gizlenmesi ve content exclusion passed. Kombine action/status/rootId/path/from/to filtresi limit1 öncesi uygulanır, ters time range400. SDK fake transport capability gate, typed camelCase/immutable result, UTF-8 byte bounds, unknown scope/status/reverse range/requestId rejection passed. İlk browser test private olmayan setPersonal adı kullanıyordu; gerçek public policy.set ile düzeltildi. Production config restored/fixture cleanup passed; public SDK history capability henüz kapalı.

## 2026-10-13 — Canlı Caddy yönetim izolasyonu

Önce service UID995 Caddy TCP/admin config200 ve dns credential field present bool doğrulandı; secret değerleri çıkarılmadı. Private Unix socket sonrası service TCP denied(ECONNREFUSED), Unix denied(EACCES), Root Unix config200. Systemctl reload ve local origin HTTPS200 passed; Caddyfile yalnız admin directive farkı. Harici CF edge403 olduğu ayrı kaydedildi. Reusable read-only tools/dev/proxy-admin-check.py passed; private runtime rollback/validate/reload logs Root600 outsideGit.

## 2026-10-13 — Actual mode ve expected-state gözlemi

Normal577 unit passed/1 ignored; fmt/Clippy -D warnings, Rust1.85 all-target ve strict TypeScript passed. Unit same-size content edit ve deep child edit digest değişimini doğruladı; hidden .ssh doğrudan snapshot reddi ve parent tree fingerprint'in incomplete olması doğrulandı. Snapshot tree enumeration maxnode sınırı list allocate edilirken uygulanır; leaf read buffer heap/zeroizing64KiB, recursion stack büyümesi önlendi. Gerçek UID/PAM/Chromium root journal: chmod old/new ve after.mode gerçek metadata, rename/move/trash before-after inode continuity, own history/filter/policy ve credential exclusion passed. Fixture cleanup/root policy restore passed. Chmod metadata-only expected condition geçici permission probe gerektirmez; içerik snapshot sadece move/rename/trash kapsamındadır. Undo apply/ledger/GC/UI entegrasyonu henüz sürüyor.

## 2026-10-13 — Sanal log quota/native/application/GC

Normal578 unit passed/1 ignored, normal integration, fmt/Clippy -D warnings, Rust1.85 all-target ve strict TypeScript passed. Gerçek Root/PAM/Chromium native quota image: User UID400 file +Root700parent inaccessible, inode/block charge, helper quota/account budget küçültme koruması, Native dış-write EDQUOT, public SDK full-quota live permanent delete systemReserve, typed own history/filter/hidden policy,504h aged own record+physicalfileGC ve36h retained child purge/crash-resume passed. Application-mode kabulü de home+ledger usage/budget ve aynı isolation passed. Old fixed8KiB fixtures log cost nedeniyle dynamic getBudget.minimumQuotaBytes/real budget ile güncellendi; overbook/min/shrink/CAS assertionları korundu. Global/account count rotation unit başka kullanıcının kayıtlarını korudu. Fixture own ledger/account/OS identity cleanup ve helper policy restore passed. Caddy private admin isolation tekrar passed.

## 2026-10-13 — Protected undo backend, tercih ve restore fence

581 unit passed/1 ignored ve normal integration; fmt/Clippy -D warnings, strict TypeScript ve Rust1.85 all-target passed. Explicit Root worker delegation/forgery testi passed. Tam birleşik Root/PAM/Chromium fixture (home/policy/virtual/native/lifecycle/Root admin/local wizard/UI files/TLS/package/journal) passed: own prefs null→Root upper, CAS stale rejection, mode000 chmod undo, file rename/move/trash ve nested directory undo, repeat claim rejection, changed contents REVISION_CONFLICT ve destination ALREADY_EXISTS sonrası iki dosyanın da korunması. Sanal gerçek UID/native quota altında rename undo passed. Admin foreign operation403, own history/current policy, native ledger charge/full-quota cleanup,504h logGC ve36h purge/crash-resume passed. Fixture cleanup/helper policy restore passed; production backend değiştirilmedi. İlk fixture çöp kutusu adayını private path nedeniyle gizliyordu; görünürlük original logical path üzerinden düzeltildi ve tam kabul tekrar geçti. Unit offline restore epoch rotation ve audit preservation, duration clamp/CAS/generation unchanged ve single-use durable claims passed. Gömülü history/undo UI ve genel trash/staging GC sonraki aşamalardır.

## 2026-10-13 — Gömülü history/undo UI

Gerçek Root/PAM/Chromium application quota fixture passed: own dialog/filter, SDK üzerinden cancel(no mutation)/confirm(real restore), own duration pref persist/inherit, Escape close ve launcher focus restoration. Mode000/rename/move/trash/directory/changed content/occupied target/foreign actor negatif testleri aynı kabulde tekrar passed. Fixture private policy restore/cleanup passed. Başlangıçta Playwright implicit browser.newPage context ikinci page'i reddediyordu; explicit newContext kullanıldı. Async launcher disable nedeniyle focus body'ye kayıyordu; dialog explicit restoreFocus ile düzeltildi ve kabul tekrar geçti. UI module embedded asset/version/default recovery namespace'e eklendi; Türkçe/İngilizce etiketler dosya adlarına uygulanmaz. ui unit5 passed, fmt/Clippy -D warnings, sdk regression, strict TypeScript ve Rust1.85 all-target passed. Genel lifecycle/gerçek servis migration son kabulü henüz tamamlanmadı.

## 2026-10-13 — Root journal ceiling regression

Snapshot enabled=false + değiştirilmiş max_entries/max_total/max_record_bytes/undo_max testinde restricted.journal Root config ile birebir eşleşti. helper policy3 unit passed; fmt/Clippy -D warnings passed. Bu düzeltme file operation client snapshot ile protected journal bypass edilmesini önler.

## 2026-10-13 — Watchdog thread confinement

Explicit Root kernel test User nobody UID/GID65534 için world-readable dış dosyaya DAC baseline erişimini doğruladı; Landlock sonrası yeni thread ve onun child thread'i dış dosyayı okuyamadı, izinli iç dosyayı okuyabildi ve PR_GET_NO_NEW_PRIVS=1. Private worker valid/tampered/expired/replayed delegation ve Root600 file denial tekrar passed. Helper142 unit passed/1 ignored; fmt/Clippy -D warnings passed. Normal worker yalnız sandbox sonrası watchdog oluşturur; Root supervisor watchdog privileged control tarafında kalır.

## 2026-10-13 — Protected trash expiry ve audit-independent token

Gerçek Root/PAM/Chromium application fixture:504h aged unchanged trash User worker tarafından silindi, changed contents korundu; CLI trash_cleaned1/trash_skipped1 ve expired operation logs2 doğrulandı. Own history/undo/UI, virtual quota/ledger, old504h log physical release ve fixture cleanup/helper policy restore aynı kabulde tekrar passed. Unit token successful record'i audit TTL sonrası da korudu; own token cap100 sonrası yeni intent atomik reddedildi ve Root journal orphan row oluşmadı. Lifecycle hours/work bounds doğrulandı. accounts31 unit passed, fmt/Clippy -D warnings, strict TypeScript, Rust1.85 all-target ve private worker MAC/UID denial passed. Legacy user-info purge production yüzeyinden çıkarıldı. Stage GC, protected auth/admin control log ve fiziksel migration devam ediyor.

## 2026-10-13 — Subordinate ID fence ve tam Native kabul

Parser/interval root vs nonroot ownership, overlap/overflow/zero/malformed değerler unit passed; fmt/Clippy -D warnings ve Rust1.85 all-target passed. Gerçek tam birleşik Root/PAM/Chromium Native fixture virtual UID/GID değerlerinin nonroot subuid/subgid içinde olmadığını, numeric UID0 reservation bulunduğunu ve generated OS hesapların subordinate mapping almadığını doğruladı. Native EDQUOT/ledger/quota/CAS, Root virtual admin tenant sınırı,36h purge/crash-resume,504h log/trash cleanup, own undo/UI ve TLS/package/security policy kabulü aynı çalışmada passed. Helper config, global pool/cursor ve subordinate ID dosyaları exact içerikle geri geldi; file modes644/owner0/group0 korundu. Fixture safe1m UID range kendi OS users/reservations ile tamamen temizlendi.

## 2026-10-13 — Exact staging cleanup

Root/PAM/Chromium application kabulünde private24h as-of sentetik Root token ve User-owned staging dosyası gerçek Wizard worker tarafından temizlendi; recent mtime/ctime koşulu taşıyan dosya korundu; host clock değişmedi. Root-level staging parent split hatası bulundu/düzeltildi ve kabul tekrar geçti. Unit pending/unknown token'ın audit TTL sonrasında kalmasını, unrelated path reddini ve successful publish sonrası token removal'ı doğruladı. Lifecycle3 unit passed, fmt/Clippy -D warnings, Rust1.85 all-target passed; history/undo/UI, subordinate ID fence, ledger/quota/trash aynı kabulde tekrar passed. Fixture stage token/OS account/policy/subid cleanup passed. General protected control/auth audit ve fiziksel servis migration sürüyor.

## 2026-10-13 — Protected auth/control audit ve offline lifecycle regression

587 unit passed/1 ignored, normal integration, fmt/Clippy -D warnings ve Rust1.85 all-target passed. Control unit intent-before-result, double completion conflict, finished rotation, pending-only cap rejection ve control-char claims reddini doğruladı. Gerçek Root/PAM/Chromium Login/SetAccountAccess successful protected facts ve tüm fixture password/PAM credential absence passed. History/undo/UI/subid/quota/ledger/trash/staging kabulü aynı çalışmada passed. Mount/PID namespace Wizard clean install/reinstall reject, native image/remount, update/rollback, metadata backup/restore/repair, tamper rejection ve uninstall passed. Kaldırma DB byte-equality testi yeni control event nedeniyle geçersizdi; bütün non-control metadata + PRAGMA integrity + Wizard_uninstall success fact ile doğrulandı. Recovery sırasında eski DB handle tutulmaz.

## 2026-10-13 — HTTP range audit ve son birleşik kabul

Journal birim testleri5/5: iki ardışık parça tek Root kaydında6bayt; pending/replay/farklı path reddi; içerik kayda girmez. Clippy all-targets/Dwarnings geçti. Tam Root opt-in Chromium/PAM/native ext4 kabulü geçti:100000bayt gerçek HTTP download tek own-history kaydında; TLS/WSS/customUI/CRUD/chmod/undo/CAS/tenant isolation, subordinate ID dışlama, Root virtual admin, native log kotası,504saat log ve trash,24saat staging,36saat account purge/resume, protected auth/control audit. Fixture temizlendi, Root helper policy geri yüklendi, mevcut üretim korunmuştur.

## Final release/systemd kabulü — 2026-10-13

Son general588library passed/1PAM ignored; PAM ayrı Root private fixture'da passed. Normal helper binary/integration passed. Explicit guarded Root migration/SQLite/concurrency/counters3, chmod0001, descendants1, worker1, sameUIDticket1, runtimeSettings1, real-helper lifecycle/cancellation2 passed. fmt/ClippyDwarnings/Rust1.85alltargets/release/strictTypeScript passed. İki host-dependent bare-helper testi live roots=[] ortamında başarısız olmuştu; runtime policy gevşetilmeden açık fixture ile düzeltildi ve tam suite tekrar passed.

Wizard namespace PATH conflict/nooverwrite/repair/uninstall ve4ctl protected intent testleri passed. Gerçek final-release install/update sonrası4binary SHA256 source build ile eşleşti; mevcut Linux identity/role/web-password-hash backup ile aynı. NSS servicehome doğru; native mount/service/timer enabled/active, gc success.26legacy regular file hash ve original browser file60B korundu.

Production Chromium HTTPS/WSS, secure HttpOnly cookie, original file read, create/save/stale-revision denial, streaming download, HTTP requestId→protected record, chmod/undo, systemd restart/shutdown popup/unsaved input/relogin passed. Direct origin ve public Cloudflare browser login passed. Caddy systemd reload ve private admin isolation passed.

Backend standard Rustls TLS+dedi­cated CA trust etkinleştirildi. Verified backend health200, origin200, plaintext000/rejected. Isolated Caddy same-name fake certificate502 ile reddetti; fake backend0HTTP request. Sertifika doğrulaması kapatılmadı. Final production CRUD/restart kabulü bu topology ile yeniden passed. Private key Root:tealdrive0640; public cert Root644; expiration2027-10-13T21:06:35UTC.

Production UUID fixture files0; Wizard temp dirs0; shared-policy driver live server varken mutation başlamadan reddetti. Private rollback/audit/user source verileri cleanup olarak silinmedi.

## 14 Ekim 2026: sabit admin kişisel kotası

40 account library testi geçti (eski remainder davranışı dahil). Yeni testler: Root-only atama, callback/DB rollback, generation yenilenmesi, sabit own limit korunarak çocuk create/resize, own+children+pending toplamından aşağı budget reddi, aşırı tahsis ve bozuk korumalı değer reddi. Clippy all-targets -D warnings ve fmt geçti. Gerçek private NSS/mount/PID namespace Chromium kabulünde 64 MiB admin bütçesi içinden 32 MiB own quota ayrıldı; 17 MiB stream upload/download/Range ve iptal/tamper/generation revoke kabulü geçti. Mevcut kullanım 17 MiB iken own quota 8 MiB'ye indirme reddedildi. Native `storage sync` ve image net64→128MiB büyümesi sonrasında dosya SHA256 korundu. Production DB/config/homes değiştirilmedi. Source binaries dev profilinde build edildi; disk yaklaşık95GiB boş.


## 14 Ekim 2026: gerçek Caddy/backend TLS yenilemesi

`cargo test --locked --lib wizard::tls_renewal -- --include-ignored`:2passed (gerçek Root/OpenSSL fixture dahil). Clippy all-targets -Dwarnings geçti. `TEALDRIVE_CORE_TRANSFER_ACCEPTANCE=1 TEALDRIVE_CORE_VIRTUAL_ADMIN=1 TEALDRIVE_CORE_TLS_RENEWAL=1 TEALDRIVE_TEST_WEB_PASSWORD_FILE=PRIVATE_PATH python3 tools/dev/core-transfer-acceptance.py` geçti. Test tüm TLS dizinini private mount'a ayırır; gerçek Caddy private Unix admin ile çift trust reload, aktif backend pair yenilemesi,60s canlı TLS refresh, exact DER identity, HTTP200 health, eski trust retirement, pending private key cleanup, fresh certificate için dailycheck no-op doğrulandı.17MiB/nativequota/adminscope/imageexpand kabulü de aynı koşuda passed. Host production TLS/config/NSS/image korunur. Disk95GiB boş.

## 14 Ekim 2026: ek Noise WS şifrelemesi

Native shared channel: fresh ephemeral keys, wrong key, replay ve bit-flip rejection geçti. Cacophony Noise_NK_25519_ChaChaPoly_SHA256 bilinen handshake/ciphertext/hash ve directional transport vektörü geçti. Gerçek Chromium WASM↔Rust kabulünde bağımsız pin, yanlış pin, plaintext pre-auth istemci reddi, replay, tamper, nonfinal kısa fragment reddi ve 17MiB upload/download/Range/abort/scope-revoke/image-expand geçti. Aynı kabulde gerçek private Caddy TLS dual-trust renewal de geçti; Noise ve TLS birlikte çalıştı. Normal workspace testleri599 library passed/2explicit ignored, channel unit+vector ve integration targets passed. İki eski e2e Cache-Control exact beklentisi mevcut private,no-store/Vary:Cookie güvenlik davranışına güncellendi. Root overlay key-path regression geçti. Wasm artifact tools/dev/build-channel.sh ile Cargo.lock üzerinden yeniden üretildi; hash değişmedi. Strict TypeScript consumer fixture ayrı doğrulanır. Snow upstream formal audit iddiası yoktur. REST/cookie/native HTTP Noise kapsam dışındadır; yalnız TLS ile korunur. Production release/key/config henüz değiştirilmedi.

## 14 Ekim 2026: ortak staging ledger

24 upload testi geçti. Yeni gerçek child-process testi: başka prosesin80byte aktif rezervasyonu varken30byte ek rezervasyon100byte cap altında reddedildi; child SIGKILL sonrasında10byte kalan staging yeniden sayıldı,90byte rezervasyon kabul edildi,91byte reddedildi. Transfer-count sınırı ve orphan/legacy file charge, symlink ledger rejection doğrulandı. Clippy workspace all-targets-Dwarnings geçti. Private native quota/Chromium/Noise17MiB upload/download/abort/tamper/scope revoke/image expansion kabulü yeni file ledger ile yeniden koşulur; sonucu nihai aşama raporuna yazılır.

## 14 Ekim 2026: cross-connection upload resume

26 upload library testi geçti; registry session/UID isolation, policy recheck, prefix digest, expiry ve session revocation cleanup testleri dahil. Workspace603library passed/2explicit ignored; channel unit/vector ve normal integration geçti. Gerçek Chromium+Noise+native helper kabulünde bağlantı ilk chunk sonrasında kesildi, yeni WS/Noise handshake aynı HttpOnly session'ı devraldı, prefix hash doğrulanarak2MiB dosyanın kalan kısmı tamamlandı ve bütün içerik eşitlendi. Wrong-file prefix denemesi reject/abort edilir; failed target yokluğu kontrol edilir. 17MiB büyük transfer, abort/tamper/global scope revoke/native quota ve image expansion aynı koşuda geçti. Strict TypeScript tüketici fixture yeni resumable/resumeTransferId/onTransfer API'leriyle geçti. Serverrestart sonrası otomatik upload resume bilinçli desteklenmez; staging actual bytes ortak ledger'da sayılır ve Root GC temizler.


## 14 Ekim 2026: random bootstrap/ilk parola değişimi

41 account testi geçti; restricted grant operations denial, Root-only flag, generation revocation, aynı başlangıç parolasının reddi ve başarılı parola değişimi testi dahil. Private namespace fresh auth records + native image üzerinde `bootstrap admin64MiB32MiB` random password/global virtualalias oluşturdu; ikinci çağrı aynı accountID döndürdü. Gerçek Chromium+Noise ilk girişte zorunlu parola modalı, session roots empty/isAdminfalse, admin403, SDK PASSWORD_CHANGE_REQUIRED ve client flag'i çıkarılmasına rağmen server PERMISSION_DENIED doğrulandı. Geçici parola yeni43byte test parolasıyla değiştirildi, yeniden login/general admin API200 ve tüm17MiB/resume/prefixmismatch/abort/native quota/scope/image kabulü geçti. Üretim account/credential/home değiştirilmedi.


## 14 Ekim 2026: Linux-admin → virtual-admin migration

Root credential migration unit: farklı caller/stale source veya target generation reddi, eski ve yeni loginlerde aynı özgün parola kabulü, yeni UID korunması passed. Descriptor copy unit: içerik/manifest eşitliği, byte/inode budget reddi, var olan hedefe yazmama, symlink/hardlink reddi passed. Private mount/PID/NSS/native volume fixture'da gerçek onaylı test Linux hesabının protected hash'iyle sanal tealdrive oluşturuldu; test/hello gpt.md orijinal içerik/sourceUID korundu, hedef generatedUID oldu, iki hash eşitliği değerleri loglamadan doğrulandı. İkinci migration aynı hedefi idempotent döndürdü. Chromium yeni sanal kullanıcıya özgün test web parolasıyla giriş yaptı; global yönetim,17MiB/noise/resume/prefixmismatch/abort/quota/scope/imageexpand ve gerçekCaddyTLS kabulü passed. Üretim migration henüz yapılmadı.

## 14 Ekim 2026: clean install ve SDK lifecycle son kontrolleri

Fresh native install seçenekleri gerçek private NSS/home/systemd namespace testinde geçti; mevcut mount unit'inin fixture'a kopyalanması temiz kurulum senaryosunu bozduğu için yalnız private fixture units temizlendi. Eski install/update/binaryrollback/metadatarestore/repair/uninstall/remount kabulü yeniden geçti. Rust1.85 locked workspace/alltargets check passed. Normal workspace609library/607passed/2explicitignored; channel unit/vector ve integration targets geçti.

Eşzamanlı loader kabulü wasm-bindgen global instance init yarışını yakaladı. Deterministic finalize-channel-runtime.py postprocessing tek pending init promise sağlar; async init sırasında sync init reddedilir. Bu özel kripto değildir, WASM lifecycle guard'dır. Node gerçekWASM testi iki initializer'ın aynı promise/exports/memory'yi paylaştığını, eski Noise Rust handle'ının start/free ile bozulmadığını ve incremental SHA256 abc vektörünü doğrular. Gerçek Chromium aynı loader ile iki pinned WS client'ı eşzamanlı başlattı ve tüm bootstrap/passwordchange/17MiB/resume/prefixmismatch/noise/adversarial/TLSdualtrust/imageexpand kabulü geçti. SDK1.1.0 additive noiseLoader/typed transport adapters/abortUpload API sağlar; finalization kayıp cevap `UPLOAD_COMMIT_UNCONFIRMED`, UI iptal düğmesi commit aşamasında kapalıdır. Retained transfer abort yalnız aynı session'a aittir (unit passed).
