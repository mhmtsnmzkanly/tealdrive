# TealDrive SDK geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## 2026-10-12 SDK güncellemesi — önce bunu okuyun

Yeni public model üç iş alanıdır: `auth`, `user`, `admin`. Gezinti, kota ve diğer ortak dosya alt grupları `user` altında toplanır. `handshake/disconnect` kök yaşam döngüsüdür. Sanal kullanıcı revoke yapamaz; yetkili admin kapatır, logout bütün kullanıcılara açıktır.

State, helper doğrulamalı server tarafından yönetilir; managed mode varsayılan öneri, client mode özel UI seçeneğidir. `currentDir/goPath/validatePath`, sayfa yenileme/reconnect, transient event ve kalıcı condition davranışının yetkili sözleşmesi [STATE.md](STATE.md) dosyasındadır. Aşağıdaki explicit-path örnekler client mode'da da kullanılabilir; managed mode her çağrıda path yazmayı gerektirmez.

Kesin akış: handshake → WebSocket bağlantısı/protokol anlaşması → auth-ready → aynı WS üzerinde login → authenticated. Girişsiz kanal yalnızca sınırlı handshake/auth mesajları içindir; dosya/admin çağrıları reddedilir. İleride anahtar anlaşması için aşamalar sürümlenir. Handshake JS'ten TLS sertifika hatalarını aşamaz.

## Yetkili başlangıç sözleşmesi

Son kullanıcı örneği `API.handshake()` olduğu için önceki public `connect()` bunun içine alınır. SDK özel TLS kriptografisi değildir.

```javascript
import { createClient } from '/sdk/v1/tealdrive.js';
const API = createClient();
await API.handshake();
await API.auth.login({ username, password });
const page = await API.user.list({ rootId: 'home', path: '', limit: 100 });
```

Handshake WebSocket bağlantısını açar ve sürümlü protokol anlaşmasını yapar. Oturum yoksa auth-ready; login aynı WS üzerinde kimliği doğrular. Mevcut oturum doğrulanırsa doğrudan authenticated olabilir. Giriş öncesi yalnızca allowlisted handshake/auth mesajları kabul edilir. Öncesinde ağ yöntemleri HANDSHAKE_REQUIRED, özel yöntemler AUTH_REQUIRED verir; on/disconnect istisnadır.

Semver: Patch düzeltme, minor uyumlu özellik, major uyumsuz değişiklik. Her feature'da /sdk/v1 artmaz. Olaylar: server.shutdown, session.revoked, connection.changed, quota.warning.

[SDK kullanım/JSDoc taslağı](../../sdk-api.js) 56 yöntem örneğidir; implementasyon değildir. `api.user.trash.move` yüzeye dahildir. Kodlar mevcut Rust sözleşmesiyle uygulamada eşlenir; root config, createAdmin, setUid, executeShell browser API'si yoktur.


## 5. JavaScript API yüzeyi

### 5.1 Yükleme ve bağlantı

```html
<script type="module" src="/ui/main.js"></script>
```

```javascript
import { createClient, TealDriveError } from '/sdk/v1/tealdrive.js';

const api = createClient(); // Aynı origin; keyfi uzak API varsayılanı yok.
const info = await api.handshake(); // auth akışı, SDK/API sürümü, kamusal yetenekler
const session = await api.auth.getSession(); // oturum yoksa null

if (session) {
  // Aynı WS: geçerli oturum varsa authenticated; yoksa auth-ready.
}

api.on('server.shutdown', ({ deadlineSeconds }) => {
  showShutdownMessage(deadlineSeconds);
});
api.on('session.revoked', () => showLogin());
api.on('connection.changed', ({ status }) => updateConnectionBadge(status));
```

`showShutdownMessage`, `showLogin` ve diğer görünüm fonksiyonları UI geliştiricisine aittir. SDK UI popup'ı çizmez; yapılandırılmış sonuç/olay üretir. Session cookie HttpOnly; SDK cookie veya helper biletini JS'e açmaz. CSRF, request ID, WS codec ve transfer akışı SDK içinde yönetilir.

