# SDK state, gezinti ve bildirim sözleşmesi

Tarih: 2026-10-12. Kullanıcının son SDK/state önerisi; hedef tasarımdır, uygulanmış değildir.

[Dizin](README.md) · [SDK](SDK.md) · [Server](SERVER.md) · [Helper](HELPER.md)

## 1. Üç ana SDK bölümü

- `API.auth`: login/logout/enrollment/parola. `revokeSelf` yalnızca gerçek Linux kullanıcılarında; sanal kullanıcıda server/helper `REVOKE_NOT_ALLOWED` döndürür. Yetkili admin kendi sanal alt hesabını disable/revoke edebilir. Logout sanal kullanıcıda da serbesttir.
- `API.user`: Bütün hesapların ortak dosya, gezinti, trash, audit, undo, kişisel politika, kota ve bildirim işlemleri. İç alt gruplar bu namespace altında kalabilir.
- `API.admin`: Atanmış kapsamda yönetim; admin yaratma veya root config değiştirme yetkisi yoktur.
- `API.handshake`, `API.disconnect`, transport olayları SDK kök yaşam döngüsüdür; dördüncü iş alanı değildir.

Eski taslak `files/trash/archives/audit/undo/policy/quota` üst namespace'leri `user` altında toplanır. Bunlar henüz yayımlanmış SDK API'si değildir; mevcut Rust wire sözleşmesini gereksiz kırma gerekçesi oluşturmaz.

## 2. Handshake ve bağlantının güveni

Handshake doğru endpoint, HTTPS/WSS, sunucu origin/protokol kuralları ve oturum durumunu hazırlar. Tarayıcı TLS/sertifika doğrulamasını yapar; SDK sertifika hatasını aşamaz ve HTTP'den gelen scripti güvenli hâle getiremez.

**Kesin kullanıcı kararı:** `handshake()` WebSocket bağlantısını başlatır; login bundan sonra aynı bağlantı üzerinde yapılır. HTTP discovery → login → WS önerisi geçersizdir.

Hedef durum sırası: `disconnected → connecting → negotiating → auth_ready → authenticated`. WebSocket upgrade sırasında TLS/Origin/taşıma kontrolleri yapılır; upgrade kullanıcıya dosya yetkisi vermez. Kimliksiz kanalda yalnızca allowlisted protokol handshake/auth/enrollment mesajları, bounded kontrol yükleri ve zaman aşımı kabul edilir. Dosya/state/admin çağrıları `AUTH_REQUIRED` ile reddedilir. Kimliksiz bağlantı sayısı, idle/negotiation süresi ve auth maliyeti sınırlanır; auth başarısızlığında helper dosya motoru açılmaz.

Geçerli mevcut oturum varsa handshake kimliği yeniden doğrulayıp authenticated durumuna geçebilir. Yoksa `auth_ready` hazır olur ve login beklenir; login başarılı olmadan kullanıcı state'i gönderilmez. HTTP cookie/session kurulması, CSRF, origin, reconnect ve mevcut HTTP download kanıtının WS login ile nasıl bağlanacağı yeni protokol çalışmasının parçasıdır; session sırrı URL/subprotocol/log'a konmaz. Eski server'ın girişsiz WS reddi yeni dar pre-auth durum makinesiyle açıkça değiştirilecek; genel yetkisiz dosya dispatch açılmayacak.

Handshake ileride uygulama katmanında ek şifreleme için sürümlü aşamalar taşıyabilir: protokol/capability anlaşması, destekleniyorsa anahtar anlaşması ve karşı taraf doğrulaması, ardından auth-ready. Böyle bir özellik bu revizyonda uygulanmaz. Zorunlu şifre modu desteklenmiyorsa downgrade yerine hata; tekrar oynatma, mesaj sırası ve anahtar yaşam döngüsü ayrıca tasarlanır. PGP zorunlu seçim değildir; özel kripto yazmak yerine bakımı yapılan protokol/kütüphane değerlendirilir. Server-helper sınırı ayrıca tanımlanır: browser-server şifrelemesi server'ın plaintext erişimini otomatik engellemez.

`require_tls=true` durumunda HTTP login/parola gönderimi başlamadan reddedilir. Sunucu, proxy bilgisine yalnızca güvenilir bağlantıdan inanır. Sertifika/şifre modu SDK kullanıcısının rastgele seçtiği parametre değildir. TLS kullanılmayan açık config modunda handshake bunu güvenli şifreli bağlantı diye göstermez.

## 3. State'in sahibi — son karar

