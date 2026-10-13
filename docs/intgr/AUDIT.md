# TealDrive AUDIT geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Kaynak ve sınırlar

Mevcut `src/audit/` tek sink; hedef server/security/user akışlarıdır. Kullanıcı yalnızca kendi güvenli/sayfalı geçmişini; admin atanmış kapsamı görebilir. Raw kullanıcı parametresi log path'i seçmez.

Chmod gerçek eski/yeni mode helper'dan gelir. Log plaintext parola, cookie, CSRF, dosya içeriği taşımaz. Root audit yazımı UID düşmüş dosya motorunun tekrar root olması demek değildir; ayrı dar doğrulanmış audit adımı tasarlanır.

Kritik yönetim audit intent yazılamazsa başlamaz; işlem sonrası audit hatası yapılmış işi geri almış/hiç yapılmamış gibi göstermez. Journal/undo ve audit farklı veri türleridir. Log kota notu [DECISIONS.md](DECISIONS.md).


### 5. Audit eski/yeni değerleri ve kayıt ayrımı — orta/büyük

- Chmod: Helper'da işlemden önce/sonra gerçek `old_mode`/`new_mode`; dosya kimliği, aktör, istek ve sonuç.
- Diğer değişiklikler: Kaynak/hedef path, politika/ayar için güvenli alan farkları, dosya sürüm/hash bilgisi; dosya içerikleri/parolalar kaydedilmez.
- Genel server logu, hesap ID'si bazlı kullanıcı işlemleri ve güvenlik logu; login failure/rate limit/kayıt/sıfırlama güvenlik akışına girer.
- Başarısız denemede doğrulanmamış username'i dosya adına dönüştürme; hesap logu sayısını, boyutunu, rotation ve retention'ı sınırla.
- Tek olay ID'siyle hedefler arasında ilişki kur; bazı hedeflere yazılamama ve disk dolması davranışını açıkça tanımla.
- Audit metadata geri alma için referanstır; içerik yedeği veya otomatik güvenli restore değildir. Sonradan değişen dosyaya eski chmod körlemesine uygulanmaz.
- Kritik helper olaylarının server'dan bağımsız kayıt yolu tasarlanır. Hesap sağlama, parola sıfırlama ve yetki/politika değişiminde audit zorunludur; kayda alınamıyorsa işlem başlatılmaz. İşlem başladıktan sonraki kayıt/işlem arızaları için intent/result ve toparlanma akışı gerekir; dosya işlemlerinin audit zorunluluğu yapılandırılabilir.
- Panelde kullanıcı yalnızca kendi işlem geçmişini sayfalı/filtreli görüntüler. Helper imzalı kimlikten hesap ID'sini doğrular; istekte başka username/UID verilerek log okunamaz. Görüntüleme raw log dosya indirmesi değildir; gizli alanlar ve yasaklı path bilgileri filtrelenir.
- Kabul: Eski/yeni mode gerçekten uygulanmış değer; başarısız işlemler başarı gibi görünmez; secrets yok; diğer kullanıcı loglarına erişim yok.



## 8. Ek iş: Beklenen duruma bağlı geri alma — büyük

- Audit ile geri alma kaydı farklıdır: Audit olay kanıtını, işlem journal'ı ise hesap ID, kaynak/hedef, dosya kimliği, eski/yeni mode, hash ve gerekli saklanan veriyi tutar. Kullanıcı bazında ilişkilendirilir.
- İlk kapsam silme/trash, taşıma, yeniden adlandırma ve chmod. Dosya içerik sürümleri/snapshot ayrı sonraki özellik olarak değerlendirilir.
- Server uygun geri alma adaylarını gösterir; helper hem görüntüleme yetkisini hem uygulama anındaki güncel politikayı doğrular. Politika değişimi veya revoke sonrası eski yetki kullanılamaz.
- Helper hash ve gerekli kimlik/metadata koşullarını işlem içinde denetler; hedef doluysa üzerine yazmaz, koşul uyuşmuyorsa kararlı hata kodu döner.
- Hash tek başına dış müdahale olmadığını kanıtlamaz. Kabul edilen garanti beklenen mevcut durumun eşleşmesidir; harici süreç yarışlarına mutlak garanti verilmez.
- Silinmiş içerik hash ile yeniden üretilemez; geri alma için veri trash'te tutulmalıdır. Saklanan içerik yok/bozuk veya süre dolmuşsa aday uygulanamaz olarak gösterilir.
- İç kayıtlar genel dosya API'sinden gizlenir; kullanıcının OS erişimiyle değiştirebilmesi dikkate alınır. İmzalı metadata olsa da erişilebilir veri güvenilir varsayılmaz.
- Sanal hesapta trash/staging kotaya dahildir. İşlem journal'ı ve log büyüklüğü ayrıca sınırlanır.
- Geri alma yeni bir audit olayıdır; eski kaydı silmez/değiştirmez. Çökme/yarım işlem toparlanması gerekir.
- Bağımlılık: Kullanıcı politikası, kimlik nesli, audit farkları ve editör revision altyapısından sonra. Ana işleri geciktirirse ayrı özellik aşamasına alınabilir.
- Kabul: Dış değişiklik, eksik veri, hash uyuşmazlığı, hedef çakışması, politika daraltma, başka kullanıcı geçmişi, aynı işlemi tekrar geri alma ve paralel restore testleri.


