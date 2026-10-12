# TealDrive WIZARD geliştirme sözleşmesi

> **Durum:** Aşağıdaki “Uygulanan komutlar” gerçek CLI sözleşmesidir. Sonraki hedef örnekler tasarım notlarıdır; henüz uygulanmış sayılmaz.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Uygulanan komutlar (12 Ekim 2026)

Dört binary vardır: server, setuid helper, **setuid olmayan** wizard ve bağımsız `tealdrive-ctl`. Wizard kurulum/metadata yönetimini; ctl yalnız sabit `tealdrive.service` birimini yönetir. Caddy zorunlu değildir. Yerel başlangıç 127.0.0.1:13100 HTTP'dir; kamuya açma için proxy/TLS ve origin ayarları gerekir.

```bash
# Root tarafından onaylanan, root-owned ve yazmaya kapalı release dizini:
sudo tealdrive-wizard install --bundle /root/tealdrive-release
sudo tealdrive-wizard config validate
sudo tealdrive-wizard config show
sudo tealdrive-wizard config apply /root/approved-config.toml
sudo tealdrive-wizard diagnose
sudo tealdrive-wizard account approve alice
sudo tealdrive-wizard account approve sftp-user --allow-nologin
sudo tealdrive-wizard account list
sudo tealdrive-wizard account admin ACCOUNT_ID
sudo tealdrive-wizard account user ACCOUNT_ID
sudo tealdrive-wizard account deny ACCOUNT_ID
sudo tealdrive-wizard account allow ACCOUNT_ID
sudo tealdrive-wizard account password ACCOUNT_ID
sudo tealdrive-wizard pool set 100GiB
sudo tealdrive-wizard budget set ADMIN_ID 60GiB
sudo tealdrive-wizard storage sync
sudo tealdrive-wizard policy root ACCOUNT_ID GENERATION /root/rules.json
sudo tealdrive-wizard policy admin ACCOUNT_ID GENERATION /root/rules.json
sudo tealdrive-wizard policy personal ACCOUNT_ID GENERATION /root/rules.json

# Gerçek Linux kullanıcısının kendi hesabı; parola argv/env değil TTY:
tealdrive-wizard init
tealdrive-wizard status
tealdrive-wizard policy get
tealdrive-wizard policy personal GENERATION ./rules.json
tealdrive-wizard revoke

# Kurulumdan bağımsız servis kontrolü:
sudo tealdrive-ctl start
sudo tealdrive-ctl stop
sudo tealdrive-ctl restart
sudo tealdrive-ctl reload
tealdrive-ctl status
```

`--allow-nologin` yalnız Root'un açık istisnasıdır; root, servis kimliği, yasak listesi ve UID aralığı denetimlerini kaldırmaz. Kayıt verisi NSS ile uyuşmuyorsa sessiz hesap benimseme yoktur. Rol değişimi mevcut biletlerin generation'ını iptal eder. Hesap ayarı değiştirmek server bind/origin/audit alanlarını helper konfigürasyonuyla ezmez. `storage sync` yalnız `user_quota` modunda, mevcut DB tahsislerini kernel limitlerine uygular; allocation değiştirmez. Bütçe azaltma child/pending reservation ve sanal adminin kendi kullanımını kontrol eder.

```bash
sudo tealdrive-ctl stop
sudo tealdrive-wizard backup /root/new-backup-directory
sudo tealdrive-wizard update --bundle /root/tealdrive-release
# Update yedeğinin binaries alt dizini; DB geriye alınmaz:
sudo tealdrive-wizard rollback --bundle /home/tealdrive/storage/update-backup-UUID/binaries
sudo tealdrive-wizard restore /root/new-backup-directory
sudo tealdrive-wizard repair
sudo tealdrive-wizard uninstall
```

Bundle `manifest.json` schema_version=1, version ve dört sabit binary için SHA-256 içerir. ELF ve trusted ownership/mode/ancestor kontrolleri yapılır; checksum kaynağın imzası değildir, kaynağı Root onaylar. Güncelleme bütün dosyaları önce private olarak stage ve tekrar verify eder. Metadata/key backup Root700 dizindir; SQLite online backup WAL'i kapsar. **Kullanıcı içerikleri, native volume ve runtime preferences metadata backup'a dahil değildir; ayrı dosya sistemi yedeği gerekir.** Restore yalnız durmuş serviste, servis UID uyumuyla yapılır; restore edilmiş grants/proofs temizlenip bütün hesapların generation'ı artırılır. Binary rollback authentication DB'ye dokunmaz. Uninstall servisi başarıyla durdurmadan executable silmez; config, key, DB ve dosyalar korunur. Elle başlatılmış candidate de offline işlem öncesi durmalıdır.

