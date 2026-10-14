# Kota ve ölçüm

[Dizin](README.md) · [Depolama](STORAGE.md)

Sanal model Root havuzu → sanal admin bütçesi → adminin kendi dosya tahsisi ve çocuk tahsisleridir. SQLite tahsisi tutar; gerçek kullanım helper/kernelden gelir. Pending provisioning rezervasyonu ve eşzamanlı tahsis SQLite transaction ile sayılır. Tahsis/kullanım altına bütçe düşürme reddedilir. Tenant sanal admin başka tenant veya sistem hesabı yönetmez; Root global scope verdiğinde genel panel yönetimi ayrı yetkidir.

`virtual_accounts.enforcement` application veya user_quota seçer. Application modu cooperating helper yazmalarını kilit/bütçe kontrolüyle sınırlar; dış süreçler için sert sınır değildir. User_quota ayrı ext4/usrquota volume ve her UID için kernel limitidir; yanlış/eksik kernel limitinde dosya işlemi fail-closed olur. Mantıksal dosya byte'ları ve kernel allocated byte'ları aynı ölçüm değildir.

Sanal audit ledger gerçek UID'ye ait, Root-only parent altında0400 dosyadır; normal kaydın rezervasyonu varsayılan8192bayt ve kullanıcı kotasına dahildir. Root DB authoritative metadata'dır. Full quota permanent delete için bounded systemReserve kaydı alan açmaya izin verir; diğer işlemler log rezervasyonu gerektirebilir. HTTP range parçaları tek ledger rezervasyonunda birikir. Silinen UID'nin saklanan kayıtları Root'a aktarılır ve504saat retention sonunda bırakılır; UID retired fence eski nesil verisinin benimsenmesini engeller.

Sistem Linux kullanıcıları sanal havuza dahil değildir. Mevcut OS kotası işletim sistemi tarafından uygulanır; uygulama bu kotayı değiştirmez. İsteğe bağlı Root home-template quota_bytes uygulama tavanı ayrıca olabilir. Public quota API, helper kök kullanımını ve bu uygulama tavanını bildirir; her filesystem türündeki mevcut OS soft/hard quota bilgilerini ayrı, taşınabilir bir API'de raporlamaz. Sınırsız/OS quota kapalı sonucu bu API'den varsayılmamalıdır.

Root native havuzu filesystem capacity/headroom içinde seçer. Bu sunucuda image1GiB, pool512MiB; metadata, retired audit ve operasyonel alan için pay bırakıldı. Tahsis toplamı fiziksel boş alan veya başarılı gelecekteki yazma garantisi değildir; filesystem doluluğu da uygulanır.

Uyarı eşiği `quota_warning_percent`, varsayılan80. SDK condition observer mevcut aktif durumu hemen verir; event geçişleri ayrı sunulur. Ölçüm hatasında eski aktif uyarı kaybolmaz, stale/unknown olur. Observer varken polling varsayılan60s; UI görünümünü SDK seçmez.


14 Ekim canlı tahsis: net havuz512MiB; sanal global admin `tealdrive` bütçesi512MiB, sabit own quota128MiB, çocuk tahsisine kalan384MiB. Kendi dosyaları/geçmiş/trash bu128MiB'ı kullanır. Upload1GiB tavanı kendi kotasının yerine geçmez. Root `budget own set ID SIZE` sabit own sınırını değiştirir; mevcut kullanımın veya bütçe/tahsis tutarlılığının altına indirme reddedilir.
