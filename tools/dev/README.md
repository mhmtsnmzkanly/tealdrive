# Geliştirme ve kabul araçları

Üretim kurulum araçları değildir; canlı servisi kurmaz veya Caddy'yi değiştirmez. Gerçek tarayıcı testlerinin kaynakları `tests/live/` altında, ortak bağımlılık ve çalıştırıcı bu dizindedir. Password dosyaları, oturum JSON'u, SQLite backup ve test çıktıları repoya konmaz.

```bash
npm ci --prefix tools/dev
# Chromium bu makinede yoksa:
npx --prefix tools/dev playwright install chromium

# Deterministic Rust / SDK / browser sözleşmesi testleri:
tools/dev/check.sh unit

# Yalnız ayrılmış test sunucusu ve kullanıcı ile:
export TEALDRIVE_TEST_URL=http://127.0.0.1:13101
export TEALDRIVE_TEST_USERNAME=tealdrive-test
export TEALDRIVE_TEST_WEB_PASSWORD_FILE=/root/tealdrive-candidate-web-password
tools/dev/check.sh live ui-auth

# İlk PAM kaydı da denenecekse özel Linux parola dosyasını ayrıca belirt:
export TEALDRIVE_TEST_PASSWORD_FILE=/root/tealdrive-test-password
tools/dev/check.sh live sdk-enrollment

# Credential generation değiştirir; parola değerini aynı tutar:
TEALDRIVE_ALLOW_CREDENTIAL_MUTATION=1 tools/dev/check.sh live ui-credentials

# Root-only DB suspend testidir; public admin API testi değildir:
TEALDRIVE_ALLOW_TEST_ACCOUNT_MUTATION=1 tools/dev/check.sh live sdk-suspend
```

| Betik | Amaç / etkisi |
|---|---|
| `ui-auth` | Gerçek gömülü UI login/logout; normal kullanıcıya yönetim metadata reddi |
| `ui-credentials` | CSRF/mevcut parola reddi, UI parola değiştirme, generation artışı; opt-in mutation |
| `sdk-enrollment` | PAM ilk kayıt veya mevcut web login; CRUD/revision/chmod/quota/transfer/logout; test dosyası üretir |
| `sdk-access` | Gizli yollar/alias/parent işlemleri; Root fixture oluşturup temizler |
| `sdk-suspend` | Root DB backup+suspend+finally restore; generation artar, özel backup bırakır |

Testleri aynı hesapta eşzamanlı çalıştırmayın: password/suspend bütün oturumları geçersiz kılar. Secret file modları private olmalıdır. Kullanıcı dosyalarında çalıştırmayın; scriptlerin açık mutation opt-in kontrolü vardır. `sdk-access` fixture koşulları ve `sdk-suspend` sabit candidate DB yolu kaynak açıklamalarında belirtilmiştir.

Root/OS tests opt-in olarak Cargo'da kalır: `accounts_store`, `runtime_settings`, `helper_ticket`, `chmod_owner` ve gateway PAM testi. Ortam değişkenleri/ownership/kernel koşullarını ilgili test kaynağından okuyun; genel runner bunları otomatik çalıştırmaz.

Geri üretilebilir bağımlılıklar package-lock ile pinlenmiştir; `node_modules/` ve `results/` Git dışında tutulur. Özel üretim rollback yedekleri burada tutulmaz.


## Public admin / gerçek OS hesap kabulü

```bash
cargo build --locked --bins --example admin_acceptance_fixture
# Kurulu /home/tealdrive/bin binary'leri test edilen kaynak sürümüyle eşleşmelidir.
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 tools/dev/check.sh admin
```

Root-only, explicit opt-in. Driver UUID adlarıyla üç geçici Linux UID/GID/home oluşturur, yalnız iki hesabı protected DB'de bootstrap eder; ayrı server127.0.0.1:13102 çalıştırır. Asıl server config/binary ve mevcut kullanıcı metadata'sını değiştirmez. Primary helper policy'ye test admin/roots geçici eklenir ve öz kayıt kapatılır; byte-identical eski dosya finally'de döner. Aynı helper'ı kullanan adayın yeni enrollment talepleri test boyunca bu geçici politika ile değerlendirilir. Canlı geçişten sonra Caddy aynı helper kullanır. Driver home-installed server çalışıyorsa politika fixture'ını reddeder; offline/ayrı ortamda çalıştırın.

