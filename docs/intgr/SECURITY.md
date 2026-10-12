# TealDrive SECURITY geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Ortak güvenlik sınırları

- Server root değildir ve kullanıcı dosyasını doğrudan açmaz.
- Helper kullanıcı içeriklerini UID/GID düşürme + Landlock/openat2 sonrasında işler; root yönetimi ayrı komut sınıfıdır.
- Root korumalı bütün üst dizinler de yazılamaz; keyfi config/UID/home parametresi yoktur.
- Enroll/identity/operation biletleri farklı kapsam; caller UID, hesap nesli, süre, revoke ve policy doğrulanır.
- Root/admin/user yasakları birleşir; UI gizleme veya capability güvenlik sınırı değildir.
- Custom UI aynı origin'de güvenilen koddur. HTTPS/WSS şifreleme, handshake protokol başlangıcıdır.
- UID yeniden kullanımı, key/binary rollback ve reset eski yetkiyi canlandıramaz.
- MFA, SSH değişikliği, süreç yönetimi ve çalışan uygulama tespiti kapsam dışıdır.

Her değişiklik source/target path, identity/role, policy sahibi, fail-closed ve aktif lease iptali açısından test edilir. Mevcut mekanizmalar yeni SDK/UI için gevşetilmez.


### 3. CSP ve parola belleği güçlendirmesi — orta

- Inline handler'ları kaldır; script `unsafe-inline` iznini çıkar; custom UI ile aynı güvenlik başlıklarını uygula.
- PAM/IPC parola kopyalarını azalt; sahip olunan tamponları sıfırla. Tüm kütüphane/tarayıcı kopyaları için garanti verme.
- Kabul: Login, editor, WS, chmod, admin ve kapanış bildirimi çalışır; parola log/argv/env içine girmez.


## Dosya gizleme hedefinin güncel kapsamı

Son kullanıcı açıklaması kod düzeyinde helper config filtresidir; OS düzeyinde ayrı görünüm gereksinimi geri çekildi. Genel UID/Landlock/openat2 sınırları korunur; listeden gizleme içerik erişiminin OS düzeyinde imkânsız olduğu şeklinde anlatılmaz. Doğrudan erişim yasağının kapsamı HELPER/DECISIONS içinde açık soru olarak tutulur.

## Handshake şifrelemesinin güven sınırı

Anahtarı aynı doğrulanmamış bağlantıdan almak MITM kimlik doğrulaması değildir; saldırgan anahtarı değiştirebilir. HTTPS/WSS ve doğrulanmış sunucu kimliği temel olur. Ek mesaj şifrelemesi isteği ayrı tasarım işidir: Uç noktalar, kimlik doğrulama/anahtar güveni, nonce/sıra/replay/downgrade/rotation açıkça tanımlanmadan güvenlik garantisi verilmez.

Mevcut loglar sırları zaten kaydetmemelidir; şifreli payload'ı loglamak gereksinim değildir. Browser-server encryption TLS proxy loglarının payload görmesini azaltabilir, decrypt sonrası server/helper loglarını veya ele geçirilmiş UI'yi otomatik korumaz. Amaç TLS sonlandırıcı/arayolun plaintext görmemesi ise decrypt ucunun helper mı server mı olduğu ayrı karar gerekir.
