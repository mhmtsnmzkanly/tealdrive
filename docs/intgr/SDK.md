# TealDrive SDK

SDK, dosya yöneticisi arayüzünün **işlem ve bağlantı katmanıdır**. UI geliştiricisi binary protokol, oturum, request ID, upload parçaları, filtre/sıralama veya hata çeviri kodlarını yeniden yazmaz. SDK DOM üretmez; React/Vue/vanilla JS ile kullanılabilir.

Kaynak: `src/ui/sdk/client.js`. Tipler: `src/ui/sdk/client.d.ts`. Server bunları binary'den `/sdk/v1/tealdrive.js` ve `/sdk/v1/tealdrive.d.ts` olarak sunar; custom UI override edemez. API adları `auth / files / admin`, camelCase. Aşağıdaki tablo çalışan yüzeyi gösterir; gerçek destek Root ayarları ve handshake capability değerleriyle belirlenir.

## İlk kullanım

```javascript
import { createClient, TealDriveError } from '/sdk/v1/tealdrive.js';
const api = createClient();
await api.handshake(); // WS önce; protokol hazır, dosya yetkisi henüz yok.
await api.auth.login({ username, password }); // aynı WS; HttpOnly cookie bağlama otomatik.
const page = await api.files.list(); // doğrulanmış current directory.
```

HTTPS/WSS sertifika doğrulamasını tarayıcı yapar; Root zorunluluğu varsa handshake ek Noise NK kanalını da kurar. Kapsam ve pin modeli aşağıda açıklanır. Pre-auth sadece hello/auth mesajlarıdır. Auth başarısızsa dosya/admin erişimi yok. Login HttpOnly cookie için tek kullanımlık HTTP bridge yapar; parola HTTP body'ye gönderilmez. Bridge token storage/URL/log'a yazılmaz.

## Üç alan

| Alan | Görev |
|---|---|
| `auth` | Login, session, logout; web enrollment/password/revoke için capability kontrollü yüzey |
| `files` | Listeleme, gezinti, metin, CRUD/chmod, arşiv, transfer, trash, kota, kendi geçmiş/politika |
| `admin` | Panel ayarları; `systemUsers` ve `virtualUsers` ayrı yönetim alanları |

`handshake`, `disconnect`, `getInfo`, `on` bağlantı yaşam döngüsüdür. Root ayarı değiştirme, keyfi UID veya shell komutu yok. Admin/root ayar tavanı server/helper'da doğrulanır.

## Kapsam ve destek

| İşlev | Durum |
|---|---|
| WS-first handshake/login/session/logout | Uygulandı; `/ws/auth` ve tek kullanımlık cookie bridge |
| List/filter/type/category/extensions/sort/pagination | Uygulandı; helper pagination öncesi filtreler |
| Stat/readText/writeText/createText/createFolder/rename/move/copy/chmod | Uygulandı; text save revision zorunlu SDK alanı |
| Search | Uygulandı; recursive search sıradan list filtresinden ayrı |
| Upload progress/cancel/chunk ACK | Uygulandı; begin/chunk interim frame özel completion semantiği |
| Download | Uygulandı; streaming Response, UI kaydetme görünümünü seçer |
| ZIP/TAR.GZ create/extract, trash/restore/permanent delete | Uygulandı; feature/policy belirleyici |
| Helper quota get, condition/event observer | Uygulandı; ölçüm/hata/threshold; observer varken sınırlı polling |
| Managed/client state, browser storage | Uygulandı; kullanıcı namespace; path restore yeniden doğrulanır |
| Admin settings ve Linux user picker | Mevcut endpointlerle uygulandı |
| PAM first-enrollment + ayrı web parola login + nonce logout | Uygulandı; proof server WS'de, credentials WS'den; private grant user API'de yok |
| Password change/self-revoke, policy, virtual management | Uygulandı; gerçek helper/UID/PAM/Chromium kabulü |
| Kullanıcı geçmişi ve undo | Root journal açıkken protected backend, typed SDK ve gömülü geçmiş/undo UI tamamlandı |

Bu tablo işin bitiş raporu değildir. Son sütundaki backendler tamamlanmadan destek varmış gibi UI'de sunulmaz. Public sözleşme yeni SDK feature'ı için tasarlanmıştır; legacy wire API'nin bozularak yeniden adlandırılması değildir.

## Listeleme ve arama

