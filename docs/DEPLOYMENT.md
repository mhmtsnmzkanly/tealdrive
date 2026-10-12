# Caddy üzerinden kurulum ve kabul testi

Bu dosya hedef Linux sunucuda uygulanacak kurulum örneğidir. İlk repo incelemesinde sistem kurulumu yapılmadı. Sonraki kullanıcı talebiyle bu
sunucuda ayrı deneme hesabı, PAM ve setuid helper kurulup test edildi; ayrıntılar
[STATUS.md](STATUS.md) içinde. Mevcut çalışan Caddy siteleri değiştirilmedi.
`drive.example.com`, `alice` ve veri dizinini kendi ortamına göre değiştir.

## Gereksinimler

- Linux: Landlock ABI >=3, genellikle kernel >=6.2; `openat2` etkin olmalı.
- Derleme: kilitli bağımlılıklar için Rust >=1.85. Testlerde Node.js gerekiyor.
- PAM: `libpam.so.0`, uygun `tealdrive` PAM servisi ve giriş yapabilen normal kullanıcı.
- Caddy: alan adı/DNS ve HTTPS sertifikası için uygun sunucu erişimi.
- Helper'ın bulunduğu disk `nosuid` olmamalı. Servis `NoNewPrivileges=true` veya
  UID/GID değiştirmeyi engelleyen capability sınırlamasıyla başlatılmamalı.

`--check-config` yalnızca yapılandırma şemasını/sınırlarını kontrol eder; PAM,
Landlock ve dosya izinleri için aşağıdaki kabul testini ayrıca uygula.

## Derleme ve dosyalar

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Önce `docs/deployment/config.toml` ve `docs/deployment/helper.toml` içindeki alan adını ve
`admin_users` listesini düzenle. İki dosyada root id ve veri yolu aynı olmalı.
Helper dosyası izin tavanıdır; bir özelliği açmak için ikisinde de izin gerekir.

Aşağıdaki komutları kurulum yetkisiyle hedef sunucuda çalıştır. `alice` örnek bir
mevcut PAM kullanıcısıdır; parola veya hesap oluşturma komutu içermez.

```sh
sudo useradd --system --home-dir /var/lib/tealdrive --shell /usr/sbin/nologin tealdrive
sudo install -d -o root -g root -m 0755 /usr/local/libexec /etc/tealdrive
sudo install -d -o tealdrive -g tealdrive -m 0700 /var/lib/tealdrive
sudo install -o root -g root -m 0755 target/release/tealdrive-server /usr/local/bin/tealdrive-server
sudo install -o root -g root -m 4755 target/release/tealdrive-helper /usr/local/libexec/tealdrive-helper
sudo install -o tealdrive -g tealdrive -m 0600 docs/deployment/config.toml /var/lib/tealdrive/config.toml
sudo install -o root -g root -m 0600 docs/deployment/helper.toml /etc/tealdrive/helper.toml
sudo sh -c 'umask 077; head -c 32 /dev/urandom > /etc/tealdrive/helper.key'
sudo install -d -o alice -g "$(id -gn alice)" -m 0700 /srv/tealdrive
```

Anahtar üretimini yalnızca ilk kurulumda yap; yenilemek mevcut helper biletlerini
geçersiz kılar. Yeni bir binary kurduğunda `install -m 4755` adımını tekrar uygula:
derleme/kopyalama setuid bitini düşürebilir. Helper ve `/etc/tealdrive` ebeveynleri
root'a ait olmalı ve grup/diğer kullanıcılar tarafından yazılmamalı.

Debian/Ubuntu için örnek PAM dosyası:

```sh
sudo install -o root -g root -m 0644 docs/deployment/pam.tealdrive /etc/pam.d/tealdrive
```

Diğer dağıtımlarda `common-auth` / `common-account` bulunmayabilir; o sunucunun
standart parola ve hesap PAM stack'ini kullan. MFA veya parola yenileme gibi ek
etkileşim gerektiren PAM akışları bu iki alanlı giriş formunda desteklenmiyor.

Bu veri dizini örneği yalnızca `alice` erişimi içindir. Çok kullanıcılı kullanımda
normal Unix sahiplik/grup/ACL düzenini oluştur; servis kullanıcısına veri erişimi
vermek gerekmez. Hiçbir kullanıcı için helper Unix dosya izinlerini aşmaz.

## Servis ve Caddy

```sh
/usr/local/bin/tealdrive-server --config /var/lib/tealdrive/config.toml --check-config
sudo install -o root -g root -m 0644 docs/deployment/tealdrive.service /etc/systemd/system/tealdrive.service
sudo systemctl daemon-reload
sudo systemctl enable --now tealdrive
```

Backend `127.0.0.1:3000` üzerinde kalmalı. Caddyfile'a alan adın için şu site bloğunu
ekle; mevcut diğer sitelerin üzerine yazma:

