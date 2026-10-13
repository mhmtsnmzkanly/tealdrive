# Geliştirme temizliği kaydı

Önceki tarihli kayıtlar tarihsel durumu gösterir; en güncel sonuç aşağıdaki son bölümde yer alır.

2026-10-12. Güncel test kaynakları `tests/live/`, ortak runner/dependency lock `tools/dev/` altında korundu ve gerçek Chromium ile bu yeni konumdan çalıştırıldı.

## Kaldırılan eski dosyalar

- `/root/tealdrive-audit-live-test.py`
- `/root/tealdrive-audit-proxy-live-test.py`
- `/root/tealdrive-caddy-ws-test.py`
- `/root/tealdrive-chmod-test.py`
- `/root/tealdrive-domain-ws-test.py`
- `/root/tealdrive-login-access-live-test.py`
- `/root/tealdrive-popup-live-test.py`
- `/root/tealdrive-shutdown-live-test.py`
- `/root/tealdrive-ws-test.py`
- `/root/tealdrive-real-test-results.log`
- `/root/tealdrive-test-session.json`
- `/root/tealdrive-test.Caddyfile`
- `/root/tdrv-ui-check`

Toplam kaldırılan yaklaşık dosya byte miktarı: 39438362. Dizin metadata/hardlink hesabı dahil değildir.

Eski Python scriptleri legacy HTTP/PAM/config/protokol akışına bağlı tek kullanımlık production sandbox testleriydi; yeni gateway rollout runner'ı olarak uygun değiller. Audit/proxy/shutdown/root-policy senaryolarının yeniden uygulanması ilgili güncel PLAN kabul aşamalarında ayrıca yer alır; eski canlı testin yeni admin/TLS/undo kabulüne eşdeğer olduğu iddia edilmez. Eski UI snapshots, ekran görüntüleri, yardımcı binary kopyaları ve bağımlılık sandbox'ı yeniden üretilebilir çıktılardı. `tealdrive-test.Caddyfile` aktif config değildir; çalışan Caddy `/etc/caddy/Caddyfile` kullanıyor.

## Korunanlar

- Private Linux/web parola dosyaları ve canlı DB/config/bin/log/user home/SRV dosyaları.
- Root-private deployment/audit/proxy/login/shutdown/popup rollback yedekleri: final migration henüz bitmediği için geri dönüş kanıtıdır; repoya taşınmadı.
- Rust `target/` build/MSRV çıktıları: aktif geliştirme ve kurulu candidate binary kaynağıdır; eski tek kullanımlık script çıktısı sayılmadı.
- Root-owned auth backup dosyaları: scheduled retention/GC kararları tamamlanınca ayrıca temizlenecek.

Bu kayıt bütün proje işlerinin tamamlandığını göstermez; final rollout ve rollback acceptance PLAN.md'de bekler.

## Final teslim temizliği — 2026-10-13

Aktif eski /usr/local/bin/tealdrive-test-server ve tealdrive-candidate-server, /usr/local/libexec/tealdrive-test-helper ve tealdrive-candidate-helper public konumdan kaldırıldı. Önce setuid kaldırıldı; Root700 final-migration/retired-binaries altında executable rollback kopyaları korundu. Dört yeni PATH linki /home/tealdrive/bin altına gider.

Yeniden kullanılabilir kabul kaynakları tests/live ve tools/dev; operasyon yardımcıları tools/ops altında ayrıldı. Wizard temp namespace dizinleri otomatik temizlendi. Geçici tek-kullanımlık /tmp/td-range-edit.py kaldırılır; yeni private operational audit/backup veya gerçek kullanıcı dosyaları temizleme hedefi yapılmadı. Legacy SRV orijinali migration rollback için korunur. Tarihsel tasarım notları docs/intgr/history altında etiketlenmiştir.

## Kullanıcı talebiyle kapsamlı artık temizliği — 2026-10-13

- `cargo clean`:46739build file, Cargo raporuyla34.5GiB logical çıktı kaldırıldı; öncesinde target allocated kullanım yaklaşık31GiB idi. Canlı4binary bağımsız regular/nlink1 dosya ve sealed rollback manifest hash eşleşmesi doğrulandı. Source/Cargo.lock ve kurulu binaryler korunur; sonraki build cache yeniden oluşturur.
- `/tmp`36218known UUID test fixture directory ve532temporary file kaldırıldı. Runtime UID uploadları, açık FD/cwd/exe referansları, mounts ve symlink top-level öğeler cleanup hedefi değildir. FD-safe recursive removal kullanıldı.
- Cleaner negative/positive kabulü: open FD kaldı; UID995 dosyası kaldı; recursive fixture temizlenirken dış symlink sentinel hedefi kaldı. Son dry-run seçilen artık0.
- Generic eski `/tmp/tealdrive-audit.jsonl` silinmedi, Root-only archive içine taşındı.6old rollback directory `/root/tealdrive-runtime-backups/legacy-development-backups` altında toplandı. Root600 LOCATIONS.json eski→yeni path eşlemesini içerir. Parent runtime-backups Root700 yapıldı.
- Reusable `tools/dev/clean-artifacts.py` eklendi; dry-run varsayılan, apply explicitRoot ve allowlist. Existing tests/live, dev kabul kaynakları ve ops migration/release araçları korunur. Node dependencies19MiB, lockfiles, historical design notes ve operational credentials/DB/config/log/user data korunur.
- Cleanup sonrası public Chromium login/socket/role boundary/logout geçti; service/Caddy active. Rust kaynak davranışı değiştirilmediği için31GiB cache'i yeniden oluşturacak gereksiz full rebuild yapılmadı.

Disk son ölçüm:118GiB filesystem,20GiB used/94GiB available. Önceki genel30–31GiB alan geri kazanıldı; du/Cargo logical byte sayımı farklı ölçümlerdir.

## Kullanıcı talebiyle bütün mevcut TealDrive yedeklerinin silinmesi — 2026-10-13

`/root/tealdrive-runtime-backups` içindeki legacy/stage/final migration, metadata/key/user/binary/Caddy yedekleri ve release-bundle kopyaları silindi. `/home/tealdrive/storage/update-backup-*`, previous-tealdrive.service ve auth/accounts.db*.backup da kaldırıldı.5backup root altında156file,1807131092logical byte silindi. Live process FD/cwd/exe ve mount referansları önce denetlendi; symlink-safe removal kullanıldı.

Aktif native image/mount,4installed binary, current config/key/SQLite, password otomasyon dosyaları, audit logs ve gerçek kullanıcı dosyaları korunur. Önceki belgelerde listelenen rollback konumları artık mevcut değildir. Yeni Wizard update/restore ileride yeniden backup üretebilir; bu işlem mevcut yedekleri silmiştir. Son public browser login/socket/logout passed.
