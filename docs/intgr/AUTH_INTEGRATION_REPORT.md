# Kimlik akışı entegrasyonu — 2026-10-12

Bu aşamada yeni hesap akışı helper'dan server, WebSocket, SDK ve gömülü UI'ya bağlandı. Bütün geliştirme planı bitmiş değildir; kalan işler aşağıdadır.

## Tamamlanan davranış

- Uygun Linux kullanıcısı aynı WS'de PAM doğrulamasıyla kaydı başlatır; 300 saniye içinde web parolası oluşturur. Root deny ve NSS bilgisi completion'da tekrar kontrol edilir. Proof browser'a verilmez.
- Kayıt sonrası yalnız bağımsız Argon2id web credential doğrulanır; aktif hesabın parolası PAM ile sessiz reset edilemez. Linux parola/hash/fingerprint kalıcı tutulmaz. Web parolasının Linux parolasından farklı seçilmesi önerilir; farklılık ayrıca zorlanmaz.
- Server session private helper grant'ını taşır; public response yalnız account ID/generation/kind/role içerir. Browser storage account/origin/generation namespace kullanır; credential/grant/content saklamaz.
- WS, indirme parçaları ve quota işçisi session'a ait izni kullanır. Helper caller UID/PID/start-time/boot + persistent nonce/generation + fresh NSS doğrular; işlem gerçek UID/GID'ye düşülüp Landlock altında yapılır.
- Web mode root doğrudan NSS home ile eşleşir. Home dışındaki eski root veya başka hesabın alias'ı sunulmaz. Otomatik home root template yönetimi henüz yapılmadı.
- Logout DB nonce'ını iptal eder; suspend/revoke/generation değişimi yeni işleri reddeder, aktif lease periyodik kontrolle kapanır. Normal kontrol1s; işlem/scheduling latency eklenir. İptalden önce tamamlanmış işlem geri alınmaz.
- Global panel yönetimi root roster + DB Linux admin rolünü birlikte gerektirir. Sanal tenant admin global ayar yöneticisi değildir.
- Virtual authenticated/validated view DB effective quota verir; admin budget-child allocation, helper/config tavanıyla minimum. Fiziksel virtual provisioning ve OS/project quota kabulü henüz yapılmadı.
- UI auth SDK kullanır, password mode/capability'yi gösterir, first-registration/confirmation sunar, input temizler; logout başarısızlığını başarı saymaz. File-manager UI hâlâ mevcut transport'ta; tam SDK dönüşümü/i18n bekler.
- Bodyless204 cookie bridge yanlış interrupted audit sayılmaz; gerçekten yarım download ayrımı korunur.

## Commitler

| Commit | Değişiklik |
|---|---|
| df6cd34 | Server/session/WS/helper grant ve enrollment bağlantısı |
| acfe0fe | SDK enrollment, capability/types ve auth UI |
| 626324f | Exact home guard ve bodyless audit |
| f6ba83f | Virtual quota ceiling + matching home metadata |
| 6b4d584 | Installed setuid helper ile grant replay acceptance |
| 0f68671 | Gerçek SQLite suspend ile aktif WS iptali acceptance |

## Test kanıtı

543 unit passed; normal browser/SDK/WS/CLI integration tests passed. Opt-in PAM unit önce explicit passed. Root SQLite ownership/concurrency/backup/migration2, split config1, installed-helper same-UID process replay1 passed. Gerçek Chromium/PAM/SQLite/setuid zincirinde first-enrollment adımları, ayrı web login/Linux-password rejection, CRUD/revision, chmod000 recovery, streaming download, quota, logout/resumed socket denial passed. Embedded UI login/role/input/logout passed. Gerçek DB suspend+generation testi aktif WS'yi kapattı; finally account active olarak geri alındı. Path policy/hardlink/direct download/parent move tekrar passed. Clippy, fmt/diff, strict TypeScript ve Rust1.85 geçti.

Ayrıntılı komut ve hangi testin fixture/native olduğu [TEST_RESULTS.md](TEST_RESULTS.md) içindedir. Virtual UID provisioning, native iki-account home template, TLS direct/deployment kabulü bu sonuçların kapsamında değildir.

## Sunucudaki durum

Aday servis loopback127.0.0.1:13101, `/home/tealdrive/bin`, root-owned `policy/`, `auth/accounts.db`, service-writable `config/` ve `logs/server/` kullanır. Root split config ile çalışır; web account mode açıktır. Linux test kullanıcısının gerçek home'u `/home/tealdrive-test` üzerinden test edilir. Servis hesabının passwd home'u henüz taşınmadı. Bootstrap example, tam Wizard değildir.

Caddy `https://tdrive.novastrum.dev` hâlâ önceki üretim binary/config/root düzenini kullanır; HTTP200 doğrulandı. `/srv/tealdrive-test/Web test.md`60byte korunur. Linux şifresi ve eski kullanıcı verisi değişmedi. Candidate web parolası root-private test dosyasındadır; rapora/loga yazılmadı. Native suspend testinin private backup'ı auth dizininde tutulur.

## Kalan ana işler

[PLAN.md](PLAN.md) sırası geçerlidir: self-revoke/password change ve yönetim gateway uçları; hesap başına otomatik home/policy; virtual UID/GID provisioning ve gerçek quota izolasyonu; Wizard/install/update/repair/service; tüm files UI SDK+i18n; direct TLS/custom UI; kendi audit/journal/undo; persistent auth counters/retention GC; tam temiz kurulum ve güvenli canlı migration.

Bu aşamanın auth bağlantısı tamamlandı. Projenin tamamı yeni mimariyle kurulum/kullanım hazır diye işaretlenmedi; Caddy yeni sürüme geçirilmedi.