```caddyfile
drive.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Caddy `reverse_proxy`, WebSocket upgrade işlemini destekler; ayrı `/ws` rewrite
veya `Upgrade` header kuralı gerekmez. Host başlığını backend adresine değiştirme:
Origin denetimi dış alan adıyla eşleşmelidir.
[Resmî reverse_proxy belgesi](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).

```sh
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
curl -fsS https://drive.example.com/api/health
```

HTTPS örneğinde `behind_tls_proxy=true`, `secure_cookies=true`,
`allowed_origins=["https://drive.example.com"]` ve
`allow_plain_ws_localhost_only=false` birlikte kullanılmalı.

## Tarayıcı kabul testi

1. HTTPS üzerinden aç. Normal PAM kullanıcısı giriş yapabilmeli; yanlış parola,
   root ve nologin/system hesapları reddedilmeli. `/api/health` tek başına bunun
   çalıştığını kanıtlamaz.
2. Bağlantı durumunda Connected ve DevTools'ta `/ws` için 101 gör. Sayfa yenilemede
   oturum devam etmeli; tarayıcıya eski `app.js` cache'i gelmediğinden emin ol.
3. Klasör oluştur, küçük dosya yükle, indir ve içeriğini karşılaştır. Metni aç,
   değiştir, kaydet. Başarı mesajı yalnızca sunucu yanıtından sonra görünmeli.
4. 200'den fazla öğeyle Next page'i dene. Arama, yeniden adlandırma, çöp kutusu ve
   geri yüklemeyi kontrol et. Kalıcı silme/chmod varsayılan örnekte kapalıdır.
5. 1 MiB'den büyük dosya yükleme varsayılan olarak reddedilmeli. Gerekiyorsa iki
   yapılandırmada da limiti en çok 3 MiB'ye yükselt. HTTP indirmede büyük dosya ve
   `Range: bytes=0-9` için 206 yanıtını dene.
6. Gizli alt dizin, `.env`, anahtar dosyaları ve kök dışına sembolik bağ erişimi
   reddedilmeli. ZIP/TAR açma hedef dışına veya gizli/hassas dosyalara yazmamalı.
7. Admin olmayan hesap ayarlara ulaşamamalı. Admin PATCH/logout/reload isteğinden
   CSRF başlığını kaldırınca 403 alınmalı. Bir ayar değiştirip kaydet; servisi
   yeniden başlatınca korunmalı. Helper tavanının üzerindeki izinler verilmemeli.
8. İkinci sekmede indirme/WS açıkken çıkış yap: diğer bağlantı durmalı. Küçük kota
   ile eşzamanlı yüklemeler toplam sınırı aşmamalı ve başarısız hedefler görünmemeli.
9. `journalctl -u tealdrive -n 100` ve `/var/lib/tealdrive/audit.jsonl` kontrol et.
   Helper yetki düşürmesi başarısızsa servis güvenlik ayarlarını, setuid bitini,
   `nosuid`, PAM ve veri yolu izinlerini kontrol et.

Root yetkili, capability'leri kısıtlanmamış test ortamında gerçek helper yaşam
döngüsü ayrıca çalıştırılabilir:

```sh
sudo -E "$(command -v cargo)" test --locked --test e2e_integration_test \
  test_real_helper_binary_subprocess_lifecycle -- --ignored
