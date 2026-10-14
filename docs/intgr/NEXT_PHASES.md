# Çekirdek geliştirme: yeni aşamalar

Kullanıcının son kapsamı: hosting yönetimi eklenmez. Mevcut veri ve aktif servis korunur; değişiklikler ayrı commit ve kabul testleriyle yayınlanır.

## Birinci aşama

- [ ] UI/oturum: cookie bridge başarısızlığı, yeniden bağlantı, anlaşılır hata, aktarım ilerleme/iptal.
- [ ] Büyük dosya aktarımı: sınırlandırılmış bellek, parçalı helper yazımı, kota/iptal/temizlik; devam desteği açıkça belgelenir.
- [ ] Sanal genel panel admin: servis UID ayrı kalır; korumalı yetki kapsamı hesabın Linux/virtual türünden ayrılır. Sanal home ve başlangıç web parolası. Eski admin geçiş doğrulanmadan kaldırılmaz.
- [ ] Backend TLS: güvenilir sertifika yenileme ve proxy trust güncelleme; HTTP fallback yok.
- [ ] Ek uygulama şifrelemesi: tehdit modeli, anahtar güveni, sürümlü handshake, standart protokol/kütüphane, downgrade/replay testleri. TLS yerine geçmez; XSS veya endpoint compromise koruması iddiası yok.

## İkinci aşama

- [ ] Root Wizard ortak imaj init/status/check/expand/sync; aktif imajı ezme ve otomatik shrink yok.
- [ ] Net dağıtılabilir havuz ile gerçek ext4 kapasitesi/metadata işletim payı ayrılır. Fiziksel alan önceden ayrılır, kapasite doğrulanır.
- [ ] Atomik tahsis; kullanım ile tahsis ayrılır. Admin kendi dosyaları da sanal havuzdan düşer.
- [ ] Sistem DB/güvenlik/control audit bütçesi kullanıcı havuzundan ayrı, bounded. Kullanıcı geçmişi/trash kendi kotasından.
- [ ] Mount ve crash recovery; dış servislerin aynı dosya alanını erişim izinleriyle kullanması belgelenir, hosting provision yapılmaz.

## Kararlar ve ilerleme

- Başlangıçta kök diskte 96 GiB boş alan var. Debug sembolleri ve incremental build kapatılarak derleme alanı azaltılır. Her büyük test/release öncesinde df kontrol edilir; kullanıcı dosyaları, audit ve aktif binaries silinmez.
- SDK cookie bind veya session confirmation hatasında doğrulanmış WS kapatılmalıdır; aksi halde ikinci AuthLogin dosya komutu dispatcherına gider ve NotImplemented döner. Bu kod kusuru doğrulandı; cookie kaybının tarayıcıdaki özgül nedeni henüz doğrulanmadı.
- Büyük aktarım yalnız limit yükseltme değildir: mevcut commit tüm dosyayı Vec içine okuyup helper IPC üzerinden gönderiyor. Güven sınırları korunarak protokol genişletilmelidir.
- Ek şifreleme yaklaşımı tasarımla seçilecek; rastgele sunucu anahtarını güvenilir kabul eden özel şema uygulanmayacak.

### Oturum köprüsü düzeltmesi

SDK bind HTTP reddi, ağ hatası ve bind başarılı/session401 durumlarında kimliği temizler ve WS kapatır. Tekrar giriş yeni handshake gerektirir. Üç regression senaryosu mevcut SDK testine eklendi. `cargo test --locked --test sdk_client` passed (incremental/debug sembolleri kapalı). Canlı release henüz değiştirilmedi. Test sonrasında kök disk boş alanı95GiB.

### Genel yönetim kapsamı

