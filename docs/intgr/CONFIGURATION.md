# TealDrive CONFIGURATION geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Yetki ve alan seçimi

Critical config root/Wizard/elle, çalışma config allowlisted web ayarıdır; server politika/auth/anahtara doğrudan yazmaz. Kullanıcı policy güncellemesi helper yönetim protokolüdür.

Kullanıcı boolean pam_enroll_then_web biçimini istedi; enum geleceği kolaylaştırıyorsa kalabileceğini belirtti. linux_login_mode enum önerisi korunur, nihai alan adı kesinleşmiş sayılmaz. Root/server auth modu çelişirse startup/reload reddedilir.

Yeni TOML örnekleri mevcut AppConfig değildir. Çalışan örnekler [deployment](../deployment/) altındadır. Reload/sahiplik kontrolü [SERVER.md](SERVER.md); UI fallback notu [UI.md](UI.md).


## 3. Config örnekleri

TOML örneklerindeki `schema_version` yeni yapı için önerilen sürümdür. Boyutlar byte, süreler saniye/gün; CLI okunabilir `GiB` kabul edip byte'a çevirir. Bilinmeyen alanlar sessizce yok sayılmaz. Dosyalar atomik kaydedilir; kritik alanlar web PATCH ile değişmez.



### 3.1 `policy/server.toml` — kritik server ayarları

```toml
schema_version = 1

[server]
bind_addr = "127.0.0.1:3000"
transport = "proxy_tls" # proxy_tls | direct_tls | http
require_tls = true
settings_path = "/home/tealdrive/config/settings.toml"

[tls]
tls_cert_path = "/home/tealdrive/tls/certificate.pem"
tls_key_path = "/home/tealdrive/tls/private-key.pem"
# Bu dosyalar direct_tls modunda okunur; proxy_tls'de proxy sertifikayı yönetir.

[proxy]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
client_ip_header = "XRealIp"

[security]
allowed_origins = ["https://drive.example.com"]
secure_cookies = true
same_site = "Strict"

[auth]
linux_login_mode = "pam_enroll_then_web" # pam | pam_enroll_then_web
enrollment_ttl_seconds = 300

[helper]
binary = "/home/tealdrive/bin/tealdrive-helper"
timeout_seconds = 10

[ui]
custom_ui = false
custom_ui_path = "/home/tealdrive/ui/custom"
fallback_ui = "embedded"
required_sdk_major = 1

[audit]
server_log_path = "/home/tealdrive/logs/server/server.jsonl"
user_log_root = "/home/tealdrive/logs/users"
security_log_path = "/home/tealdrive/logs/security/security.jsonl"
critical_mutations_require_audit = true

[rate_limit]
account_max_failures = 5
lockout_seconds = 300
max_parallel_auth = 8
```

Hash Argon2 parametreleri, üst log limitleri ve enrollment üst sınırı root politikasında doğrulanır; burada örnek olmayan ayrıntılar config yokmuş gibi güvenliksiz default üretmez. `linux_login_mode` kimlik tavanıyla çelişirse startup reddedilir; helper server'ın gönderdiği modu bağımsız doğrular.



### Proxy'siz TLS seçimi

```toml
[server]
bind_addr = "0.0.0.0:8443"
transport = "direct_tls"
require_tls = true
settings_path = "/home/tealdrive/config/settings.toml"

[tls]
tls_cert_path = "/home/tealdrive/tls/certificate.pem"
tls_key_path = "/home/tealdrive/tls/private-key.pem"

[security]
allowed_origins = ["https://drive.internal:8443"]
secure_cookies = true
same_site = "Strict"
```

Bu parça ilgili tabloların yerine geçer; aynı dosyada iki `[server]` tablosu olmaz. DNS ve dış CA şart değildir; tarayıcının güvendiği yerel CA kullanılabilir. Sertifika/anahtar bozuksa veya zorunlu TLS sağlanamıyorsa HTTP'ye düşülmez. Yenileme başarısızsa hâlâ geçerli eski sertifika ile tanımlı süre devam veya durdurma politikası uygulanır; süresi dolmuş sertifika normal kabul edilmez.



