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


## Public admin / gerçek OS hesap kabulü

```bash
cargo build --locked --bins --example admin_acceptance_fixture
# Kurulu /home/tealdrive/bin binary'leri test edilen kaynak sürümüyle eşleşmelidir.
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 tools/dev/check.sh admin
```

Root-only, explicit opt-in. Driver UUID adlarıyla üç geçici Linux UID/GID/home oluşturur, yalnız iki hesabı protected DB'de bootstrap eder; ayrı server127.0.0.1:13102 çalıştırır. Asıl server config/binary ve mevcut kullanıcı metadata'sını değiştirmez. Primary helper policy'ye test admin/roots geçici eklenir ve öz kayıt kapatılır; byte-identical eski dosya finally'de döner. Aynı helper'ı kullanan adayın yeni enrollment talepleri test boyunca bu geçici politika ile değerlendirilir. Caddy eski helper kullandığından etkilenmez.

Chromium public SDK→HTTP→setuid helper yolunu kullanır; admin suspend/enable, canlı socket kapanışı, yeniden login reddi, eski cookie'nin geri açılmaması, gerçek kullanıcı UID'siyle dosya erişimi, CSRF/role/kind sınırları doğrulanır. Driver durdurduğu process yalnız kendi fixture server'ıdır; cleanup yalnız kendi UUID usernames/UIDs/DB rows/home'larını siler. Bütün DB'yi eski backup'a döndürmez. Dışarıdan helper policy değişirse onun üzerine yazmaz; root-private original dosyasını bırakarak hata verir. SIGKILL/power-loss için finally garantisi yoktur; private manifest/backup ile root onarımı gerekir.

Fixture bootstrap example bir Wizard veya public admin rol verme API'si değildir. Root önceden admin rolünü atar; browser rol/UID/path seçmez. Python/Node scriptleri credential'ları komut argümanı veya stdout'a koymaz.


```bash
# İlk onay + gerçek PAM + suspend edilmiş pending proof kabulü:
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=approval tools/dev/check.sh admin
```

Approval phase Root deny'ı yalnız test operator'ı geçici uygulayıp geri alır; browser'ın Root dosyasına yazabildiği bir uç değildir. Fixture server runtime overlay kullanır (Root config byte-preservation); var olan hesap approve ile reset edilmez. Negatif auth senaryolarından sonra test yeni transport açar; server'ın5 deneme limiti gevşetilmez.


```bash
# Gerçek gömülü UI / PAM / rol / cancel / mobil / başarısız refresh kabulü:
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Her phase sonunda aktör/hedef audit kaydı ve parola loglanmaması kontrol edilir. Driver kendi fixture log/config/runtime çıktılarını siler; üstteki sabit root-private lock dosyası reusable test koordinasyonu için kalır. Virtual gruba geçiş testi virtual OS hesap provisioning/tenant UID izolasyonu testi değildir. İlk admin ataması production Wizard ile henüz tamamlanmadı; test root-only bootstrap örneğini kullanır.
