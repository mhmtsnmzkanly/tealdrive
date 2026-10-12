# Uygulama karar günlüğü

Bu günlük kullanıcı tarafından 2026-10-12 tarihinde yetkilendirilen uygulama sırasında alınan teknik kararları tutar. Sonuç/test kanıtı ayrı TEST_RESULTS.md içinde. Güvenlik veya sözleşme değişikliği uygulanınca ilgili bileşen belgesi de güncellenir.

## D001 — Public SDK adları

Üç iş alanı `auth / files / admin`; yaşam döngüsü kökte `handshake / disconnect / on`. Metotlar camelCase: `goTo`, `getCurrentDirectory`, `observeState`, `observeQuotaWarning`. Admin altında `systemUsers` ve `virtualUsers`. Eski taslak adları henüz yayımlanmadığından duplicate alias oluşturulmaz; mevcut wire API korunur.

## D002 — State

Stateless çekirdek, isteğe bağlı stateful adapter. Konum tercihi kullanıcı/account-generation + origin ile browser storage namespace'inde; sekme içinde bağımsız RAM state. Varsayılan kalıcı konum localStorage'da kullanıcı bazında; her restore helper doğrulamasından geçer. Secret/content storage'a girmez; storage hatasında RAM'e fallback. Yeni kullanıcı girişinde önceki kullanıcının state'i temizlenir. Path restore isteği kullanıcı tercihidir, erişim yetkisi değildir.

## D003 — Ayar ve süreler

Root kritik tavan, admin çalışma ayarı, kullanıcı kişisel kısıt/tercih. Genel retention 504 saat; deleted virtual veri 36 saat; server undo upper default 24 saat; kullanıcı tercihi 1..upper, belirtilmezse server upper'a uyarlanır. Admin virtual delete varsayılan kapalı; root açabilir. Bütçe kullanım/tahsis altına otomatik düşürülmez.

## D004 — Güvenlik kapsamı

Config-based hidden öğe list/search/stat/read/download/editor/archive/undo/audit yollarında yokmuş gibi davranır. OS görünüm izolasyonu artık kapsam dışı; UID/Landlock/openat2 korunur. WS-first handshake login öncesinde yalnızca sınırlı auth/protocol işler. TLS/WSS temel; ek mesaj şifrelemesi sonraki aşama. Kendi kripto protokolü yazılmaz.

## D005 — Geliştirme ve canlı kurulum

Deployment örnekleri docs altında; mevcut test servisi ve kullanıcı verisi değişiklikler doğrulanana kadar korunur. Her bağımsız iş ayrı commit. İlk belge commit'i yeni planı ve deployment taşınmasını kayıt altına alır. Yeni feature canlıya taşınmadan kaynak/test/izin doğrulaması gerekir.

## D006 — WS login ve HttpOnly bridge

Yeni SDK /ws/auth endpoint'i ile WS-first login yapar; legacy /ws session-bound kalır. Pre-auth 64 slot, 60 saniye deadline, 64 KiB mesaj, en fazla 5 auth denemesi; yalnızca hello/login. Başarılı login aynı WS'yi yetkili dispatcher'a devreder. HTTP download/admin ile uyum için 30 saniyelik tek kullanımlık opaque bind token POST /api/session/bind ile HttpOnly cookie kurar. Token storage/URL/log'a yazılmaz; origin, doğrulanmış IP, session ve allowlist yeniden denetlenir. Bridge endpoint CSRF yerine bu tek kullanımlık kanıt+Origin kontrolü kullanır.

## D007 — Editör revision ve izin korunumu

Revision SHA256(content + aynı FD'nin inode/device/uid/gid/mode/mtime/ctime metadata)'dır; bare content hash değildir. Helper read sırasında metadata değişimini reddeder; transaction gerçek hedefi staging öncesi ve publish öncesi yeniden karşılaştırır. Panel helper işleri mevcut ortak root reservation ile sıralanır. Harici kilide uymayan writer ile son compare/rename arasındaki yarış için mutlak garanti verilmez. Legacy optional wire alanı korunur; yeni SDK ve gömülü editör revision gönderir. Staged overwrite normal izin/owner/group/POSIX ACL'yi korur; koruma yapılamazsa original değiştirilmez; özel bitler yazma sırasında korunmaz.
