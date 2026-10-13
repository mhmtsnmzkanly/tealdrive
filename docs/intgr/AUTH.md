# TealDrive AUTH geliştirme sözleşmesi

> **Durum:** Çalışan uygulama ve kabul sonuçları PLAN.md/TEST_RESULTS.md içindedir. Aşağıdaki gereksinim notları ve eski hedef şema örnekleri, çalışan API/config referansının yerine geçmez; güncel referans CONFIGURATION.md, SDK.md ve WIZARD.md dosyalarıdır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Mevcut kaynak ve kabul sınırları

`src/auth/account.rs`, `pam.rs`, `api.rs`, `src/helper/security.rs`, `src/rate_limit/login.rs`, `src/session/`, `src/admin/users.rs`.

PAM/enrollment/web parola ayrı yetki türleridir; enrollment dosya oturumu vermez. Root/admin allow + doğrulanmış self-registration etkin allowed_users oluşturur, deny üstün gelir. Wizard gerçek UID'yle yalnızca kendini ekler, admin olamaz. Sanal hesap first-password akışı Linux PAM değildir.

Account lock/silme/expiry/NSS değişimi ve desteklenmeyen PAM etkileşimleri test edilir; UID/name tek başına eski yetki devralamaz. SSH CLI kurtarma kanıtı tanımlanmalı; Linux parolasını bilenin webden otomatik reset hakkı yoktur. Sanal bütçe/roller [QUOTA.md](QUOTA.md).


### 6. Kalıcı giriş deneme sınırları — orta

- Kullanıcı+IP, hesap ve genel hız sınırı; kalıcı/sınırlı kayıt, cleanup ve eşzamanlı istek kontrolü.
- Argon2/PAM iş yüküne başlamadan uygun sınırlar; sahte username/IP ile disk/bellek büyümesini engelle.
- Kabul: Farklı IP ve restart ile sınır aşılmaz; hesabı kilitleyerek hizmet engelleme etkisi değerlendirilir.



### 13. Seçilebilir kimlik doğrulama ve öz kayıt — çok büyük

- Root config `pam` veya `pam_enroll_then_web` modu seçer; UI yalnızca keşfedilen akışı gösterir. Harici kimlik sağlayıcı ilk sürümün zorunluluğu değildir.
- İlk kayıt PAM: Kısa ömürlü tek kullanımlık enrollment yetkisi; farklı web parolası; root korumalı Argon2id kayıt deposu; kayıt yarışı/yarım işlem koruması.
- Sonraki web girişlerinde hesap varlığı/nesli/izinler ve PAM account kuralları kontrol edilir; Linux parolası tekrar saklanmaz veya istenmez.
- Sanal hesaplar PAM girişine ihtiyaç duymadan yönetici/Wizard tarafından hazırlanır, ilk parola kurulum yetkisi sınırlıdır.
- Wizard `init`: Root ayarı açıksa helper gerçek çağıran UID ve uygunluk tavanını doğrular; kullanıcı yalnızca kendi hesabını etkin `allowed_users` listesine kaydeder. Root konfigürasyonu doğrudan düzenlenmez; helper root korumalı kayıt deposunu atomik günceller. Etkin izin listesi root/admin onayları ve öz kayıtları içerir; açık root/admin engeli her zaman önceliklidir.
- Wizard `revoke`: Kullanıcının kendi web erişimini ve oturum/biletlerini kapatır; Linux hesabını silmez. Hesap engeli ile kendi revoke durumunun yeniden kayıt davranışı ayrı tanımlanır.
- Mod değiştirme/downgrade, parola değişimi/sıfırlama ve admin devri için mevcut oturumların iptali; yalnızca Linux parolasıyla kayıtlı web parolasını sıfırlama yasağı.
- Kabul: Başkasını kaydetme/admin olma yok; uygunsuz sistem hesapları yok; root öz kayıt kapatınca kayıt yok; UID tekrar kullanımı eski yetkiyi devralmaz.


## 2026-10-12 revoke sınırı

`auth.revokeSelf` ve yerel Wizard self-revoke yalnızca gerçek Linux kullanıcılarına aittir. Sanal kullanıcıların kendi hesap erişimini revoke hakkı yok; yetkili admin disable/revoke yapar. Normal logout sanal kullanıcıda da mümkündür. Capability ile gösterim ve server/helper doğrulaması birlikte yapılır; client'ın kind alanına güvenilmez. Önceki genel revoke ifadeleri bu hesap türü ayrımıyla okunur.

## Uygulanan root account gateway (2026-10-12)

`helper::account_gateway` root kontrol düzlemidir. `HelperInvocation::Account` ve `HelperProcessClient::account_command` üzerinden Login, BeginEnrollment, CompleteEnrollment, Validate ve Logout işler. DB yolu sabit `/home/tealdrive/auth/accounts.db`; istek UID/GID/rol/path belirlemez. Caller gerçek helper UID'si + parent süreç kaydıdır. Komutlar user content açmaz; DB bağlantısı komut sonunda kapanır.

