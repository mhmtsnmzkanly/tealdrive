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

## D014 — Protected SQLite/Argon2id temel deposu

Rusqlite0.40.2 bundled SQLite + RustCrypto Argon2id0.6.0, kendi parola kriptosu yok. Root-only explicit init/open, root0600 DB/sidecars, protected parents ve SQLite NOFOLLOW. Init mevcut DB'yi overwrite/adopt etmez; schema/application ID doğrulanır. WAL/FULL/foreign keys/STRICT, 3s busy timeout, short immediate tx. Root-only online backup, mevcut hedef overwrite yok. Parola public metadata'da yok, random salt, default19MiB/t2/p1, 12 Unicode karakter/1024byte, bounded PHC verify ve unknown-user dummy hash. [RustCrypto](https://docs.rs/argon2/0.6.0/argon2/) ve [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html) sözleşmeleri kullanıldı.

Admin budget bir allocation tavanıdır; gerçek kullanım helper ölçümü. Sanal admin own quota budget-child allocation; gerçek Linuxadmin home kullanımı OS kotasında kalır. Deleted tahsisi filesystem purge sonrası serbest bırakılır. DB aktör yetkisi gateway'in doğruladığı ticket'tan gelir; JSON role claim değil. Bu temel depo PAM/enrollment/UID sağlama/gateway tamamlanmadan canlı auth olarak açılmaz.

## D015 — MSRV korunumu

Projedeki Rust1.85 alt sınırı gerçek toolchain ile test edildi. Yeni crypto bağımlılığının ctutils0.4.3 sürümü Rust1.87 istiyordu; uyumlu son0.4.2 Cargo.lock'a sabitlendi. Diğer yeni ana bağımlılıklar güncel kararlı; cargo +1.85 check --locked --all-targets geçti. Dependency güncellemesinde MSRV kontrolü tekrarlanır.

## D016 — Merkezi yollar ve güvenli legacy seçimi

/home/tealdrive standart konumu; root helper config/key primary çifti sabit derleme yollarıdır, environment seçemez. Yalnızca primary config'in yokluğu /etc legacy çiftine izin verir; mevcut hatalı/izin sorunu/symlink primary reddedilir. Server lookup ve default helper binary merkezi dizini tanır. Bu değişiklik canlı veriyi/passwd home'u taşımıyor; Wizard migration ayrı aşamadır.

## D017 — Helper bilet MAC'i ve anahtar belleği

Elle yazılmış HMAC SHA256 ve karşılaştırma yerine RustCrypto hmac0.12.1 kullanıldı; mevcut sha2/digest0.10 ile uyumlu, wire MAC değişmez. verify_slice sabit süreli kütüphane doğrulaması; raw key Vec Zeroizing ile tutulur. SHA/HMAC kütüphanesi iç kopyalarına dair mutlak memory erasure garantisi verilmez. RFC4231 ve kimlik/expiry tamper vektörleri korunur. Ek application crypto protokolü eklenmedi.

## D018 — Kalıcı grant/enrollment ve explicit migration

Cookie ayrı; helper yetkisi persistent nonce ledger + account generation. Kernel caller UID/parent PID/start-time ve boot-ID kayıt bağlamı; process restart/reboot replay sınırı. Logout bir nonce, hesap suspend/revoke/PW change bütün eski generation'ları iptal eder. Grant default hard bound64/account8192global, enrollment1024; expired/revoked reclaim. Enrollment token SHA256-only storage, max300s, one-use, identity snapshot; Linux parola/hash tutulmaz. Kayıtlı aktif web hesabı PAM ile sessiz reset edilemez; own Linux self-revoke re-init yolunu açık bırakır, administrative deny kapatır. Gateway Root eligibility kontrolü completion'da yeniden yapılacak.

DB schema1→2 explicit root migration; önce0600 online backup, sonra transaction. Eski daha zayıf pending proof'lar taşınmaz; generation+1; roles/UID/home/PW/budget korunur. Yeni ledger henüz server/helper auth uçlarına bağlanmadı; completion iddiası yalnız primitives içindir.

## D019 — Süreç bağı mevcut PAM biletinde de zorunlu

Yeni root-issued PAM bileti MAC altında domain `tealdrive.identity-ticket.v2` ve kernel UID/PID/start-time/boot-ID içerir. Helper her çağrıda kendi gerçek UID'sini ve parent süreç kaydını tekrar okur; başka süreç aynı UID ile bileti kullanamaz. Caller alanı bulunmayan eski bilet elevated operation'da reddedilir. Server/helper birlikte güncellenmeli; servis yeniden başlayınca kullanıcı tekrar giriş yapar. Bu süreç bağı, henüz bağlanmamış DB generation/nonce revocation'ın yerine geçmez. Aynı server sürecini ele geçiren saldırgana karşı tek başına izolasyon sağlamaz.