Hesap türüne bağlı Linux-only genel admin denetimi kaldırıldı. Genel panel yönetimi için protected DB RoleAdmin, protected panel_scope=global ve Root-owned account_policy.admin_users listesinde açık ad birlikte zorunludur. Bu listeye dahil olmayan virtual RoleAdmin tenant kapsamında kalır; RoleUser listeye yazılsa bile admin olamaz. Linux rol komutu sanal hesap için kullanılmaz; Root Wizard account scope ACCOUNT_ID global|tenant komutu bu kapsamı atar. Server runtime overlay bu listeyi değiştiremez. Kullanıcı dosya UID/GID kimliği değişmez; Root yetkisi webden kritik yapılandırma düzenleme anlamına gelmez. Yeni DB şeması veya kullanıcıya açılan role claim gerekmedi.

Bu değişiklik source aşamasındadır; henüz canlı sürüme geçilmedi. Servis adıyla aynı panel adı için Root provisioning istisnası ayrıca dar kapsamda uygulanacak; normal self-registration/service filtreleri gevşetilmeyecek.

Yetki kapsamı regression testi ve tam library testi passed:588 başarılı,1 açık PAM testi normal koşuda ignored. Root roster dışında tenant sınırı ve roster içinde sanal genel yönetici doğrulandı.

### Panel adı ve servis kimliği

Root-only virtual admin provisioning `tealdrive` panel adına dar bir istisna tanır. Generated NSS adı/home/UID hala tdv_UID; servis NSS hesabı benimsenmez. Normal virtual kullanıcı oluşturma, Linux approval ve PAM enrollment servis adını reddetmeye devam eder. Diğer servis adlarına istisna yok. İsim collision istisnası yalnız Root create_admin yolundadır. Regression testi passed;590 library test toplamında589passed/1ignored.

### UI aktarım/giriş ilk iyileştirmesi

Yükleme progress sıfırlama ve AbortController üzerinden iptal düğmesi eklendi. Boyut hatası sunucunun gerçek byte sınırını kullanıcıya gösterir; TR/EN iptal ve disk dolu mesajları eklendi. Login submit işlem boyunca disabled; eşzamanlı ikinci login gönderilmez, başarısızlık sonrası tekrar etkinleşir. Browser codec, permission/login-secret/concurrent-submit, shutdown ve SDK integration testleri passed. Üretim release değişmedi. Derleme cache591MiB, disk95GiB boş; eski34GiB cache davranışı incremental/debug kapalı tutularak önlendi.

### Yükleme staging güvenliği

Staging create_new+0600+NOFOLLOW/CLOEXEC ile açılır; umask yanlış olsa da kullanıcı verisi world-readable olmaz. Chunk reopen symlink izleyemez.11 upload lifecycle testi geçti; mode assertion eklendi. Büyük aktarım limiti bu committe artırılmadı.

### Backend TLS ilk adım

Root-owned PEM çiftini60s aralıkla doğrulayıp canlı rustls config'e yükleme eklendi. `tls status` gerçek kurulum sertifikasını/private-key çiftini doğruladı, public fingerprint/date ve30gün yenileme uyarısı verdi. İzole gerçek listener kabulü `cargo test --locked --test tls_refresh -- --ignored`:124.44s içinde geçti; invalid key son geçerli sertifikayı korudu, yeni çift restart olmadan yayınlandı, eski CA yeni self-signed kimliği reddetti. Production cert/key/Caddy değişmedi. Otomatik sertifika üretimi ve çift-trust rollout henüz tamamlanmadı. Tasarım ve sınırlar TRANSPORT_SECURITY.md içinde.

### Büyük aktarım ilk uygulama/kabulü

UploadEnd artık dosyayı Vec/MessagePack içine almaz. Server readonly FD197 üzerinden setuid supervisor'a verir; supervisor/worker readonly regular-file,real-callerUID,0600,nlink1,exactsize kontrolü yapar. HMAC request stream_size'ı bağlar. Worker User UID/GID+Landlock'a geçtikten sonra önceden açık tanıtıcıdan64KiB bounded-buffer ile okur; her blokta cancellation denetlenir. Mevcut transaction staging/no-replace publication ve kernel/app quota korunur. Helper byte-count ile total_size eşleşmeden başarı döndürülmez.