```

Bu test `nobody`/65534 test kimliği ve geçici veri kökü kullanır. Root'un zaten sahip
olduğu yetkiyle helper çalıştırır; PAM bileti/setuid servis akışının yerine geçmez.
Tam üretim akışının kanıtı normal servis kullanıcısı üzerinden gerçek PAM girişidir.

## Kontrollü başlatma ve kapanış

```bash
sudo systemctl start tealdrive
sudo systemctl stop tealdrive
sudo systemctl restart tealdrive
sudo systemctl status tealdrive
sudo journalctl -u tealdrive -f
```

Server SIGTERM ve Ctrl+C/SIGINT yakalar. Kapanışta dinleyici kapatılır; mevcut
HTTP bağlantılarından gelen yeni istekler 503 alır. WebSocket yeni komut okumayı
bırakır, o anda çalışan komutunu tamamlayabilir. Başlamış HTTP indirmelerine ve
helper işlerine 30 saniye süre tanınır. Süre dolarsa oturum/iş iptal mekanizmaları
çalışır; iptal ve temizlik için ayrıca beş saniye beklenir. HTTP sunucusunun son
bağlantıları bırakması için en fazla iki saniye daha tanınır.

Servis dosyasında `KillMode=mixed` önemlidir: ilk SIGTERM yalnızca server'a gider,
helper'lar tamamlanma süresinde çalışabilir. `TimeoutStopSec=45s` üst sınırdır;
servis bu sürede durmazsa systemd bütün kontrol grubunu SIGKILL ile sonlandırır.
Varsayılan `KillMode=control-group` kullanılırsa helper'lar da ilk sinyalle kesilir
ve tamamlama süresi anlamını kaybeder. Unit değişikliğinden sonra
`sudo systemctl daemon-reload` çalıştırılmalıdır.

Kapanan bağlantının tamamlanmamış upload staging dosyaları temizlenir. Aynı
koruma erken hata ve dispatch panic durumlarında da çalışır. Tamamlanmamış
yüklemeler restart sonrası devam ettirilmez; oturumlar RAM'de olduğu için tekrar
giriş gerekir. Başarıyla tamamlanmış/publish edilmiş işlemler geri alınmaz.

Server helper çalışırken stdin borusunu açık tutar. İptalde borunun kapanması,
UID değiştirmiş helper'a sinyal gönderme yetkisi olmadan da iptal bildirir.
Helper yazma/kopyalama/arşivleme kontrol noktalarında işlemi bırakır ve transaction
çıktısını temizler. Bildirimden sonra bir saniyede dönmeyen helper kendi watchdog'u
ile çıkar; parent süreci helper'ı bekleyerek reap eder. Server/helper binary'lerini
bu IPC davranışı için birlikte güncelleyin.

Kapanış sonucu audit'e `ServerShutdown` / `Success` veya `Failed` olarak yazılır;
aktif audit dosyasına `fsync` uygulanır. Ayrıntılar journal'da da görünür. SIGKILL,
güç kesintisi ve çekirdekte kesilemeyen I/O için kontrollü temizlik garantisi yoktur.
Watchdog ile zorla çıkan helper'ın `.tdrv-transaction-*` çıktısı kalabilir;
server eski/geçmiş staging dosyalarını topluca tarayıp silmez. Bu kapanış akışı
çok adımlı işlemleri bir veritabanı transaction'ına dönüştürmez.

Doğrulama:

```bash
cargo test --locked --offline -- --include-ignored
cargo clippy --locked --offline --all-targets -- -D warnings
cargo build --release --locked --offline
```

Ignored helper/kernel testleri uygun Linux izinleri ve kernel desteği gerektirir.
Kapanış regression testleri admission/drain/iptal sınırını, gerçek TCP dinleyicisini,
HTTP stream kapanışını, upload temizliğini ve gerçek helper'ın kilitte beklerken
iptalini kapsar. Canlı systemd/SIGTERM/SIGINT testlerinin sonuçları STATUS.md'dedir.

### Kullanıcıya kapanış bildirimi

Bağlı web kullanıcıları kontrollü kapanış başlar başlamaz popup görür. Popup yeni
işlerin durduğunu, çalışan işlemlere en fazla 30 saniye tanındığını, eksik upload'ların
tekrar denenmesi ve kaydedilmemiş değişikliklerin kopyalanması gerektiğini açıklar.
Bağlantı kapandığında mesaj sunucunun durduğunu belirtir. Çalışan helper'ın cevabı
popup açıkken alınabilir; yeni WebSocket komutları gönderilmez.

Dismiss/Escape bildirimi kapatır, tekrar bildirimde popup yeniden açılmaz. Editörün
metni ve açık penceresi korunur; sayfa otomatik yenilenmez. Check availability
sunucuyu kontrol eder ve erişilebilir olduğunda oturum/giriş akışına döner. Servis
restart'ı oturumları sıfırladığı için tekrar giriş gerekir. Popup kapanış nedeniyle
otomatik giriş penceresiyle örtülmez; klavye odağı popup içinde tutulur ve dismiss
sonrası önceki alana döner.

Bildirim yalnızca bağlı kullanıcılara ulaştırılabilir. Ağ kesintisi/SIGKILL kapanış
bildirimi olarak etiketlenmez. Bildirim gönderimi en fazla bir saniye bekler; yavaş
istemci aktif helper'ın tamamlanmasını veya servis kapanışını durdurmaz. Güncel
arayüz kodunu almak için deploy sırasında açık olan sayfayı bir kez yenileyin.


### Proxy visitor IP and audit verification

The example Caddy route overwrites `X-Real-IP` with `{client_ip}`; the example
server config trusts loopback only. For Cloudflare in front of Caddy, configure
Caddy's global trusted proxy ranges and strict client-IP parsing as described in
CONFIGURATION.md. Reload Caddy after validation. Never expose a trusted backend
to untrusted peers. Audit `ip` is the resolved visitor; `peer_ip` is the socket peer.
Use Settings → Audit Logs or the `X-Tealdrive-Audit-Write-Failures` health response
header to check writer failures, and inspect `journalctl -u tealdrive` for details.
See AUDIT.md for event coverage, byte-count semantics and recovery limits.


### Mandatory login allowlist

Before deploying an upgraded server/helper, set `account_policy.allowed_users`
in both the service-owned server config and root-owned helper ceiling. Missing
or empty lists disable login. The example allows only `alice`; replace it with
actual intended Linux usernames. `admin_users` grants administration, not login.
Do not include `www-data`, `caddy`, `tealdrive` or other service accounts.
Adding users through the web settings cannot widen the helper's root-owned list.
