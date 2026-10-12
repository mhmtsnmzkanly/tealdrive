# TealDrive TESTS geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Dokümantasyon taşıması kontrolü

Deployment config testi `tests/config_examples.rs` artık docs/deployment kullanır. Yeni intgr TOML hedef şema örnekleridir; mevcut AppConfig testine verilmez. Linkler, fence ve JS/TOML/JSON/shell sözdizimi kontrol edilir. Bu iş canlı service restart veya feature dağıtımı değildir.


### 16. Son kabul ve dokümantasyon — büyük ve bütünleşik

- Değişiklik başına ilgili test/commit; sonrasında suite, Clippy, release build ve gerçek setuid/PAM/kernel testleri.
- Temiz desteklenen Linux ortamında yalnızca belgelerden kurulum; ikinci ortamda internetsiz/proxy'siz çalışma.
- Linux/sanal/admin/servis hesapları; yetki ve politika değişiminde açık WS/HTTP işleri; trash, eski/yeni audit, kota ve conflict.
- Gömülü/custom UI ve TR/EN; mobil/masaüstü; eski SDK ve yeni özellik uyumu.
- Caddy/Nginx/Apache opsiyonel entegrasyonları; backup/restore, update/uninstall ve güvenli kapanış.
- API, yapılandırma, kurulum, audit, güvenlik sınırları ve destek matrisi belgeleri. Test parolası ve otomasyon dosyaları repo dışında; üretim temizliği belgeli.
- Bağımsız güvenlik incelemesi önerisi; henüz yapılmış veya testlerle eşdeğer kabul edilmez.


## SDK gezinti/state güncellemesi

2026-10-12 yeni server state, helper path doğrulaması, üç SDK alanı ve event/kalıcı bildirim sözleşmesi [STATE.md](STATE.md) altında. Bu hedef henüz uygulanmış değildir; ilgili kabul testleri orada tanımlanır.
