# Admin erişimi, ilk kayıt onayı ve yönetim UI raporu

2026-10-12. Kullanıcının istediği üç iş tamamlandı. Bu, bütün PLAN.md'nin bitiş raporu değildir.

## Tamamlanan üç iş

| İş | Sonuç | Commit |
|---|---|---|
| Admin'in gerçek başka Linux hesabını askıya alma/açma kabulü | Public SDK/HTTP→setuid helper, ayrı gerçek UID/GID/home; açık socket kapanışı, login reddi, eski cookie reddi, yeni giriş/dosya erişimi geçti | `a4bee61` |
| Kayıt öncesi Linux kullanıcı onayı | Yalnız mevcut uygun NSS kimliği, password'süz Linux/User metadata, Root deny üstün, kullanıcı gerçek PAM ve ayrı web password ile tamamlar | `a166d83` |
| Yönetim UI | Accounts modalı, system/virtual grupları, aday dropdown, approve/suspend/enable, pending/active/suspended durumu; gerçek Chromium/PAM kabulü geçti | `3aec4f0` |

## Davranış ve yetki

- Global Linux admin için protected DB Admin rolü + Root admin roster gerekir. Root'un kritik dosyalarına browser yazmaz. İlk admin ataması Root'un işi; production Wizard rol atama hâlâ ayrı geliştirme maddesidir.
- `API.admin.systemUsers.list()` uygun yerel adayları; `accounts()` kayıtlı Linux metadata'sını getirir. `approve({username})` yeni OS hesabı oluşturmaz; helper UID/GID/home'u NSS'den alır, role=User sabittir. UI kullanıcıya UID/home/role/parola atama alanı sunmaz.
- Onay için `authentication.linuxApproval`; mevcut erişim yönetimi için `accountManagement` capability gerekir. Eski handshake'de eksik capability'ler false döner.
- Öz kayıt açıksa uygun kullanıcı onaysız kaydolabilir. Kapalıysa Root whitelist veya admin onayıyla ilk PAM kaydı açılır. Root deny, sistem/servis/UID/shell kontrolleri üstün kalır. Bilinen servis adları yüksek UID/interactive shell ile de onaylanamaz.
- Mevcut hesabı tekrar approve etmek409; suspend/revoke/deleted durumları sessiz reset edilmez. Enable/suspend ayrı accountId işlemleridir. UID alias/aynı isim yarışları409; altyapı hatası503.
- Suspend generation artırır; grant'ler ve bekleyen PAM proof'ları geçersiz olur. Enable eski cookie/grant/proof'u yeniden geçerli kılmaz. Dosyalar ve OS hesabı korunur. Root-managed admin satırlarına bu UI/API müdahale etmez.
- Virtual admin global Linux yönetimi yapamaz; helper yalnız kendi virtual children'ına yetki verir. UI virtual grubu sunar; provisioning/delete/quota araçları açılmadı.

## UI ve SDK

Hesap ekranı ayrı embedded same-origin ES module `src/ui/static/accounts.js`; bütün hesap çağrıları gerçek SDK üzerinden yapılır. Kullanıcı adı/home metinleri DOM textContent ile yazılır. Cancel confirmation backend'e mutation göndermez. Busy sırasında double-submit/tab değiştirme engellenir. Failed refresh eski kontrolleri temizler. Close sonraki gecikmiş cevapların render'ını engeller; kabul edilmiş backend işlemini geri alma vaat etmez.

Klavye Tab containment/Escape/focus return ve mobil390px tablo düzeni doğrulandı. İlk sayfa bağlantı göstergesindeki yanlış connected sınıfı düzeltildi; gerçek ProtocolReady öncesinde **Not connected** gösterilir. Test kullanıcı kaydı tamamlanmadan yeşil bağlantı göstergesini başarı saymaz.

SDK AccountMetadata/UserCandidate/UserCandidates ve approve parametre/hata sözleşmesi güncellendi; sdk-api.js gerçek çağrı örnekleriyle eşlendi. Full file-manager SDK migration ve TR/EN ayrı PLAN işi olarak kalır.

## Test kanıtı

- Normal Cargo: **544 unit passed,1 explicit PAM ignored**, normal integration testleri geçti. Opt-in root/kernel testleri bu sayıya dahil değildir.
- Clippy all-targets -D warnings, fmt, diff, JS syntax/Prettier checks geçti.
- Rust **1.85.0** all-target check ve strict TypeScript d.ts geçti.
- Access phase: public admin suspend/enable, real UID helper read/list; active socket<=5s kapanış, suspended login reddi, eski cookie401, yeni generation login, normal user/CSRF/kind/self-admin403.
- Approval phase: öz kayıt kapalı unapproved PAM reddi; Root deny global admin'den üstün; unknown role/UID/home JSON422, servis isimleri403, tekrar approve409; web login PAM öncesi reddi; suspend sırasında ve yeniden enable sonrasında eski PAM proof reddi; yeni gerçek PAM + web password enrollment geçti.
- UI phase: gerçek gömülü ekrandan suspend/enable/approval, cancel→generation değişmedi, pending→PAM→active, user management button hidden, system/virtual tab, mobile/keyboard/focus, 503 intercepted refresh→stale kontrollerin temizlenmesi→retry geçti. 503 testi browser intercept'tir; gerçek helper outage kabulü sayılmaz.
- Audit: AccountApproved/AccountSuspended/AccountEnabled aktör ve hedef account ID/username içeriyor. Linux/web parola string'leri loglarda bulunmadı. Public API helper Root config'ini değiştirmedi.
- Final cleanup: **0 fixture OS user,0 fixture DB account row**; fixture grants/proofs/home/config/log/runtime çıktıları kaldırıldı; helper config byte-identical geri döndü. Mevcut account metadata'sına/bütün DB backup restore'una müdahale edilmedi.

Tekrarlanabilir araçlar:

```bash
cargo build --locked --bins --example admin_acceptance_fixture
# Kurulu candidate binary'lerin source ile eşleşmesi gerekir; test runner install/deploy yapmaz.
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 tools/dev/check.sh admin
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=approval tools/dev/check.sh admin
TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE=1 TEALDRIVE_ADMIN_ACCEPTANCE_PHASE=ui tools/dev/check.sh admin
```

Root-only explicit fixture: üç UUID Linux hesap, ayrı server13102 ve geçici helper policy. SIGKILL/power-loss için finally garantisi yok; root-private manifest/original backup ile onarım gerekir. Dış Root policy değişirse driver üzerine yazmaz. Ayrıntılar tools/dev/README.md.

## Mevcut yayın ve sınırlar

Aday sunucu **127.0.0.1:13101** üzerinde son binary'lerle yeniden başlatıldı; normal test kullanıcısının UI auth smoke testi yapıldı. Caddy'deki **tdrive.novastrum.dev eski yayın** korunuyor. Mevcut kullanıcıya sessiz admin rolü verilmedi; native test adminleri sonunda kaldırıldı. `/srv/tealdrive-test/Web test.md` ve mevcut kullanıcı verileri taşınmadı.

Onay tek başına otomatik home profile üretmez. File roots hâlâ Root config'inde gerçek NSS home ile eşleşmelidir. Yeni virtual OS provisioning, native virtual iki-UID izolasyonu/OS quota, Root Wizard/ilk admin atama, policy management ve automatic home templates; TLS/custom UI; user audit/undo/retention GC; full migration/final install kabulü PLAN.md'de bekler.

**Sıradaki iş:** gerçek NSS home'a bağlı otomatik kullanıcı profilleri ve protected kullanıcıya özel erişim politikaları; Root tavanı ve yasakların birleşimi korunarak uygulanacak.
