# TealDrive AUTH geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

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
