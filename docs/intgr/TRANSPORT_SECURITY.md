# Taşıma güvenliği: yenileme ve ek şifreleme

## Durum

Backend TLS mevcut üretimde etkin. Yeni source sürümünde Root-owned PEM çifti60s aralıkla doğrulanıp canlı rustls config'e yüklenir. Hatalı key/cert/mode/symlink son doğrulanmış çifti değiştirmez; mevcut WS bağlantıları korunur, yeni bağlantılar yenilenen çiftle kurulur. Düz HTTP fallback yok. TLS'nin etkinlik/bind değişimi hâlâ restart gerektirir. Bu özelliğin canlı yayını henüz yapılmadı.

Yeni Root Wizard `tls status` sertifika geçerlilik tarihleri/public fingerprint,30gün yenileme uyarısı ve çift doğrulamasını gösterir. Private key veya parola göstermez.

## Proxy güveni ve yenileme

Self-signed backend sertifikasını tek adımda değiştirmek pinned proxy trust'ı bozar. Yenileme sırası: yeni çifti ayrı Root-owned alanda üret/doğrula; proxy trust'a eski+yeni public sertifikayı ekle ve validate/reload yap; backend çifti atomik yayınla; yeni TLS kimliğini ve domain sağlık/girişini doğrula; sonra eski trust'ı kaldır. Key dosyası yalnız Root ve servis group tarafından okunur. Eşleşmeyen çift ara durumda listener son geçerli çifti korur. Yeni source Root Wizard prepare/activate/renew akışı ve isteğe bağlı Caddy adaptörü içerir; Caddy zorunlu değildir. Komutlar ve kabul aşağıdadır.

## Ek şifreleme: tasarım sınırları

İlk source uygulaması aşağıda açıklanmıştır. Standart çift yönlü channel protocol kullanılır; sunucunun aynı ağdan gelen anahtarını denetlemeden kabul eden özel ECDH/AES şeması yapılmayacak. Noise NK önceden güvenilen responder static key ve ephemeral handshake ile seçilmiştir. HPKE tek başına replay/downgrade prevention sağlamaz; bu nedenle sadece HPKE eklemek yeterli sayılmaz. Referanslar: https://noiseprotocol.org/noise.html ve https://www.rfc-editor.org/rfc/rfc9180.html .

Tehdit modeli: browser↔backend payload'ın pasif TLS-termination/proxy loglarına açık metin düşmesini azaltmak; TLS zorunluluğu ve Origin/oturum yetkileri korunur. MITM'e bağımsız koruma yalnız client key trust/pin güvenilir kanaldan sağlanıyorsa vardır. Aynı saldırgan SDK/HTML'yi değiştirebiliyorsa JS anahtar pinini de değiştirebilir; XSS veya backend compromise koruması iddiası yok.

Yayın koşulları: standart/vetted implementation, browser↔Rust test vectors, authenticated transcript, direction-specific keys/counters, replay/out-of-order/bit-flip/downgrade rejection, bounded plaintext/ciphertext sizes, yeniden bağlantıda yeni handshake, key rotation, SDK type/capability açıklaması. HTTP admin/auth/download gibi yan uçlar kapsam dışında kalırsa encrypted-all iddiası yapılmaz; fallback ve kapsam Root policy ile açık belirlenir.


## Root yenileme akışı (14 Ekim 2026)

```sh
# Mevcut TLS çifti üzerinde yeni self-signed P-256/SHA256, 365 günlük SAN sertifikası hazırla.
tealdrive-wizard tls prepare tealdrive-origin.local
# Public çift-güven paketi: /home/tealdrive/policy/origin-trust.pem
# Caddy: tek explicit trust-pool satırını pakete geçir, validate + private Unix admin reload,
# key/cert yayınla, gerçek backend TLS kimliğini bekle, eski trust'ı kaldır + reload.
tealdrive-wizard tls activate tealdrive-origin.local --caddy /etc/caddy/Caddyfile
# Sadece 30 gün içinde bitecekse yenile; yarım kalan işlemi her zaman devam ettir.
tealdrive-wizard tls renew tealdrive-origin.local --caddy /etc/caddy/Caddyfile
# Root günlük systemd kontrolünü kur (ilk yenileme zamanı daily + en fazla1saat jitter).
tealdrive-wizard tls auto tealdrive-origin.local --caddy /etc/caddy/Caddyfile
```

