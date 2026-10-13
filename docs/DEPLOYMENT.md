# Kurulum ve güncelleme

Yeni kurulumun merkezi `/home/tealdrive`dır. Dört binary ve Root onaylı manifest kullanan Wizard, servis/PAM dosyalarını ve PATH kısayollarını kurar. Caddy zorunlu değildir. [Wizard komutları](intgr/WIZARD.md), [çalışan config](intgr/CONFIGURATION.md), [dosya yapısı](intgr/STORAGE.md) ve [bu sunucunun raporu](intgr/FINAL_REPORT.md) temel referanslardır.

## Gereksinimler

Linux Landlock ABI>=3/openat2, çalışır PAM/NSS ve Rust>=1.85. Mode000 chmod geri alma için fchmodat2 destekleyen Linux6.6+ gerekir. Binary filesystem'i nosuid/noexec olmamalı; server unit'i helper'ın ilk setuid geçişini engellememeli. User worker kalıcı UID/GID düşüşünden sonra no_new_privs+Landlock uygular. Native sanal kota ayrı ext4/usrquota volume ister; desteği doğrulanmadan etkinleştirmeyin.

Debian/Ubuntu PAM örneği common-auth/common-account kullanır; dağıtım farklıysa yerel PAM stack'ini uyarlayın. SSH/MFA etkileşimli doğrulama bu iki alanlı web girişinin parçası değildir.

## Build ve ilk kurulum

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --bins
sudo python3 tools/ops/package-release.py \
  --bin-dir /root/tealdrive/target/release \
  --output /root/approved-tealdrive-release --version release-20261013
sudo /root/approved-tealdrive-release/tealdrive-wizard install \
  --bundle /root/approved-tealdrive-release
sudo tealdrive-wizard config validate
sudo tealdrive-wizard diagnose
```

Bundle yolu kendi checkout'unuza göre değiştirilir; Root source/build'ü onaylar. SHA256 kaynağın imzası değildir. Install mevcut kurulumu ezmez, servisi otomatik başlatmaz. İlk ayar loopback HTTP127.0.0.1:13100, web hesapları/self-registration/journal açık; sanal hesaplar kapalıdır. Root/service/nologin/system hesabı uygunluğu bağımsız korunur. İlk admin otomatik yoktur:

```sh
sudo tealdrive-wizard account approve alice
sudo tealdrive-wizard account list
sudo tealdrive-wizard account admin ACCOUNT_ID
# Alice kendi SSH oturumunda PAM sonrası web parolasını belirler:
tealdrive-wizard init
```

Wizard TTY parola girişinde echo kapatır. Parola argv/env'ye verilmez. Web enrollment da desteklenir; ilk PAM kanıtı kısa ömürlü/tek kullanımlıktır. Sonraki giriş web parolasıdır; Linux parolası kalıcı tutulmaz. Sanal admin provisioning Root tarafından ayrıca yapılır.

## TLS seçenekleri

Yerel/offline kullanım HTTP bootstrap ile mümkündür. İnternete açmadan require_tls, secure cookie ve exact HTTPS allowed_origins birlikte seçilir. Direct TLS cert/key veya güvenilir yerel TLS proxy kullanılır. Caddy/NGINX/Apache seçimi API'yi değiştirmez; proxy WSS upgrade ve doğru Host/proto header sağlamalıdır. Backend yalnız loopback dinler, yalnız bilinen proxy adreslerine güvenir.

```caddyfile
drive.example.com {
    reverse_proxy 127.0.0.1:13100 {
        header_up X-Real-IP {client_ip}
    }
}
```

Caddy yönetim API'sini uygulama UID'sinden ayırın: [private admin socket](deployment/CADDY_ADMIN.md). Cloudflare gibi dış proxy varsa yalnız doğrulanmış CIDR'ler güvenilir seçilir; kullanıcıdan gelen IP header'ına körlemesine güvenilmez. Cloudflare resmi [IPv4](https://www.cloudflare.com/ips-v4)/[IPv6](https://www.cloudflare.com/ips-v6) listesi ve [Caddy trusted proxy](https://caddyserver.com/docs/caddyfile/options#trusted-proxies) yapılandırması temel alınır.

## Native sanal depolama

```sh
sudo tealdrive-ctl stop
sudo tealdrive-wizard storage init 1GiB
sudo tealdrive-wizard storage status
sudo tealdrive-wizard pool set 512MiB
# Üretilen tam config'te virtual_accounts.enabled=true/enforcement=user_quota seç:
sudo tealdrive-wizard config apply /root/approved-config.toml
sudo tealdrive-wizard storage sync
sudo tealdrive-wizard virtual admin create tenant-one 256MiB REQUEST_UUID
sudo tealdrive-ctl start
```

Image kurulumu boş mountpoint, sanal hesap/intent yokluğu ve offline server ister; mevcut filesystem adopt/format edilmez. Runtime metadata backup, kullanıcı dosyası veya image yedeği değildir.

## Güncelleme ve bakım

```sh
sudo tealdrive-ctl stop
sudo tealdrive-wizard backup /root/new-metadata-backup
# Kullanıcı içeriklerini/image/settings overlay'i ayrıca yedekleyin.
sudo tealdrive-wizard update --bundle /root/approved-tealdrive-release
sudo tealdrive-wizard diagnose
sudo tealdrive-ctl start
sudo tealdrive-wizard gc
sudo systemctl enable --now tealdrive-gc.timer
```

Binary rollback DB'yi geri almaz; offline restore hesap nesillerini ve undo epoch'unu yeniler, eski grant/proofları iptal eder. Uninstall server'ı durdurur, binary/kendi PATH linklerini kaldırır; config/DB/key/kullanıcı dosyalarını silmez. Legacy dosya geçişi için [no-replace migration aracı](../tools/ops/README.md) vardır; backup ve dış writer durdurma operator sorumluluğudur.

Canlı kabul [tools/dev](../tools/dev/README.md) altında opt-in; production-files testi gerçek servisi yeniden başlatır ve bir benzersiz fixture oluşturup file owner UID ile temizler. İzole Wizard testi gerçek systemd yerine stub kullanır; gerçek servis kabulü ayrı yapılır.

## Aynı hostta güvenilmeyen Linux kullanıcıları varsa

Canlı kurulumda backend ayrıca direct TLS kullanır; Caddy yalnız Root-managed origin sertifikasını doğrular. Sıradan UID, server kapalıyken13100 portunu bind edebilse de doğru private key olmadan Caddy'nin backend kimliğini taklit edemez. Standart uygulama:

```caddyfile
drive.example.com {
    reverse_proxy https://127.0.0.1:13100 {
        header_up X-Real-IP {client_ip}
        transport http {
            tls_trust_pool file /home/tealdrive/policy/origin-cert.pem
            tls_server_name tealdrive-origin.local
        }
    }
}
```

Root config'te behind_tls_proxy=false, require_tls=true, tls_cert_path=/home/tealdrive/policy/origin-cert.pem ve tls_key_path=/home/tealdrive/tls/origin-key.pem kullanılır. Sertifikanın SAN/SNI eşleşmesi gerekir; public certificate0644, private key Root:tealdrive0640. Caddyfile Root:caddy0640 ve private admin socket korunur. `tls_insecure_skip_verify` kullanılmaz. Bu sunucunun origin sertifikası1yıl geçerli; bitmeden Root cert/key ve Caddy trust pool birlikte güncellenmelidir.