Source default per-file1GiB, Root hard ceiling64GiB; max_in_flight_upload eski alan adı/alias uyumluluk için korundu. Frame/chunk limitleri büyütülmedi. Small in-process UploadHelper adaptörünün eski arayüz fallback'i3MiB ile sınırlıdır; production stream override kullanır. Server staging başlangıçta beklenen boyut+1GiB boş alan ister; her chunk'ta1GiB sistem boşluk payı kontrol edilir ve process içi staging yazıları serialize edilir. Bu dış programların diski doldurmasına karşı mutlak garanti değildir. Root kullanıcı kotasından büyük dosya begin'de reddedilir.

Kabul:591library test toplamında590passed/1ignored.17MiB helper testi IPC request<4KiB ve blok blok içerik doğrular. Config/browser/SDK/WS-auth integration passed. Gerçek private namespace Chromium+setuid/User-worker testinde17MiB upload/download,68progress,HTTP206range,abort cleanup,unsafe staging mode ve truncation reddi, yeniden bağlantı ve failed target yokluğu passed. Production helper ceiling hâlâ1MiB; release yapılmadan canlı büyük upload destekli sayılmaz. Kesinti sonrası cross-connection upload resume henüz uygulanmadı; disconnect/abort staging temizler. Native HTTP download Range desteği mevcut ve büyük dosya yolu test edildi.

```sh
TEALDRIVE_CORE_TRANSFER_ACCEPTANCE=1 \
TEALDRIVE_TEST_WEB_PASSWORD_FILE=/root/private-existing-test-password \
python3 tools/dev/core-transfer-acceptance.py
```

Araç gerçek onaylı tealdrive-test hesabının cloned auth DB'sini kullanır; tüm home/policy/bin/runtime/auth mount'ları private namespace içindedir. Fixture ve credentials kopyaları finally cleanup edilir, live config/home/DB değişmez. Kaynak binaries target/debug altında önceden hazırlanır.

### API cache güvenliği

Bütün /api/ cevapları,401/403 ve file download dahil,private,no-store ve Vary:Cookie alır. SDK HTTP/bind/download fetch cache:no-store kullanır. Bu kişisel cevapların browser/proxy cache yazımını önler; eski mobil401 olayının kesin cache kaynaklı olduğu iddia edilmez. WS/auth ve SDK testleri geçti. Proxy operatörü bu başlıkları zorla override eden cache rule kullanmamalı.

### Protected yönetim kapsamı

Root scope ataması mevcut security_state tablosundaki panel_scope:ACCOUNT_ID kaydıyla korunur; yeni dosya veya schema-version değişikliği yok. Eski Linux RoleAdmin kayıtlarının davranışı korunur; yeni virtual RoleAdmin varsayılan tenant. `account scope ID global|tenant` scope/generation değişimini tek DB transaction'ında yapar, eski grants/proofs geçersiz kalır. RoleUser scope yükseltemez; web komutu yok. Root config roster ikinci sınır olarak korunur. Gateway Validate cevabı effective global_admin flag'ini Root'un güncel roster'ıyla sınırlar; stale Server config scope revocation'ı uzatamaz. Genel admin Root kritik tavanlarını, signing key veya DB'yi yazamaz.

592library tests:591passed/1explicit PAM ignored. Scope regression: virtual kimliği/UID/home değişmeden Root global ataması, tenant'a geri dönüş, eski generation reddi, RoleUser ve self-authority scope ataması reddi. Production admin/homes bu aşamada değiştirilmedi.

Protected SQLite gerçek Root kabulü3test geçti: WAL/private UID erişim reddi, eşzamanlı bütçeler, explicit migration/backup/proof invalidation ve kalıcı login sayaçları. WS-auth+SDK integration geçti. Clippy all-targets -Dwarnings passed.

### Sanal genel admin: gerçek izole kabul

