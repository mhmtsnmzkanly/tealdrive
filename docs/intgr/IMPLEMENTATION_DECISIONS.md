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

## D020 — Ayrı root account komut yüzeyi ve kapalı aktivasyon

HelperInvocation::Account + typed account_command: web Login / PAM BeginEnrollment / one-use CompleteEnrollment / Validate / nonce Logout. Root config web_accounts seçer, caller server_uid kernelden doğrulanır; DB path sabit, JSON authority yok. Linux kayıt mevcut root/system/nologin filtrelerini zorunlu tutar; self-registration yalnız authenticated subject allowlist'ini PAM çağrısı için geçici genişletir, root dosyası yazılmaz. Root denied_users DB approval/self-registration üstünedir. Virtual adı UID ile NSS'ye bağlanır; UID/GID/home + nologin/range doğrulanır.

Mode default disabled ve rollout tamamlanana kadar açılmaz. enabled=true eski PAM authentication ve eski PAM operation ticket'ını reddeder; backend hatasında sessiz PAM geri dönüşü yok. Operation grant/server/SDK/UI bağlantısı ayrı aşama. Linux kayıt ilk başta user rolü açar; admin rolünü Root yönetim aracı verir. SessionGrant Debug nonce'ı gizler; yeni web-password alanı Zeroizing serde tamponudur.

## D021 — Okunamayan dosyada owner chmod