```javascript
await api.files.list();
await api.files.list({
  filter: { name: 'report', type: 'file', extensions: ['pdf', 'txt'] },
  sort: { by: 'modifiedAt', direction: 'desc' },
  limit: 100,
  cursor: null,
});
await api.files.search({ path: 'projects', query: 'config', extension: 'toml' });
```

Filter kategori presetleri: All/Folders/Code/Images/Media/Documents/Archives/Hidden/Writable/Symlinks. `includeHidden` erişim izni vermez. Extensionlar dot içermeyen `pdf` veya compound `tar.gz`; en fazla64, her biri32 byte. Name query1024 byte. Limit helper üst sınırına tabidir. Cursor aynı filtre/sort/path ile kullanılır; mutable dizinlerde list snapshot garantisi yoktur. Unknown options sessiz ignored olmaz.

## Metin ve çakışma

```javascript
const file = await api.files.readText({ path: 'notes.txt' });
await api.files.writeText({
  path: 'notes.txt', content: editor.getValue(), expectedRevision: file.revision,
});
await api.files.createText({ path: 'new.txt', content: 'Yeni dosya' });
```

Read revision content+dosya kimliği/metadata'dır. Stale revision `REVISION_CONFLICT`; UI metni koruyup karşılaştırma/yeniden yükleme sunar. Create-only `missing` mevcut dosyayı overwrite etmez. Helper karşılaştırmayı transaction içinde yapar; dış writer ile son compare/publish yarışı için mutlak garanti yoktur.

## State ve gezinti

```javascript
await api.files.goTo('/musics');
api.files.getCurrentDirectory(); // son doğrulanmış '/musics'; host absolute path değil.
const unsubscribe = api.files.observeState(state => render(state));
await api.files.refreshState();
unsubscribe();

// Kendi UI store'unu kullanmak isteyen:
const custom = createClient({ stateMode: 'client', storage: null });
```

Managed default; client mode açık root/path verir. Browser storage kullanıcı/account-generation + origin kapsamında konum tercihi; secret/editor content tutulmaz. Restore path doğrulanır. Storage disabled/dolu/bozuksa RAM fallback. Cache getter filesystem anlık gerçeği değildir; stale field bunu belirtir. Başka kullanıcıya ait state gösterilmez.

## İki bildirim biçimi

```javascript
api.files.on('quota.warning.entered', quota => showToast(quota));
api.files.observeQuotaWarning(({ active, quota, status }) => {
  active ? showPersistentAlert(quota, status) : hidePersistentAlert();
});
```

Condition abonelikte hemen mevcut durum verir; quota.get/sonraki polling değişimleri yansır. Eşik payload'dadır, event adına %80 gömülmez. Ölçüm hatasında eski active condition silinmez; unknown/stale döner. Observer varken default polling60s; `quotaPollMs:0` otomatik polling'i kapatır. Bağlantı kapanışı ve unsubscribe timer'ı temizler. Server/frontend transport olayları: server.shutdown, session.revoked, connection.changed, files.changed.

## İptal ve mutasyon güvenliği

```javascript
const controller = new AbortController();
await api.files.upload({ directory: 'uploads', file,
  signal: controller.signal, onProgress: p => showProgress(p),
});
const response = await api.files.download({ path: 'uploads/report.pdf' });
// response.body stream; UI kendi save/preview davranışını seçer.
```

Abort/timeout beklemeyi sonlandırır; genel mutasyonun yapılmadığını kanıtlamaz. `operationMayHaveCompleted` kontrolü vardır; körlemesine retry yok. Upload başarısızlığında abort cleanup denenir; kopmada helper/server lease staging temizliğini yapar. Sayfa state restore upload resume değildir.

## Hatalar, sürümleme ve detay referansı

TealDriveError: code/params/requestId/retryable/status. Rust legacy bitişik hata adları SDK'da kararlı snake-case kodlara normalize edilir. Kodlar UI i18n sözlüğünde çevrilir; raw HTML gösterilmez. Handshake olmadan HANDSHAKE_REQUIRED; auth olmadan AUTH_REQUIRED.

Major yalnızca uyumsuz değişimde; minor yeni uyumlu feature; patch düzeltme. Ortak options/result/state/event/hata şemaları genişlemeyi kolaylaştırır. UI geliştiricisi yeni feature görünümünü ekler; mevcut list filter'ı yeniden yazmaz.