Chromium public SDK→HTTP→setuid helper yolunu kullanır; admin suspend/enable, canlı socket kapanışı, yeniden login reddi, eski cookie'nin geri açılmaması, gerçek kullanıcı UID'siyle dosya erişimi, CSRF/role/kind sınırları doğrulanır. Driver durdurduğu process yalnız kendi fixture server'ıdır; cleanup yalnız kendi UUID usernames/UIDs/DB rows/home'larını siler. Bütün DB'yi eski backup'a döndürmez. Dışarıdan helper policy değişirse onun üzerine yazmaz; root-private original dosyasını bırakarak hata verir. SIGKILL/power-loss için finally garantisi yoktur; private manifest/backup ile root onarımı gerekir.

Fixture bootstrap example bir Wizard veya public admin rol verme API'si değildir. Root önceden admin rolünü atar; browser rol/UID/path seçmez. Python/Node scriptleri credential'ları komut argümanı veya stdout'a koymaz.


```bash
# İlk onay + gerçek PAM + suspend edilmiş pending proof kabulü:
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=approval tools/dev/check.sh admin
```

Approval phase Root deny'ı yalnız test operator'ı geçici uygulayıp geri alır; browser'ın Root dosyasına yazabildiği bir uç değildir. Fixture server runtime overlay kullanır (Root config byte-preservation); var olan hesap approve ile reset edilmez. Negatif auth senaryolarından sonra test yeni transport açar; server'ın5 deneme limiti gevşetilmez.


```bash
# Gerçek gömülü UI / PAM / rol / cancel / mobil / başarısız refresh kabulü:
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Her phase sonunda aktör/hedef audit kaydı ve parola loglanmaması kontrol edilir. Driver kendi fixture log/config/runtime çıktılarını siler; üstteki sabit root-private lock dosyası reusable test koordinasyonu için kalır. Virtual gruba geçiş testi virtual OS hesap provisioning/tenant UID izolasyonu testi değildir. Root Wizard account approve/admin komutları eklendi; bu browser fixture test root-only bootstrap örneğini kullanır.


Otomatik home kabulü aynı izole fixture ile çalışır (güncel helper/server binary ve example gerekir):

```sh
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Root template static roots'u değiştirir; geçici hesapların home'u NSS'den gelir. Existing production Root policy restore edilir. `.env` açık/.ssh doğrudan kapalı, public HTTP download ve cross-home reddi ayrıca doğrulanır. Native config geçici olduğu için mevcut adayda eşzamanlı yeni enrollment çalıştırmayın.

Protected policy public SDK/UI/actual UID kabulü:

```sh
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_POLICY_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Temporary policy rows also cleaned; never changes a permanent user's policy/role/password.

Virtual/real kernel quota acceptance adds `TEALDRIVE_VIRTUAL_ACCEPTANCE=1` and optionally `TEALDRIVE_NATIVE_QUOTA_ACCEPTANCE=1` to home/admin UI acceptance. Native mounts a private temporary128MiB ext4+usrquota image on empty /home/tealdrive/virtual-homes; never remounts host root FS. Requires Root mount/quota support and current compiled fixture. UID/GID/sysdata/pool/cursor rollback refuses externally changed identities/data.


Wizard clean-install kabulü (Root; yalnız private mount namespace):

```sh
cargo build --locked --bins
TEALDRIVE_WIZARD_ACCEPTANCE=1 tools/dev/check.sh wizard
# Gerçek geçici UID/PAM/TTY ve yönetim reddi:
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_HOME_ACCEPTANCE=1 TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Local Wizard testi admin fixture'ın yalnız ikinci geçici hesabını self-revoke/PAM re-enroll eder; UID/rolü korur. TTY ECHO kapalı olduğu gönderim öncesi kontrol edilir; parola terminal çıktısında bulunmamalıdır. Installed Wizard/helper/server güncel olmalıdır. Test çıktısı secret terminal buffer'ını hata halinde de yazdırmaz. Namespace systemctl stub'ı gerçek servis kabulü değildir.


Root-created virtual admin + native lifecycle/crash-resume kabulünü eklemek için aynı virtual/native koşuya `TEALDRIVE_ROOT_VIRTUAL_ACCEPTANCE=1 TEALDRIVE_LIFECYCLE_ACCEPTANCE=1` ekleyin. Root test yeni admin/child üretip kendi finally bloğunda temizler; retained deleted child timestamp'i yalnız fixture DB satırında değişir. Global gc testinden önce unrelated eligible account olmadığını doğrular; varsa test çalışmaz. Test sonu Rootpool/cursor/retired fixture UID markers geri alınır; üretim UID fence'leri korunur.


