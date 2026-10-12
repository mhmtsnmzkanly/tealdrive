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