Bütün imzalar ve tipler [client.d.ts](../../src/ui/sdk/client.d.ts), runtime [client.js](../../src/ui/sdk/client.js), kullanım taslağı [sdk-api.js](../../sdk-api.js). [STATE.md](STATE.md), [TEST_RESULTS.md](TEST_RESULTS.md), [IMPLEMENTATION_DECISIONS.md](IMPLEMENTATION_DECISIONS.md).

## İlk kayıt: hangi parola, hangi bağlantı?

```javascript
const info = await API.handshake();
if (info.authentication.webPassword) {
  // Normal girişte web parolası kullanılır; Linux parolası reset yöntemi değildir.
  await API.auth.login({ username, password: webPassword });
}
```

Yeni uygun Linux hesabı için UI iki ayrı adım sunar:

```javascript
const pending = await API.auth.beginEnrollment({ username, linuxPassword });
// pending: { enrollmentRequired: true, expiresAt: epochSeconds }
// Linux input'unu temizleyin; pending içinde token/id yoktur.
const session = await API.auth.completeEnrollment({ webPassword });
// Aynı API ve WS; 300 saniye içinde. Parola 12+ Unicode karakter / <=1024 UTF-8 byte.
// session: accountId/accountGeneration/accountKind/role + username/roots/CSRF.
```

Kayıt başladıktan sonra sayfa/bağlantı kapanırsa yeniden PAM gerekir. SDK Linux veya web parolasını storage'a yazmaz. Managed storage namespace account ID/generation/origin ile ayrılır; içerik/credential yok. Virtual hesap PAM self-enrollment kullanmaz; admin/root provisioning bekler. Root deny/suspend, kayıt formu açıkken değişse bile completion'da tekrar denetlenir.

`getInfo().authentication.webPassword` giriş yöntemidir; `changePassword()` endpoint'inin hazır olduğu anlamına gelmez. Güncel sürümde parola değişimi ve uygun hesapların self-revoke akışı uygulanmıştır; virtual hesap revoke yetkisini yönetici kullanır. UI dosya işlemleri ortak SDK ve TR/EN arayüzünü kullanır.


## Kayıt onayı ve hesap erişimi (uygulandı)

```js
const candidates = await API.admin.systemUsers.list(); // uygun yerel NSS adayları
const accounts = await API.admin.systemUsers.accounts(); // kayıtlı Linux metadata
const approved = await API.admin.systemUsers.approve({username: "example-user"});
// approved.role === "user", approved.passwordEnrolled === false; kullanıcı PAM ile tamamlar.
await API.admin.systemUsers.deny({accountId: approved.id}); // suspend + eski grant/proof iptali
await API.admin.systemUsers.allow({accountId: approved.id}); // Root deny/NSS kontrolü + yeni generation
```

`authentication.linuxApproval` onay uçlarının capability'sidir; eski server'da false. UID/GID/home/role/parola args kabul edilmez. Root/admin atanması bu API'nin işi değildir. Tenant virtual admin global Linux işlemlerini yapamaz; Root tarafından açık global scope verilen admin genel panel yönetimi yapabilir. Virtual children create/list/disable/enable/delete/quota Root politikası ve owner bütçesine göre uygulanır. SDK types: AccountMetadata/UserCandidate/UserCandidates; hata401/403/409/503 kanonik karşılıklarla döner.


## Çalışan policy API

`getInfo().authentication.policyManagement` true olduğunda çalışır; eski server default false. Personal yalnız kendi katmanını, system/virtual yönetim yalnız admin katmanını değiştirir. Root kuralları salt okunur; başka Root feature/filesystem kısıtları da uygulanır. Success bütün hedef session'larını iptal eder.

```js
const view = await api.files.policy.get();
await api.files.policy.set({ expectedGeneration: view.account.generation, rules: [{ path: "private", recursive: true, deny: ["discover"] }] });
// Kişisel save kendi oturumunu kapatır; yeni handshake/login gerekir.
const target = await api.admin.virtualUsers.getPolicy({ accountId: "acct_customer" });
await api.admin.virtualUsers.setPolicy({ accountId: target.account.id, expectedGeneration: target.account.generation, rules: [{ path: "production", recursive: true, deny: ["chmod", "delete", "move"] }] });
```

Add/remove convenience method'ları tek CAS write kullanır, conflict'te retry yapmaz. Get revision-bound `ruleId` ve `revision` döndürür; remove yalnız editable layer ID'sini alır. `admin.policy` kind verilmezse virtual admin için virtual, Linux admin için Linux seçer; açık system/virtual method'ları tercih edilir. Tam result/options/error tipleri client.d.ts'de.