### 5.2 Namespace ve yöntemler

| Namespace | Önerilen yöntemler | Sonuç / yetki |
|---|---|---|
| `api` | `handshake`, `getInfo`, `disconnect`, `on` | Sürüm, bağlantı, olay aboneliği |
| `api.auth` | `getSession`, `login`, `beginEnrollment`, `completeEnrollment`, `changePassword`, `logout`, `revokeSelf` | Kimlik akışı; sadece gereken kısa ömürlü yetki |
| `api.user` | `list`, `stat`, `readText`, `writeText`, `createFolder`, `rename`, `move`, `copy`, `chmod`, `search`, `upload`, `download` | Kullanıcının etkin home/kök politikası |
| `api.user.archives` | `create`, `extract` | Feature + kaynak/hedef politika kontrolleri |
| `api.user.trash` | `move`, `list`, `restore`, `deletePermanently` | Sadece kendi trash'i; kalıcı silme ayrıca izinli |
| `api.user.audit` | `listOwn` | Kendi doğrulanmış işlem geçmişi |
| `api.user.undo` | `listOwn`, `apply` | Güncel politika + beklenen durum; feature'a bağlı |
| `api.user.policy` | `getOwn`, `addPersonalRule`, `removePersonalRule` | Yalnızca kişisel katman |
| `api.user.quota` | `getOwn` | Sanal kota veya OS kota durumu |
| `api.admin.settings` | `get`, `patch` | Yalnızca webden değişebilir ayarlar |
| `api.admin.systemUsers` | `list`, `allow`, `deny` | Root'un devrettiği hesap kapsamı |
| `api.admin.virtualUsers` | `list`, `create`, `setQuota`, `disable`, `enable` | Yönetilen alt hesap ve admin bütçesi |
| `api.admin.policy` | `get`, `addRule`, `removeRule` | Yalnızca admin katmanı ve atanmış kapsam |
| `api.admin.quota` | `getBudget` | Atanmış bütçe, kullanım, tahsis |

Root güvenlik tavanı için browser `setCriticalConfig`, `createAdmin`, `setUid`, `executeShell` veya `writeHelperKey` API'si yoktur. Admin yöntemleri server/helper'da kimlikle denetlenir; UI'de düğmeyi gizlemek yetki kontrolü değildir. Henüz uygulanmayan feature capability'de kapalıdır; SDK `FEATURE_DISABLED` döndürür.

### 5.3 Login ve ilk web parolası

```javascript
// Normal giriş: pam modunda Linux; ayrı parola modunda kayıtlı web parolası.
await api.auth.login({ username, password });
// Aynı WS bağlantısı artık authenticated; dosya işlemleri açılır.

// Kayıt ekranında yalnızca henüz web parolası olmayan uygun Linux hesabı:
const enrollment = await api.auth.beginEnrollment({
  username,
  linuxPassword,
});
await api.auth.completeEnrollment({
  enrollmentId: enrollment.id,
  webPassword,
});

await api.auth.changePassword({ currentPassword, newPassword });
await api.auth.logout();
```

UI aynı parolaları kalıcı state/localStorage/sessionStorage'da tutmaz; input ve gereksiz referansları temizler. Enrollment normal dosya oturumu değildir. Sanal hesap ilk parola kurulum kanıtı ayrı, tek kullanımlık akıştır; Linux PAM enrollment'a sahte UID ile girmez. Enrollment için Linux parolasından farklılık o anda denetlenir; sonradan Linux parolası aynı yapılırsa sürekli farklılık garantisi yoktur.

### 5.4 Listeleme ve revision ile düzenleme