`TEALDRIVE_CORE_VIRTUAL_ADMIN=1` kabulü private mount/PID namespace içinde /etc NSS dosyaları,Root auth/policy/binaries/runtime ve128MiB quota-enabled ext4 volume oluşturdu. Root Wizard tty echo kapalı doğrulanarak `tealdrive` sanal adminini64MiB budget ile oluşturdu; OS adı tdv_UID oldu,service UID995 benimsenmedi. Root scope global ataması sonrası gerçek Chromium UI settings ve system/config yönetim200;17MiB upload/stream download/Range/abort/tamper cleanup yeniden geçti. Root scope tenant'a dönüşte eski cookie401 ve yeni login config403/settings disabled doğrulandı; tenant RoleAdmin/home/UID korundu. Host NSS'de yeni tdv account yok, production auth/home/image değişmedi, fixture/loop mount otomatik temizlendi.

### Net havuz ve imaj büyütme ilk uygulaması

Yeni Root komutları storage init --pool-size SIZE,storage check ve storage expand --pool-size SIZE. Eski storage init SIZE fiziksel boyut arayüzü korunur. Net başlangıç pool hesaplanan physical reserve+metadata payı ile hazırlanır; ext4/native kapasite ölçülmeden metadata pool yayınlanmaz. Küçük ext4 testi160MiB physical ile128MiB net kapasitenin garanti olmadığını gösterdi;64MiB fixed overhead minimumu ve ölçüme bağlı bounded ek büyütme eklendi. Hesaplanan başlangıç boyutu küçük havuzda en az128MiB, genel havuzda125% veya pool+64MiB büyük olanıdır.

Expansion Root-owned0600 inode/device/target journal ile resume eder; farklı image/target veya shrink reddedilir. Her retry bütün image extentlerini fallocate ederek sparse/partial previous allocation'ı karşılar, host diskte1GiB headroom ister. Mount loop block device ve backing image inode eşleşmesi doğrulanır. Loop capacity+resize2fs+native quota+actual capacity kontrolünden sonra Root DB pool değişir. User quotas otomatik değişmez. Pending expansion varken pool set reddedilir.

Gerçek private NSS/native quota/Chromium kabulü genişletildi:128→192MiB image,64→128MiB net pool,gerçek resize2fs,mevcut17MiB dosyada SHA256 korunması,pending temizliği ve shrink rejection passed. Production image/metadata büyütülmedi. CLI status physical/net/delegated/child allocated ayrımını gösterir. Sabit admin own quota ve sistem budget işleri bu değişiklikte henüz yok.

Imaging sonrası tam library:593toplam/592passed/1PAM ignored. Clippy all-targets -Dwarnings passed. Init/grow physical hedef pool125% veya pool+64MiB minimumuyla başlar; gerçekkapasite yetmezse Root journal hedefi kontrollü büyütülür, en fazla3iterasyon sonra matching resume mümkündür. Native büyütme kabulü finalkod ile tekrar geçti.

### Ayrı sistem bütçesi ilk sınırları

Root-only [system_storage]: max_metadata_db_bytes=536870912,max_staging_bytes=4294967296,max_staging_transfers=128,staging_min_free_bytes=1073741824. Web overlay SysStorage'yı değiştiremez. Root SQLite her connection'da max_page_count sınırını uygular; büyüme SQLITE_FULL ile atomic reddedilir. journal_size_limit16MiB checkpoint sonrası best-effort WAL sınırıdır, toplam DB+WAL disk hard quota iddiası yok.

Upload staging process-wide declared-byte ve transfer-count reservation kullanır; ActiveUpload clone'ları Arc ile aynı rezervasyonu paylaşır, last drop serbest bırakır. Zero-byte uploads da transfer slot tüketir; Sys stage budget user image allocation'ından ayrıdır. Bu servis UID'sine bütün OS filesystem'lerinde uygulanmış kernel kota değildir; başka process/stale files/external writes için disk headroom/lifecycle ve operasyon denetimi gerekir. Native user kotası ayrı kernel mekanizmasıyla korunur.

596library tests:595passed/1explicit PAM ignored; metadata page full/no partial insert,stage budget clone-release/zero-byte count,Root SysStorage overlay rejection passed. Gerçek Root SQLite3acceptance tests migration/WAL/privacy/concurrent budgets/login limits yeniden passed. Production config/DB budget henüz değiştirilmedi.
