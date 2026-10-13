# TealDrive STORAGE geliştirme sözleşmesi

> **Durum:** Protected SQLite/Argon2id, PAM enrollment/web parola, Root Wizard/ctl, home/politika, sanal UID/GID/native kota yönetimi uygulanmıştır. Bileşen bazında kesin kabul durumu PLAN.md ve TEST_RESULTS.md içindedir; users.d/audit/undo gibi hedef parçalar çalışan özellik sayılmaz.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Hibrit model

Kesin karar: Kota/hesap/home/UID/nesil metadata SQLite'ta; kullanıcı içeriği gerçek dosya sisteminde. DB bütün kullanıcı dosyalarının kapsayıcısı değildir. Root kontrollü SQLite WAL/journal/backup izinleri de korunur.

Kota kullanım kaynağı helper; [QUOTA.md](QUOTA.md). Kullanıcı loglarını kendi disk/kotasından tüketme notu [DECISIONS.md](DECISIONS.md) altında açık muhasebe kararıdır; merkezi root korumalı konumla çelişmesi gerekmez.


## 2. Dosya yapısı

```text
/home/tealdrive/                         root:root; 0755
├── bin/                                root:root; 0755
│   ├── tealdrive-server                 root:root; 0755
│   ├── tealdrive-helper                 root:root; 4755
│   └── tealdrive-wizard                 root:root; 0755 (setuid değil)
├── policy/                             root:tealdrive; 0750
│   ├── server.toml                     root:tealdrive; 0640
│   ├── helper.toml                     root:root; 0600
│   ├── helper.key                      root:root; 0600
│   └── users.d/                        root:root; 0700
│       └── <account-id>.toml           root:root; 0600
├── auth/                               root:root; 0700
│   └── accounts.db                     root:root; 0600
├── config/                             tealdrive:tealdrive; 0700
│   └── settings.toml                   tealdrive:tealdrive; 0600
├── data/                               tealdrive:tealdrive; 0700
│   └── runtime/                        servis çalışma verileri
├── logs/                               root:root; 0711
│   ├── server/                         tealdrive:tealdrive; 0700
│   ├── security/                       root:root; 0700
│   └── users/                          root:root; 0700
├── tls/                                root:tealdrive; 0750
│   ├── certificate.pem                 root:tealdrive; 0640
│   └── private-key.pem                 root:tealdrive; 0640
├── ui/                                 root:root; 0755
│   └── custom/                         root onaylı paket; servis yazamaz
├── cache/                              tealdrive:tealdrive; 0700
└── virtual-homes/                      root:root; 0711
    ├── <account-id-a>/                 ilgili sanal UID:GID; 0700
    └── <account-id-b>/                 ilgili sanal UID:GID; 0700
```

İzinler hedef ilkeleri gösterir; ACL ve servis kısıtları Wizard tarafından denetlenir. Root korumalı bütün ebeveynler yazmaya kapalıdır; symlink veya güvensiz sahiplik reddedilir. TLS anahtarını servis okumalıdır; helper imza anahtarını okuyamaz. Dört binary vardır: server, helper, wizard ve bağımsız ctl.

Gerçek kullanıcının home'u taşınmaz:

```text
/home/alice/                            mevcut Linux sahibi/izinleri
├── projects/
├── .env                                varsayılan erişilebilir
├── .ssh/                               varsayılan panel erişimi kapalı
└── .tealdrive/                          helper tarafından yönetilen iç alan
    ├── trash/
    ├── staging/
    └── operations/                     geri alma için doğrulanan metadata
```

İç alan normal list/search/download API'lerinden gizlidir. Trash/restore komutları helper üzerinden erişebilir. Linux kullanıcısı SSH'tan değiştirebilir; içerik ve metadata güvenilir sayılmaz. Root korumalı journal ve kullanıcı home'undaki veri birbirine doğrulama kaydıyla bağlanır. Sanal home'larda da aynı iç düzen kullanılır.

Systemd unit `/etc/systemd/system/tealdrive.service`, PAM kaydı `/etc/pam.d/tealdrive` altında kalır. İsteğe bağlı `/usr/local/bin/tealdrive-wizard` bağlantısı sadece komut kolaylığıdır; asıl binary merkezde kalır. `/home` mount'unda `nosuid` veya `noexec` varsa bu kurulum modeli çalışmaz; Wizard sessiz alternatif kullanmaz, açık hata verir.

### Depoların sorumlulukları

