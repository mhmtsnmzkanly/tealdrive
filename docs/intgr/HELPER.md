# TealDrive HELPER geliştirme sözleşmesi

> **Durum:** Protected identity/grants, home/politika yönetimi, ayrı sanal UID/GID/native kota ve Root supervisor→user worker ayrımı uygulanmıştır. Journal/undo ve protected lifecycle tamamlandı. Aşağıdaki eski hedef şema örnekleri çalışan config yerine geçmez; kesin durum PLAN.md ve TEST_RESULTS.md içindedir.

Ortak motor root/global + kullanıcı root/admin/personal yasaklarını birleştirir; config snapshot helper root yasaklarını silemez. Gizli öğe doğrudan read/stat/download'da NotFound, list/search/recursive archive'da yok. Parent move/rename, final extraction target, restore ve trash delete tekrar kontrol edilir. Hardlinkli normal dosyalar default erişilemez. Root kritik tavanı TOML/CLI ile; yetkili admin ve kullanıcı kendi sınırlı politikalarını protected gateway ile yönetir.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Kaynak ve geliştirme sınırları

İlgili kod: `src/helper/security.rs`, `policy.rs`, `ipc.rs`, `sandbox.rs`, `text.rs`, `chmod.rs`, `archive.rs`, `trash.rs` ve `src/fs/`.

Dosya motoru kullanıcı UID/GID'sine kalıcı geçişten sonra çalışır. Root yönetim komutları ayrı sınıftır. Server'ın verdiği keyfi UID/home/config yoluna güvenilmez; ticket caller UID, hesap nesli, expiry ve güncel politika kapsamını doğrular. Revoke/reload aktif lease'leri de etkiler. Politika eksikliğinde fail-closed ile server çalışma config'inde son geçerli snapshot farklı davranışlardır.


### 3.2 `policy/helper.toml` — root güvenlik tavanı

```toml
schema_version = 1

[identity]
server_uid = 995 # kurulumda gerçek tealdrive UID'si yazılır; sabit varsayılmaz
key_path = "/home/tealdrive/policy/helper.key"
accounts_path = "/home/tealdrive/auth/accounts.db"
linux_login_mode = "pam_enroll_then_web"
reject_root = true

[registration]
self_registration = true
minimum_regular_uid = 1000
allow_nologin_self_registration = false
# Root/admin onaylı hesaplarla self-registration aynı etkin listede birleşir.
allowed_users = []
denied_users = ["root", "tealdrive", "caddy", "www-data", "nginx", "apache"]

[home]
source = "account_database"
allow_home_symlink = false
reject_paths = ["/", "/etc", "/proc", "/sys", "/dev", "/home/tealdrive"]

[features]
save_text_file = true
chmod_file = true
extract_archive = true
delete_permanently = false
chown = false
special_mode_bits = false

[defaults]
hidden_project_files_allowed = true
deny_system_paths = [".ssh", ".gnupg", ".aws", ".kube"]
deny_private_key_patterns = ["id_rsa", "id_ed25519", "*.key", "*.pem"]
# .ssh kararı kesin; diğer listedeki sır örnekleri kurulumda gözden geçirilir.
# .env bu varsayılan yasakların içinde değildir.

[limits]
max_upload_bytes = 1048576
max_text_edit_bytes = 131072
max_directory_page_size = 500

[virtual_accounts]
enabled = true
home_root = "/home/tealdrive/virtual-homes"
uid_min = 200000
uid_max = 299999
shell = "/usr/sbin/nologin"
# UID aralığı örnektir; mevcut hesaplarla çakışma Wizard tarafından kontrol edilir.

[quota_pool]
total_bytes = 107374182400 # 100 GiB
enforcement = "application" # application | project_quota | dedicated_filesystem
include_trash = true
include_staging = true

[audit]
critical_mutations_require_audit = true
max_file_bytes_ceiling = 52428800
max_archives_ceiling = 20
retention_days_ceiling = 365
```