## Sanal provisioning ve kota — çalışan API

`authentication.virtualAccounts` true gerekir; delete ayrıca `virtualDelete` Root opt-in ister.

```js
const child = await api.admin.virtualUsers.create({name:"customer-a",password:"an-initial-web-password",quotaBytes:1048576,idempotencyKey:crypto.randomUUID()});
await api.admin.virtualUsers.setQuota({accountId:child.id,quotaBytes:2097152,expectedGeneration:child.generation});
const budget = await api.admin.quota.getBudget();
// delete: fresh metadata generation required; files retained36h by default.
```

UID/GID/home/role/shell never input; password consumed once, replay never resets it. SDK error.params.idempotencyKey supports network retry; SDK does not persist credentials. ExpectedGeneration or numeric-string expectedRevision (one only) accepts quota/delete. Quota errors QUOTA_EXCEEDED409; stale metadata CONFLICT409. Native physical measurements labeled allocated_bytes; application logical_bytes. Accounts changed event lets custom UI refresh; target sessions invalidate on quota/delete.


## Native browser URL ve runtime settings sözleşmesi

```js
const url = api.files.getDownloadUrl({rootId:"home",path:"large.zip"});
// <a href=url> ile browser disk indirmesi; secret query yok.
const media = api.files.getPreviewUrl({rootId:"home",path:"photo.jpg"});
const {config,mode} = await api.admin.settings.inspect();
await api.admin.settings.patch({changes:{quota_warning_percent:80},persist:false});
await api.admin.settings.reload();
```

URL üretmek dosyanın varlığını/iznini garanti etmez; request helper'a gider. `download()` hâlâ Response stream döndürür; büyük dosyayı gereksiz blob'a çevirmeniz gerekmez. Settings changes mevcut backend snake_case patch şemasındadır; persist varsayılantrue eski API uyumunu korur, false runtime-only. `inspect` camelCase config + split/legacy mode döndürür; Root kritik ayarı patch edilemez. RootSummary hiddenFilesAllowed optional eski sunucu uyumuyla eklenmiştir. WebSocket close1001/Server shutting down bilgisi SDK'da shutdown event'ine dönüşür; normal ağ kaybı shutdown sayılmaz.

## Kayıtlı hesaplarda sayfalama

```javascript
const first = await api.admin.virtualUsers.listPage({limit: 100, filter: 'project', status: 'active'});
if (first.hasMore) {
  const next = await api.admin.virtualUsers.listPage({limit: 100, filter: 'project', status: 'active', cursor: first.nextCursor});
}
const linux = await api.admin.systemUsers.accountsPage({limit: 100, enrolled: false});
```

Sonuç `{items, nextCursor, hasMore}`. Limit1..200 (varsayılan100), filter en fazla128 UTF-8 byte, cursor256 byte; kontrol karakterleri yasak. Filter username içinde SQLite ASCII case-insensitive eşleşmedir; Unicode case-folding iddiası yoktur. Status active/suspended/revoked/deleted; enrolled boolean. Filtre ve canlı yetki SQL LIMIT öncesi uygulanır. Cursor son public username'dir, yetki token'ı değildir; eşzamanlı değişen kayıtlar için snapshot garantisi yoktur. Sanal admin yalnız kendi çocuklarını görür, sistem hesabı yönetimi yapamaz. `virtualUsers.list()` eski array sözleşmesini korur; seçenek verilirse yeni backend ile filtreli ilk sayfanın array'ini döndürür. Tüm sayfalar için listPage kullanın. Capability `authentication.account_pagination`; destek yoksa seçenekler sessiz atılmaz. Linux aday listesindeki registered bilgisi mevcut sayfanın dışında kalan kayıtların yeniden onaylanmasını önler.

## Kendi işlem geçmişi sözleşmesi

```javascript
const page = await api.files.audit.list({
  limit: 100, action: 'write', status: 'success', rootId: 'home',
  path: 'projects/', from: 0, to: Math.floor(Date.now() / 1000),
});
const matched = await api.files.audit.list({requestId: operationResult.requestId});
if (page.hasMore) {
  const next = await api.files.audit.list({limit: 100, action: 'write', cursor: page.nextCursor});
}
```