**Son kullanıcı açıklaması:** State, SDK'nın isteğe bağlı kolaylık katmanıdır. Kalıcı server view-state deposu ve helper'ın UI state saklaması gerekmiyor; önceki server-owned persistent state önerisi geçersizdir.

- Stateless çekirdek açık root/path/revision argümanlarıyla çalışır; stateful SDK katmanı current directory, seçim/açık dosya referansı ve notification yansımasını tutarak aynı çekirdeği kullanır.
- Server session/yetki, mesaj yönlendirme ve işlem koordinasyonunu yönetir. Helper gerçek path/izin/hash doğrular ve sonuçta gerekli düzeltmeyi döndürür. SDK bu doğrulanmış sonucu yerel state'e uygular.
- Sayfa yenilemede kendi güvenli konum tercihini geri almak için isteğe bağlı storage adapter kullanılabilir; saklanan path yetki kanıtı değildir, handshake/login sonrası yeniden doğrulanır. Varsayılan per-tab/persist süresi kesinleşmemiştir.
- Parola, session/helper sırrı ve dosya içeriği location storage'a yazılmaz. Kaydedilmemiş editor içeriği ayrı opt-in tasarımdır.
- Bağlantı kopunca eski state stale olarak görünür; mutasyonlar otomatik replay edilmez. Başka hesabın state'i taşınmaz; hesap değişiminde/revoke'da cache temizlenir.
- Server/helper tarafından yol silinmesi veya politika değişimi bildirildiğinde SDK en yakın erişilebilir üst dizine döner; erişim yoksa hata/state verir. Dış süreç değişiklikleri bir sonraki doğrulamada fark edilebilir.

## 4. Gezinti API'si

```javascript
const API = createClient({ stateMode: 'managed' }); // SDK yerel state katmanı
await API.handshake();
await API.auth.login({ username, password });

const check = await API.user.validatePath('/musics');
if (check.valid) {
  await API.user.goPath('/musics');
}

console.log(API.user.currentDir()); // '/musics': son doğrulanmış state
const page = await API.user.list(); // aktif doğrulanmış dizin

const unsubscribe = API.user.observeState((state) => {
  renderBreadcrumb(state.currentDirectory);
  renderConnection(state.connection);
});
```

`/musics` kullanıcının home/kökü altındaki sanal yoldur; host `/musics` değildir. Kendi gerçek home displayPath bilgisi gerekiyorsa yalnızca güvenli kendi path'i sunulabilir; işlemler root-relative referanslarla yapılır. İsimler camelCase önerisidir; `CurrentDir` yerine `currentDir` kullanılır.

- `goPath` helper'da yeniden doğrular; `validatePath` ön kontrolü yetki bileti değildir, arada path değişebilir.
- `currentDir()` eşzamanlı cache getter'ıdır; state hazır değilse `STATE_NOT_READY`. Bağlantı kopunca son bilinen path tutulur ama `stale=true` olur; güncel filesystem garantisi değildir.
- `list()` managed mode'da SDK yerel current-directory snapshot'ından açık kök/path üretir; helper her işlemde doğrular.
- Sekmede iki paralel gezinme ve eski cevapların yenisini ezmesi için state revision ve istek sırası denetlenir.
- Yazma çağrıları resolved file reference ve beklenen revision'a bağlıdır; işlem sırasında kullanıcı başka klasöre geçerse yanlış dosyaya yazılmaz.
- `user.delete/move` tamamlandığında current dizin yok olmuşsa veya policy kapatmışsa erişilebilir en yakın üst dizine, gerekirse home'a dönülür. Hiç erişilebilir kök yoksa home güvenliymiş gibi gösterilmez; erişim kapalı state'i döner.
- Başka uygulama/SSH dosyayı değiştirirse anlık haberdarlık garantisi yoktur. Bir sonraki doğrulanmış işlem/refresh/reconnect bunu düzeltir; gerektiğinde sınırlı helper sorgusu yapılır. Server UID'sine tüm home'ları izleme erişimi verilmez.

## 5. Reconnect ve refresh

Handshake/reconnect kimliği yeniden doğrular. SDK yerel/restored path ve açık dosya referanslarını server üzerinden helper'a doğrulatır. Silinmiş/gizlenmiş path veya farklı hesap eski konumu diriltemez.

Session/işlem/event revision ve kimliği doğrulanmış sonuçlarla yerel state güncellenir; out-of-order reply yeni state'i ezemez. Eksik olayda refresh yapar, kalıcı notification güncel kota ölçümünden yeniden kurulur. Bağlantı kopunca mutasyon replay edilmez; durum/idempotency gerekir. State resume upload resume garantisi değildir.

