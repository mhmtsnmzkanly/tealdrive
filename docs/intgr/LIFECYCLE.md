# Korumalı yaşam döngüsü

Root helper config:

```toml
[lifecycle]
trash_cleanup_enabled = true
staging_cleanup_enabled = true
trash_retention_hours = 504
staging_retention_hours = 24
max_cleanup_per_run = 100
```

Trash ve staging temizliği uygulanmıştır. Ayarlar Root kritiktir, server PolicySnapshot bu değerleri değiştiremez. Legacy trash_retention_days alanı yeni protected cleanup otoritesi değildir; yeni kurulum saat alanlarını kullanır.

```sh
sudo tealdrive-wizard gc
# virtual_purged / operation_logs_expired / trash_cleaned / trash_skipped / staging_cleaned / staging_skipped
```

Root global görevi User dosyalarını Root olarak silmez. Root-owned aynı Wizard executable yeni worker açar; worker Root DB/config'ten kendi accountId/operationId kaydını doğrular, NSS/home/current policy ve nested mount kontrolü yapar, DB'yi kapatır, UID/GID kalıcı düşürür ve Landlock uygular. Parent death signal ve30s child deadline; trash işi için60s run bound ve max_cleanup_per_run. Suspend otomatik veri silme tetiklemez; inactive hesap trash worker'a verilmez. Deleted virtual child purge36h ayrı süreçtir.

Trash UUID artık Root-generated operation UUID'dir. Root SQLite trash_expiry_v1 kuyruğu intent öncesi bounded reservation, actual after-state ve result taşır. Kullanıcı writable info/deleted_at/path authority değildir. Audit rotation/TTL cleanup bu temizlik otoritesini silmez; ayrı token own/global journal count sınırlarına tabidir. Restore/permanent delete veya başarılı undo token'ı emekli eder. Purged virtual account token'ları da temizlenir.

Unchanged eski trash içeriği User UID altında doğrulanır. File inode/metadata/content state, directory bounded tree state eşleşir; manifest en çok10000node/depth64. Özel/symlink/nested mount, değişmiş durum, farklı owner veya current policy reddinde veri korunur. Regular büyük/unreadable dosyada içerik okunmadan pinned metadata koşulu kullanılabilir; directory incomplete tree temizlenmez. Yeni veya değiştirilmiş child metadata tekrar denetlenir, dolu directory rmdir ile korunur; dış writer'larla mutlak transaction garantisi yoktur.

Başarısız/changed job token'ı needsReview olarak korunur; her saat aynı değişmiş veriye tekrar silme denemesi yapılmaz. Root incelemesi veya yetkili kullanıcı kendi trash permanent-delete işlemi gerekir. Eski audit genel TTL sonunda silinebilir; needsReview token yalnız korumalı temizlik metadata'sıdır, içerik değildir. Missing target cleanup info'yu emekli eder; başarı sayısı dosya byte silindi iddiası değil tamamlanan trash temizliği sayısıdır.

Eski User msgpack tarihini okuyup doğrudan purge eden yardımcı yalnız legacy unit fixture'ında derlenir; production çağrı yüzeyi değildir. GC eski/missing Root token için kullanıcı metadata'sını kullanarak geniş izinli fallback yapmaz.

## Yarım kalmış geçici çıktılar

Growing işlemler `.tdrv-transaction-<Root operation UUID>` yolu için staging_expiry_v1 token'ını FS başlamadan ayırır. Başarılı publish token'ı kaldırır; failed/unknown/pending kayıt korunur. Audit rotation token'ı kaldırmaz. Worker24h cutoff sonrası exact Root slot, logical destination/current policy, owner/device/mtime/ctime ve bounded manifest doğrular. Sonradan değişmiş veya erişilemeyen artifact needsReview ile korunur. Legacy bilinmeyen temp dosyaları glob ile silinmez.

Private `__staging-expiry-worker` yalnız real/effective UID0 Root CLI içindir; production GC kendi current epoch'unu geçirir. Explicit acceptance fixture gelecekteki as-of değerini kendi sentetik dosyaları için kullanır; host clock değişmez. Runtime systemd PrivateTmp, server geçici upload namespace'ini ayrı yönetir; bu bölüm kullanıcı home içindeki unpublished transaction çıktıları içindir.