```javascript
const session = await api.auth.getSession();
const rootId = session.roots[0].id;

const page = await api.user.list({
  rootId,
  path: '', // seçilen kökün başlangıcı
  limit: 100,
});

const file = await api.user.readText({ rootId, path: 'projects/app/.env' });
// file: { content, revision, metadata, capabilities }

try {
  await api.user.writeText({
    rootId,
    path: 'projects/app/.env',
    content: editor.getValue(),
    expectedRevision: file.revision,
  });
} catch (error) {
  if (error instanceof TealDriveError && error.code === 'REVISION_CONFLICT') {
    showConflictDialog(); // mevcut düzenlemeyi silme
  } else {
    showError(error.code, error.params);
  }
}
```

Path home/köke göre relatiftir; OS absolute path veya `..` kabul edilmez. SDK yardımcı validation yapabilir ama helper tekrar kontrol eder. Revision token opaque kabul edilir; UI token'dan izin veya path çıkarmaz. Helper içerik hash'ı/dosya kimliğini kaydetme içinde doğrular; ayrı hash ön sorgusu güvenlik şartı değildir.

### 5.5 Move, rename ve chmod

```javascript
await api.user.rename({
  rootId,
  path: 'notes/old.md',
  newName: 'new.md',
  expectedRevision: entry.revision,
});

await api.user.move({
  source: { rootId, path: 'notes/new.md' },
  destination: { rootId, path: 'archive/new.md' },
  expectedRevision: entryAfterRename.revision,
});

const current = await api.user.stat({ rootId, path: 'projects/app/config.toml' });
await api.user.chmod({
  rootId,
  path: 'projects/app/config.toml',
  mode: 0o640, // sayı; "0640" UI'de parse edilir
  expectedRevision: current.revision,
});
```

`entry`/`entryAfterRename` UI'nin list/stat veya önceki işlem sonucundan aldığı güncel kayıtlardır. Değişiklik sonuçları güncel revision döndürür; eski token tekrar kullanılmaz. `chmod` özel bit kabul etmez. UI Others yazma ve erişim kaybı uyarısını gösterir; helper eski/yeni mode'u gerçek dosyadan kaydeder. Default hedef üzerine yazma yoktur; overwrite gerekiyorsa ayrı açık API/politika tasarlanır.

### 5.6 Upload/download ve iptal

```javascript
const controller = new AbortController();

const result = await api.user.upload({
  rootId,
  directory: 'uploads',
  file: fileInput.files[0],
  signal: controller.signal,
  onProgress: ({ transferredBytes, totalBytes }) => {
    updateProgress(transferredBytes, totalBytes);
  },
});

await api.user.download({ rootId, path: 'uploads/report.pdf' });
// Tarayıcı indirme akışı; büyük dosyayı zorunlu olarak RAM'e almaz.

controller.abort(); // yalnızca ilgili devam eden transferi iptal eder
```

Yükleme tamamlandı sonucu helper publish onayından sonra döner. İptal/bağlantı kopmasında staging temizliği mevcut kontrollü lifecycle ile korunur. Transfer API'si `Blob`/`File` kabul eder; programcı binary WS chunk formatını elle yazmaz.

### 5.7 Kendi audit geçmişi, trash ve geri alma

```javascript
const history = await api.user.audit.listOwn({ limit: 50, cursor: null });
// events + nextCursor; username seçerek başka kullanıcı logu istenemez.

const trash = await api.user.trash.list({ rootId, limit: 50 });
await api.user.trash.restore({ rootId, trashId: selectedTrash.id });

const undoPage = await api.user.undo.listOwn({ limit: 20 });
const candidate = undoPage.items.find((item) => item.canUndo);
if (candidate) {
  await api.user.undo.apply({
    operationId: candidate.operationId,
    expectedState: candidate.expectedState,
  });
}
```