| Depo | İçerik | Yazma yetkisi |
|---|---|---|
| `server.toml` | Dinleme/TLS/proxy, auth modu, UI güven kaynağı | Root/Wizard |
| `helper.toml` | İşlem tavanı, öz kayıt uygunluğu, havuz/kimlik sınırları | Root/Wizard |
| `users.d/*.toml` | Katmanlı kullanıcı yasakları | Root veya yetkili helper yönetim komutu |
| `accounts.db` | Hesap ID/nesil, kayıt izinleri, admin rolleri, parola özetleri, kota tahsisleri, revoke durumu, sınırlı güvenlik sayaçları, geri alma journal'ı | Root/kimlik doğrulayan helper |
| `settings.toml` | Webden değişebilir çalışma tercihleri | Panel servisi; şema ve tavanla sınırlı |
| Kullanıcı home'u | Kullanıcı dosyaları, trash/staging | Kullanıcı UID'sine düşmüş helper |
| Güvenlik/kullanıcı logları | Helper'ın doğruladığı olaylar | Helper'ın sınırlı audit yazma yolu |
| Server logları | HTTP/WS/servis teşhisi | Panel servisi |

`accounts.db` örnek hedef SQLite deposudur; root sahibi olmak backup/WAL/journal dosyalarının izinlerini de kapsar. Kimlik deposu browser veya server'a ham olarak sunulmaz. Kullanıcı verisi için doğrudan dosya erişimi root'a geri dönmez; korumalı kayıtlar dar ayrı yönetim/audit adımıyla tutulur.



- `server.toml` ile `settings.toml` iki ayrı şema olacak; isimler uygulamada sonlandırılır. Güvenlik için kritik alan sınıflandırması belgelenecek.
- Örnek kritik alanlar: Helper/anahtar/politika yolları, dinleme/TLS/trusted proxy güven sınırı, kimlik doğrulama modu, öz kayıt uygunluk kuralları, Linux kimlik eşleşmeleri, root tavanı, sanal hesap sağlama yetkisi, custom UI güven kaynağı.
- Web ayarları bu sınırları daraltabilir; hiçbir şekilde genişletemez. Eski admin config yanıtları gizli anahtarları/depo içeriğini göstermeyecek.
- Home üst dizini root korumalı olsa da servis erişecek alt dizinlere gerekli traversal izni verilecek; sanal kullanıcı başka hesapların dizinlerine erişemeyecek.
- Systemd unit ve PAM kaydı sistem konumlarında kalır. Proxy dosyaları ayrı ürünlerin sorumluluğudur. İşletim sistemi kayıtlarında Linux hesabı oluşturmak sanal hesabın gereğidir.
- Log sahipliği tek bir üst dizin izniyle değil, yazan bileşenin yetkisine göre belirlenecek. Server log yazma yetkisi güvenlik tavanını değiştirme yetkisi vermez.


## Uygulanan hesap deposu

`accounts.db` root:root0600; ebeveynler root sahibi/yazmaya kapalı. Symlink, geniş DB/WAL/SHM/journal izinleri ve bilinmeyen schema sürümü reddedilir. Init mevcut dosyayı benimsemez veya overwrite etmez. WAL, synchronous FULL, foreign keys, strict tablolar, trusted_schema OFF; mutasyonlarda immediate transaction. Online SQLite backup committed WAL içeriğini de taşır, yeni root0600 hedefe yazılır; hedef üzerine yazılmaz.

Tablolar accounts, quota_pool, admin_budgets, security_state, enrollment, login_limits. Son iki tablo auth gateway bağlantısı için hazırdır, henüz canlı login davranışını değiştirmez. Kullanıcı dosyaları ve gerçek kullanım sayaçları SQLite'a yazılmaz. Public Account metadata parola/hash içermez. Username/UID tekil; gerçek OS eşleştirme gateway'de ayrıca zorunludur.

Root havuzdan admin bütçesi verir; admin yalnızca kendi sanal alt hesaplarına tahsis yapar. Linux hesap kotası burada tutulmaz. Root bir nologin UID'ye bağlı sanal admin yaratabilir; sanal adminin kendi dosya üst sınırı admin bütçesi eksi alt hesap rezervasyonlarıdır. Gerçek kullanım helper'dan ayrıca ölçülmelidir; bütçe azaltırken kullanımı da denetleme gateway işidir, DB allocation kontrolü kullanım ölçümü olarak sunulmaz. Deleted hesap tahsisi veri purge başarılı olana kadar korunur.

Parola PHC Argon2id v19, random16-byte salt, 19MiB/t2/p1 başlangıç maliyeti. En az12 Unicode karakter, en fazla1024byte. Verification PHC maliyetleri ve output/salt boyutları bounded; bilinmeyen hesap için dummy hash çalışır. Parola/root hash public JSON'a çıkmaz. Parola değişimi/suspend/revoke/delete generation artırır; yeni helper ticket denetimi bu generation'a bağlanacak. Sanal kullanıcı self-revoke yapamaz; logout ayrı oturum işlemidir.

