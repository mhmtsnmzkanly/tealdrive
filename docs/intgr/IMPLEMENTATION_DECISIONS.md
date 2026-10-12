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

## D008 — Kota sorgusu ortak helper taraması

Yeni ayrı path-based scanner yerine mevcut FD tabanlı quota taraması yeniden kullanılır. Paylaşımlı flock, symlink takip etmeme, hardlink inode dedup, depth/entry sınırı ve cancellation. Limit helper'ın kısıtlanmış config'inden; warning percent server çalışma ayarı (1..100), default80. HTTP session/allowlist ve lease doğrulanır; ölçüm hatası503, used=0 değil. Yeni /api/quota mevcut config-root düzeninde çalışır; DB sanal tahsisi sonraki hesap altyapısına bağlanacak.

## D009 — SDK v1 çekirdeği

ES module gömülü /sdk/v1/tealdrive.js, TS declarations ayrı .d.ts; framework/DOM bağımsız. auth/files/admin ve stateless/managed katman aynı transport kullanır. Field isimleri public camelCase, wire snake_case. RPC progress ile terminal ack ayrı; UploadBegin/Chunk'ın özel ack'ları tamamlanma sayılır. Caller Abort genel mutasyonun yapılmadığı kanıtı değildir. Unimplemented backend capability'leri FEATURE_DISABLED; sahte başarı yok. User navigation storage restore helper'da doğrulanır; out-of-order cevap state'i ezemez. Observer mevcut condition'ı hemen verir, polling yalnızca kota observer varken, disconnect/unsubscribe temizler. Rust legacy error adları SDK'da normalize edilir.

## D010 — Saat cinsinden audit saklama ve uyumluluk

Yeni audit varsayılanı retention_hours=504. Eski retention_days okunur ve açıkça tanımlandığında önceliklidir; yeni unit PATCH eski unit'i temizler. Null/0 yaş sınırını kapatır, archive count sınırı devam eder. Aktif dosya yaş nedeniyle silinmez; cleanup event yazılırken yapılır. Trash/undo/account retention ayrı yaşam döngüsüdür; audit ayarı bunları etkilemez.

## D011 — Root config ve çalışma overlay'i

Migration kırılmaması için explicit --runtime-config split modunu açar; Wizard yeni kurulumda bunu kullanacak. Root config tam AppConfig ve trust/parent ownership denetimli, settings kısıtlı ConfigPatch. Güvenlik tavanı her PATCH/persist'te root dosyasından yeniden okunur; runtime dosyası root alanı ekleyemez. Feature/limit/session/search/helper/audit root tavanı aşılmaz; login limiter yalnızca sıkılaşır. Root log dizinini seçer, admin aynı dizinde ad seçebilir. İki dosya reload'da atomik snapshot olarak değerlendirilir; bind değişimi restart gerektirir. Legacy tek dosya migration için korunur ve split güvenliği sağlıyor diye sunulmaz.

## D012 — HTTP ve WS SDK hata kodları

İndirme hata yanıtı diğer SDK yöntemleriyle aynı TealDriveError sözleşmesine çevrilir; tek DOWNLOAD_FAILED altında kaybolmaz. 401 AUTH_REQUIRED, 403 PERMISSION_DENIED, 404 NOT_FOUND, 429 RATE_LIMITED ve 503 SERVICE_UNAVAILABLE; sunucunun açık code alanı uppercase canonical forma çevrilir. Retryable bilgisi tekrar deneme önerisidir; SDK mutasyonu kendiliğinden tekrar etmez.

## D013 — Ortak path policy ve dolaylı erişim

Yeni access_policy deny union; root/global ve username root/admin/personal katmanları. Discover tüm alt ağacı kapatır (recursive yanlış false olsa da), NotFound; diğer action yasağı PolicyDenied. Default .ssh/.gnupg/.aws/.kube bileşenleri her derinlikte kapalı. .env bu motorun default yasağı değil; legacy sensitive varsayılanı migration'da ayrı değerlendirilecek. Özel staging path gerçek destination'a çevrilerek kontrol edilir; açık generic staging bypass yok. Parent taşıma/silme bounded tree preflight, source Read ve target Write; archive readonly'de yasaklı child omission. Trash metadata bounded/no-follow ve original/current policy kontrolü. Hardlink inode alternatifi için root default reject_file_hardlinks=true; meşru multi-link file da kapalı. Bu path politikası harici Linux owner rename'lerini engelleyen inode etiketi veya ayrı OS izolasyonu değildir.
