# Çekirdek aşamalar — tamamlandı ve canlıda

14 Ekim 2026. Yetkilendirilen iki aşama tamamlandı; üretim kabulü ve çıktı temizliği yapıldı. Detaylı [bitiş raporu](FINAL_REPORT.md), [test kanıtları](TEST_RESULTS.md), [kararlar](DECISIONS.md).

## Tamamlanan sıra

1. UI/cookie bridge/oturum, büyük dosya bounded helper aktarımı, download/Range, cancel/progress, aynı session upload resume.
2. Sabit admin own quota ve atomik tenant/pending tahsis muhasebesi (`b01b461`).
3. TLS prepare/activate/renew/auto, strict Caddy trust/aktif ayar eşitliği (`991a7b4`, `bebfda4`).
4. Standart Noise NK native/browserWASM, pins/downgrade/replay/tamper kontrolleri (`49ee5dc`).
5. Süreçler arası staging budget ledger (`354475b`), reconnect/prefix hash (`e4a38bb`).
6. Fresh native install/random restricted virtual global admin (`0657ddd`), mevcut Linux admin dosya/hash migration (`afe120c`).
7. SDK1.1, WASM singleton initialization, belirsiz commit sonucu, retained abort (`9d0b240`, `b35dc46`, `87e5c3e`); migration O_PATH/FIFO güvenliği (`088e584`); bounded256KiB HTTP ve büyük aktarım kabulü (`f5a915e`).
8. Dört binary release20261014-87e5c3e; production migration, Noise/TLS timer,1GiB upload ceiling; direct/public Chromium kabulü ve temizlik.

## Son üretim durumu

- URL https://tdrive.novastrum.dev; server/Caddy active, health200; installed four-binary manifest hash eşitliği doğrulandı.
- Web admin `tealdrive`:virtual/global/UID200000; home `/home/tealdrive/virtual-homes/tdv_200000`; önceki web parola hash'i aynıdır. Eski `tealdrive-admin` web hesabı suspended; orijinal UID1001/home korunur.
- Linux service `tealdrive` UID995/GID986 locked/nologin; web dosya kimliği değildir.
- Image1GiB/native quota; netpool512MiB; adminbudget512MiB/own128MiB/childremaining384MiB. Kullanıcı geçmiş/trash kendi kotasında.
- Backend TLS+strict Caddy; Noise WS etkin, HTTP/REST TLS-only. Günlük TLS renewal30gün eşik, hourlyGC etkin.
- Source ve copied5entry/4891byte manifest,93byte kullanıcı dosyası ve password hash doğrulandı.
- Rootdisk118GiB/used19GiB/free95GiB. Test/build geçişlerinde boş alan92GiB altına inmedi.

## Son testler

Workspace633passed/0failed/13explicitignored (library607passed/2ignored); RootSQLite3/3+gerçekTLSrefresh1/1 ayrıca passed. fmt/Clippy/Rust1.85/ES2022TS/WASMsingleton/Cacophonyvector passed. Namespace256MiB disk-backed upload/download/Range/resume/abort/tamper/Noise negative testleri passed; peakRSSserver32416KiB/helper33044KiB. Fresh install, mevcut Linux migration/global scope revoke, fixed quota, native imaj genişletme+hash, real Caddy dualtrust passed.

Production direct/public admin login/logout+dosya+quota+Noise passed; CRUD/revisionconflict/chmodundo/download-audit, real systemd restart/shutdownpopup/unsaved editor/relogin passed; public17MiB upload/download/Range passed. Dedicated fixture dosyaları temizlendi.

## Temizlik

Reusable araçlar tools/dev, tests/live ve tools/ops içinde. Completed development timer/service+state, temporary admin secret/log/config/manifest,4376known fixture directories/63temporary files, Python cache ve ilk7993build files/2.6GiB, son1629files/523.6MiB temizlendi. Successful update'in otomatik backup/previous-unit kopyası ve geçici release bundle silindi. Aktif kullanıcı home'ları, native image, Root auth/migration journal/config/key/audit ve mevcut private test credential dosyaları korunur. Eski rollback yedeği yoktur.

## Bu iki aşama dışında

Hosting/domain/app process/DB yönetimi, MFA/SSH ayarı, restart ötesi resume, tutarlı tam user-data+metadata disaster recovery, uzun soak ve bağımsız güvenlik incelemesi ayrı kapsamdır. Metadata restore kabulü full native-volume recovery garantisi değildir. Staging budget OS-wide compromised-UID hardquota değildir; DB page cap WAL'ı kapsamaz. Noise formal audited diye tanıtılmaz; bağımsız pin yoksa trust TLS discovery'dir. Kapsam içi bekleyen iş yoktur.
