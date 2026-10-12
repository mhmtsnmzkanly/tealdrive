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