## Saat bazlı saklama, log kotası ve undo — son karar

- Genel audit/trash varsayılan retention 504 saat (21 gün); yeni config alanı retention_hours. Eski retention_days alanı migration'da saate çevrilir, bilinmeyen birime sessiz yorum yok.
- Kendi-görüntülenebilir sanal işlem loglarının gerçek byte kullanımı helper ölçümüne/hesap kotasına dahildir; loglar merkezi root korumalı yerde kalabilir. Güvenlik/server akışları ayrı sistem bütçesi. Gerçek Linux OS kotası root-owned logu kapsamaz; bu hesapların merkezi logları ayrı bütçede kalır.
- Log yazarken kota/disk dolması davranışı bounded reserve/rotation/retention ile planlanır; kritik audit kaydı gerekli olduğunda yazılamıyorsa işlem reddedilir. Kota doldu diye güvenlik olayı sessizce kaybolmaz veya kullanıcıya sınırsız log büyütme hakkı verilmez.
- Undo ilk kapsamı taşıma, yeniden adlandırma, trash'e silme ve chmod. Kullanıcı kendi undo retention_hours değerini seçer: min 1, max server upper; server upper varsayılanı 24 saat.
- Undo süresi genel 504 saat log/trash saklaması değildir. Örnek: Audit olayı hâlâ görünürken 24 saat dolmuş işlem undo yapılamaz.
- Helper veri/sürüm/hash ve current policy'yi uygulama anında tekrar doğrular; gizli path geçmişten veya undo adayından sızmaz.
- Silinen hesap verisi ayrı root-controlled 36 saat default retention; suspended veri silme tetiklemez. Süre sonrası purge kayıt/kimlik ve devam eden işlerle uyumlu yapılır.

## 24 saat açıklaması — kullanıcı düzeltmesi

24 saat kullanıcı tercihi varsayılanı değil, server geri alma üst sınırının varsayılanıdır. Kullanıcı 1..server-max saat seçer; server-max root/admin yetki tavanı içinde yapılandırılır.

## Uygulanan protected journal temeli (2026-10-13)

Root supervisor before/after operasyon kayıtları accounts.db içindedir; versioned operation_v1 alanları accountId/generation/requestId/action/rootId/sourcePath/targetPath/requestedMode/status/bytes/errorCode/time. `pending`, `success`, `failed`, `unknown` ayrılır; pending başarı değildir. Varsayılan henüz kapalı `[journal] enabled=false`, retention_hours504, undo_max_hours24, max_entries_per_account2048, max_record_bytes8192. Bu bir geliştirme aşamasıdır: kullanıcıya kendi history API/UI, gerçek eski/yeni metadata, sanal log kota muhasebesi, GC ve undo bağlantısı tamamlanmadan capability açılmıyor. Track edilenler mutasyonlar + readText/DownloadFile; HTTP range/chunk aggregate server audit kapsamı ayrıca mevcut. Bounded journal dolduğunda mutasyon başlamadan static structured error döner. Sonuç kaydı hatası gerçekleşmiş işlemi hiç yapılmamış gibi sunmaz. Dosya içerikleri/credentials protected journal'da da tutulmaz.

## Gözlenen metadata

Chmod kaydı requestedMode yanında actual oldMode/newMode içerir; aynı pinned inode fstat'ı kullanılır. before/after ObservedFileState (optional, eski kayıtta null) device/inode decimal string, kind/mode/uid/gid/size/nlink/time, opaque digest ve mevcutsa content/tree digest taşır. Chmod contents'i değiştirmediğinden permission undo condition metadata/identity'dir; geçici chmod ile sır okumak veya mode000 dosyanın içeriğini açmak yok. Move/rename/trash hash'leri User worker read permission ve current policy altında bounded64MiB/10000node/depth64. Incomplete fingerprint tam veri gibi değerlendirilmez.

## Çalışan quota ledger ve history capability

Own history API/SDK artık Root `[journal] enabled=true` ve web-account modelinde authentication.audit_history=true ilan eder. Direct backend/native/application/own-scope/policy testleri geçti. Old configs defaultfalse kalır; undo API/UI devam ediyor. `logReserveBytes` ve `logStorage` userQuota/systemBudget/systemReserve ayrımını gösterir. Padded ledger copy/rootDB primary intent-result aynı event ID'dir; plaintext file contents yok. Root account/global count cap ve kontrollü rotation/504h GC uygulanır. Linux logs sistem bütçesinde, virtual events gerçek UID kotasında. Delete-after36h log UIDRoot'a aktarılır, Root policy kalan retention'a göre temizler.
