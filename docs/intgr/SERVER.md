# TealDrive SERVER geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Mevcut kod ve yeni reload gereksinimi

`src/main.rs`, `src/ws/server.rs`, `src/config.rs`, `src/admin/api.rs` ve `src/auth/api.rs`. Watcher/SIGHUP reload mevcut; yeni çoklu şemalara uyarlanacak. Server kullanıcı home'una doğrudan erişmez.

Kullanıcı config değişince otomatik ayar güncelleme ve Wizard `reload` istedi:

1. Atomic rename/dosya olayları debounce edilir; aday config bütünü parse/şema/sahiplik/path/dependency doğrulamasından geçer. Yarım ayarlar yayınlanmaz.
2. Geçerli yeni snapshot revision/generation ile atomik uygulanır. Bozuk server çalışma config'inde son geçerli snapshot ve açık hata; helper politikası kayıp/bozuksa ilgili erişim reddedilir.
3. Server root-only policy/auth deposunu okumaz. Helper her başlangıçta güncel tavanı okur; aktif işler/oturumlar iptal kontrolünü sınırlı doğrulanmış sorguyla yeniler.
4. Canlı uygulanabilecek alanlar: UI çalışma tercihleri, search/audit limitleri, kota warning eşiği; writer rotation/reopen lifecycle test edilir.
5. Auth/politika değişimi oturum/bilet iptali veya yeniden doğrulama gerektirir. Eski snapshot yeni yetki açamaz.
6. TLS cert/key birlikte doğrulanır; yeni context yeni bağlantılara atomik uygulanır. Sertifika hatası HTTP'ye düşürmez; eski geçerli sertifikanın süre politikası tanımlanır.
7. Bind/transport, servis UID, helper binary veya DB yolu ilk sürümde restart-required raporlanır; reload başarısı diye gizlenmez.
8. Wizard reload doğrulama ve uygulanan revision sonucunu gösterir; SIGHUP root yönetim yetkisi vermez.
9. Kabul: Atomic save, bozuk aday, çok alanlı değişiklik, izin bozulması, aktif WS/upload, cert renewal ve restart-required testi.

Reload matrisi [CONFIGURATION.md](CONFIGURATION.md), komut [WIZARD.md](WIZARD.md) ile birlikte güncellenir.


### 12. Proxy'siz ve internetsiz çalışma — büyük

- Açık çalışma modları: proxy arkasında HTTPS; doğrudan uygulama TLS; açıkça seçilen yerel/kapalı ağ HTTP.
- Kritik config'te `require_tls`, `tls_cert_path`, `tls_key_path` benzeri açık ayarlar; yapılandırılmış sertifika/anahtarı oku. Zorunlu TLS'de eksik/geçersiz dosya veya başarısız yenileme HTTP'ye düşürmez. Proxy TLS modu ayrı tanımlanır; yalnızca güvenilen bağlantılardan TLS bilgisi kabul edilir.
- Doğrudan TLS için sertifika/anahtar yükleme, private CA ve yenileme davranışı; dış ACME servisi zorunlu olmaz. Server özel TLS anahtarını gerekli minimum yetkiyle okuyabilir; helper imza anahtarıyla aynı erişim sınıfı değildir.
- HTTP modu güvenlik sınırları: PAM/web parolası ağda şifrelenmez; internetsiz olmak güvenli ağ anlamına gelmez. Loopback varsayılanı, açık ağ bağlama seçimi ve cookie/origin uyumu.
- Caddy/Nginx/Apache örnekleri opsiyonel; installer proxy dosyalarını değiştirmez.
- Kabul: Dış ağa ihtiyaç olmadan login/WS/upload/download; TLS/proxy/origin/IP spoofing testleri. Doğrudan TLS mevcut kodda yok, yeni iştir.


## SDK gezinti/state güncellemesi

2026-10-12 yeni server state, helper path doğrulaması, üç SDK alanı ve event/kalıcı bildirim sözleşmesi [STATE.md](STATE.md) altında. Bu hedef henüz uygulanmış değildir; ilgili kabul testleri orada tanımlanır.

## WS-first handshake gereksinimi

2026-10-12 kullanıcı düzeltmesi: SDK handshake doğrudan WS açar; login aynı bağlantıda. Upgrade'de TLS/Origin, pre-auth'ta yalnızca allowlisted handshake/auth/enrollment ve limit/timeout; authenticated olmadan hiçbir dosya/admin/state erişimi yok. Mevcut login zorunlu upgrade modeli değiştirilirken genel dispatcher kimlik kapısı korunur. İleride uygulama şifrelemesi için protokol aşamaları sürümlü: [STATE.md](STATE.md).

## State/aktör kapsam düzeltmesi — 2026-10-12