Direct TLS ve production'dan ayrı Caddy proxy kabulü: admin/home fixture'a `TEALDRIVE_TLS_ACCEPTANCE=1` ekleyin.13103/13104/13105 loopback portları gerekir. Kendi selfsigned cert/key, Caddy adminoff/XDGprivate dirs; mevcut Caddyfile okunmaz/değişmez. Bu test production certificate güven zinciri testi değildir; HTTPS/WSS/cookie/helper/shutdown/invalidkey/header sınırları testidir.


Custom UI acceptance: admin/home fixture'a `TEALDRIVE_UI_PACKAGE_ACCEPTANCE=1` ekleyin. Root-owned uniquepackage ve server13106; global theme activation yapılmaz. CLIcheck, sameSDKauthfiles, assetallowlist, incompatible/override fallback ve `/ui/default` browser testi.

`sudo tools/dev/check.sh proxy-admin`: Caddy-only read-only service UID/TCP+Unix isolation and Root control probe; no config/credentials output. Setup [CADDY_ADMIN.md](../../docs/deployment/CADDY_ADMIN.md). Caddy olmayan kurulumlarda gerekli değildir.

## Canlı sürümün son kabulü

```sh
export TEALDRIVE_TEST_URL=https://tdrive.novastrum.dev
export TEALDRIVE_TEST_USERNAME=tealdrive-test
export TEALDRIVE_TEST_WEB_PASSWORD_FILE=/root/tealdrive-candidate-web-password
# Gerçek public Cloudflare yolu:
tools/dev/check.sh live ui-auth
# Public DNS'i atlayıp aynı güvenilir sertifika/domain ile doğrudan origin:
export TEALDRIVE_TEST_ORIGIN_HOST=tdrive.novastrum.dev
tools/dev/check.sh live ui-auth
# Root-only: tek UUID dosyası, CRUD/conflict/undo/audit, gerçek service restart/popup:
TEALDRIVE_PRODUCTION_ACCEPTANCE=1 tools/dev/check.sh live production-files
```

Son test gerçek tealdrive.service'i restart eder; yalnız ayrılmış hesabı kullanın. Dosya fixture'ı gerçek sahibi UID ile finally'de kaldırılır; protected audit facts silinmez. TLS sertifikası doğrulaması kapatılmaz. Native/full-policy fixture kurulumdan önce boş özel alanda çalıştırılmıştır; artık canlı helper politikasını değiştirmek için production server önce durmalıdır. Wizard namespace testi `/usr/local/bin` dahil ayrı bind mount kullanır.

Operasyon yardımcıları ayrı [tools/ops](../ops/README.md) altındadır.

`TEALDRIVE_BACKEND_TLS_ACCEPTANCE=1 python3 tools/dev/backend-tls-check.py`: production değiştirmeden13107/13108 üzerinde private Caddy/fake same-name certificate; yanlış backend502 ve0HTTP teslimi. Dedicated production cert public dosyası gerekir.

## Test artıklarını temizleme

```sh
# Root-only dry-run; known /tmp UUID fixtures, default age>=1h:
sudo python3 tools/dev/clean-artifacts.py
# Test süreçlerini bitirdikten sonra apply:
sudo python3 tools/dev/clean-artifacts.py --apply
# Repo build/cache ayrı; önce installed/rollback binarylerini doğrulayın:
cargo clean
```

Cleaner runtime UID staging, open FD/cwd/exe paths, mounted trees ve arbitrary /tmp names üzerinde çalışmaz. Source/test scriptlerini veya operational backups'ı silmez. Güncel sonuç [CLEANUP.md](CLEANUP.md) içindedir. `node_modules` küçük ve browser kabulü için kullanışlı olduğu için korundu; package-lock üzerinden yeniden kurulabilir.

Core transfer kabulü: `TEALDRIVE_CORE_TRANSFER_ACCEPTANCE=1 TEALDRIVE_TEST_WEB_PASSWORD_FILE=/root/private-test-password python3 tools/dev/core-transfer-acceptance.py`. Root, unshare/mount, mevcut onaylı test hesabı, private web parola dosyası ve güncel target/debug dört binary gerekir. Production auth/home/config/binaries değiştirilmez; private namespace/cloned DB ve ephemeral fixture kullanılır.