Typed AuditQuery/AuditRecord/AuditPage; result `{items,nextCursor,hasMore}`. Limit1..200 default100; path source/target case-sensitive substring (256UTF-8 bytes), rootId128, action32 ASCII alnum, cursor160; controls/unknown options rejected. from/to inclusive Unix epoch seconds, from<=to. requestId UUID traces OperationResult to protected helper event. Pending/success/failed/unknown distinct; bytes/mode request recorded, plaintext file contents/credentials absent. Same filters across cursor pages; live listing is not a snapshot. No accountId/UID override; helper derives own scope and current hidden-path policy. Legacy array endpoints unaffected. `getInfo().authentication.auditHistory` is enabled when Root journal mode and web accounts are active; actual metadata and log quota acceptance completed. No fake successful history/undo.

`files.deletePermanently({path,rootId?,signal?})` canlı path için açık ve geri alınamaz silmedir; Root feature +helper policy gerektirir. Trash ID için ayrı `files.trash.deletePermanently` korunur. Native full-quota alan açma kabulü doğrudan public SDK ile yapıldı. AuditRecord optional oldMode/newMode/before/after ve logReserveBytes/logStorage yeni metadata alanlarıdır; eski backend null/omitted ile uyumludur. Root journal false ise FEATURE_DISABLED gerçek config durumudur.

## Koşullu geri alma

`getInfo().authentication.undo` Root journal ve web hesapları açıkken true. `files.undo.list(AuditQuery)` typed UndoPage/UndoCandidate döndürür; `apply({operationId,expectedState})` Root kayıt ve gerçek dosya doğrulamasından geçer. `getPreferences`/`setPreferences({hours,expectedRevision})` kendi süre tercihidir; null Root üst sınırını izler. Event `files.changed` operation=undo ile gelir. Kullanım, hata/claim/crash davranışı ve gerçek sınırlar [UNDO.md](UNDO.md) içindedir.

HTTP download/preview yanıtındaki `x-tealdrive-request-id`, protected range kaydının requestId'sidir. Akış tüketildikten sonra `files.audit.list({requestId:response.headers.get('x-tealdrive-request-id')})` ile ilişkilendirilebilir. bytesRead/rangeNextOffset helper teslimini ifade eder; tarayıcı diske kaydetme veya tam indirme kanıtı değildir.


### Adminin kendi kotası ile dağıtılabilir bütçe

```js
const budget = await api.admin.quota.getBudget();
// Root fixed rezervasyonu yaptıysa: "fixed"; eski hesaplarda "remainder".
console.log(budget.ownQuotaMode);
// Yeni alt hesaplara dağıtılabilecek alan (fixed modda own kullanım tekrar düşülmez).
console.log(budget.availableBytes);
// Adminin kendi dosyaları için kalan gerçek alan.
console.log(budget.ownAvailableBytes);
// Fixed modda sabit kişisel limit, legacy modda null.
console.log(budget.ownQuotaBytes);
```

Bu alanlar additive ve eski server'larda optionaldır. `usedBytes` helper ölçümüdür; ayrılan bütçe gerçek disk kullanımı değildir. Kişisel kota Root CLI ile değiştirilir, web kritik Root bütçesini yükseltemez.


### Şifreli WS transport

`createClient({encryption:'required',trustedServerKeys:[PUBLIC_HEX_KEY]})` ardından `await api.handshake()` kullanılır. Bu ayarlar dosya/auth API'lerini değiştirmez. Default `auto` server greeting gerektiriyorsa Noise kurar. `getInfo().transport` negotiated WS encryption, protocol ve key trust türünü döndürür. TLS discovery ile bağımsız pin birbirinden ayrıdır; HTTP routes/native download kapsam dışında olduğundan `httpEncryptedChannel:false` gösterilir. Ayrıntılı kullanım/sınırlar: [TRANSPORT_SECURITY.md](TRANSPORT_SECURITY.md).


### Kesintiden sonra upload

```js
const api=createClient();
await api.handshake();
let transfer;
try {
  await api.files.upload({directory:'/uploads',file,
    onTransfer: info => { transfer=info.transferId; },
    onProgress: info => renderProgress(info.transferredBytes,info.totalBytes)
  });
} catch(error) {
  if(!error.params?.resumeTransferId) throw error;
  await api.handshake(); // Aynı HttpOnly oturum; yeni WS ve gerekiyorsa Noise handshake.
  await api.files.upload({directory:'/uploads',file,
    resumeTransferId:error.params.resumeTransferId
  });
}
```