Test komutu `TEALDRIVE_WIZARD_ACCEPTANCE=1 python3 tools/dev/wizard-acceptance.py`: private mount namespace içinde home, systemd, PAM ve systemctl izole edilir; mevcut yayın ve kullanıcı dosyaları değişmez. Systemctl stub olduğu için bu test gerçek systemd start/stop kabulü sayılmaz. Gerçek PAM/local init ve lifecycle/native recovery ayrı kabul adımlarıdır.

## Sanal admin ve yaşam döngüsü komutları

```sh
# Root havuzu önce atanır; request UUID tekrar denemede aynen kullanılır.
sudo tealdrive-wizard virtual admin create tenant-one 60GiB REQUEST_UUID
# Parola güvenli TTY prompt'undan; her admin ayrı nologin UID/GID/home.
sudo tealdrive-wizard pending list
sudo tealdrive-wizard pending cancel 'virtual_create_v1:OWNER:REQUEST_UUID'
sudo tealdrive-wizard storage sync
sudo tealdrive-wizard gc
# Kurulum timer dosyalarını yazar, otomatik etkinleştirmez:
sudo systemctl enable --now tealdrive-gc.timer
```

Root virtual admin oluşturma, pool rezervasyonunu kalıcı intent ile ayırır; metadata+ilk web parolası+admin budget aynı SQLite transaction'da yayınlanır. Tekrarda aynı request/name/budget aynı hesabı döndürür ve parolayı reset etmez. Sanal admin sistem kullanıcılarının/global Root ayarların yöneticisi değildir. Child reservation ve adminin kendi dosyaları aynı budget'tan düşer.

`pending cancel` yalnız henüz OS UID/GID/home oluşmamış intent'i iptal eder; mevcut veri/kimliği silmez. UID retired olur. OS adımı başlamışsa aynı request key ile create yeniden denenir; native yanlış limit `storage sync` ile mevcut protected allocation'a döner. Native ölçüm/limit hatasında session/file işlemi fail-closed kalır.

`gc` yalnız deleted **sanal User** hesaplarını, varsayılan36 saatten sonra en fazla100'lük batch'te temizler. Linux hesabı/home'u veya suspended hesap silinmez; sanal adminin otomatik silinmesi bu yüzeyde yoktur. Root directory reservation ve global provisioning lock altında yeni worker başlatır. Worker bütün UID/GID'leri düşürüp ek grupları temizler, Landlock ile tek home'a kapanır, sonra içeriği temizler. Symlink hedefi takip edilmez; nested mount reddedilir. Root yalnız boş generated home ve doğrulanmış tdv_UID hesabı/grubunu kaldırır. Purge intent kesinti sonrası tekrar ilerler, UID fence tekrar kullanımını engeller; retained DB tombstone quota0/passwordNULL olur. Her worker30 saniye sınırında; timer hourly, Root, oneshot ve explicit enable'dır. Genel trash/audit/undo cleanup bu gc komutunun henüz kapsamı değildir.

## Henüz uygulanmayan hedef yüzeyi

Aşağıdaki eski tasarım örnekleri (`config set/edit`, remote admin login, audit/undo CLI, tam lifecycle/migration) uygulanan komut listesinde bulunmadıkça kullanılamaz.

## Reload ve kaynak

Wizard çekirdeği uygulanmıştır; kapsamlı migration/lifecycle yüzeyi sıradaki aşamadır. Setuid olmayan CLI gerçek yerel UID veya remote admin session kullanır; serbest hedef kullanıcı parametresine güvenmez.

```bash
sudo tealdrive-wizard config validate
sudo tealdrive-wizard reload
sudo tealdrive-wizard service status
```

Uygun alanlar otomatik yenilenir; `reload` aynı doğrulama yolunu elle tetikler. Çok alanlı değişiklikte `config edit` tek aday üretir; art arda config set güvenliksiz ara durumu yayınlamamalı. Restart-required açıkça gösterilir. Ayrıntı [SERVER.md](SERVER.md).


## 4. TealDrive Wizard komutları