`allowed_users` root'un doğrudan onayladığı Linux hesaplarını belirtir. **Etkin allowed_users** bu liste + root/admin tarafından verilmiş geçerli onaylar + helper'ın doğruladığı öz kayıtlar olup, `denied_users` ve hesap uygunluk kuralları üstün gelir. Boş listeyle kurulmuş bir hesap kendiliğinden webden giriş yapamaz; uygun kullanıcı Wizard `init` ile kayıt oluşturabilir. Öz kayıt kapatılınca yeni öz kayıt engellenir; mevcut kaydı iptal etmek ayrı `revoke`/root deny işlemidir.

OS UID aralığı veya `shell` webden seçilemez. Nologin gerçek SFTP hesapları root'un açık uygunluk onayıyla eklenebilir; sanal nologin hesapların tanınması TealDrive sağlama kaydıyla yapılır. TealDrive hesabı root korumalı ebeveynin sahibi olduğu için bu modele kendi home'undan dosya kullanıcısı olarak alınmaz.

Argon2 maliyeti ve güvenlik sayaçları için ek sınırlar uygulama geliştirilirken benchmark ile seçilir; belgedeki kota ve UID aralığı örneklerdir, gerçek kurulum kararı değildir.



### 3.4 `policy/users.d/<account-id>.toml` — kullanıcı politikası

```toml
schema_version = 1
account_id = "acct_example_alice"
account_generation = 1
# UID/home eşleşmesi accounts.db ve OS kaydıyla doğrulanır; buradan keyfi UID seçilmez.

[root]
source = "home"

[root_constraints]
read_only = false
uploads_allowed = true
chmod_allowed = true

[[root_rules]]
path = ".ssh"
recursive = true
deny = ["discover", "read", "write", "chmod", "delete", "move"]

[[admin_rules]]
path = "projects/production"
recursive = true
deny = ["chmod", "delete", "move"]

[[personal_rules]]
path = "private"
recursive = true
deny = ["discover", "read", "write", "chmod", "delete", "move"]

[[personal_rules]]
path = "projects/app/.env"
recursive = false
deny = ["read", "write"]
```

`discover` yasağı normal API'de görünmezlik ve doğrudan erişim yasağı anlamına gelir; alt ağaçta uygulanır. Salt okuma yasağı varsa kayıt adı görünür olabilir ama içerik indirilemez. Kopyalama, arşivleme ve restore da kaynak/hedefin ilgili discover/read/write kurallarına tabidir.

`root_rules` yalnızca root tarafından, `admin_rules` atanmış yönetici tarafından, `personal_rules` ilgili kullanıcı tarafından yönetilir. Kullanıcı bir alanı root/admin katmanına taşıyamaz; istek rolü gerçek kimlikten çözülür. Global sistem dosyası default'u root/admin tavanı kapsamında uygulanır; kullanıcı `.ssh` yasağını yalnızca kendi kuralını silerek kaldıramaz. `.env` varsayılan açık olup örnekte kişisel seçimle kapatılmıştır.

Bozuk policy fail-closed davranır. Default kullanımı açık hesap kaydıyla tanımlanır; eksik zorunlu policy geniş erişim üretemez. Dosya uzantısı/regex eşleştirmesi eklenirse ayrı şema/limit tanımlanır; örnek ilk yüzey exact relative path + recursive kullanır.



## 4. Yetki modeli ve çelişkilerin çözümü

### Root, panel yöneticisi ve kullanıcı

