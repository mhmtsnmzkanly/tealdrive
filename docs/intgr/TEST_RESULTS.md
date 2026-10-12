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