Aşağıdaki alan yalnız root config'tedir; runtime settings'e eklenemez:

```toml
[web_accounts]
enabled = false # Eski kurulum varsayılanı; protected layout + DB + doğru home root hazırlanırsa true.
server_uid = 995 # Örnektir; kurulumdaki gerçek tealdrive UID'sini Wizard belirleyecek.
self_registration = true
denied_users = ["root", "www-data", "caddy"]
virtual_min_uid = 200000
virtual_max_uid = 299999
```

Linux hesabının UID/GID/home ve shell'i NSS'den alınır; kayıt sırasında root/service/system/nologin ve virtual UID aralığı reddedilir. Self-registration root allowed_users listesini değiştirmez; yalnız doğrulanmış kişinin DB kaydını oluşturur. Root deny, DB administrative deny ve suspended/deleted durumları üstündür. Self-registration kapatılması önceden onaylı hesapları silmez. Kayıt tamamlanırken root policy ve NSS tekrar okunur. PAM sadece Linux ilk kaydında; aktif web parolası PAM ile sessizce sıfırlanamaz.

Sanal hesabın logical adı Linux adıyla aynı olmak zorunda değildir; DB UID üzerinden NSS çözülür, DB UID/GID/home eşleşir, root virtual UID aralığı ve nologin shell gerekir. Sanal UID sağlama/OS password lock işi Wizard/gateway yönetim aşamasında tamamlanacak.

**Durum:** Helper + server/WS/SDK/UI enrollment ve session operation-grant bağlantısı uygulanmıştır (aşağıdaki sınırlar geçerlidir). `enabled=true` legacy helper PAM route/ticket'ını reddeder; server web gateway'e gider, sessiz PAM fallback yok. Mevcut Caddy konfigürasyonu değiştirilmedi.

### Server/WS/SDK bağlantısı tamamlanan kısım

Web mode şimdi `login_handler` üzerinden root gateway'i kullanır. SDK `auth.beginEnrollment({username,linuxPassword})` / `auth.completeEnrollment({webPassword})` aynı WS'de çalışır; enrollment ID kabul etmez/vermez. Begin session açmaz, completion açar. `handshake().authentication` giriş parolasının türünü ve enrollment desteğini belirtir. UI kayıt seçeneğini capability'ye göre gösterir, Linux doğrulamasından sonra yeni web parolası + confirmation ister. Eski PAM mode action'sız login çalışmaya devam eder.

Oturum callback/HTTP cookie bind/resume/GET session ve admin endpointleri root-backed identity'yi doğrular. Helper dosya komutları kendi session grant'ını kullanır; logout nonce'ı iptal eder, suspend/revoke/password-generation değişimi yeni işleri reddeder, aktif lease'ler doğrulama döngüsünde kapanır. Public session account_id/account_generation/account_kind/role içerir, grant içermez.

**Sınır:** Bu stage yeni kayıt/giriş/çıkış ve file grant bağlantısıdır. Self-revoke/password change web endpointleri, virtual provisioning/management, hesap başına otomatik home root template, Wizard ve canlı migration hâlâ PLAN'dadır. Global panel admin için DB Linux admin role + root admin_users gereklidir. Sanal admin global config yöneticisi değildir.

Aday kurulum `/home/tealdrive` protected layout, root split config ve SQLite ile127.0.0.1:13101'de çalışır. Linux test kullanıcısının gerçek home'u `/home/tealdrive-test`; eski canlı file root `/srv/tealdrive-test` korunur. Tealdrive servis hesabının passwd home'u canlı migration'a kadar eski `/var/lib/tealdrive-test` değerinde kalır. `examples/init_accounts.rs` explicit root bootstrap örneğidir; Wizard değildir ve mevcut DB overwrite etmez.


## İlk kayıt öncesi admin onayı

Root'un hem admin roster'ında hem DB'de Linux/Admin rolünde bulunan kullanıcı `API.admin.systemUsers.approve({username: "example-user"})` çağırabilir. Root kullanıcıyı servis/sistem/deny politikasıyla reddederse admin bunu aşamaz. Başarılı onay yalnız var olan Linux kimliğine password'süz User kaydı açar; kullanıcı PAM ve ayrı web password enrollment'ını kendisi yapar. Root allowed_users config'i değişmez. `self_registration=true` ise uygun kullanıcılar onaysız öz kayıt yapabilir; false olduğunda mevcut Root whitelist veya admin onayı gerekir. Mevcut hesapta409 alınır; enable/suspend farklı işlemlerdir. Home profile yoksa kayıt/giriş yapılabilir ama files roots boş kalır; Root home profili sonraki geliştirme aşamasıdır.
