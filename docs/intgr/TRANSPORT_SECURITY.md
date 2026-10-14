# Taşıma güvenliği: yenileme ve ek şifreleme

## Durum

Backend TLS mevcut üretimde etkin. Yeni source sürümünde Root-owned PEM çifti60s aralıkla doğrulanıp canlı rustls config'e yüklenir. Hatalı key/cert/mode/symlink son doğrulanmış çifti değiştirmez; mevcut WS bağlantıları korunur, yeni bağlantılar yenilenen çiftle kurulur. Düz HTTP fallback yok. TLS'nin etkinlik/bind değişimi hâlâ restart gerektirir. Bu özelliğin canlı yayını henüz yapılmadı.

Yeni Root Wizard `tls status` sertifika geçerlilik tarihleri/public fingerprint,30gün yenileme uyarısı ve çift doğrulamasını gösterir. Private key veya parola göstermez.

## Proxy güveni ve yenileme

Self-signed backend sertifikasını tek adımda değiştirmek pinned proxy trust'ı bozar. Yenileme sırası: yeni çifti ayrı Root-owned alanda üret/doğrula; proxy trust'a eski+yeni public sertifikayı ekle ve validate/reload yap; backend çifti atomik yayınla; yeni TLS kimliğini ve domain sağlık/girişini doğrula; sonra eski trust'ı kaldır. Key dosyası yalnız Root ve servis group tarafından okunur. Eşleşmeyen çift ara durumda listener son geçerli çifti korur. Yeni source Root Wizard prepare/activate/renew akışı ve isteğe bağlı Caddy adaptörü içerir; Caddy zorunlu değildir. Komutlar ve kabul aşağıdadır.

## Ek şifreleme: tasarım sınırları

Bu özellik henüz uygulanmadı. Standart çift yönlü bir channel protocol seçilmeli; sunucunun aynı ağdan gelen anahtarını denetlemeden kabul eden özel ECDH/AES şeması yapılmayacak. Noise NK önceden güvenilen responder static key ve ephemeral handshake ile bir adaydır. HPKE tek başına replay/downgrade prevention sağlamaz; bu nedenle sadece HPKE eklemek yeterli sayılmaz. Referanslar: https://noiseprotocol.org/noise.html ve https://www.rfc-editor.org/rfc/rfc9180.html .

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