Undo adayında `canUndo`, `reasonCode`, `expiresAt`, kaynak/hedef ve güvenli fark bilgileri bulunur. Hash uyuşmazlığı, kayıp trash içeriği veya hedef çakışması varsa görünür ama uygulanamaz. `canUndo=true` yalnızca listeleme anındaki durumdur; helper uygulama anında yeniden doğrular. Güncel yasaklı path/sır bilgileri geçmiş ekranından sızdırılmaz. Eski içeriğin yedeği yoksa hash ile dosya yeniden üretilemez.

### 5.8 Kişisel politika ve admin çağrıları

```javascript
await api.user.policy.addPersonalRule({
  path: 'private',
  recursive: true,
  deny: ['discover', 'read', 'write'],
});

const settings = await api.admin.settings.get();
await api.admin.settings.patch({
  expectedRevision: settings.revision,
  changes: { ui: { default_locale: 'tr' } },
});

const created = await api.admin.virtualUsers.create({
  name: 'customer-one',
  quotaBytes: 20 * 1024 ** 3,
});
// Hesap awaiting_password durumunda; creator plaintext parola vermez.
// İlk kurulum kanıtı ayrı kısa ömürlü, tek kullanımlık güvenli akışla iletilir.
```

Sanal hesap oluşturma idempotency anahtarıyla korunur; cevap kaybolunca tekrar create başka UID üretmez. Kimlik/politika kaydı tamamlanmadan hesap enabled olmaz. Admin bütçe/rolü ve hedef sahipliği helper'da doğrulanır. Ayar PATCH critical config'i kabul etmez; revision conflict eski ayarın üzerine yazmayı engeller.

### 5.9 Hata ve i18n sözleşmesi

```javascript
// Örnek SDK hatasının alanları:
const errorShape = {
  code: 'PERMISSION_DENIED',
  params: { operation: 'read' },
  requestId: 'req_example',
  retryable: false
};

// UI sözlük örneği:
const messages = {
  tr: {
    PERMISSION_DENIED: 'Bu işlem için erişim izniniz yok.',
    REVISION_CONFLICT: 'Dosya değişmiş. Kaydetmeden önce karşılaştırın.',
    QUOTA_EXCEEDED: 'Kota sınırı aşıldı.',
    UNDO_STATE_MISMATCH: 'Dosyanın durumu değiştiği için işlem geri alınamıyor.',
  },
};
```

Son bloktaki hata nesnesi şekil örneğidir; actual hatalar `TealDriveError` olarak fırlatılır. Parametreler raw HTML değildir; UI text olarak yerleştirir. Server çevrilmiş kullanıcı cümlesi göndermez; teknik audit/log açıklaması kullanıcı mesajından ayrıdır. Bilinmeyen kod genel hata gösterir, uygulama çökmez. SDK mutasyonları hata sonrası körlemesine tekrar denemez; idempotency veya operation-status kontrolü gerekir.



## 7. API uyumu ve geliştirme sınırları

- JS SDK public yüzeyi camelCase, wire/Rust sözleşmesi mevcut formatla uyumludur; dönüşümü SDK yapar. Config TOML alanları snake_case kalır.
- `/sdk/v1/` aynı major içinde kararlı kalır; yeni yöntem/alan/feature eklenebilir. Bir yöntemin anlamı sessizce değişmez.
- Server `getInfo` ve WS handshake ile capability sunar. Mevcut özelliklerin isim/çalışması kritik sorun olmadıkça korunur. Yeni özelliği kullanmayan custom UI çalışmaya devam eder.
- Capability sadece UX bilgisidir; güvenlik kararı helper'dadır. Client capability değiştirmek izin açmaz.
- Root config, yönetici kısıtı, kişisel kısıt, Linux izinleri ve sandbox birlikte uygulanır; en dar erişim geçerlidir.
- Kullanıcı log görüntüleme ve undo endpoint'leri kimliği session'dan alır; kullanıcı parametresinden alınmaz. Admin erişimi yalnızca atanmış kapsamdır.
- Belgede önerilen API komutları mevcut `docs/API.md` ile geliştirme sırasında eşlenir; yeni isimler eskilerin bozulduğu anlamına gelmez. Yeni revision şartları eski client için sessiz güvensiz bypass oluşturamaz; uyumlu geçiş veya açık feature/version şartı gerekir.
- Log/journal/restore metadata gerçek kullanıcı dosyalarının yedeği değildir. Başka bir uygulamanın erişimini helper yasakları kısıtlamaz.



