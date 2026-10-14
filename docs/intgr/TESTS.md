# TealDrive TESTS geliştirme sözleşmesi

> **Durum:** Aşağıdaki maddeler tarihsel kabul hedeflerini de içerir; hepsi yapılmış test anlamına gelmez. Gerçekte çalıştırılan testler ve sınırlar [TEST_RESULTS.md](TEST_RESULTS.md), güncel teslim [FINAL_REPORT.md](FINAL_REPORT.md) içindedir.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Dokümantasyon taşıması kontrolü

Deployment config testi `tests/config_examples.rs` artık docs/deployment kullanır. Güncel tam şema CONFIGURATION.md içindedir; history altındaki eski hedef TOML örnekleri çalışan parser referansı değildir. Linkler, fence ve JS/TOML/JSON/shell sözdizimi kontrol edilir. Bu iş canlı service restart veya feature dağıtımı değildir.


### 16. Son kabul ve dokümantasyon — büyük ve bütünleşik

- Değişiklik başına ilgili test/commit; sonrasında suite, Clippy, release build ve gerçek setuid/PAM/kernel testleri.
- Temiz desteklenen Linux ortamında yalnızca belgelerden kurulum; ikinci ortamda internetsiz/proxy'siz çalışma.
- Linux/sanal/admin/servis hesapları; yetki ve politika değişiminde açık WS/HTTP işleri; trash, eski/yeni audit, kota ve conflict.
- Gömülü/custom UI ve TR/EN; mobil/masaüstü; eski SDK ve yeni özellik uyumu.
- Caddy/Nginx/Apache opsiyonel entegrasyonları; backup/restore, update/uninstall ve güvenli kapanış.
- API, yapılandırma, kurulum, audit, güvenlik sınırları ve destek matrisi belgeleri. Test parolası ve otomasyon dosyaları repo dışında; üretim temizliği belgeli.
- Bağımsız güvenlik incelemesi önerisi; henüz yapılmış veya testlerle eşdeğer kabul edilmez.


## SDK gezinti/state güncellemesi

State, helper path doğrulaması, auth/files/admin ayrımı ve event/kalıcı bildirim davranışı uygulanmıştır. Güncel kullanım [STATE.md](STATE.md), tipler [client.d.ts](../../src/ui/sdk/client.d.ts), kabul kanıtları [TEST_RESULTS.md](TEST_RESULTS.md) içindedir.