- Root/Wizard kurulum güvenlik tavanını, kayıt uygunluğunu ve admin yetkilerini belirler.
- Panel yöneticisi kullanıcıya yönelik kısıtları root'un izin verdiği aralıkta yönetir. Server root politika dosyalarını doğrudan yazmaz.
- Kullanıcı Wizard ile yalnızca gerçek çağıran UID'ye ait hesabın erişimini ve kişisel kısıtlarını yönetir; kullanıcı adı parametresi başkasının kimliği olamaz.
- Kişisel yasak kaldırılırken root/admin yasağı kalkmaz. Birleşimde her katmanın yasağı geçerlidir; kuralın kaynağı arayüzde belirtilir.
- Webden yönetilebilen kısıtların yalnızca server'ın yazdığı dosyada durması bağımsız helper korumasını zayıflatır. Bu nedenle kullanıcı politikası güncellemeleri için helper dar, şeması doğrulanan yönetim komutları sunacak.
- Helper yönetim komutları yalnızca kimlik ve root kontrollü yetki doğrulamasıyla kullanılacak; keyfi dosya yazma, shell çalıştırma veya config yolu seçme sağlamayacak.
- Ele geçirilmiş server geçerli bir admin yetkisini kullanabilir. Root tavanı buna rağmen aşılamaz; tüm admin işlemlerinin zararsız kalacağı iddia edilmez.

### Eksik politika ve fallback

- Root kontrollü genel default politika geçerli olmak zorunda.
- İlk kayıt sırasında hesap için açık başlangıç kaydı/politikası oluşturulur. Kayıtsız bir hesap sessizce tam home erişimi almaz.
- Hesap kaydı default kullanacak şekilde açıkça tanımlıysa kullanıcı override'ı bulunmaması normaldir.
- Gerekli dosya silinmiş, bozuk veya güvenilirliği doğrulanamıyorsa yeni geniş izinli default üretmek yerine erişim reddedilir. Canlı yenilemede son geçerli daha geniş politikaya süresiz güvenilmez.

### Trash ve geçici dosyalar

- Kullanıcının home'unda ayrılmış iç dizinler tutulacak; normal listeleme/arama/download işlemlerinden erişilemeyecek.
- Helper bunları tümüyle göremez hâle getirilemez: Trash/restore ve cleanup için ilgili komutların erişmesi gerekir. Yalnızca bu komutlara sınırlı erişim verilir.
- İşlemler kullanıcı UID'siyle çalıştığından dizini sadece `tealdrive` yazabilir yapmak mevcut modelle çelişir; root dosya motoruna dönüş önerilmez.
- Gerçek Linux kullanıcısı SSH üzerinden kendi iç dizinlerini değiştirebilir. Helper metadata'yı güvensiz kabul edip doğrular; bu dizinler dokunulmaz yedek sayılmaz.



### 7. Hash ile editör çakışma kontrolü — orta/büyük

- Helper okuduğu byte'lardan hash üretir; içerik ile aynı sürümü temsil eden token döner.
- Kaydetmede yeni içerikle birlikte `expected_revision` gönderilir. Helper yazma işleminin içinde mevcut dosya sürümünü karşılaştırır; farklıysa conflict kodu döner.
- Ayrı “önce hash sor, sonra kaydet” akışı tek başına yeterli değildir; arada değişiklik olabilir.
- Metadata değişikliği/silme/yeniden yaratma için dosya kimliği ve gerekli metadata sürümü de değerlendirilir; atomik publish ve sahiplik/mode/ACL korunumu incelenir.
- Dış süreç advisory kilide uymuyorsa kontrol ile publish arasındaki tüm yarışları taşınabilir biçimde engelleme garantisi yoktur; sınır belgelenir.
- Kabul: İki panel oturumu + harici writer ile conflict; kullanıcı düzenlemesi kaybolmaz; fark gösterme/yeniden yükleme seçenekleri.



### 11. Kullanıcı politikaları ve home kökleri — büyük