### 4. i18n ve hata sözleşmesi — orta

- HTTP/WS için kararlı hata kodları ve parametreler; Türkçe/İngilizce istemci sözlükleri ve bilinmeyen kod fallback'i.
- Kullanıcıya gösterilen açıklama ile teknik server logunu ayır. Server teknik teşhis kaydı tutabilir; istemci yerelleştirilmiş mesaj üretir.
- Eski mevcut `error`/mesaj alanları için uyumlu geçiş ve kaldırma takvimi yaz.
- Kabul: Aynı hatanın iki dilde karşılığı, hassas path/secret sızıntısız parametreler ve eski client davranışı.



### 9. Kararlı JS SDK ve sürümlenmiş API — büyük

- Mevcut kontrat/codec üstüne `TealDriveAPI` benzeri arayüz: login/session, list/read/write/move/chmod, upload/download, admin işlemleri ve olaylar.
- UI helper'a doğrudan konuşmaz; SDK → server HTTP/WS → helper. Browser'a helper bileti/anahtarı açılmaz.
- Transport, request correlation, CSRF, shutdown, reconnect, binary codec ve hata çeviri kodlarını UI'den ayır.
- chmod uygulama/validation API'si ve diğer işlemler SDK'da ortak; tüm güvenlik kararları server/helper'da tekrar doğrulanır.
- API sürümü, capability keşfi ve eklenebilir alanlar; mevcut API kritik bir sorun dışında değişmez. Yeni özellik eski client'ı zorunlu güncellemeye itmez; custom UI'de yeni özelliğin gösterilmesi tema geliştiricisinin sorumluluğudur.
- SDK kendi sabit route'undan gömülü sunulur; custom paket aynı path'i override edemez.
- Kabul: Gömülü UI ve örnek minimal UI aynı SDK ile çalışır; eski/yeni client uyumu ve bilinmeyen özellik davranışı.


## 2026-10-12: Geniş SDK kapsamı ve kararlı sözleşme

Kullanıcı SDK'nın baştan geniş tutulmasını istiyor: Sonradan eksik bir özellik eklemek mevcut kullanıcı kodunu veya temayı bozmamalı. Bu, gelecekte hiçbir yöntem eklenmeyeceği garantisi değildir; eklemelerin geriye uyumlu olması gereğidir.

- Üç ana iş alanı auth/user/admin korunur; her yeni özellik için yeni üst namespace oluşturulmaz.
- Uygulamadan önce kapsam matrisi çıkarılır: Auth/kayıt/session, navigation/state, metadata/list/search, editor/revision, create/rename/move/copy/delete, chmod, upload/download/cancel/progress, archive, trash/restore, audit/undo, quota, kişisel/admin policy, sanal hesap ve ayarlar.
- Her yöntemde input/result şeması, hata kodları, async/cancel davranışı, yetki, revision/idempotency, pagination, event ve condition etkisi tanımlanır. Yalnızca fonksiyon adlarının listesi yeterli değildir.
- Geniş kapsam tasarımı bütün feature'ların ilk sürümde açık olması anlamına gelmez. Uygulanmayan/yetkisiz alan capability'de görünür ve kararlı hata verir; sahte başarılı stub olmaz.
- Yeni opsiyonel alanın default'u eski davranışı korur. Eski parametre/sonuç/event anlamı sessiz değişmez; optional field sonradan required yapılmaz, bilinmeyen ek alan eski client'ı çökertmez.
- State snapshot/revision, notification condition, error ve mutation result gibi ortak sözleşmeler bir kere tanımlanıp tüm yöntemlerde kullanılır. Her feature bağımsız biçim icat etmez.
- Güvenlik nedeniyle zorunlu kırılım varsa açık major/migration/deprecation uygulanır; güvensiz eski davranış sırf uyum adına sürdürülmez.
- Eski custom UI'nin yeni server ile çalışması contract testleriyle doğrulanır. Yeni özelliği UI'de sunmak tema geliştiricisinin sorumluluğu; mevcut özellikleri yeniden yazmak zorunda kalmamalı.
- CRUD/session temellerini tamamlamadan sınırsız generic execute veya keyfi komut API'si eklenmez. Geniş SDK, root yetki alanını genişletme gerekçesi değildir.

