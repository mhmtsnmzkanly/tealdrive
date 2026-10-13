# Taşıma güvenliği: yenileme ve ek şifreleme

## Durum

Backend TLS mevcut üretimde etkin. Yeni source sürümünde Root-owned PEM çifti60s aralıkla doğrulanıp canlı rustls config'e yüklenir. Hatalı key/cert/mode/symlink son doğrulanmış çifti değiştirmez; mevcut WS bağlantıları korunur, yeni bağlantılar yenilenen çiftle kurulur. Düz HTTP fallback yok. TLS'nin etkinlik/bind değişimi hâlâ restart gerektirir. Bu özelliğin canlı yayını henüz yapılmadı.

Yeni Root Wizard `tls status` sertifika geçerlilik tarihleri/public fingerprint,30gün yenileme uyarısı ve çift doğrulamasını gösterir. Private key veya parola göstermez.

## Proxy güveni ve yenileme

Self-signed backend sertifikasını tek adımda değiştirmek pinned proxy trust'ı bozar. Yenileme sırası: yeni çifti ayrı Root-owned alanda üret/doğrula; proxy trust'a eski+yeni public sertifikayı ekle ve validate/reload yap; backend çifti atomik yayınla; yeni TLS kimliğini ve domain sağlık/girişini doğrula; sonra eski trust'ı kaldır. Key dosyası yalnız Root ve servis group tarafından okunur. Eşleşmeyen çift ara durumda listener son geçerli çifti korur. Otomatik çift üretim/yayın ve proxy adaptörü hâlâ yapılacak iştir; Caddy zorunlu hâle getirilmeyecek.

## Ek şifreleme: tasarım sınırları

Bu özellik henüz uygulanmadı. Standart çift yönlü bir channel protocol seçilmeli; sunucunun aynı ağdan gelen anahtarını denetlemeden kabul eden özel ECDH/AES şeması yapılmayacak. Noise NK önceden güvenilen responder static key ve ephemeral handshake ile bir adaydır. HPKE tek başına replay/downgrade prevention sağlamaz; bu nedenle sadece HPKE eklemek yeterli sayılmaz. Referanslar: https://noiseprotocol.org/noise.html ve https://www.rfc-editor.org/rfc/rfc9180.html .

Tehdit modeli: browser↔backend payload'ın pasif TLS-termination/proxy loglarına açık metin düşmesini azaltmak; TLS zorunluluğu ve Origin/oturum yetkileri korunur. MITM'e bağımsız koruma yalnız client key trust/pin güvenilir kanaldan sağlanıyorsa vardır. Aynı saldırgan SDK/HTML'yi değiştirebiliyorsa JS anahtar pinini de değiştirebilir; XSS veya backend compromise koruması iddiası yok.

Yayın koşulları: standart/vetted implementation, browser↔Rust test vectors, authenticated transcript, direction-specific keys/counters, replay/out-of-order/bit-flip/downgrade rejection, bounded plaintext/ciphertext sizes, yeniden bağlantıda yeni handshake, key rotation, SDK type/capability açıklaması. HTTP admin/auth/download gibi yan uçlar kapsam dışında kalırsa encrypted-all iddiası yapılmaz; fallback ve kapsam Root policy ile açık belirlenir.