- Genel default + root kullanıcı tavanı + admin kısıtları + kişisel kısıtlar; her katmanda yasak öncelikli.
- Root kontrollü helper yönetim protokolü; web admin yetkisi ve Wizard gerçek UID kontrolü.
- Home'u NSS/hesap kaydından helper çözer; home/UID/grup değişiminde hesap nesli ve eski oturum/bilet iptali.
- Görme yasağı tüm alt ağaca erişim yasağıdır; okuma/yazma/chmod ayrı sınırlanabilir.
- Listeleme, metadata, search, HTTP/WS download, editor, upload, copy/move, arşiv ve trash/restore için kaynak/hedef/alt içerik kontrolü.
- Hardlink/symlink, üst klasörü taşıma veya arşivleme ile gizli içerik sızması için negatif testler. Desteklenmeyen güvenli durumlar reddedilir.
- `.ssh` ve sistem/kimlik anahtarları varsayılan kapalı; `.env` ve diğer proje dotfile'ları varsayılan açık. Görme/okuma/düzenleme kişisel/admin kurallarıyla ayrı daraltılabilir. Root/admin yasağı kişisel ayarla kaldırılamaz. Mevcut blanket hidden/sensitive filtreleri bu seçici modele açık migration ile dönüştürülür.
- Kabul: İki hesap izolasyonu, gerçek özel home, eksik/bozuk politika, canlı revoke ve tüm dolaylı erişim denemeleri.


## SDK gezinti/state güncellemesi

2026-10-12 yeni server state, helper path doğrulaması, üç SDK alanı ve event/kalıcı bildirim sözleşmesi [STATE.md](STATE.md) altında. Bu hedef henüz uygulanmış değildir; ilgili kabul testleri orada tanımlanır.

## 2026-10-12: Görünmezlik hedefinin son açıklaması

**Son kullanıcı açıklaması önceki OS görünüm izolasyonu isteğini değiştiriyor:** Helper config'inde gizlenen `.env` gibi öğeler, list-directory fonksiyonunda atlanır. Özel mount/namespace veya dosya adının OS düzeyinde hiç var olmaması hedefi bu kapsamdan çıkarıldı.

Mevcut root/UID/GID/Landlock/openat2 sınırları korunur. Gizleme uygulama/helper politikasıdır; OS'in yasaklı inode'u erişilemez yaptığı şeklinde sunulmaz. Liste/search/preview/direct read/archive davranışları ortak policy motoruyla tutarlı olmalı. Sadece UI'de filtre yeterli değildir.

Açık kullanıcı sorusu: Gizleme yalnızca listeden kaldırma mı, doğrudan path ile erişimi de engelleme mi? Öneri ikisi; kullanıcı yanıtı gelene kadar bu ayrım kesinleşmiş sayılmaz. Mevcut izinler bu taslak nedeniyle gevşetilmez.

## Ayrılmış uygulama işleri — Helper

1. Kimlik/hesap türü/UID-GID/home doğrulama, mevcut bilet ve revoke güvenliği.
2. Genel/root/admin/kişisel politika birleşimi; kullanıcıya özel güvenilir policy kayıtları.
3. Config tabanlı helper listeleme filtresi ve ilgili işlem politikalarının ortak uygulanması; özel OS görünüm izolasyonu kapsamdan çıkarıldı.
4. Kullanıcı home'u içinde doğrulanmış navigate/validate/stat; state konumu için güvenli gerçeklik sonucu. Kalıcı UI state helper işi değildir.
5. Listeleme filtre/sıralama/sayfalama ve ayrı recursive search; yetkisiz metadata/isim sızmaması. Limitler ve kararlı cursor tanımlanmalı; sonradan SDK'ya bütün dizini filtreletmek zorunlu olmamalı.
6. CRUD/chmod/archive/trash dosya motoru; revision/hash karşılaştırması, atomik publish ve izin/ACL korunumu.
7. Kota DB kaydı ve helper kullanım ölçümü; ortak havuz/rezervasyon/yazma sınırı. Yüzde uyarısının UI sunumu helper işi değildir.
8. Yetkili sanal hesap sağlama, kişisel/admin politika değişimi ve root sınırlı yönetim komutları.
9. Doğrulanmış eski/yeni audit verisi, dar korumalı log yazma/okuma ve koşullu undo; root dosya motoruna dönüş yok.
10. IPC boyut/timeout/cancel/reap/çökme toparlanması; server kapandığında kaynak temizliği.

