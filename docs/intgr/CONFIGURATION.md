# Çalışan konfigürasyon sözleşmesi

[Dizin](README.md) · [Tam alan referansı](../CONFIGURATION.md) · [Wizard](WIZARD.md)

Root `policy/server.toml` ve `policy/helper.toml` tam AppConfig'dir. Server dosyası okunabilir0644; helper ceiling0600 ve key0600 Root'tadır. `config/settings.toml` sınırlı ConfigPatch overlay'idir, server UID yazabilir. Root, admin ve kişisel yetki aynı dosyada birleştirilmez. Önceki schema_version/auth/transport örnekleri [tarihsel taslaktır](history/CONFIGURATION-before-final-2026-10-13.md).

Root Wizard ilk dosyaları gerçek service UID ile üretir. `config apply` tam aday dosyayı validate eder ve helper ceiling önce olmak üzere atomik kaydeder. Overlay Root yetkilerini genişletemez; login uygunluğu, origin/proxy/TLS, home, roller, lifecycle/journal/control_audit panel PATCH alanları değildir.

Aşağıdaki parçalar **Wizard'ın ürettiği tam dosyada** ilgili blokları değiştirmek içindir; mevcut blokları tekrar eklemeyin:

```toml
# Top-level: otomatik home ile statik roots birlikte kullanılmaz.
roots = []

[server]
bind_addr = "127.0.0.1:13100"
behind_tls_proxy = true
require_tls = true

[session]
secure_cookies = true
same_site = "Strict"

[security]
allowed_origins = ["https://drive.example.com"]

[proxy]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
client_ip_header = "XRealIp"

[home_template]
root_id = "home"
read_only = false
uploads_allowed = true
hidden_files_allowed = true
is_web_root = false
# Gerçek base_path helper'ın doğruladığı NSS home'dan gelir.

[journal]
enabled = true
retention_hours = 504
undo_max_hours = 24
max_entries_per_account = 2048
max_total_entries = 32768
max_record_bytes = 8192

[control_audit]
retention_hours = 504
max_records = 16384

[lifecycle]
trash_cleanup_enabled = true
staging_cleanup_enabled = true
trash_retention_hours = 504
staging_retention_hours = 24
max_cleanup_per_run = 100

[virtual_accounts]
enabled = true
allow_admin_delete = false
deleted_retention_hours = 36
max_accounts = 4096
enforcement = "user_quota"
max_inodes = 100000
```

Native mode seçilmeden volume kurulmalıdır; enabled/Root pool tek başına admin oluşturmaz. `web_accounts` gerçek service UID ve sanal UID aralığını içerir; UID tahmin ederek değiştirmeyin. Self-registration varsayılan açıktır, Root uygunluk/deny üstün gelir. `.ssh/.gnupg/.aws/.kube` varsayılan discovery/read dahil yasak; `.env` yeni profilde açıktır. Legacy sensitive/hide politikası migration'da açıkça korunabilir.

Direct TLS için behind_tls_proxy=false, tls_cert_path ve tls_key_path birlikte kullanılır; require_tls=true, secure cookie ve HTTPS origins sürer. Proxy'siz loopback bootstrap HTTP varsayılanıdır; public plaintext için güvenli fallback yoktur. Bind/TLS değişimi restart gerektirir; çalışma ayarları ve uygun policy reload olabilir.

Çöp kutusu retention Root lifecycle saatidir. Split panelde legacy trash_retention_days düzenlenmez. Undo kullanıcı tercihi1..Root undo_max_hours; null Root sınırını izler. Legacy audit retention_days giriş uyumu vardır; yeni süreler saat cinsindedir.