Otomatik Caddy adaptörü yalnız bir explicit `tls_trust_pool file OLD_CERT_OR_TRUST_PATH` satırı ve private Unix admin socket kabul eder. Birden fazla eşleşme, karmaşık/import altında gizlenen eşleşme veya TCP admin durumunda geniş kapsamlı otomatik değişiklik yapmaz. Diğer sitelerin yapılandırması korunur; adapted JSON veya private key loga verilmez. Caddyfile mode/UID/GID korunur. Ara yapılandırma validasyondan geçmeden aktif dosyaya yazılmaz. Reload başarısızsa eski dosya geri yazılır ve çift güven korunur.

Nginx/Apache/başka proxy için `prepare` sonrasında public paketi proxy trust kaynağı yapıp kendi reload işlemini gerçekleştirin; sonra `tls activate NAME` çalıştırın. Bu explicit Root komutu proxy'nin önceden hazırlandığı anlamına gelir; Wizard dış proxy reload'unu doğruladığını iddia etmez. Generic akış eski public trust'ı korur; yeni backend kimliği doğrulandıktan sonra proxy kendi trust yenileme süreciyle eski sertifikayı kaldırabilir. Doğrudan TLS istemcileri de önceden pakete güvenmelidir. Public/domain sertifikasının ACME yenilemesi ayrıdır.

Yenileme kaydı `/home/tealdrive/tls/renewal/` Root0700 dizinindedir: eski/yeni private key0600 ve phase state0600. Başarıda hepsi temizlenir. Hata halinde aynı hostname/Root TLS yollarıyla `activate` yeniden denenir; başka hedef pending işlemin üzerine yazılmaz. Key/cert iki atomic rename ile yayınlanır; kısa süreli mismatch sırasında listener son geçerli çifti sunar. Yeni leaf'i kabul eden doğrulanmış TLS probe ve exact DER eşitliği başarılı olmadan eski trust kaldırılmaz. Probe loopback backend bind gerektirir; uzak bind için adaptör sessizce atlamaz. Full power failure aralarında son geçerli sertifikanın bellekte kalması garanti değildir; pending durum ve eski çift Root onarımı için tutulur.

Kabul: gerçek Root/OpenSSL hazırlık, SAN/pair validation, private key mode, değişmeden resume, farklı hostname reddi ve hazırlık sırasında active cert korunması passed. İzole mount/PID/NSS/TLS dizininde gerçek Caddy listener + private admin socket ile çift trust yükleme, canlı backend60s refresh, strict TLS identity + HTTP health, eski trust kaldırma, private pending key temizliği ve fresh cert30gün kontrolünde yenilememe passed. Production Caddy/cert/key değiştirilmedi; otomatik timer bu testte host'a kurulmadı.


## Ek WebSocket şifrelemesi: çalışan source uygulaması

