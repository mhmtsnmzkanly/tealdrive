# Geliştirme temizliği kaydı

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