Önerilen binary `tealdrive-wizard`. Root veya gerçek UID rolü OS'tan alınır. `--account` başkasını seçmeye yalnızca yetkili root/admin yönetiminde izin verir; sıradan kullanıcı komutlarında hedef gerçek UID'dir. Wizard setuid değildir; sınırlı yetkili adımlar helper'a gider. Yönetim oturumu varsa parola/token terminal prompt'uyla alınır; argv/env/history içine yazılmaz.

### 4.1 İlk kurulum — root

```bash
sudo ./tealdrive-wizard install --bundle ./tealdrive-release
# Etkileşimli: TLS/proxy modu, auth modu, ilk admin, havuz, platform kontrolü.

sudo /home/tealdrive/bin/tealdrive-wizard diagnose
sudo /home/tealdrive/bin/tealdrive-wizard config validate
sudo /home/tealdrive/bin/tealdrive-wizard service start
```

Bundle root onaylı kaynak olmalı; bozuk checksum/signature, güvensiz symlink ve izinler reddedilir. İndirme gerekiyorsa açık seçenek olur; offline bundle ile internet olmadan kurulur. İlk admini yalnızca root oluşturur; kurulum varsayılan parola üretip loga basmaz.

### 4.2 Root yapılandırma ve hesap yönetimi

```bash
sudo tealdrive-wizard config show --section server
sudo tealdrive-wizard config set server.transport direct_tls
sudo tealdrive-wizard config set server.require_tls true
sudo tealdrive-wizard config set tls.tls_cert_path /home/tealdrive/tls/certificate.pem
sudo tealdrive-wizard config set tls.tls_key_path /home/tealdrive/tls/private-key.pem
sudo tealdrive-wizard config validate
sudo tealdrive-wizard service restart

sudo tealdrive-wizard pool set --size 100GiB
sudo tealdrive-wizard admin create --name admin-a --kind virtual --budget 60GiB
# Web parolası / ilk parola kurulumu güvenli prompt; parola argümanı yok.

sudo tealdrive-wizard access allow --linux-user alice
sudo tealdrive-wizard access allow --linux-user sftp-account --allow-nologin
sudo tealdrive-wizard access deny --linux-user alice
sudo tealdrive-wizard registration set --self-registration false
sudo tealdrive-wizard account list
```

`config set` tek bir değeri şemaya göre atomik kaydeder; dinleme/TLS gibi birlikte değişmesi gereken alanlar için `config edit` ile bütün aday dosya düzenlenip doğrulanabilir. Restart gerektiren değişiklikler açıkça belirtilir; root doğrulama yapılmadan servisi bozuk ayarla yeniden başlatmaz. `show` gizli değerleri çıkarır. `access deny` geçerli oturum ve işleri iptal eder; sadece liste satırı silmez.

### 4.3 SSH kullanıcısının kendi erişimini yönetmesi

```bash
tealdrive-wizard init
# UID doğrulaması → uygunluk/root deny kontrolü → kayıt + etkin allowed_users.
# Auth moduna göre web parolası güvenli prompt'ta belirlenir.

tealdrive-wizard status
tealdrive-wizard passwd
tealdrive-wizard revoke

tealdrive-wizard policy list
tealdrive-wizard policy deny --path private --recursive --operations discover,read,write
tealdrive-wizard policy deny --path projects/app/config.toml --operations chmod
tealdrive-wizard policy remove --rule personal-rule-id
```

`init` admin yetkisi vermez, root/admin yasağını kaldırmaz. `passwd` kimlik ve mevcut kayıt durumunu doğrular; parola sıfırlama root yetkisinin ayrı işlemidir. İlgili UID'de bir SSH oturumunun parolası sızmışsa UID yetkisi de risk altındadır; CLI varlığı bunu ortadan kaldırmaz. Mevcut parola değiştirme ve kurtarma kanıtı geliştirme sırasında açık şemaya bağlanır.

`revoke` Linux hesabını silmez; web erişimi ve tüm web oturum/biletlerini kapatır. Root engeli yoksa sonradan `init` mümkün; yeni kayıt eski parolayı veya biletleri canlandırmaz. Kişisel policy remove yalnızca kişisel katmanı değiştirir.

### 4.4 Sanal adminin alt hesap yönetimi

