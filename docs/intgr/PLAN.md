# TealDrive uygulama ve kabul planı

[Dizin](README.md) · [Karar günlüğü](IMPLEMENTATION_DECISIONS.md) · [Test kanıtları](TEST_RESULTS.md)

Bu dosya güncel uygulama durumudur. Bileşen belgelerindeki “hedef şema” parçaları tamamlanmış özellik sayılmaz. Her bağımsız değişiklik ayrı commit ve uygun testle tamamlanır. Canlı Caddy servisi kabul/migration bitene kadar eski kurulumda korunur.

## Tamamlanan temel işler

| İş | Durum / kanıt |
|---|---|
| UI kaynaklarını dosyalara ayırma, aktif uygulama ve chmod uyarıları, parola alanı temizliği | Tamamlandı; deterministic + gerçek Chromium |
| CSP dış script kullanımı, auth/IPC parola tamponlarını zeroize | Tamamlandı |
| WS-first handshake/login ve tek kullanımlık HttpOnly cookie bridge | Tamamlandı; gerçek TCP/Chromium/PAM |
| Helper list extension/filter/sort/pagination | Tamamlandı; filter pagination öncesi |
| Content+metadata revision, çakışan edit reddi, mode/owner/ACL korunumu | Tamamlandı; unsaved editor input korunur |
| FD tabanlı helper quota ölçümü ve yüzde uyarısı | Mevcut config-root düzeninde tamamlandı; DB tahsis bağlantısı bekliyor |
| SDK auth/files/admin çekirdeği, tipler, state, storage, observers, progress/abort ve HTTP hata sözleşmesi | Tamamlandı; bazı gelecekteki backend capability'leri kapalı |
| Root config + sınırlı service-writable settings overlay, atomik persist/reload | Tamamlandı; root dosyası byte-identical kabul testi |
| Ortak access_policy, katman yasakları, gizli öğelerde NotFound, recursive/alias/trash kontrolleri | Tamamlandı; gerçek setuid/browser negatif test |
| SQLite metadata/Argon2id/kota tahsis/nesil/backup temeli | Tamamlandı; root account gateway eklendi, server/WS/SDK/UI auth bağlantısı tamamlandı; yönetim uçları bekliyor |
| Persistent nonce/enrollment ve process-bound PAM ticket | Tamamlandı; root migration ve gerçek setuid replay reddi test edildi |
| /home/tealdrive yol seçimi, güvenli legacy fallback | Tamamlandı; fiziksel kurulum/migration bekliyor |
| Audit retention_hours=504 ve eski retention_days uyumu | Tamamlandı; trash/undo/account GC bundan ayrıdır |

## Şimdiki geliştirme sırası

