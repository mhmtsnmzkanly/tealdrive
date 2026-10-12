# Web kimliği ve hesap erişim yönetimi aşama raporu

Bu rapor önceki AUTH_INTEGRATION_REPORT.md üzerine eklenen işleri kapsar. **Bütün geliştirme planının bitiş raporu değildir.** Güncel kalan işler PLAN.md'de tutulur; Caddy üzerinden çalışan mevcut üretim sürümü korunmuştur.

## Tamamlanan değişiklikler

1. Web parola değiştirme: mevcut parola root helper'da doğrulanır; Argon2id hash güncellenir, account generation artırılır. Eski bütün grant'ler geçersizdir. Cookie temizlenir, SDK state temizlenir ve bağlantı kapanır. Linux parolası değişmez.
2. Linux kullanıcısının kendi web erişimini iptali: OS hesabı/dosyaları korunur. Sanal kullanıcı reddedilir. Root kayıt politikası uygunsa PAM ile yeniden kayıt mümkündür; administrative deny/suspend üstün gelir.
3. UI Account security formu: mevcut/yeni/tekrar parola, hata mesajları, Linux hesapta erişim kapatma onayı. Parola alanları gönderim ve iptalde temizlenir.
4. Root-backed yönetim: kayıtlı Linux/virtual hesap metadata listesi, mevcut hesabın enable/suspend akışları. NSS aday listesi ayrı SDK method olarak korunur. Linux global admin için DB role+root roster birlikte; virtual admin yalnız kendi child hesapları üzerinde yetkilidir. Endpoint account kind'i helper'da denetlenir; role/UID/path client tarafından seçilemez. Admin hesaplarına ve kendi admin hesabına müdahale ilk sürümde kapalıdır.
5. Effective quota: admin budget ve child toplamı tek SQLite snapshot'ında okunur; concurrent yönetim işlemi farklı snapshot sonuçlarını birleştiremez. Root-only DB2 kabul testi geçti.
6. SDK bağımsız auth capability'leri, metadata tipleri ve mutation sonrası session temizliği. Create/delete/setQuota tamamlanmadığı için sahte capability açılmadı.

## Test kanıtı

- Tam Cargo paketi: **544 unit passed, 1 explicit PAM ignored**, normal integration testleri geçti.
- Explicit root/PAM test passed: enrollment, web login, başka süreç ticket sınırları için önceki kanıt; bu aşamada yanlış mevcut parola reddi, generation değişimi, eski grant reddi, öz revoke/yeniden enrollment ve normal user yönetim reddi genişletildi. Memory fixture DB kullanır; production DB değildir.
- SDK contract passed: capability yokluğu ve invalid parola, başarılı password/revoke sonrası state ve bağlantı temizliği.
- Gerçek Chromium localhost aday: CSRF403, yanlış parola401, UI parola değişimi/logout/login, generation artışı; parola değeri korunur. UI login/logout ve user management metadata403 ayrıca kontrol edilir.
- Clippy all-targets -D warnings, fmt, diff ve MSRV Rust1.85 kontrolleri geçti. TypeScript strict tip kontrolü geçti.
- Yönetim yetki türetme unit testi: root roster gereksinimi, virtual admin global Root yükseltme reddi, user role reddi.

**Kabul sınırı:** Başarılı public admin suspend/enable → gerçek diğer OS hesabı testi henüz yapılmadı. Önceki native root suspend testi public admin endpoint testi değildir. Virtual provisioning, OS quota ve iki gerçek virtual UID izolasyonu henüz tamamlanmadı.

## Commitler

- `43ea6d5` — protected web parola değiştirme ve Linux self-revoke; HTTP/SDK/UI/test/docs.
- `46ce0d8` — helper'da protected account listesi ve erişim transitions; SDK/types/test/docs.
- `eab54fd` — quota snapshot race düzeltmesi.
- Geliştirme araçlarının düzenlenmesi ve temizlik kayıtları ayrı commit ile kaydedilir.

## Mevcut yayın ve kalanlar

Aday yeni binary'ler `/home/tealdrive/bin/`, backend `127.0.0.1:13101`. Caddy'deki eski servis `127.0.0.1:13000`; kullanıcı dosyaları taşınmadı. Yeni adayın root'u gerçek NSS home olan `/home/tealdrive-test`; eski `/srv/tealdrive-test` kökü üretimde korunur.

İlk admin onayı/provisioning, policy management ve automatic home templates; virtual UID/GID lifecycle/native quotas; Wizard; tam SDK/UI/i18n; TLS/custom UI; user audit/undo; persistent limiter/retention GC; final install/migration acceptance **kaldı**.

## Geliştirme araçları

Güncel gerçek tarayıcı testleri `tests/live/`, ortak çalıştırıcı ve pinli Node bağımlılıkları `tools/dev/`. Eski tek kullanımlık root sandbox scriptleri ve eski UI snapshot/çıktıları bunların kapsamıyla karşılaştırılarak temizlendi (13 dosya/dizin girdisi, yaklaşık39.4MB). Özel production rollback yedekleri, parola dosyaları, canlı config/binary/DB ve kullanıcı dosyaları temizlik dışında tutulur. Ayrıntı `tools/dev/CLEANUP.md`.
