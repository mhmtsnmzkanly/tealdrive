# Doğrulanmış geri alma

Root `[journal] enabled=true` ve web hesapları açıksa `/api/undo` ile SDK `files.undo` kullanılabilir. Kendi başarılı chmod, rename, move ve trash işlemleri kapsam içindedir. Kalıcı silme ve içerik üzerine yazma geri alınmaz. Gömülü UI History and undo düğmesi kendi geçmişi/adayları, filtreler, old/new mode, süre tercihi ve ayrı geri alma onayı sunar.

```javascript
const preferences = await API.files.undo.getPreferences();
// hours=null: Root üst sınırını izle. Varsayılan üst sınır 24 saattir.
await API.files.undo.setPreferences({
  hours: Math.min(6, preferences.maximumHours),
  expectedRevision: preferences.revision,
});
const page = await API.files.undo.list({ limit: 50, action: "rename", path: "uploads" });
const candidate = page.items.find(item => item.available);
// Arayüz önce operation.sourcePath/targetPath ve expiresAt değerlerini gösterir.
// Kullanıcı geri alınacak işlemi seçtikten sonra:
if (candidate) {
  await API.files.undo.apply({
    operationId: candidate.operation.id,
    expectedState: candidate.expectedState,
  });
}
```

Liste own-account/current-policy süzgecinden geçer. Admin başka hesabın kaydını uygulayamaz; operationId yetki belgesi değildir. Liste history ile aynı limit/cursor/action/status/requestId/rootId/path/from/to filtrelerini destekler. `available` kayıt koşullarının uygunluğunu belirtir; dosyanın uygulama anında hâlâ uygun olduğu garantisi değildir.

Kullanıcı tercihi 1..Root üst sınırı veya null olabilir. Üst sınır azaltılırsa etkin süre de azalır; tercih güncellemesinde CAS revision gerekir. Tercih değişimi login generation veya oturumları iptal etmez. Süre işlemin createdAt değerinden başlar; sonradan tercih büyütülse de Root üst sınırı aşılamaz.

Helper güncel grant/NSS/home/policy doğrular, ortak home mutation lock'unu tutar ve bekleme sonrası tekrar doğrular. Kaydı Root DB'den alır; tarayıcı inverse path, UID/GID veya chmod değeri belirleyemez. Root tek kullanımlık pending claim'i önce commit eder. HMAC ile bağlı worker kalıcı User UID/GID düşürme ve Landlock sonrasında gerçek dosya durumunu karşılaştırır.

- Chmod: aynı pinned inode metadata/digest, mode, owner, link count ve zamanlar eşleşmelidir. Geçici okuma izni açılmaz; mode000 Linux6.6+ pinned syscall ile geri alınabilir. Eski SUID/SGID/sticky bitler desteklenmez.
- Rename/move/trash: bounded content/tree digest ve inode kimliği eşleşmelidir. Eksik/çok büyük/gizli alt öğeli gözlem geri almaya uygun sayılmaz. Hedef `renameat2(RENAME_NOREPLACE)` ile üzerine yazılmadan yayımlanır.
- Trash ters yolu ve ID protected kayıttandır; kullanıcı yazabilir trash metadata'sı yetki kaynağı değildir. Başarılı dönüşün ardından özel info kaydı FD üzerinden temizlenir; cleanup hatası gerçekleşmiş dönüşü sahte başarısızlık yapmaz.

Tamamlanan denemede original audit korunur ve `undoStatus` success/failed/unknown olur; `undoErrorCode` güvenli hata sınıfını tutar. Pending/unknown veya başarısız uygulanmış deneme otomatik tekrar edilmez; kullanıcı güncel dosyaları incelemelidir. Bu tercih belirsiz crash sonrası ikinci bir mutasyon başlatmayı engeller. Henüz başlamamış policy/kimlik reddi claim tüketmez.

Offline metadata restore undo_epoch değerini rastgele değiştirir. Eski audit görüntülenebilir kalır; eski yedekteki undo yetkisi yeniden canlanmaz. Normal policy generation değişimi güncel izinler altında tekrar değerlendirilir. SSH/Node/PHP gibi dış writer'ların bütün yarışlarını engelleyen bir inode etiketi veya transaction garantisi verilmez.
