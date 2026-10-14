# Yetkilendirilen çekirdek aşamalar — kapanış

14 Ekim 2026: iki aşama uygulanıp bu sunucuda yayınlandı. Güncel [ilerleme](PHASE_PROGRESS.md), [detaylı rapor](FINAL_REPORT.md), [testler](TEST_RESULTS.md).

## Birinci aşama

- [x] UI/oturum köprüsü, yeniden bağlantı, hata/ilerleme/iptal; kapanış ve unsaved editör koruma.
- [x] Büyük dosyada bounded-memory upload/download/Range, staging bütçesi, kota/temizlik; aynı session süreli resume ve açık restart sınırı.
- [x] Servis UID'den ayrı virtual global panel admin, kendi home ve sabit own kota; mevcut dosya/parola korunarak migration; doğrulama sonrası eski web erişimi kapatma.
- [x] Backend canlı TLS refresh, Root renewal/generic proxy ve isteğe bağlı Caddy dualtrust/active config güvenliği.
- [x] Standart Noise NK uygulama kanalı, native/WASM shared implementation, pins/downgrade/replay/tamper testi ve TLS-only HTTP kapsamının açıklanması.
- [x] SDK1.1 typed/bundler/custom transport, runtime concurrency, canonical transfer UUID, explicit retained abort ve belirsiz commit semantiği.

## İkinci aşama

- [x] Root native storage init/status/check/mount/sync/expand; preallocation, grow-only, recovery ve no-overwrite.
- [x] Net dağıtılabilir havuz / ext4 physical capacity / metadata-headroom ayrımı; doğrulanmadan net kapasite yayınlamama.
- [x] Atomik tahsis/kullanım/pending rezervasyonu; sabit admin own quota, çocuk bütçesi ve usage-altına-shrink reddi.
- [x] Root DB/control audit/staging bounded bütçeleri ayrı; kullanıcı geçmiş/trash kendi kotasında. Cross-process ledger'in OS hardquota olmadığı belgeli.
- [x] Native imaj remount/genişletme/data hash kabulü; dış web servisleri için Unix izin/çakışma notları. Hosting provision eklenmedi.

## Teslim ve temizlik

- [x] Ayrı commitler ve karar kayıtları; normal/Root/real Chromium/native Caddy gelişmiş testleri.
- [x] Production release, admin migration, Noise, TLS renewal timer, upload tavanı; direct/public kabul.
- [x] Disk düzenli ölçüm, geçici geliştirme çıktısı ve update backup temizliği; reusable test/ops araçlarını koruma.
- [x] Detaylı bitiş raporu ve güncel config/kurulum/SDK belgeleri.

## Gelecekte ayrı kapsam olabilecekler

Tutarlı full user-data+metadata backup/restore, bağımsız security review/uzun soak ve server restart ötesi upload resume. Hosting/domain/process/database yönetimi, MFA ve SSH politikası bu teslimin işi değildir. Bunlar tamamlandı diye sunulmaz.