1. **Protected kimlik ve yönetim gateway'i.** Auth komutları/helper NSS kontrolleri uygulandı; server/WS/SDK/UI enrollment ve operation grant bağlantısı tamamlandı. Self-revoke/password change helper→HTTP→SDK→UI tamamlandı; kayıtlı hesap listesi ve erişim açma/askıya alma uçları eklendi. Kayıt öncesi Linux admin onayı ve sistem/sanal erişim yönetimi UI tamamlandı; public admin→gerçek OS hesap/PAM/Chromium kabulü geçti. Virtual provisioning/silme/kota/politika yönetimi ve Root Wizard rol atama kaldı. DB hesabı + NSS UID/GID/home doğrulaması; root rolü/deny/kayıt tavanları; PAM veya PAM sonrası web-password modeli. DB'yi server açmaz. SDK yeni capability'leri yalnızca gerçek backend tamamlanınca açar.
2. **Bilet ve oturum yaşam döngüsü.** Hesap ID/generation ve aktif session nonce; logout nonce iptali, self-revoke tüm web erişimini iptal eder. Sanal self-revoke yasak. Suspend/delete aktif oturum ve helper lease'lerine uygulanır. Root-owned rolü server claim'i genişletemez. Eski ticket/session uyumu explicit migration davranışıyla çözülür.
3. **Enrollment ve öz kayıt.** İlk PAM kanıtı tek kullanımlık, kısa ömürlü ve caller/kimlik bağlamına bağlı. Linux parolası kalıcı tutulmaz. Mevcut web-parolalı hesap Linux parolasıyla sessiz reset edilemez. Wizard gerçek UID'nin kendi kaydını açar; deny/suspend üstün gelir.
4. **Home kökleri ve hesap politikası yönetimi.** Başlandı: ortak identity-derived home çözümleyicisi/helper legacy daraltması ve dört negatif/kota testi eklendi. Template config, HTTP/WS bağlantısı ve policy yönetimi henüz tamamlanmadı. Gerçek home helper/NSS/DB'den; kullanıcı path/UID seçmez. Root/admin/kişisel kurallar protected gateway'den değiştirilir. Config-root'tan home moduna geçiş izinleri sessiz genişletmez.
5. **Sanal OS kimlikleri ve gerçek quota enforcement.** Ayrı nologin UID/GID, locked OS password; root pool→admin budget→child allocation. Kullanım helper ölçümü; eşzamanlı rezervasyon ve budget azaltma için kullanım denetimi. Linux OS kotası ayrı.
6. **Wizard.** Ayrı binary; install/update/repair/uninstall, init/revoke, account/policy/quota, service start/stop/restart/reload, check/doctor/backup. Root-required ile own-user komutları ayrılır. Temiz kurulum, migration, nosuid/noexec ve rollback kabulü.
7. **SDK tamamlama ve UI entegrasyonu.** Yeni auth/admin/policy/history/undo uçlarının gerçek testleri; bütün methods/options/results/events tipleri ve okunabilir cookbook/sdk-api.js. Gömülü UI SDK kullansın; auth/file-manager modülleri, TR/EN i18n ve koşullu uyarılar.
8. **TLS/custom UI.** Caddy zorunlu değil; direct TLS cert/key + require_tls, proxy TLS ve açık yerel HTTP. Güvenli reload/restart sınırları. Root-approved manifest/paket, same-origin assets, SDK override yok ve embedded fallback.
9. **Audit/journal/undo.** Eski/yeni mode ve doğrulanmış işlem verisi, kullanıcı kendi audit geçmişi, merkezi güvenlik logu. Move/rename/trash-delete/chmod için beklenen duruma bağlı undo. Kullanıcı log quota muhasebesi ve bounded cleanup.
10. **Kalıcı güvenlik sayaçları ve yaşam döngüsü.** Hesap/IP/global login limitleri; genel504 saat, deleted virtual36 saat, undo user1..server-max (server-max default24). Admin delete root izni default kapalı. Dosya retention sonunda silinir, suspend silmez.
11. **Son kabul ve canlı geçiş.** Tam test/Clippy/fmt/release/MSRV, kernel/root/PAM/SDK/browser/TLS/proxy/iki hesap izolasyonu; temiz install/update/repair/restore/offline, mevcut dosyaları koruyan migration. Belgeler/commitler/detaylı bitiş raporu.

## Kesin kararlar

- Sanal hesap başına ayrı Linux UID/GID; yalnız web, OS parola locked/nologin.
- Root kritik tavan; admin çalışma ayarı; kullanıcı yalnız kendi tercihi/kısıtı. Yasakların birleşimi genişletilemez.
- Öz kayıt varsayılan açık, yalnız uygun hesap ve kendisi; root/admin deny üstün.
- `.ssh` ve sistem sır alanları panelden görünmez/erişilemez. `.env` yeni home profilinde açık; legacy sensitive ayarı migration'da açıkça dönüştürülür.
- Kullanıcı dosyası SQLite'a konmaz. Kota allocation DB'de, kullanım helper'da.
- SDK isteğe bağlı managed state; kullanıcı/origin namespace browser storage; secret/content yok; path restore tekrar doğrulanır. Helper kalıcı UI-state tutmaz.
- Ek application encryption ana özelliklerden sonra; TLS yerine özel JS kripto yapılmaz.
- Kendi kripto/iletişim algoritması icat edilmez; mevcut standart implementasyonlar kullanılır.
- 24 saat kullanıcıya dayatılmış sabit tercih değildir; server undo üst sınırının varsayılanıdır.
- Sanal kullanıcı operation log byte'ları application kotasına dahil; Linux merkezi logları sistem bütçesinde. Quota uyarısı default80, yapılandırılabilir.
- Root bütçeyi kullanım/tahsis altına sessiz düşürmez; ölçüm ve açık yeniden tahsis gerekir.

## Güvenlik kabul kapıları

- Home erişimi ortak erişim politikası/NSS/generation doğrulaması tamamlanmadan açılmaz.
- DB/keys/roles root kontrolünde; web veya runtime dosyası Root authority iddia edemez.
- Yeni enroll/virtual/undo capability tamamlanmadan FEATURE_DISABLED kalır; sahte başarı yok.
- Root veya kullanıcı verisi taşıma/silme test fixture'ı ile gerçek kullanıcı verisi karıştırılmaz.
- Kernel confinement veya zorunlu TLS kurulamazsa geniş izinli fallback yapılmaz.
- Path politikası dış Linux sahibinin SSH ile rename'ini veya tüm dış writer yarışlarını engelleyen inode etiketi olarak sunulmaz.

MFA, SSH kapatma, kullanıcı uygulamalarının süreç yönetimi ve çalışan proje tespiti bu kapsamda değildir. Admin hesap sayısı, UID aralığı ve operasyon üst sınırlarında gerekli teknik seçimler IMPLEMENTATION_DECISIONS.md'ye gerekçesiyle kaydedilir.