```bash
tealdrive-wizard admin login --url https://drive.example.com
# Yönetim kimliği prompt; session dosyası yok veya seçilen kısa ömürlü 0600 cache.

tealdrive-wizard virtual create --name customer-one --quota 20GiB
tealdrive-wizard virtual list
tealdrive-wizard virtual quota --account acct_customer_one --size 15GiB
tealdrive-wizard virtual disable --account acct_customer_one
tealdrive-wizard virtual enable --account acct_customer_one
tealdrive-wizard admin logout
```

Sanal nologin admin SSH'tan kendi OS hesabıyla oturum açamaz; bu komutlar izinli bir terminal hesabından HTTPS yönetim girişiyle veya root tarafından kullanılabilir. Panel yönetimi de aynı yetki kontrollerine tabidir. CLI'nin başka UID'de çalışması sanal adminin yetkisini Linux yetkisine dönüştürmez. `virtual create` helper'ın root tarafından verilen oluşturma yetkisini, bütçeyi, hesap adını ve UID çakışmasını denetler; `--admin`/`--uid` gibi ayrıcalık seçimi sunmaz.

### 4.5 Log, servis, güncelleme ve kaldırma

```bash
tealdrive-wizard audit list --limit 50
tealdrive-wizard undo list --limit 20
tealdrive-wizard undo apply --operation op_example --expected-state state_example

sudo tealdrive-wizard service status
sudo tealdrive-wizard service stop
sudo tealdrive-wizard backup --output /root/backups/tealdrive-backup.tar
sudo tealdrive-wizard update --bundle ./tealdrive-release
sudo tealdrive-wizard rollback --release previous
sudo tealdrive-wizard uninstall
```

`audit/undo` sıradan UID'de kendi hesabıyla; root tüm hesapları seçebilir. Remote virtual session varsa hesap o oturumdan seçilir. Undo komutları ilgili feature uygulandıktan sonra kullanıma açılır. Backup varsayılan config/auth/policy ve yönetim kayıtlarını kapsar; kullanıcı verisi için ayrı açık kapsam seçilir. Parola özetleri ve helper anahtarı içeren yedek hassastır; root erişimi gerekir.

Uninstall varsayılan veri/logları korur; silme ayrı açık onay ve kapsam ister. Gerçek Linux hesapları/home'ları silinmez. Sadece kendi sağladığı sanal hesapları kayıttan doğrulayabilir. `rollback` binary rollback ile şema geri dönüşünü ayırır; auth/kayıt neslini eski hâle getirip iptal edilmiş kimliği canlandırmaz. Server/helper birlikte güncellenir.



### 15. TealDrive Wizard — çok büyük

- Üç rol: Root kurulum yönetimi; root tarafından atanmış sınırlı web yönetimi; sıradan UID'nin kendi hesabını yönetmesi.
- Kurulum/güncelleme/diagnose/backup/restore/uninstall; kullanıcı `init`, `revoke`, kişisel yasak yönetimi; sanal kullanıcı sağlama/kapama.
- Wizard binarysi kendisi setuid yapılmaz; gerekli yetkili işlerde dar helper yönetim protokolü veya root çalıştırma kullanılır. Sıradan kullanıcıya keyfi root işlem verilmez.
- Parolayı argv/env yerine TTY güvenli prompt'tan al; otomasyon için secret sızıntısız giriş tasarla.
- Uninstall önce servis/kayıtları kaldırır; veri ve log silme ayrı açık seçenek/onay. Mevcut gerçek Linux hesapları ve home'ları hiçbir uninstall yolunda silinmez.
- Kendi oluşturduğu sanal UID'leri ve dosyaları kayıtla takip et; başka hesap/dizin çakışmasında hata; anahtarlar güncellemede korunur.
- Kabul: Temiz kurulum, idempotence, eksik kurulumdan toparlanma, update/rollback, root olmayan `init/revoke`, yetki aşma negatif testleri, veri koruyan kaldırma.


## Suspended ve delete ayrımı

Disable/suspend giriş engelidir; veri silmez, aktif oturum/biletleri iptal eder. Admin sanal hesap delete çağrısı yalnızca root ayarı izinliyse kabul edilir. Deleted veri varsayılan 36 saat korunur; purge ayrı kontrol edilmiş lifecycle işidir. Süre dolunca root dışındaki gerçek Linux kullanıcı hesapları/home'ları temizleme hedefi olmaz. Root silme tavanı Wizard ile değişir; CLI bu kapıyı bypass etmez.