SDK kapsam matrisi ve ortak tipler implementasyon öncesi ayrı incelenebilir çalışma olacak; mevcut sdk-api.js kullanım taslağı tek başına tam sözleşme sayılmaz.

## Admin hesap yönetimi ayrımı — 2026-10-12

Kesin kullanıcı isteği: Mevcut sistem/Linux hesapları ve TealDrive sanal hesapları yönetimde ayrı namespace kullanır.

```javascript
// Mevcut Linux hesapları; OS hesabı oluşturma/silme yok.
await api.admin.systemUsers.list({ limit: 50 });
await api.admin.systemUsers.allow({ accountId: 'acct_alice' });
await api.admin.systemUsers.deny({ accountId: 'acct_alice' });

// Yalnızca yönettiği TealDrive alt hesapları.
await api.admin.virtualUsers.list({ limit: 50 });
await api.admin.virtualUsers.create({
  name: 'customer-one',
  quotaBytes: 20 * 1024 ** 3,
  idempotencyKey: 'unique_request_key',
});
await api.admin.virtualUsers.setQuota({
  accountId: 'acct_customer_one',
  quotaBytes: 10 * 1024 ** 3,
  expectedRevision: 'current_account_revision',
});
await api.admin.virtualUsers.disable({ accountId: 'acct_customer_one' });
```

- systemUsers yalnızca root tarafından devredilmiş mevcut Linux hesaplarının web erişimini yönetir; OS hesabını silmez, Linux kotasını değiştirmez.
- virtualUsers adminin kendisine bağlı sanal alt hesaplarını yönetir; başka admin yaratamaz, root bütçesini/havuzunu aşamaz. Enable/disable ve parola kurtarma yetkisi de aynı kapsamda doğrulanır.
- Namespace seçimi güvenlik kanıtı değildir. Server/helper gerçek account kind, owner admin ve yetkiyi denetler; sanal hesabı systemUsers API'sine veya tersi göndererek sınır aşılamaz.
- Ortak panel ayarları ve yönetici bütçe bilgisi admin.settings/admin.quota altında kalır. Mevcut admin.policy yöntemleri kind/atanmış kapsam doğrulamalı ortak policy motorudur; gerekirse kullanıcı yönetimi altına presentation wrapper eklenir, kontrol çoğaltılmaz.
- Üst ortak dosya alanı için files adı ve goTo/getCurrentDirectory/observeQuotaWarning önerileri henüz açık naming önerisidir; uygulamadan önce tüm örneklerle birlikte kesinleştirilir. Bu revizyon admin ayrımını netleştirir.

## Ayrılmış uygulama işleri — SDK

