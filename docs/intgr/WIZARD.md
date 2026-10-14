# TealDrive Wizard ve servis kontrolü

> **Durum:** Çalışan CLI sözleşmesi. İzole kurulum/restore ve gerçek systemd/TLS kabulü geçti; eski hedef komutlar history arşivindedir.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Uygulanan komutlar (13 Ekim 2026)

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

`gc` yalnız deleted **sanal User** hesaplarını, varsayılan36 saatten sonra en fazla100'lük batch'te temizler. Linux hesabı/home'u veya suspended hesap silinmez; sanal adminin otomatik silinmesi bu yüzeyde yoktur. Root directory reservation ve global provisioning lock altında yeni worker başlatır. Worker bütün UID/GID'leri düşürüp ek grupları temizler, Landlock ile tek home'a kapanır, sonra içeriği temizler. Symlink hedefi takip edilmez; nested mount reddedilir. Root yalnız boş generated home ve doğrulanmış tdv_UID hesabı/grubunu kaldırır. Purge intent kesinti sonrası tekrar ilerler, UID fence tekrar kullanımını engeller; retained DB tombstone quota0/passwordNULL olur. Her worker30 saniye sınırında; timer hourly, Root, oneshot ve explicit enable'dır. Aynı gc komutu korumalı trash/staging tokenlarını ve operation log retention'ını da işler; undo süresi ise apply sırasında doğrulanır. Ayrıntılar LIFECYCLE.md içindedir.


## Kurulum kolaylığı ve depolama

Wizard install/update/repair dört `/usr/local/bin/tealdrive-*` symlinkini Root sahipliği ve tam hedef kontrolüyle kurar. Unrelated executable/link üzerine yazmaz; uninstall yalnız kendi linklerini kaldırır. İlk install komutu bundle içindeki Wizard'ın tam yoluyla çağrılır. Servis hesabı nologin; mevcut yanlış service home'u açık migration ile düzeltilir.

```sh
sudo tealdrive-ctl stop
sudo tealdrive-wizard storage init 1GiB
sudo tealdrive-wizard storage status
# Root pool gerçek capacity altında; native volume kurulmadan enforcement açmayın.
sudo tealdrive-wizard pool set 512MiB
sudo tealdrive-wizard audit list 50
sudo tealdrive-wizard diagnose
```

`audit list 1..200` Root-only kontrol/yönetim geçmişidir. Kullanıcı dosya geçmişi ve undo CLI komutu yoktur; public SDK/UI üzerinden sunulur. `config set/edit`, `undo list` gibi eski taslak komutlar mevcut değildir. Parola komutları güvenli TTY kullanır; parola argv/env'ye verilmez. Root'un ilk admin rolü vermesi zorunludur, varsayılan admin yaratılmaz.

Native mount ünitesi ve GC timer systemd altında kalır. Install timerı otomatik açmaz; diagnose/recovery kabulünden sonra operator enable eder. Bu sunucuda mount/servis/timer enabled ve gerçek restart/shutdown popup kabulü geçti.

## Yeni aşama: TLS tanılama (source sürümü)

```sh
sudo tealdrive-wizard tls status
```

Root-only, read-only: PEM çifti ve güvenli izinler doğrulanır; public certificate fingerprint/tarihler ve30gün yenileme uyarısı gösterilir. Yeni server sürümü60s aralıkla valid TLS çiftini reload eder; yanlış çift/mode/symlink mevcut çalışan sertifikayı değiştirmez. Bu kod henüz canlı kurulumda yayınlanmadı. Proxy trust yenileme ayrıca gereklidir; TRANSPORT_SECURITY.md içindeki sırayı uygulayın.

## Yeni aşama: yönetici kapsamı (source sürümü)

```sh
# Mevcut RoleAdmin hesabının yönetim kapsamı; yalnız Root:
sudo tealdrive-wizard account scope ACCOUNT_ID global
sudo tealdrive-wizard account scope ACCOUNT_ID tenant
```

Hesap türü/home/UID/GID değişmez. Tenant rolü korunur; general scope ayrıca Root DB ve policy roster ile atanır. Eski oturumlar generation değişimiyle iptal edilir. `account admin/user` Linux rol komutlarıdır; virtual RoleAdmin'i RoleUser'a çevirmek için kullanılmaz. `virtual admin create tealdrive BUDGET REQUEST_UUID` Root-only panel alias'ını ayrı tdv_UID kimliğiyle oluşturabilir; Linux tealdrive service enrollment kapalı kalır. Bu yeni komutlar henüz installed production binary'de yayınlanmadı.

## Yeni aşama: net imaj havuzu ve büyütme (source sürümü)

```sh
sudo tealdrive-wizard storage init --pool-size 30GiB
sudo tealdrive-wizard storage status
sudo tealdrive-wizard storage check
sudo tealdrive-ctl stop
sudo tealdrive-wizard storage expand --pool-size 50GiB
sudo tealdrive-ctl start
```

--pool-size net dağıtılabilir toplamı ifade eder,artış miktarı değildir. Physical image daha büyük hazırlanır ve mevcut host blokları baştan ayrılır. Init explicit native virtual mode açar; mevcut image/mount/home ezilmez. Grow eski dosya ve User UID kotalarını korur. Root-owned expansion.json yarıda kalan aynı target'ı sürdürür. Disk/resize/validation hatasında eski pool korunur; retry aynı komutla yapılır. Shrink yok. Başka uygulamaların aynı FS'ye yazması online ext4 büyütmesinde kernel tarafından yönetilir; operasyonun bağımsız backup/snapshot olduğu iddia edilmez. Bu komutlar source aşamasındadır,production image henüz değiştirilmedi.


### Sabit admin kişisel kotası

```sh
# Root tarafından verilen toplam bütçe (kişisel + alt hesap tahsisleri)
tealdrive-wizard budget set ADMIN_ID 10GiB
# Bu bütçenin 1 GiB kısmını adminin kendi dosyalarına ayır
tealdrive-wizard budget own set ADMIN_ID 1GiB
# Commit edilmiş kernel limitlerini metadata ile eşitle
tealdrive-wizard storage sync
```

Komut yalnız aktif sanal RoleAdmin kabul eder. Kullanım altına küçültme, fazla tahsis, Linux servis hesabına uygulama reddedilir. Kişisel kotanın değişmesi mevcut oturumları geçersiz kılar. Çocuk kotası verilmesi bu sabit kotayı küçültmez. Henüz explicit geçiş yapılmamış eski adminler remainder modunda kalır.
