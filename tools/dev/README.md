# Geliştirme ve kabul araçları

Üretim kurulum araçları değildir; canlı servisi kurmaz veya Caddy'yi değiştirmez. Gerçek tarayıcı testlerinin kaynakları `tests/live/` altında, ortak bağımlılık ve çalıştırıcı bu dizindedir. Password dosyaları, oturum JSON'u, SQLite backup ve test çıktıları repoya konmaz.

```bash
npm ci --prefix tools/dev
# Chromium bu makinede yoksa:
npx --prefix tools/dev playwright install chromium

# Deterministic Rust / SDK / browser sözleşmesi testleri:
tools/dev/check.sh unit

# Yalnız ayrılmış test sunucusu ve kullanıcı ile:
export TEALDRIVE_TEST_URL=http://127.0.0.1:13101
export TEALDRIVE_TEST_USERNAME=tealdrive-test
export TEALDRIVE_TEST_WEB_PASSWORD_FILE=/root/tealdrive-candidate-web-password
tools/dev/check.sh live ui-auth

# İlk PAM kaydı da denenecekse özel Linux parola dosyasını ayrıca belirt:
export TEALDRIVE_TEST_PASSWORD_FILE=/root/tealdrive-test-password
tools/dev/check.sh live sdk-enrollment

# Credential generation değiştirir; parola değerini aynı tutar:
TEALDRIVE_ALLOW_CREDENTIAL_MUTATION=1 tools/dev/check.sh live ui-credentials

# Root-only DB suspend testidir; public admin API testi değildir:
TEALDRIVE_ALLOW_TEST_ACCOUNT_MUTATION=1 tools/dev/check.sh live sdk-suspend
```

| Betik | Amaç / etkisi |
|---|---|
| `ui-auth` | Gerçek gömülü UI login/logout; normal kullanıcıya yönetim metadata reddi |
| `ui-credentials` | CSRF/mevcut parola reddi, UI parola değiştirme, generation artışı; opt-in mutation |
| `sdk-enrollment` | PAM ilk kayıt veya mevcut web login; CRUD/revision/chmod/quota/transfer/logout; test dosyası üretir |
| `sdk-access` | Gizli yollar/alias/parent işlemleri; Root fixture oluşturup temizler |
| `sdk-suspend` | Root DB backup+suspend+finally restore; generation artar, özel backup bırakır |

Testleri aynı hesapta eşzamanlı çalıştırmayın: password/suspend bütün oturumları geçersiz kılar. Secret file modları private olmalıdır. Kullanıcı dosyalarında çalıştırmayın; scriptlerin açık mutation opt-in kontrolü vardır. `sdk-access` fixture koşulları ve `sdk-suspend` sabit candidate DB yolu kaynak açıklamalarında belirtilmiştir.

Root/OS tests opt-in olarak Cargo'da kalır: `accounts_store`, `runtime_settings`, `helper_ticket`, `chmod_owner` ve gateway PAM testi. Ortam değişkenleri/ownership/kernel koşullarını ilgili test kaynağından okuyun; genel runner bunları otomatik çalıştırmaz.

Geri üretilebilir bağımlılıklar package-lock ile pinlenmiştir; `node_modules/` ve `results/` Git dışında tutulur. Özel üretim rollback yedekleri burada tutulmaz.