1. auth/ortak dosya alanı/admin namespace'leri; systemUsers/virtualUsers ayrımı. Ortak dosya alanının user/files son adı uygulamadan önce tek seferde kesinleşmeli; kullanıcı son örnekleri files adını kullanıyor.
2. WS-first handshake → login, sürümlü protokol/capability ve ileride anahtar anlaşması için genişleme noktaları. Güvenlik algoritması bu görevle otomatik seçilmez.
3. Promise/request correlation, hata kodları, CSRF/session/download köprüsü; UI'den bağımsız HTTP/WS adaptörü.
4. Geniş kapsam matrisi ve tip sözleşmeleri; options nesneleri, varsayılanlar, geriye uyum/contract testleri.
5. files.list(options): mevcut dizin için filter/sort/limit/cursor. Opsiyonel yeni alanlar eski çağrıyı bozmaz. Recursive search ayrı yöntem.
6. Server state snapshot/patch yansıması; managed/client mode, navigation getter/action, reconnect/refresh ve revision sırası.
7. Event + kalıcı condition observer; quota warning payload ve abonelikten çıkma. SDK DOM/popup çizmez.
8. Transfer chunk/ACK, progress, cancel, timeout ve durum/idempotency kontrolü. Kopmada mutasyon körlemesine replay edilmez.
9. Kararlı i18n kodları/parametreleri, capability ile desteklenmeyen feature hatası; güvenlik kararı değil UX.
10. JSDoc/examples, schema sürümleme ve eski custom UI ile uyum testleri.

```javascript
await api.files.list();
await api.files.list({
  filter: { name: 'report', extensions: ['pdf', 'txt'], type: 'file' },
  sort: { by: 'modifiedAt', direction: 'desc' },
  limit: 100,
  cursor: null,
});
```

Filtre SDK'da typed/şema ön kontrolü ve serialization; gerçek yetkili sonuç helper'da üretilir. Path normalize yardımcıları helper doğrulamasının yerine geçmez. DB/OS erişimi ve güvenlik politikası uygulaması SDK işi değildir.

## İkisinin arasında server'ın işleri

TLS/Origin/WS upgrade ve pre-auth sınırı; session lifecycle, kalıcı view-state, snapshot/olay yönlendirme, helper orchestration, config/reload, rate-limit ve authenticated HTTP download. Helper gerçekliği doğrular, server state'i saklar, SDK yansımayı sunar. Bunlar için [SERVER.md](SERVER.md), [STATE.md](STATE.md).

## Son açıklama önceliklidir — 2026-10-12

- SDK stateless çekirdeği ve isteğe bağlı stateful kolaylık katmanı sunar; önceki persistent server view-state önerisi uygulanmayacak. Server session/yetkiyi, helper gerçek dosya/path'i denetler. STATE.md güncellendi.
- Root/admin ayar ayrımı aktör ayrımıdır: Root tavanı/OS kimliği/critical startup; panel admini root'un devrettiği panel özellikleri ve kendi hesap kapsamı. Server süreci admin rolü değildir.
- Kullanıcının geniş SDK hedefi yalnızca geriye uyum değil, yaygın ihtiyaçları ilk yüzeyde kapsayarak custom UI'nin tekrar implement etmesini azaltmaktır. list filter/sort/pagination, search scope, cancel/progress, error/i18n ve state/event seçenekleri baştan kapsam matrisinde tanımlanır. Sonsuz geleceği öngörme garantisi verilmez.
- Ortak filter semantiği/presetleri SDK sözleşmesinde belgelenir; gerçek filtreleme helper'dadır. Optional yeni alanlar eski list davranışını korur; feature/capability keşfi SDK içinde standardize edilir.
- Handshake anahtarıyla ek mesaj şifreleme isteği ayrı değerlendirilir; anahtarın doğrulanması olmadan MITM iddiası yok. SECURITY.md sınırları geçerlidir.

## Son tercih API etkileri

Kullanıcı bazlı browser storage state adapter'ı sunulur; helper doğrulaması zorunludur. User/preferences API'si undo retention_hours seçimini server min/max ile birlikte sunmalıdır; UI üst sınırı tahmin etmez. Suspended/deleted hesap oturumları iptal edilir; sanal kullanıcı self-revoke yetkisi kazanmaz. Log/audit ekranı yalnızca kendi güvenli kayıtlarını gösterir; hidden path normal metadata/undo üzerinden görünmez.
