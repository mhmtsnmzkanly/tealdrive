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