`getInfo().transfers.uploadResumeSeconds` kalan transferin en fazla ne kadar tutulacağını ilan eder (Rootdefault1800s);0desteksiz/kapalıdır. `resumable:false` eski disconnect-cleanup davranışını seçer. `onTransfer` transferId,resumedBytes,expiresInSeconds döndürür. Resume aynı oturum, aynı root/path/name/size ve eşleşen prefix gerektirir; yanlış dosya `RESUME_FILE_MISMATCH` ile reddedilir ve staging abort edilir. SHA256 incremental64KiB bellekle hesaplanır. Başka oturum, TTL sonu veya serverrestart sonrası transfer geri alınamaz. Commit cevabı kaybolduysa resume token verilmez; hedefi kontrol etmeden yeniden yükleyip overwrite denenmez. BrowserStorage'a File içeriği veya parola yazılmaz; reload için aynı File tekrar seçilmelidir.


### İlk girişte geçici parola

Session metadata `passwordChangeRequired` olduğunda dosya/yönetim erişimi yoktur; roots boş ve isAdminfalse'dur. Custom UI kendi parola formunu gösterip `api.auth.changePassword({currentPassword,newPassword})` çağırır; sonra yeni parolayla yeniden login gerekir. SDK bu durumdaki diğer API'leri `PASSWORD_CHANGE_REQUIRED` ile reddeder; bu bir UI kuralı değil protected helper/server yetki sınırıdır. Yeni parola geçici paroladan farklı olmalıdır.


### Bekleyen yüklemeyi iptal etme ve commit belirsizliği

`await api.files.abortUpload({transferId})` aktif veya bağlantı sonrası tutulmuş aynı oturumun yüklemesini iptal eder; farklı oturuma ait ID NotFound olarak döner. Sonlandırma/commit aşamasında cevap kaybolursa SDK `UPLOAD_COMMIT_UNCONFIRMED` ve operationMayHaveCompleted döndürür; otomatik tekrar commit/overwrite yapmaz. UI bu aşamada iptal düğmesini kapatır ve kayıp cevap durumunda dosya listesini kontrol etmeyi ister.

### Bundler ve transport adaptörleri

Varsayılan import server'ın `/sdk/v1/tealdrive.js` modülüdür ve aynı origin cookie/CSRF/Origin kurallarını kullanır. React/Vue/başka UI framework'üne bağlı değildir. Kendi bundler'ınız modül yollarını değiştiriyorsa initialized runtime'ı `noiseLoader` ile verin:

```js
const noiseLoader=async()=>{
  const module=await import('./crypto/noise.js');
  await module.default({module_or_path:wasmAssetUrl});
  return module;
};
const api=createClient({noiseLoader,encryption:'required',trustedServerKeys:[publicKey]});
await api.handshake();
```

Aynı loader fonksiyonunu paylaşan eşzamanlı istemciler tek initialize promise kullanır; başarısız initialize sonraki denemeyi engellemez. Aksi halde wasm-bindgen global instance'ı yarışla değiştirilip mevcut opaque handle'lar bozulabilir. Gerekirse `fetch`, `WebSocket`, `location` dependency adapter'ları typed options olarak verilebilir; cookie/CSRF/Origin güvenlik sözleşmesini adaptör korumalıdır. Cross-origin erişim/CORS veya cookie güvenlik kuralları SDK tarafından sessizce gevşetilmez. Wire/contract override'ları test altyapısı içindir; public TypeScript adapter yüzeyi değildir.


### Aktarım kimliğinin public biçimi

`onTransfer().transferId`, reconnect hatasındaki `params.resumeTransferId` ve belirsiz commit hatasındaki `params.transferId` standart tireli UUID metnidir. SDK MessagePack binary UUID'yi bu biçime dönüştürür; resume/abort çağrılarında wire byte temsiline geri çevirir. Kimliği aynı HttpOnly session dışında kullanmak yetki vermez. Geçersiz resume kimliği `INVALID_ARGUMENT` üretir.


`files.upload()` sonucu `UploadResult`: `transferId:string`, `bytesWritten:number`, `sha256Verified:boolean|null` ve inherited `operation/requestId` alanlarıdır. Başarı sonucu kimliği de callback/resume kimliğiyle aynı canonical UUID'dir. Bu alanlar strict TypeScript tüketici ve gerçek binary MessagePack cevap testiyle doğrulanır.