Bu depo tek başına yeni auth modelinin tamamlandığı anlamına gelmez. Protected IPC, PAM enrollment ve bilet/aktif oturum iptali tamamlanmadan SDK enrollment/virtual backend capability açılmaz.

## Merkezi yol seçimi uygulandı

Standart yollar src/layout.rs içindedir. Helper root config/key çifti /home/tealdrive/policy altında; yalnızca primary config yoksa eski /etc çiftine geçilir. Corrupt/symlink/unreadable primary fallback değildir. Server default lookup merkezi server.toml'ı da içerir; helper binary default merkezi bin'i tercih eder. Bu yol seçimi mevcut servisi/dosyaları otomatik taşımaz. Wizard izinli migration/install ayrıca yapılacak.

## Schema2: kayıt kanıtı ve oturum ledger'i

Session grants accountID/generation + gerçek caller UID/parent PID/start-time/boot-ID ile bağlı, süreli ve iptal edilebilir. Root helper bu alanları kernel'den çözer; request JSON rol/PID iddiası seçemez. Logout tek grant, suspend/revoke/password-change account generation üzerinden tüm grant'leri geçersiz kılar. 64 hesap başına, 8192 toplam grant ve 1024 bekleyen enrollment temel hard sınırları; expired/revoked kayıtlar yeniden kullanılamaz ve kapasiteden temizlenir.

Enrollment raw token DB'de değil SHA256 özetiyle tutulur, max300s, tek kullanımlık ve caller/NSS snapshot'a bağlı. Linux parola veya Linux parola hash'i saklanmaz. Existing enrolled hesapta sessiz PAM reset yasak; kendi Linux self-revoke sonrası re-init mümkün, administrative deny/suspend/revoke bunu kapatır. Tamamlama sırasında OS kimliği ve root eligibility gateway'de yeniden çözülecek.

Schema1 startup tarafından otomatik yükseltilmez. Explicit root migration private online backup alır, transaction içinde schema2'ye geçer, pending eski proof'ları iptal eder ve account generation artırır. UID/home/role/password/budget korunur. Schema2 normal open sonrası gateway hazır olana kadar bu primitive'ler network auth capability açmaz.

## Root tarafından ayrı native kota diski kurulumu

```bash
sudo tealdrive-ctl stop
sudo tealdrive-wizard storage init 1GiB
sudo tealdrive-wizard storage status
# Root helper/server config'te virtual_accounts.enforcement = "user_quota" seçin.
# İlk komutun poolCapacityBytes çıktısını aşmayın; disk metadata/headroom tahsis edilmez.
sudo tealdrive-wizard pool set 800MiB
sudo tealdrive-wizard storage sync
```

`storage init` yalnız yeni, boş ve sanal kimlik/provision intent bulunmayan kurulumda çalışır. `/home/tealdrive/storage/virtual-pool.ext4` Root600, önceden posix_fallocate ile gerçek disk blokları ayrılmış ext4/usrquota imajıdır; minimum128MiB ve4096-byte hizalı. Image sparse overcommit yapılmaz. Host/root disk yeniden mount edilmez. Virtual homes ayrı loop filesystem olarak nodev/nosuid/noexec/usrquota bağlanır. FD tabanlı kernel quota sorgusu başarılı olmadan tamamlanmış sayılmaz. Boot mount unit ve tealdrive.service dependency drop-in Root tarafından yazılır; native disk yokken servis bağımlılığı başarılı başlamaz. Bu adım sanal hesabı veya provisioning yetkisini otomatik açmaz, Root pool/budget ataması ayrıdır.

`storage mount` yalnız doğrulanmış var olan image'ı bağlar; formatlamaz, nonempty mountpoint altındaki verileri örtmez. `storage status` gerçek mounted filesystem/kernelquota/capacity/free bilgisini verir. Root native pool set bu capacity sınırını uygular. Root tarafından önceden hazırlanmış ayrı quota filesystem de status/pool denetiminde kullanılabilir; onun boot mount yönetimi Root'a aittir. Root, image ve quota diskini diğer Root işlemleriyle doldurursa kullanıcı tahsisi disk kapasitesi garantisi değildir.

Format/mount sonrası başarısızlıkta image veri kaybını önlemek için otomatik silinmez; Root teşhis/onarım gerekir. Fallocate ilk adımının hatasında sadece yeni ve henüz formatlanmamış dosya temizlenir. Uninstall, veri/image ve mount dependency'lerini korur. Online Wizard metadata backup image/user contents backup'ı değildir; bu disk ayrıca offline/uygun snapshot yöntemiyle yedeklenmelidir.