Son açıklamada kalıcı gezinme state'i server sorumluluğundan çıkarıldı; isteğe bağlı SDK katmanına geçti. Server session, authenticated sonuç/event, quota sorgu yönlendirmesi ve helper orchestration tutar. Root/admin config ayrımı aktör yetkisidir; “server” sözcüğü kullanıcı açıklamasında panel adminini ifade ediyordu, OS servis sürecinin root yetkisi değildir. Önceki server-owned view-state ifadeleri bu son kararla geçersizdir.


## Uygulanan TLS taşıma sözleşmesi

Caddy zorunlu değildir. Aynı server direct rustls HTTPS veya yerel TLS-terminating proxy arkasında çalışır. TLS1.2/1.3 standart rustls/ring provider; özel JS/PGP protokolü yoktur. `require_tls` eski configte absent olduğunda legacy declaration korunur; yeni Wizard explicitfalse yalnız loopback bootstrap üretir. Public config explicittrue olmalıdır.

```toml
# Doğrudan HTTPS
[server]
bind_addr = "0.0.0.0:13100"
behind_tls_proxy = false
require_tls = true
tls_cert_path = "/home/tealdrive/tls/certificate.pem"
tls_key_path = "/home/tealdrive/tls/private-key.pem"

[session]
secure_cookies = true

[security]
allowed_origins = ["https://drive.example.com"]
```

```toml
# Caddy / nginx / Apache yerel TLS terminasyonu
[server]
bind_addr = "127.0.0.1:13100"
behind_tls_proxy = true
require_tls = true
# cert/key burada verilmez.

[proxy]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
client_ip_header = "XForwardedFor"

[session]
secure_cookies = true

[security]
allowed_origins = ["https://drive.example.com"]
```

Proxy modunda require_tls=true loopback bind, explicit trusted peer ve yalnız tek `X-Forwarded-Proto: https` ister. Eksik/bad/duplicate/untrusted header426/TLS_REQUIRED ve TlsRequired audit reason üretir. HTTPS başlığı TLS'nin yerine geçmez; burada origin→proxy güven sınırı Root'un koruduğu yerel bağlantıdır. Cloudflare gibi uzak edge için origin üzerinde direct TLS veya yerel proxy TLS gerekir; backend public plain HTTP'ye açılmaz. Proxy TLS terminating config başlığı client değerinden bağımsız kendisi yazmalıdır.

Direct cert+key beraber gerekir; TLS/proxy iki mod birden seçilemez. Cert/key absolute Root-managed path, bütün parent Root-owned/non-writable/non-symlink; fileRoot-owned/nlink1, key en fazla0640 (world permissions yok), cert0644 olabilir. Service'in okuyabilmesi için TLSdirRoot:tealdrive0750 ve keyRoot:tealdrive0640 kullanın; helper.key buna dahil değildir, Root600 kalır. Let's Encrypt live symlink yerine deploy hook ile gerçek PEM dosyalarını bu Root dizine atomik kopyalayın. Parse/mode/key mismatch hatası startup'ı reddeder, HTTPfallback yoktur. Private PEM kaynak tamponu zeroize edilir; TLS server key'i çalışırken TLS context'te bulunur.

Bind/mode/requireTLS/cert-path/key-path değişimi tüm config modlarında restart-required; reload eski active config'i koruyarak reddeder. Sertifika içeriği değiştiğinde de restart yapılır; panelden cert management/reload yoktur. DirectTLS ve HTTP aynı bounded drain/shutdown event/audit yolunu kullanır.

Uygulama standardı: [rustls TLS server](https://docs.rs/rustls/latest/rustls/server/struct.ServerConfig.html), [axum-server](https://docs.rs/axum-server/0.7.3/axum_server/). Bağımlılıklar Cargo.lock'ta; axum-server0.7.3 MSRV1.82, proje1.85 ile test edildi.0.7.2 güncel hyper-util Buf sınırıyla derlenmediği için0.7.3 seçildi.

## Kalıcı web-account giriş sınırlaması

Helper'ın Root-owned accounts.db login_limits tablosu PAM enrollment, web login ve mevcut parola değiştirme doğrulamasından önce bounded rezervasyon tutar. Root helper.toml `[rate_limit] max_login_failures = 5`, `lockout_duration_seconds = 300`; hesap/IP/global default5/25/200, ilk attempt'tan300 saniye. Başarı kendi slotunu iade eder. Restart/bağlantı yenileme sayacı kaldırmaz; limit HTTP/WS429 `RATE_LIMITED` olur. Server'ın in-memory ve pre-auth socket sınırları ek savunma olarak kalır. IP ancak güvenilir proxy ayarına göre çözülür; Root helper hesap/global sınırı IP'den bağımsızdır. SQLite path/key scope istemci tarafından seçilmez; sayaçta parola yok.