### Açık seçimle yerel HTTP

```toml
[server]
bind_addr = "127.0.0.1:3000"
transport = "http"
require_tls = false
settings_path = "/home/tealdrive/config/settings.toml"

[security]
allowed_origins = ["http://127.0.0.1:3000"]
secure_cookies = false
same_site = "Strict"
```

Kapalı ağ HTTP erişimi ayrı açık Wizard seçimi ister. İnternetsiz olması parola trafiğini şifrelemez. HTTP ile gönderilen özel JS şifrelemesi TLS yerine geçmez. Proxy TLS modunda backend loopback/korumalı ağda kalır; arbitrary forwarded-proto başlığına güvenilmez.



### 3.3 `config/settings.toml` — webden değişebilir ayarlar

```toml
schema_version = 1

[ui]
default_locale = "tr"
page_size = 200

[audit]
max_file_bytes = 10485760
max_archives = 5
retention_days = 30
file_mutations_require_audit = false

[trash]
retention_days = 30 # öneri; kullanıcı kesin süreyi henüz belirlemedi

[search]
max_depth = 20
default_max_results = 100

[features]
save_text_file = true
chmod_file = true
extract_archive = true
delete_permanently = false
```

Panel yöneticisi bu ayarları tavanın içinde güncelleyebilir; normal kullanıcı sistem çalışma ayarlarını değiştiremez. Log yolları, custom UI güven kaynağı, auth modu, kota havuzu, TLS ve kritik limitler bu dosyaya eklenmez. Aktif güvenlik politikasını belirleyen kullanıcı kuralları `settings.toml` üzerinden değiştirilmez; helper yönetim komutundan geçer. TLS/auth/politika kayıtları unknown-field kontrolüyle PATCH'te reddedilir.



### 8. Merkezi dizin ve kritik/çalışma ayarlarının ayrılması — orta/büyük

- Bölüm 3'teki dosya düzeni; root ebeveyn kontrolü ve sabit güvenilir helper yolları.
- İki config şeması ve birleşim kuralları; kritik alanları admin PATCH'ten çıkar, disk izinleriyle de engelle.
- Web ayarı yazımları atomik; değişebilir runtime config root tavanını aşamaz. Kritik config yalnızca root/Wizard/elle yönetilir.
- Mevcut sistem için anahtar/config/log koruyan migration, rollback ve service paths.
- Kabul: Servis binary/politika/auth/critical config değiştiremez; `noexec`/`nosuid` ve servis mount kısıtları açıklayıcı hata verir.


## Yeni saat birimi ve hesap silme ayarları

Aşağıdaki parçalar hedef şema örneğidir; root/panel/kullanıcı katmanları ayrıdır.

```toml
# Root policy/helper ayarları
[account_lifecycle]
admin_can_delete_virtual_users = false # önerilen güvenli başlangıç; root açabilir
# Bu default kullanıcı tarafından ayrıca seçilmedi.
deleted_account_retention_hours = 36
```

```toml
# Panel yöneticisinin tavan içinde yönetebileceği çalışma ayarları
[audit]
retention_hours = 504

[trash]
retention_hours = 504

[undo]
max_retention_hours = 24 # server üst sınırının varsayılanı
min_retention_hours = 1
```

```toml
# Kullanıcı tercihi örneği; helper 1..server-max doğrular
[undo]
retention_hours = 12
```

Tam config örneklerinde eski retention_days/30-gün taslakları bu son saat kararına göre migration sırasında güncellenecek; güncel belirleyici bu bölümdür. Default/upper ilişkisi bütün aday config olarak doğrulanır. Admin silme yetkisi root virtual-users tavanıdır; gerçek Linux OS hesabını panelden silme hakkı değildir.

## 24 saat açıklaması — kullanıcı düzeltmesi

24 saat kullanıcı tercihi varsayılanı değil, server geri alma üst sınırının varsayılanıdır. Kullanıcı 1..server-max saat seçer; server-max root/admin yetki tavanı içinde yapılandırılır.