SDK nesne/method isimleri, popup, çeviri ve browser reconnect helper içinde uygulanmaz. Server'dan gelen path/filter/role güvenilir varsayılmaz; helper bağımsız doğrular.

## Gizli öğe erişimi — kesin son kapsam

Gizlenen dosya/dizin webden hiç yokmuş gibi davranır: Liste/search dışı doğrudan stat/read/download/editor, recursive copy/archive, trash/undo ve audit görüntüleme de güvenli policy kontrolünden geçer. Known hidden ile missing yanıtları dışarıda aynı bulunamadı davranışına normalize edilir; iç audit gerçek policy nedenini kaydedebilir. UI filtresi yetmez; helper tüm source/target/alt içerik yollarını kontrol eder. Yeni isimle erişim/hardlink gibi dolaylı erişim sınırları test edilir, OS seviyesinde görünmezlik garantisi iddia edilmez.


## Uygulanan hesap policy gateway'i

Protected DB policy records Root/admin/personal deny katmanlarını saklar. Root-only config ceiling ve DB katmanları helper'da snapshot restrict öncesi birleştirilir. PolicyGet/Set komutları canlı process-bound grant/NSS/role/kind/owner denetimlidir; web Root katmanını seçemez. Personal yalnız kendisine, admin yalnız yetkili scope'a yazar. Değişiklik atomik generation artırır ve pending enrollment proof'larını temizler. Bozuk policy fail-closed. TOML kullanıcı kuralları Root-owned kalır. Bkz. D035 ve TEST_RESULTS policy kabulü.

## Root supervisor ve dosya worker'ı

Setuid Operation önce protected grant/NSS/home/policy/native quota doğrular ve mutation directory lock'unu alır. Sonra aynı helper executable'ı fresh process olarak çalıştırır; watch thread sonrasında unsafe fork callback içinde dosya/DB işi yapılmaz. Worker özel Root-created pipe, kernel parent UID/EUID, parent PID, expiry ve domain-separated standart HMAC-SHA256 delegation doğrular. Server Worker invocation/config/UID uydurarak bu yolu kullanamaz; key hiçbir IPC payload'ına gönderilmez. Parent executable'ını ptrace benzeri okuma izniyle doğrulamak yerine protected key ve özel pipe kullanılır; ek CAP_SYS_PTRACE gerektirmez.

Worker bütün UID/GID değerlerini kalıcı düşürür, Landlock uygular ve dosyayı o kullanıcı olarak işler. Root DB/key/directory FD'leri CLOEXEC; dosya worker'ına verilmez. Root supervisor kullanıcı içerik işlemini kendi Root yetkisiyle yürütmez; lock'u response alınana kadar tutar. Parent exit/cancellation pipe EOF ile worker'a gider; mevcut cooperative cancellation/watchdog korunur. Root auth/yönetim komutları tek helper; dosya Operation geçici supervisor+worker iki süreç kullanır. Binary sayısı değişmez, worker aynı helper binary'dir; per-user daemon değildir. Bu ayrım Root-only journal'ın kullanıcı dosyalarına Root olarak erişmeden saklanabilmesi için temeldir; henüz history/undo capability açılmaz.

## Uygulanmış undo gateway

Protected AccountCommand History(undoOnly)/ApplyUndo/UndoPreferences/SetUndoPreferences current grant ile kendi scope'unu türetir. Inverse User worker aynı helper executable ve HMAC-signed Root record üzerinden çalışır; kullanıcı writable trash info kullanılmaz. Metadata backup restore eski undo epoch'u invalidate eder; User worker'a Root DB FD verilmez. [UNDO.md](UNDO.md) güncel sözleşmedir.