## 6. Etki-tepki ile kalıcı bildirim ayrımı

```javascript
// Etki-tepki: yalnızca eşik ilk aşıldığında; popup/toast için.
const stopEvents = API.user.on('quota.warning.entered', (quota) => {
  showToast(`Kullanım %${quota.usagePercent}`);
});

// Kalıcı durum: abonelikte hemen mevcut durum, sonra değişiklikler.
// Alan açılınca active=false gelir; UI kendi alert'ini kaldırır.
const stopWarning = API.user.warningQuota((warning) => {
  if (warning.active) {
    showPersistentAlert(warning.quota);
  } else {
    hidePersistentAlert();
  }
});

// Özel UI isterse bütün state'i kendi framework store'una bağlar.
const stopState = API.user.observeState((state) => updateStore(state));
```

`warningQuota` kota condition observer'ının kolaylık yöntemidir; ek bağımsız protokol değildir. Bildirim payload: kararlı ID/kod, active/severity, ölçüm zamanı/freshness ve kota parametreleri. Geliştirici popup/navbar/alert gösterimini seçer; SDK DOM çizmez.

Sabit `KOTA_%80` adı kullanılmaz: Eşik ayarlanabilir olduğundan event adı `quota.warning.entered`, payload `warningPercent=80` olur. Kalıcı condition yeniden sorgulanan doğrulanmış durumdan kurulur; yenileme/reconnect veya sonradan abonelik uyarıyı kaybetmez. Toast geçmişi sonsuza dek saklanmaz; aynı olay id/revision ile tekrar gösterim engellenir.

Ölçüm bozulursa eski alarm sessizce temizlenmez; `unknown/stale` durum verilir. Kota yeniden ölçülüp eşik altına indiğinde clear gelir. Admin bütçe/threshold değişimleri yeniden değerlendirilir.

## 7. SDK state'ini kullanmak zorunlu değil

```javascript
const API = createClient({ stateMode: 'client' });
await API.handshake();
await API.auth.login({ username, password });
const page = await API.user.list({ rootId: 'home', path: 'musics' });
```

Managed mode'da SDK gezintiyi kolaylaştıran yerel state tutar; client mode'da UI kendi breadcrumb/selection/location state'ini yönetip açık path gönderir. Her iki durumda auth/policy/revision/helper kontrolü zorunludur. Ortak SDK React/Vue/vanilla JS ve farklı UI'lerde DOM varsayımı olmadan kullanılabilir; arbitrary cross-origin endpoint desteği bu kararın otomatik sonucu değildir.

## 8. Test ve bağımlılıklar

- Yeni iş: Stateless çekirdek + stateful SDK adapter ve storage seçeneği; doğrulanmış sonuç/event/condition sözleşmesi; helper navigation ve revision kontrolü. Kalıcı server view-state deposu bu kapsamdan çıkarıldı.
- Silinen current-dir, policy deny, home değişimi, iki sekme, out-of-order reply, reload/reconnect, session/account değişimi, quota entered/clear/unknown ve sonradan observer testleri.
- Managed ve client mode eşdeğer yetki kontrolü; virtual revoke hem UI capability hem server/helper tarafında reddi.
- TLS/Origin başarısızlığı; handshake yokken çağrı, login yokken dosya çağrısı; reconnect'te mutasyonun tekrar yapılmaması.
- Kullanıcı ve session state'i güvenlik kararıyla karıştırılmaz; mevcut immutable per-operation policy ve lease iptali korunur.

## Browser storage — kesin kullanıcı kararı

State browser storage'da kullanıcı bazında saklanır. SDK kimlik doğrulandıktan sonra kararlı hesap ID/nesil + server origin/API kimliğiyle namespace oluşturur; username tek başına anahtar değildir. Account değiştirme/logout/revoke sırasında aktif cache ve subscription temizlenir; sonraki account eski snapshot'ı almaz. Saklanan state izin kanıtı değildir; path/açık dosya referansı helper'da restore sırasında yeniden doğrulanır. Dosya içeriği, parola, cookie/token/helper bileti storage'a girmez.

Storage adapter browser depolaması kapalı/dolu/bozuksa memory fallback ile çalışır; state geri yüklenememesi auth veya güvenlik kontrolünü devre dışı bırakmaz. LocalStorage/sessionStorage ve sekme başına ayrım teknik seçimdir, henüz kullanıcı tarafından kesinleştirilmedi.