Standart `Noise_NK_25519_ChaChaPoly_SHA256`, Snow0.10.0 (Rust1.85) ile uygulanır. Aynı `crates/channel` Rust kaynağı native backend ve browser WebAssembly için derlenir. Noise algoritmaları yeniden yazılmadı. Snow upstream resmi bir bağımsız audit almadığını belirtir; bu entegrasyonun testleri bağımsız güvenlik incelemesinin yerine geçmez. Kaynaklar: [Noise rev34](https://noiseprotocol.org/noise.html), [Snow](https://github.com/mcginty/snow), [Cacophony vektörleri](https://github.com/mcginty/snow/blob/v0.10.0/tests/vectors/cacophony.txt).

```sh
# Root, yeni bir dosya seçer. Var olan anahtarın üzerine yazılmaz.
tealdrive-wizard channel keygen /home/tealdrive/tls/channel.key
# Çıktıdaki publicKey gizli değildir; güvenilir kanaldan SDK geliştiricisine verilir.
```

```toml
[server]
require_tls = true
# Diğer mevcut TLS veya trusted TLS proxy ayarları korunur.
channel_key_path = "/home/tealdrive/tls/channel.key"
```

Key32rawbytes, Root:service0640; parentRoot-owned ve group/world-writable olamaz. Root config'te alan bulunması her iki WS endpoint'inde şifrelemeyi zorunlu yapar; plaintext TDRV auth/file komutu dispatch edilmez. Yanlış key/mode/path başlangıçta reddedilir. Web overlay anahtarı değiştiremez. Varsayılan alan yoktur ve eski WS uyumluluğu korunur. Alan yeni binary gerektirir.

Bağlantı: WSS→`TDN1 + serverPublicKey` greeting→Noise NK48byte client handshake→48byte server handshake→encrypted TDRV ServerHello/ClientHello/ProtocolReady→login. Greeting uygulama verisi taşımaz. Sabit prologue sürümü/uygulama bağlamını handshake transcript'ine bağlar. Directional keys ve sıralı nonce/replay denetimi Noise tarafından yönetilir. Her bağlantıda yeni ephemeral key ve nonce dizisi kurulur.15s ilk handshake deadline, mevcut pre-auth slots/60s auth deadline korunur. Çözme/fragment/protokol hatası bağlantıyı kapatır.

Noise65535byte record sınırı nedeniyle TDRV frame'leri60000byte parçalar halinde şifrelenir. Encrypted plaintext'in ilk byte'ı0(devam)/1(son), devam parçaları tam60000byte olmalıdır. Frame toplamı Root max_frame_payload+header sınırını aşamaz. Sıra değiştirme/bit-flip/replay AEAD doğrulamasında reddedilir. Frame/chunk uygulama limitleri büyütülmez;17MiB aktarım hâlâ bounded64KiB helper stream ve256KiB upload chunks kullanır. WebSocket close/ping metadata'sı şifrelenmiş uygulama payload'ı değildir.

```js
import { createClient } from '/sdk/v1/tealdrive.js';
const api = createClient({
  encryption: 'required',
  // Root tarafından bağımsız/güvenilir kanaldan verilen public hex key.
  trustedServerKeys: ['GERCEK_64_HEX_PUBLIC_KEY']
});
await api.handshake();
console.log(api.getInfo().transport);
await api.auth.login({ username, password });
```

`auto` varsayılanı server zorunluluğuna uyar. `required` plaintext WS server'ı reddeder. `disabled` encrypted server greeting'ini reddeder. Pin listesi verilmesi de plaintext downgrade'u reddeder; eski+yeni pin birlikte verilerek Root key rotation yapılabilir. Pin yoksa getInfo keyTrust=`tls-discovery`: MITM'e TLS'den bağımsız koruma iddiası yoktur. Yanlış pin `SERVER_KEY_MISMATCH`; zorunlu özellik yokluğu `ENCRYPTION_REQUIRED`; encrypted record hataları `ENCRYPTED_CHANNEL_FAILED`. SDK handshake başarısızsa dosya/auth yöntemleri kullanılamaz.

**Kapsam sınırı:** Bu kanal WS login/enrollment, dosya komutları, upload chunks, sonuçlar ve WS bildirimlerini kapsar. REST yönetim/password-change uçları, cookie bridge, session ve native HTTP streaming/Range downloads TLS kullanır; Noise kapsamında değildir. `getInfo().transport.httpEncryptedChannel=false` bunu açık belirtir. Proxy'nin gördüğü cookie veya REST yetkileri bu özellik ile gizlenmez. 'Bütün trafik uçtan uca şifreli' veya 'proxy operatörü hesabı taklit edemez' iddiası yapılmaz. İhtiyaç tüm HTTP uçlarını da kapsayacaksa ayrıca kanal-bound session ve typed RPC/stream tasarımı gerekir.

Embedded runtime yaklaşık139KiB WASM+9KiBJS'dir; yalnız encrypted greeting geldiğinde yüklenir, CDN kullanılmaz. `/sdk/v1/noise.js` ve `noise_bg.wasm` custom UI tarafından override edilemez. CSP `wasm-unsafe-eval` ile WebAssembly derlemesine izin verir; JS unsafe-eval eklenmez. Rebuild: `tools/dev/build-channel.sh`; wasm-bindgen-cli0.2.122 ve wasm32 target gerektirir. Cargo.lock, Rust kaynakları ve artifactlar repodadır; third-party notices aynı dizindedir.

Kabul: Cacophony NK handshake/ciphertext/transcript/directional transport vektörü, native wrongkey/replay/tamper/fresh ephemerals, gerçek ChromiumWASM↔Rust17MiB pinned transfer, wrong pin, exact replay ve bit-flip rejection passed. Required/pin plaintext downgrade SDK regression testleri de vardır. Üretimde bu source özelliği henüz etkinleştirilmedi.