Chmod O_RDONLY açılışına bağlı kalmaz; openat2/O_PATH ile inode pinlenir, Linux6.6+ raw fchmodat2 AT_EMPTY_PATH ile chmod uygulanır. Read yasağı chmod yasağı sayılmaz; Discover ve Chmod kontrolleri bağımsız korunur. Glibc /proc/path emulation kullanılmaz. Eski kernel yalnız okunabilir file için ikinci FD ile dev/ino eşleşmesi kontrol edilen fchmod fallback alır; mode000 recovery bu kernelde güvenli olarak başarısız kalabilir. Symlink/special-bit/hardlink/root-readonly/policy kontrolleri devam eder. [Linux man-pages](https://www.man7.org/linux/man-pages/man2/fchmod.2.html) AT_EMPTY_PATH Linux6.6 sınırını belirtir.

## D022 — Web oturumu ile helper izni birlikte taşınır

Web mode giriş, server Authenticator üzerinden root AccountCommand::Login çalıştırır; Authenticated grant server session'a bağlanır. WS, download chunks ve quota worker Cancellation/lease context içinde yalnız bu session grant'ını taşır. Grant varsa legacy username-cache PAM ticket gönderilmez. Helper web mode'da persistent nonce/generation/caller + fresh NSS doğrular ve seçilen config-root'un doğrudan account.home ile eşleşmesini zorunlu tutar; kullanıcı içindeki subdirectory ayrı root yapılmaz, home-root altında relative path olur. Virtual logical adı için initgroups gerçek NSS adı üzerinden; kalıcı UID/GID drop ve Landlock devam eder. Private DB connection helper file engine'e taşınmaz. Public session response yalnız account ID/generation/kind/role; grant serialize edilmez.

Bu aşama config-root'u kendiliğinden bütün hesapların home'una dönüştürmez. Yeni home profile/root template işi PLAN 4'tedir. Eski /srv test root'u gerçek /home/tealdrive-test ile eşleşmediği için reddedildi; candidate root açıkça gerçek home'a ayarlandı, canlı /srv dosyaları taşınmadı. DB admin rolü + root admin roster global panel ayarlarına birlikte gerekir; virtual tenant admin global server yönetimi alamaz.

## D023 — Suspend/revoke açık lease'lerde de uygulanır

Root grant doğrulama ayrı, process-wide bounded8 semaphore kullanır; file-operation slotları dolduğunda güvenlik kontrolü aç kalmaz. Aktif WS/download/quota lease root grant'ını periyodik (normalde1s) doğrular; invalid/unavailable olduğunda local session revoke + cancellation. Yeni helper operation admission'da da doğrulanır. Scheduling/DB/helper latency iptal gecikmesine eklenir; işlem admission ile iptal arasında bitmiş olabilir, geçmiş etkileri rollback garantisi verilmez. Logout kalıcı nonce iptali doğrulanmadan başarı/cookie clear üretmez; backend unavailable503 ve kullanıcı tekrar deneyebilir. UI503'ü başarılı logout saymaz.

## D024 — Enrollment aynı WS'de; browser'a proof verilmez

AuthLogin additive action alanı login/enrollmentBegin/enrollmentComplete. Eski action'sız login korunur. Begin PAM, tek kullanımlık root proof server WS closure'da tutulur; cevap yalnız enrollment_required/expires_at. Completion password aynı WS'den, HttpOnly cookie bridge mevcut akıştan. Preauth5attempt ve64KiB sınırı korunur; PAM proof sonrası deadline300s. Reload/disconnect pending kayıt taşımaz; tekrar PAM gerekir. İlk active web account PAM ile sessiz reset edilemez. Parola12Unicode/1024UTF8/noNUL SDK+root'ta doğrulanır. Root denied_users ve NSS tamamlanırken tekrar okunur.

ServerHello.authentication gerçek mode/enrollment/self-registration bilgisini yayımlar; SDK getInfo aynı bilgiyi camelCase sunar, eski hello için false fallback. UI giriş/kayıtta SDK kullanır; dosya UI'sı mevcut transport'ta (tam files SDK dönüşümü/i18n sonraki aşama). Önceden kapalı stub olan enrollmentId kaldırıldı; proof/token API/browser storage'a çıkmaz. Account ID/generation public metadata ile kullanıcı storage namespace'i credential generation'a bağlanır.


## D025 — Bodyless cookie bridge audit sonucu

204/205/304 HTTP body üretmez; Hyper body wrapper'ı poll etmeyebilir. HttpAudit bu status'ları expected0 kabul eder, başarılı cookie bridge'i yanlış ConnectionClosed olarak işaretlemez. HEAD semantiği korunur; gerçek yarım download hâlâ Failed.

## D026 — Virtual grant quota ve görünür home roots

Authenticated/Validated account view sanal hesapta effectiveQuota verir: virtual admin bütçe-child allocations; tahsis yoksa0. File helper root quota'yı bu DB tavanıyla minimum alır, service snapshot genişletemez. Linux OS quota ölçümü ayrı PLAN işidir. Web session/login response sadece NSS home ile eşleşen configured root'ları sunar; başkasının alias'ı görünmez. Henüz root template atanmayan hesap authenticated olabilir, roots boş gelir; SDK başarılı auth'u directory refresh hatasına çevirmez. Automatic home templates/provisioning + gerçek iki UID quota kabulü bekler.

Web parola ayrı bir credential/hash'tir; kullanıcının Linux parolasıyla aynı değeri seçmesi server tarafından ayrıca karşılaştırılmıyor. Farklı değer seçilmesi önerilir. Linux parola/fingerprint'ı kalıcı tutmadan ve yanlış PAM denemeleriyle OS lockout üretmeden ilk kayıt yapılır.


## D027 — Web parola değiştirme ve öz erişim iptali

`POST /api/auth/password` mevcut web parolasını root helper'da doğrular; yeni parola mevcut Argon2id kurallarına tabidir. Helper grant/caller/NSS/root policy kontrolünden sonra DB generation artırır; bütün eski grant'ler geçersiz olur. Başarılı çağrı HttpOnly cookie'yi temizler, SDK state'i temizleyip bağlantıyı kapatır; yeniden giriş gerekir. Linux parolası değişmez. Yanlış mevcut parola mevcut login limiter'ına dahildir.

`POST /api/auth/revoke` yalnız Linux hesabının kendi erişimini kapatır. Sanal hesap helper'da reddedilir; logout kullanabilir. Kullanıcı dosyaları ve OS hesabı tutulur. Öz iptal administrative deny değildir: root/self-registration politikası uygunsa yeniden PAM kaydı yapılabilir. Suspend/administrative deny üstün kalır. Her iki işlem CSRF ve canlı process-bound grant gerektirir; grant/kimlik client JSON'undan alınmaz. `authentication.password_change` ve `authentication.self_revoke` bağımsız capability'lerdir; eski web-password sürümü bu uçlara sahip sayılmaz.

UI Account security formunda parola alanları gönderim/iptalde temizlenir. JS/HTTP parolalarının bellekte mutlak silinmesi vaat edilmez. Başarılı değişiklikler PasswordChanged/SessionRevoked audit kaydı üretir; parola içerikleri kayda yazılmaz.


## D028 — Hesap listesi ve erişim yönetimi

Registered account metadata listesi NSS aday listesinden ayrıdır: SDK `admin.systemUsers.list()` mevcut aday listesini korur, `accounts()` DB'ye kayıtlı Linux hesaplarını getirir. `allow/deny({accountId})` yalnız mevcut hesabın web girişini açar/askıya alır; yeni OS hesabı veya rol oluşturmaz. Başkasının parola hash'i ve grant'leri dönmez. Linux admin için DB admin rolü + root admin_users listesi birlikte gerekir; virtual admin Root listesinde adı bulunsa bile global yetki kazanmaz. Virtual admin yalnız kendi virtual children'ı yönetir. Endpoint Linux/virtual türünü helper'a sabit parametre olarak gönderir; başka türün ID'si reddedilir.

Admin öz askıya alma ve başka admin hesabına müdahale bu ilk yönetim yüzeyinde kapalıdır; root Wizard ayrı olacak. Enable root deny/NSS uygunluğunu genişletemez. Account deletion/provisioning/setQuota tamamlanmadığı için virtual_accounts capability açılmadı; yalnız tamamlanmış list/enable/disable `authentication.account_management` ile açılır. Henüz kayıtlı olmayan Linux kullanıcıları öz PAM enrollment kullanır; admin onayıyla ilk kayıt Wizard/approval aşamasında ele alınacak.


## D029 — Sanal admin effective quota snapshot'ı

Admin budget ile child reservation toplamı tek SQLite SELECT içinde okunur. İki farklı sorgu concurrent yönetim işlemiyle farklı snapshot'ları birleştirip gereğinden büyük kalan kota gösterebilirdi. Tahsis işlemleri mevcut immediate transaction'larını korur; kullanım helper ölçümüdür. Bu düzeltme OS quota/provisioning tamamlandı anlamına gelmez.


## D030 — Admin kabul fixture'ının sınırları

Public admin pozitif kabul testi mevcut gerçek kullanıcıyı admin'e yükseltmez veya askıya almaz. Yeni UUID OS identities/root-only metadata bootstrap ve ayrı yerel server kullanılır. Root policy fixture süresince geçici genişletilir; eski byte'lar atomik restore, cleanup sadece fixture identities/grants/proofs/home'larıdır. Server veya browser'a Root authority/rol verme komutu eklenmez. Root example yalnız development bootstrap'tır; final Wizard yetki atama akışı halen ayrı PLAN işidir.


## D031 — Admin onayı ve bekleyen PAM kanıtları

`admin.systemUsers.approve({username})` / `POST /api/admin/system/approve` yalnız Root roster + DB Linux admin rolündeki aktöre açılır. Helper NSS'den UID/GID/home alır, Root mandatory eligibility/deny uygular ve DB'de password'süz Linux/User metadata kaydı oluşturur. Root allowed_users dosyası değişmez; mevcut OS hesabı, parola veya rol değiştirilmez. Öz kayıt kapalıyken onaylı kullanıcı ilk PAM→web password kayıt akışına devam edebilir. Öz kayıt açıkken uygun hesapların onay beklemesi zorunlu değildir. Var olan hesap409; approve suspended/revoked/deleted hesabı yeniden açmaz. UID alias yarışında SQLite unique constraint409; arbitrary DB/I/O hatası503 kalır.

Root/sys/service identity filtresi discovery ile helper'da aynı shared reserved-name kontrolünü kullanır. Yüksek UID/interactive shell tek başına bilinen servis adına izin vermez. `.env`/home policy veya Root UID tavanları onayla genişletilmez. Bu aşama automatic home template oluşturmaz; dosya erişimi hâlâ Root'un gerçek NSS home ile eşleşen profiline bağlıdır.

Access transition aynı immediate transaction'da bekleyen enrollment proof'larını da siler. Suspend→enable sonrası önceki PAM proof canlanamaz; kullanıcı yeni PAM doğrulaması yapar. Native kabulde eski cookie/grant ve eski proof ayrı ayrı reddedildi. Onay/enable/suspend başarıları actor ve target account ID/username ile audit'e yazılır; secret yoktur. TLS/central root journal rollout'u ayrı PLAN maddesidir.


## D032 — Yönetim UI yetki ve lifecycle sınırı

Yönetim yeni embedded same-origin ES module'dür; SDK üzerinden server/helper kontrollerine gider. Server/Root kimliği UI'da yeniden icat edilmez. UI RoleAdmin + capability, Linux global scope için isAdmin+accountKindLinux gösterim uygular; helper gerçek nihai yetkiyi bağımsız doğrular. UI Root admin rol verme/OS UID/path/virtual provisioning/delete/quota işlemlerini açmaz. RoleAdmin satırları Root-managed; user yalnız kendi child virtual grubuna erişir.

Mutation confirmation cancel before-request semantics: iptal backend'e göndermez. Loading/error stale controls temizlenir; modal close request epoch'ını geçersiz kılar, kabul edilmiş server işlemini geri alma vaat edilmez. Whole file-manager migration/i18n gelecekteki iştir. Accounts module ve INDEX_HTML asset-version digest'e eklendi; module no-cache ile sunulur. Bağlantı kurulmadan HTML'de connected sınıfı gösteriliyordu; bu yanlış başlangıç sınıfı kaldırıldı, Native test gerçek session/ProtocolReady sonrası bekler.


## D033 — Home çözümleyicisi ve açık migration sınırı

`policy::home::resolve` yalnız protected gateway'den doğrulanmış Account ile kullanılır. Helper operation grant/NSS doğrulaması ve Root snapshot kısıtlamasından sonra legacy profilleri hesabın home'una daraltır. Aktif/login-enabled, non-root UID ve generation kontrolleri tekrar uygulanır. Sistem ağaçları ve `/home/tealdrive` uygulama verileri dosya kökü olamaz; sanal hesap için yalnız `/home/tealdrive/virtual-homes/<hesap>` doğrudan çocuk yolu istisnadır. Gerçek symlink/inode doğrulaması mevcut FD/openat2 motorunun işidir; lexical çözümleyici bunların yerine geçmez.

Yeni home template çözümü ayrı çağrı olarak hazırlanmıştır; template path yerine doğrulanmış account.home kullanır, read-only/upload/quota ve ortak politikaları korur. Var olan exact-home legacy profili template ile sessizce değiştirilemez. `.env` sensitive kuralları bu aşamada kaldırılmaz. Runtime/Root config template seçimi, bütün HTTP/WS bağlantıları ve kullanıcı policy yönetimi henüz eklenmediğinden otomatik home özelliği tamamlanmış sayılmaz ve capability açılmaz. Sanal allocation eksikse kota sıfır; Root kota daha düşükse o korunur.


## D034 — Home template uçtan uca bağlantısı

D033'ün hazırladığı resolver artık Root-only `AppConfig.home_template` seçimiyle HTTP login/session/download/preview, WS file dispatch ve bağımsız privileged helper'a bağlıdır. Otomatik mod yalnız web_accounts enabled + boş static roots ile seçilir. Template materialize edilirken alan kaldırılır, root path doğrulanmış account.home olur. Helper gelen snapshot'tan home otoritesi almaz. WS hesabı bağlantı başlangıcında protected grant ile çözülür; her dosya helper çağrısı güncel grant/NSS/Root config'i yeniden doğrular. Root config değişimleri her frame'de yeniden materialize edilir; generation ve NSS revoke mevcut lease mekanizmasındadır.

Legacy sensitive deny/warn listelerini template seçimiyle otomatik silmek izin genişlemesi olurdu; Root açık migration sırasında inceler. Yeni kurulum için config örneği .env pattern'lerini iki listeden de çıkarır, sistem bileşeni deny motoru korunur. Native fixture'da .env deny kaldırılınca ilk deneme POLICY_DENIED verdi: warn_read listesi de mevcut sensitive motorunda erişim kontrolüne katılıyor. Test config'i açıkça iki listeyi güncelledi; production Root kuralları gevşetilmedi.

Aday binary/helper birlikte yenilendi; eski binary'ler private rollback dizininde tutulur. Caddy backend'i ve gerçek kullanıcı verileri taşınmadı. Kullanıcı admin/personal policy mutation uçları ve Wizard Root rol/policy düzenlemesi henüz uygulanmadı; home maddesi bütünüyle tamamlanmış sayılmaz.


## D035 — Protected kullanıcı politikası kalıcılığı ve CAS

Kullanıcı policy metadata'sı root-only accounts.db `security_state` içinde `account_policy_v1:<account-id>` şemalı/versioned bounded JSON olarak tutulur. Ayrı kullanıcı dosyalarını service yazabilir yapmak veya Root-owned users.d'yi server'a açmak yerine mevcut protected SQLite control plane kullanılır. İlk explicit hesap kaydı default kullanır; var olan bozuk/version-unknown kayıt sessiz default'a düşmez. En fazla512 kural/katman, serialized record512KiB, shared AccessPolicy doğrulaması uygulanır.

Root CLI store Authority::Root ile Root katmanını yönetebilir. Web gateway Root katmanını reddeder. Personal yalnız kendisi; admin yalnız Root roster+DB Linux admin veya kendi virtual children scope. Kind endpoint'e bağlıdır. Root TOML'deki eski admin/personal etiketli kurallar bile web açısından immutable Root rules olarak gösterilir. Bunlar editable DB katmanıyla birleştirilip bağımsız helper ceiling olarak uygulanır; snapshot onları çıkaramaz.

Katman replacement + generation++ + pending PAM proof silme aynı immediate transaction'dır. expectedGeneration zorunlu CAS'tir. Eski cookie/grant/lease geçersizdir, SDK kendi hesabı değiştiyse session/state'i temizler. Admitted eski işlemin değişiklikten önce tamamlanması tüm harici writer yarışlarını sıfırlama iddiası değildir; yeni helper girişleri güncel generation/NSS/Root policy kontrolünden geçer. Rule ID revision-bound index'tir (`personal_0` vb), kalıcı inode kimliği değildir. SDK add/remove current revision'ı okur, tek CAS write yapar; conflict otomatik yeniden denenmez.

`authentication.policy_management` ayrı capability; eski hello'da default false. SDK files.policy ve admin system/virtual policy surfaces tipler/cookbook ile bağlandı. UI salt okunur üst katmanlar ve checkbox deny seçimi kullanır; başarılı kişisel save yeniden girişe yönlendirir. Servis DB'yi açmaz, Root config'i yazmaz. PolicyChanged audit actor/target ve operation içerir; policy/parola içeriği merkezi loga yazılmaz. Trusted Root journal/undo sonraki ayrı aşamadır.


## D036 — Ayrı UID/GID sanal provisioning ve idempotency

Virtual create, helper control plane'de Root tarafından açılır; varsayılan kapalıdır ve explicit home template gerektirir. Admin yalnız kendi bütçesiyle User çocuk oluşturur; Root rolü/UID/GID/home/shell API'den alınmaz. Login alias ASCII lowercase64 karakter, mevcut NSS adını/service adını sahiplenemez. OS adı tdv_<UID>, home /home/tealdrive/virtual-homes/tdv_<UID>, ayrı aynı-numaralı GID, locked OS password ve nologin shell sabittir. İlk web password Argon2id ile metadata commit'inde yazılır; server DB'yi açmaz.

Root-only provision.lock, UID/GID/NSS/DB çakışma kontrolü, bounded1024 slot scan, max account ve owner budget kontrolleri uygulanır. virtual_create_v1:<owner-id>:<UUID> versioned intent syscall'dan önce kalıcılaşır ve bütçe ayırır. Enrolled account+hashed password+completed intent tek SQLite transaction'dır. Aynı key/spec aynı hesabı döndürür; replay parola reseti değildir ve ilk parolayı saklamak için ikinci verifier/log tutulmaz. Pending intent actor scope'a bağlıdır; Root repair/recovery Wizard aşamasında bağlanacaktır.

Pending reservations parent/child budgets'e dahildir; malformed state geniş quota üretmez. Parent virtual admin'in kendi kullanımı dağıtılamaz. Delete default Root false; açılırsa yalnız User çocuk, generation CAS, erişim hemen iptal, dosya/OS kimliği default36h tutulur. Purge/retired UID fencing lifecycle/Wizard aşamasında tamamlanacaktır; suspended asla otomatik silinmez.

## D037 — Native quota, FD ve geçiş sırası

Root enforcement seçimi application veya user_quota. Application mode logical bytes/helper cross-process directory flock; native mode Linux quotactl_fd VFS user quota, allocated space + inode hard limits kullanır. Caller block-device/path seçemez; verified home FD kullanılır. Kernel hard limit missing/mismatch/unsupported ise fallback yapılmaz. Native yeni tahsis>=4096,1024 katı; zero kernel limit unlimited anlamına geldiğinden internal zero için over-restrictive1KiB kullanılır. Native FS metadata/staging alanı kullanır; atomic edit için geçici headroom gerekebilir.

Kaynaklar: [Linux quota UAPI](https://man7.org/linux/man-pages/man2/quotactl.2.html), [Q_SETQUOTA](https://man7.org/linux/man-pages/man2/Q_SETQUOTA.2const.html). Host root FS remount edilmedi; kabul ayrı temporary ext4+usrquota loop volume'de yapıldı. Kernel6.8.0 üzerinde uid file writes, hardquota bypass probe ve UID ownership doğrulandı. Diğer FS'ler probe/test olmadan desteklenmiş sayılmaz.

Quota increase DB reservation commit sonrası kernel child limit'i yükseltir; shrink child kernel limit'i önce daraltır. Parent limit ters yönde güncellenir. DB quota generation/CAS ve durable virtual_quota_sync_v1 repair marker kullanır; OS/DB tamamen tek atomik kaynak değildir. Failure over-restrictive kalır, native limit mismatch normal file access'i reddeder; Wizard native-sync bu repair yolunu tamamlayacaktır.

Flock bekleme sırasında quota/policy değişebiliyordu. Yazma lock'u alındıktan sonra grant/generation/NSS, fresh Root config+DB rules ve budget tekrar kontrol edilir. Virtual admin WS quota view her frame'de fresh validation alır; HTTP quota handler home materialization eksikliği de düzeltildi. Kullanım tipi logical_bytes/allocated_bytes açıkça döner. Trash metadata quota'ya dahil edildi. ENOSPC/EDQUOT aynı DISK_FULL sözleşmesi; text-save mapper'ın DiskFull'u INTERNAL_ERROR'a düşürmesi düzeltildi.


## D038 — Offline Wizard ve dar yerel UID yönetimi

Kurulum ile servis kontrolü ayrı binary'lerde. Wizard hiçbir zaman setuid değildir; helper'ın yerel komutları kernel/process-derived caller UID ile sınırlanır. Linux hesabının init/revoke/personal policy işlemlerine hedef UID/path parametresi eklenmez. Nologin istisnası yalnız Root listesiyle; servis/root zorunlu denetimleri devam eder. Root approval ve role değişikliği server transport ayarlarını değiştirmez. Update önce reverified private staging yapar, eski binary setini metadata yedeğinin yanında tutar. Binary rollback generation'ı geriye götürmez. Metadata restore grants/enrollment'ı temizler ve generation'ı artırır. Root bütçe azaltma provisioning global lock ve directory reservation altında gerçek kullanımı da kontrol eder. Native repair yalnız mevcut allocation'ı uygular; eksik kernel desteğinde uygulama moduna sessiz dönüş yoktur.


## D039 — Root sanal admin rezervasyonu ve güvenli deleted purge

Root-created admin de ayrı nologin UID/GID; `__root__` intent pool bütçesini OS oluşturma öncesinde ayırır. Pool/budget azaltımı bu pending tahsisi de korur; final metadata+password+budget tek transaction. Allocator unfinished intent UID'sini atlar; NSS backend hatasını “kullanıcı yok” saymaz. Cancel yalnız hiçbir OS/home state'i olmayan niyetlerde yapılır; başladığında aynı key ile resume. Deleted child purge36h, Rootglobal/directory locks, yeni executable worker→kalıcı UID/GID düşüşü→Landlock→contents removal; Root yalnız boş generated home/NSS kimliği kaldırır. Nested mount reddedilir. Intent ve retired UID fence OS silme öncesi kalıcıdır; OS zaten kalkmışsa yalnız doğrulanmış intent ile resume edilir. Tombstone quota0/passwordNULL, grant cleanup. Suspended/Linux/virtual-admin otomatik silinmez. Hourly timer explicit enable; genel audit/trash/undo retention ayrı aşamada tamamlanacak.


## D040 — Gömülü UI de public SDK müşterisidir

UI'ye özel ikinci WebSocket/MessagePack pipeline kaldırıldı. Auth/files/admin aynı SDK bağlantısında; DOM adapter SDK'nın camelCase sonucunu mevcut DOM view alanlarına dönüştürür. Optional managed state aktif, per account/origin/generation storage; path helper validate olmadan güvenilmez. UI editor read tamamlanmadan input açmaz; geç listing cevabı view generation ile atılır; mutation opening root'unda kalır. Büyük native download ve media için secret olmayan URL convenience method'ları eklendi; backend authorization atlanmaz. Config inspect mode/read, reload ve persistfalse eklenirken settings.get/patch default davranışı korunur.


## D041 — UI dili yetki/state değildir

EN/TR sözlük anahtarları ve parametreleri aynı tutulur. Sadece data-i18n işaretli caption/attribute çevrilir; user filename/content untouched. Form label translation ayrı span üzerinde yapılır; child input'lar korunur. Dil tercihi origin+accountId tarayıcı namespace'inde en/tr değeridir; başka hesaba taşınmaz. Root config/DB'ye dil kaydı gerekmez. SDK kararlı hata kodunu sunar, UI onu çevirir. Unknown label English fallback; canonical protocol/options değerleri çevrilmez.


## D042 — Standart TLS ve explicit offload sınırı

rustls/ring + axum-server0.7.3; kendi crypto/anahtar protokolü yok. Root TLS paths aynı anda cert/key, Groupread640/private root-safe parent, symlink/hardlink/world access rejected. Explicit requireTLStrue proxy mode loopback+trusted proxy+singleHTTPS header; directTLS actual TLS socket. Broken TLS startup noHTTPfallback. Bind/transport/cert paths hotreload veya DynamicConfig.store ile değiştirilemez, legacy configte de restart gerekir. Certificate content refresh restart sınırında; webadmin cert/key management yapmaz. Provider PEM input zeroize, certificates/public keys publicinfo; TLS private key service memory threat modeliyle uyumludur.


## D043 — Root-approved immutable UI snapshot ve sabit kurtarma namespace'i

UI package sadece Root; all-declared-assets validation, optional SHAmap, bounded16MiB snapshot. Missing/incompatible/unsafe package whole embedded fallback; valid-package undeclaredasset404. Critical static_dir webpatch bütün modlarda kapalı. Versioned Rootdirectory copy +Rootserverconfig activation; helper key/DB unchanged. `/ui/default` route custom package'den bağımsız builtin assets/locale imports ile çalışır. SDK namespace override yasak, fallback `/sdk/*` unknownpath404. CSP connectself/workernone; ServiceWorker ile fixedSDK interception yapılmaz. Custom JS trusted same-origin applicationcode olduğu için manifest integrity iyi niyet garantisi değildir.

## D044 — Metadata sayfalama ve canlı tenant sınırı

2026-10-13: Username keyset cursor, bounded limit ve SQL ön-filtre; cursor yetki taşımaz. Eski array API korunur, yeni SDK page metotları açık sonuç sözleşmesi sunar. Root CLI tüm kayıtları bounded sayfalarla tarar. Linux adayların registered bilgisi sayfalama/filtreleme nedeniyle yanlış yeniden onay seçeneği üretmez. Purge tombstone'ları native sync/diagnose sırasında kaldırılmış NSS kimliği olarak hata sayılmaz. Sanal admin bütçesi kendi kullanımını da düşen availableBytes ile gösterilir.

## D045 — Helper'da kalıcı giriş rezervasyonu

2026-10-13: PAM/web-parola doğrulamasından önce kısa SQLite IMMEDIATE işleminde slot ayrılır; paralel helper süreçleri veya servis restart'ı sınırı sıfırlamaz. Başarılı doğrulama yalnız kendi slotunu iade eder, diğer hataları silmez. Crash/forbidden/backend error slotu pencere sonuna kadar tüketir. Root helper config max_login_failures (default5), lockout_duration_seconds (default300) belirler. Aynı pencere için hesap5, normalize IP25, global200 varsayılan; IP/global katsayıları5/40. Pencere ilk denemeden başlar; eşik sonrası yeni denemeler reddedilir. Hashlenmiş etiketler ve sayaçlar dışında parola tutulmaz; etiket hash'i anonimlik garantisi değildir. En fazla10000 satır; süresi dolanlar girişte temizlenir. Dolduğunda geniş izinli fallback yok. IP server'ın trusted proxy çözümlemesinden gelir; ele geçirilmiş server IP iddiasını değiştirebilir, hesap/global Root sınırını kaldıramaz. Own-UID Wizard'da IP yok, hesap/global sınır geçerli. Legacy salt-PAM modunda eski in-memory limit korunur; kalıcı sayaç protected web-account modelinde uygulanır.

## D046 — Host mount yerine Root-only preallocated native volume

2026-10-13: Wizard fixed Root600 ext4 image, gerçek backing block allocation, bounded min/alignment ve boş kurulum denetimi. Standart mke2fs/ext4 quota ve kernel quotactl_fd; kendi quota motoru yok. mkfs.ext4 distro symlink'i güvenli utility kontrolünde reddedildiğinden gerçek Root binary mke2fs -t ext4 kullanılır. Shared provisioning lock; offline server check. Boot mount + service dependency, noexec/nosuid/nodev. Pool usable total capacity'den Root reserve ve5%headroom çıkarılır; anlık free'ye göre mevcut tahsisler yanlış reddedilmez. Root diğer kullanımı yönetir. Genel metadata backup kullanıcı/image verisi yerine geçmez. PID+mount namespace kabul testi gerçek /proc kullanır; üretim process'lerini görmez ve fake /proc ile quota utility davranışını bozmaz.

## D047 — Fresh executable privsep ve dar internal delegation

2026-10-13: Root helper supervisor grant/NSS/policy/quota doğrular; ayrı aynı-executable worker User IDs +Landlock ile FS işler. Worker context Root-created pipe/kernelRootparent +30s expiry/PID/realUID/nonce/domain-separated existing HMAC-SHA256 ile bağlıdır. Privileged key worker doğrulaması sonrası açılıp kapanır, kullanıcı dosya işlemi sırasında açık Root FD/key yok. Parent exe proc inspection bazı container'larda CAP_SYS_PTRACE gerektireceğinden güvenilir IPC+protected-key delegation seçildi; Root caps genişletilmez. Account/global authority server'a verilmez. Worker başına yeni süreç maliyeti güvenlik journal sınırı için kabul edilir. Root parent quota lock'u tutar; stdin EOF bounded cancellation kullanılır. Non-elevated dev helper ve identity rejection eski structured PermissionDenied yanıtını korur.

## D048 — Move/rename/trash restore atomik no-overwrite

2026-10-13: Önce hedef yok kontrolü ardından fs::rename dış writer yarışında üzerine yazabiliyordu. Move/rename/trash transfer/restore artık doğrulanmış directory FD'leri üzerinden renameat2 RENAME_NOREPLACE kullanır. Private trash parent'a geçici, dar logical-parent alias yalnız bu helper adımında uygulanır; genel gizli alan politikasını açmaz. Undo hedef çakışması da aynı primitive üzerinden reddedilecek. Dış süreç dosya içeriğini değiştirebilir; burada sağlanan garanti hedef adını ezmemektir.

## D049 — Protected operasyon intent/result temeli

2026-10-13: accounts.db security_state altında versioned operation_v1/account/time/UUID anahtarları; mevcut Root-only SQLite/WAL/backup güven sınırı tekrar kullanılır. Yeni operasyon kaydı Root grant/NSS/policy doğrulamasından sonra, kullanıcı worker başlamadan commit edilir. Sonuç success/failed/unknown; crash sonrası pending başarı sayılmaz. Request ID server audit ile eşleştirme içindir. Request dosya içeriği, parola, cookie/grant veya free-form exception metni kaydedilmez. Bounded record/account count/retention; intent yazılamazsa structured error ve hiç işlem başlatmama. Sonuç kaydı yazılamazsa gerçekleşmiş dosya işlemi sahte rollback/hata olarak sunulmaz, pending history mesajı taşır. Kullanıcıya history/undo ve log-kota entegrasyonu tamamlanana kadar yeni journal default disabled ve SDK capability kapalıdır; eski kuruluma sessiz log kotası yükü eklenmez.

## D050 — Own history scope, filters ve protokol uyumu

2026-10-13: GET/api/audit→protected History grant; accountId/UID seçimi yok. SQL action/status/root/path/from/to/requestId filtreleri LIMIT öncesi; helper current home/persisted deny policy görünmeyen source/target kayıtlarını saklar. Bounded1000-row visibility scan sonraki cursor ile sürer; toplam/gizli count verilmez. DESC timestamp+UUID keyset, cursor authority değildir. SDK typed filters/immutable results; bilinmeyen seçenekleri kabul etmez. AuthenticationCapabilities audit_history extension kullanılır: v1 FeatureFlags deny_unknown_fields sözleşmesine yeni flag eklemek eski Rust istemcilerinde handshake bozabilirdi. Backend quota/actual metadata/UI tamamlanana kadar advertised flag false kalır.

## D051 — Yerel proxy admin API ayrı yetki sınırı

2026-10-13: Somut UID995/config endpoint okuma testiyle Caddy localhost2019 yönetimi DNS credential sızıntısı yolu olarak doğrulandı. Dosya640 yeterli değildi. Caddy Root/Caddy-only0700 RuntimeDirectory Unix socket'e taşındı; TCP kapandı. Systemd ExecReload private address ile güncellendi, graceful reload ve originHTTPS200 geçti. Backend/domain değişmedi; CF edge bu sunucudan403 ayrı final ağ kabulü konusudur. Root/Caddy credential değerleri ne test çıktısına ne Git'e girer. Caddy opsiyoneldir; genel kurulum proxy'nin ayrı yönetim/secret sınırını korumalıdır.

## D052 — İşleme göre beklenen durum ve actual pinned chmod metadata

2026-10-13: Chmod old/new mode aynı O_PATH inode'dan gerçek fstat değerleri; istenen mode gerçek old/new sayılmaz. Chmod undo yalnız permission mutation olduğu için identity/UID/GID/mode/nlink/mtime/ctime metadata fingerprint ile korunacak; dosya içeriği değiştirilmez. Mode000 ve read-denied dosyada hash için geçici chmod/read yetkisi verilmeyecek. Move/rename/trash expected-state içerik SHA256 +metadata; directory için alt node/content fingerprints. Snapshot User UID+Landlock ve mevcut Read/Discover policy altında, max10000 nodes/depth64/64MiB; incomplete state tam fingerprint gibi sunulmaz. Root protected record before/after class optional/versioned, cihaz/inode JS precision kaybı olmaması için decimal string. Intent record byte tavanı post-state metadata için2048 byte rezervasyonla işlem öncesi denetlenir. External writer ile mutlak atomic tree snapshot veya inode-label security iddiası yok.

## D053 — Gerçek UID log kotası, rezervasyon ve temizlik istisnası

2026-10-13: Sanal own-operation ledger `/home/tealdrive/virtual-homes/.tealdrive-audit/<account-id>/<operation-id>.json`, Root-only700 parent, User UID:GID400 file. Worker'a writable FD verilmez. Dosya aynı quota filesystem'de UID'ye gerçek blok/inode yükü sağlar. Config max_record_bytes4096-blok hizasında preallocated reservation; default8192 byte/event, payload +sonuç/padding dahil gerçek ayrılmış alan. Native kernel usage zaten kapsar; application ölçümünde helper güvenilir ledger filelen'i home'a ekler, çift sayım yok. Önce rezervasyon, sonra FS; başarısızlıkta mutation başlamaz. Başarılı FS'den sonraki ledger-result hatası fake rollback olmaz, pending warning gösterilir.

Kota doluyken yalnız Root-feature/policy izinli live permanent delete bounded systemReserve olayıyla yapılabilir; kendi dosyasını silerek alan açma log kotası tarafından engellenmez. Kritik Root/security metadata ayrı sistem bütçesi; Linux merkezi logları da sistem bütçesinde. Journal defaultmax2048/account ve32768global; oldest completed nonundo event veya Root undo-max sonrası event bounded rotation, pending/unknown yakın kayıtlar korunur. Retention504h Root GC500 batch; gerçek charge dosyası ve record silinir. Deleted virtual purge36h sonrasında retained log inode UIDRoot'a aktarılır, eski kullanıcı budget'ı serbest kalır. Min virtual allocation journal açıkken64KiB (büyük reservation varsa yükselir), getBudget.minimumQuotaBytes ve UI bildirir. Root config enable eski hesaba sessiz migration değildir.

## D054 — Tek kullanımlık koşullu undo ve offline restore fence

2026-10-13: Kendi protected Root operation kaydı inverse source/target/mode kaynağıdır. Root current grant/NSS/home/policy + ortak lock sonrası tekrar denetim, durable pending claim, signed User worker ve actual expected-state karşılaştırması kullanılır. Move/rename/trash full bounded digest + inode; chmod pinned metadata ve eski0777 mode. Restore undo_epoch döndürür, audit kalır. Pending/unknown veya uygulanmış başarısız deneme otomatik tekrar edilmez; belirsiz sonuçta ikinci mutasyon başlatmamak seçildi. Pref null Root max'a bağlıdır; min1,maxRoot(default24), own CASrevision, login generation değişmez. Liste actual FS doğrulaması değildir; uygulama anında check zorunludur. Trash internal target görünürlüğü original path'ten hesaplanır; private info kullanıcı yazabilir olduğu için yetki kaynağı yapılmaz. Ayrıntılı mevcut API ve sınırlar UNDO.md.

## D055 — Journal ayarı server snapshot'ından türetilemez

2026-10-13: PolicySnapshot restrict artık journal alanını tamamen Root ceiling'den alır. Server kendi config kopyasında enabled=false veya farklı kayıt boyutu/count/undo limitleri göndererek protected kayıt zorunluluğunu veya quota maliyetini değiştiremez. Helper otoritesi server capability ilanından bağımsızdır. Runtime web patch journal alanı sunmaz.

## D056 — User worker watchdog thread confinement sonrasında başlar

2026-10-13: Kernel Landlock politikası mevcut sibling thread'lere otomatik uygulanmaz; yalnız sonradan oluşturulan descendants miras alır. Standalone helper watch flag'i önce kaydeder; invocation decode sonrası Root supervisor için watch başlar, Worker için UID/GID drop ve Landlock sonrası başlar. Private worker confinement öncesi ve identity resolution sonrası tek thread zorunludur; güvenli olmayan multi-thread fallback yok. Fresh process/watch-parent pipe modeli korunur. Kaynak: [Linux Kernel Landlock inheritance](https://docs.kernel.org/userspace-api/landlock.html#inheritance).

## D057 — Audit'ten bağımsız bounded trash cleanup token

2026-10-13: Journal rotation veya TTL eski trash event'ini kaldırabileceği için GC authority ayrı trash_expiry_v1 token'ıdır. UUID Root operation ID'den gelir; signed worker artifact context tarafından kullanılır. Queue intent-before-FS, current scope ve actual after-state ile bounded tutulur; User writable trash info otorite değildir. Root CLI fresh Wizard worker User UID/GID+Landlock ile cleanup yapar. Değişmiş/denied veri needsReview ile korunur, otomatik retry yok. Undo/restore/delete ve36h physical account purge token'ı emekli eder. Root lifecycle hours audit TTL'den bağımsızdır; staging_root artifact adlandırma/24h config temeli eklendi, staging GC devam ediyor.

## D058 — Virtual UID/GID ve subordinate ID eşlemeleri ayrıdır

2026-10-13: POSIX UID farklılığı, başka Linux kullanıcısının subuid/subgid aralığıyla çakışırsa tek başına izolasyon değildir. Helper allocator her iki dosyada non-Root delegasyonlarını dışlar; kullanılabilir configured alt aralıklarını yalnız UID0 için rezerve eder. Shadow password-file lock + trusted owner/link/mode/read + atomic/fchmod/fsync kullanılır. Numeric owner0 Root adının değişmesine bağlı değildir. Yeni nologin virtual OS hesapları SUB_UID_COUNT=0/SUB_GID_COUNT=0 ile subordinate eşleme almaz. Root sonradan elle overlap yaratmamalıdır; dosya dışı subid NSS provider fail-closed desteklenmez. Fixture UID aralığı1m ve kendi Root reservations temporary/byte-identical restore ile test edilir. Kaynak: [Linux user namespaces](https://man7.org/linux/man-pages/man7/user_namespaces.7.html), [subuid(5)](https://man7.org/linux/man-pages/man5/subuid.5.html).

## D059 — Exact staging token ve Root private as-of kabulü

2026-10-13: Yarım kalmış dosyalar için Root-generated operation ID'den türeyen private slot ve ayrı staging_expiry_v1 token kullanılır. Successful publish token emekli eder; pending/unknown/failed metadata audit TTL'den bağımsız korunur. User worker current policy/owner/device/mtime/ctime/manifest doğrular; yeni/changed staging korunur. Root private worker test clock kabulü host clock değiştirmeden gerçek UID/Landlock temizliği test eder; web yüzeyi değildir. Varsayılan24h ve Root max_cleanup sınırı.
