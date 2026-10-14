/**
 * TealDrive SDK v1 kullanım/JSDoc referansı; implementasyon /sdk/v1/tealdrive.js içindedir.
 * Kaynak: docs/intgr/SDK.md ve src/ui/sdk/client.d.ts. audit/undo için Root journal ayarı açık olmalıdır.
 * Import edilince ağ çağrısı veya dosya değişikliği yapmaz. Her örnek ayrı çağrılabilir;
 * mutasyon örnekleri gerçek SDK ile çalıştırılırsa veri değiştirir. Placeholder'ları değiştirin.
 *
 * Hedef SDK: import { createClient } from '/sdk/v1/tealdrive.js';
 * const API = createClient(); await API.handshake(); await API.auth.login(...);
 * handshake WS bağlantısını ve sürümlü protokolü başlatır; login aynı WS üzerinde yapılır.
 * Pre-auth kanalda yalnızca sınırlı handshake/auth; dosya/admin mesajları AUTH_REQUIRED.
 * Güncel handshake server isterse standart Noise NK kanalını TLS yanında başlatır; SDK uygulaması gömülü WASM kullanır.
 * Handshake tamamlanmadan ağ yöntemleri HANDSHAKE_REQUIRED verir; on/disconnect istisnadır.
 * Login olmadan dosya/yönetim yöntemleri AUTH_REQUIRED verir.
 * HTTPS/WSS zorunluluğu korunur; Noise yalnız WS payload kapsamındadır, REST/native download TLS kullanır. HTTP üzerinden dağıtılmış JS için güven iddiası yoktur.
 * TLS_REQUIRED, CONNECTION_FAILED, PROTOCOL_UNSUPPORTED transport hatalarıdır.
 * Ortak ağ hataları: CONNECTION_FAILED, SERVER_SHUTTING_DOWN, TIMEOUT, VALIDATION_FAILED.
 * Mutasyonlar ayrıca AUDIT_UNAVAILABLE veya OPERATION_STATUS_UNKNOWN verebilir.
 * Cevap kaybında aynı mutasyonu körlemesine tekrar etmeyin; idempotency/durum sorgusu gerekir.
 * createClient/connect tasarımının güncel önerisi: connect başlangıcını handshake içinde topla.
 *
 * @typedef {Object} FileRef
 * @property {string} rootId Oturumdan gelen kök kimliği; OS path'i değildir.
 * @property {string} path Köke göre relative yol; absolute path/.. yasaktır.
 * @typedef {Object} MutationResult
 * @property {string} operationId İşlem/audit korelasyon kimliği.
 * @property {string} [revision] İşlem sonucundaki güncel dosya revision'ı.
 * @typedef {Object} QuotaView
 * @property {number|null} limitBytes DB/OS kota limiti; bilinmiyorsa null.
 * @property {number|null} usedBytes Helper ölçümü; hata sıfır kullanım değildir.
 * @property {number} reservedBytes Devam eden işlem rezervasyonları; ölçümden ayrı.
 * @property {number|null} usagePercent Sonlu limit varsa kullanım yüzdesi.
 * @property {number} warningPercent Ayarlanabilir uyarı eşiği; örnek 80.
 * @property {boolean} warning Eşik aşılmış mı?
 * @property {string|null} measuredAt Helper ölçüm zamanı.
 * @property {boolean} stale Cache sonucu eski mi?
 * @typedef {Object} TealDriveError
 * @property {string} code Çeviri anahtarı/kararlı kod.
 * @property {Object} params Güvenli yapılandırılmış çeviri parametreleri; HTML değildir.
 * @property {string} requestId Teşhis/audit kimliği.
 * @property {boolean} retryable Otomatik mutasyon tekrarı izni anlamına gelmez.
 *
 * ServerInfo, Session, FileEntry, TrashEntry, AuditEvent, UndoCandidate, PolicyView,
 * AccountView, BudgetView hedef SDK'nın ayrıca yayımlayacağı veri tipleridir.
 * Revision/expectedState tokenleri opaque kabul edilir; güvenlik kararı helper'dadır.
 * Şifreleri kaynak kodu, console, localStorage/sessionStorage içine kaydetmeyin.
 */

export const examples = {
  // API.files.validatePath('/musics'); — Helper doğrulamalı ön kontrol; goPath tekrar doğrular.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, PERMISSION_DENIED. */
  validatePath: (API) => API.files.validatePath('/musics'),

  // API.files.goTo('/musics'); — Managed gezintiyi helper doğrulamasıyla değiştirir.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, PERMISSION_DENIED, STATE_CONFLICT. */
  goPath: (API) => API.files.goTo('/musics'),

  // API.files.getCurrentDirectory(); — Son doğrulanmış sanal path; stale state güncel disk garantisi değildir.
  /** @param {Object} API @returns {string} @throws {TealDriveError} STATE_NOT_READY. */
  currentDir: (API) => API.files.getCurrentDirectory(),

  // API.files.observeState(callback); — Mevcut snapshot hemen, sonraki değişiklikler geldikçe.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  observeState: (API) => API.files.observeState((state) => console.log(state.currentDirectory)),

  // API.files.observeQuotaWarning(callback); — Kalıcı condition; active=false gelince UI uyarıyı kaldırır.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  warningQuota: (API) => API.files.observeQuotaWarning((warning) => console.log(warning.active, warning.quota)),

  // API.files.on('quota.warning.entered', callback); — Eşiğin aşılma olayı; yüzde payload'da.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  userEvent: (API) => API.files.on('quota.warning.entered', (quota) => console.log(quota.usagePercent)),

  // API.handshake(); — Sunucu bağlantısını ve protokol/yetenek anlaşmasını başlatır. Giriş ve dosya çağrılarından önce gereklidir. TLS şifrelemesini kendisi oluşturmaz.
  /**
   * Sunucu bağlantısını ve protokol/yetenek anlaşmasını başlatır. Giriş ve dosya çağrılarından önce gereklidir. TLS şifrelemesini kendisi oluşturmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.handshake();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<ServerInfo>} Örnek çağrının sonucu.
   * @throws {TealDriveError} TLS_REQUIRED, PROTOCOL_UNSUPPORTED, CONNECTION_FAILED.
   */
  handshake: (API) => API.handshake(),

  // API.getInfo(); — Handshake ile edinilen kamusal sunucu ve API bilgilerini döndürür.
  /**
   * Handshake ile edinilen kamusal sunucu ve API bilgilerini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.getInfo();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<ServerInfo>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED.
   */
  getInfo: (API) => API.getInfo(),

  // API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds)); — Olay dinleyicisi ekler; dönen fonksiyon aboneliği kaldırır. quota.warning, session.revoked, connection.changed de desteklenir.
  /**
   * Olay dinleyicisi ekler; dönen fonksiyon aboneliği kaldırır. quota.warning, session.revoked, connection.changed de desteklenir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds));
   * @description SDK yöntemi parametreleri: event: string; callback: (payload: object) => void.
   * @returns {function(): void} Örnek çağrının sonucu.
   * @throws {TealDriveError} INVALID_EVENT.
   */
  on: (API) => API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds)),

  // API.disconnect(); — Bağlantıyı kapatır; oturumu sonlandırmaz. Tekrar handshake gerektirir.
  /**
   * Bağlantıyı kapatır; oturumu sonlandırmaz. Tekrar handshake gerektirir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.disconnect();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} CONNECTION_FAILED.
   */
  disconnect: (API) => API.disconnect(),

  // API.auth.getSession(); — Aktif oturum, kökler ve yetenekleri döndürür; oturum yoksa null.
  /**
   * Aktif oturum, kökler ve yetenekleri döndürür; oturum yoksa null.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.getSession();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<Session|null>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, CONNECTION_FAILED.
   */
  getSession: (API) => API.auth.getSession(),

  // API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" }); — Sunucunun seçtiği PAM veya web parolası yöntemiyle giriş yapar; plaintext parola kodda kalıcı tutulmaz.
  /**
   * Sunucunun seçtiği PAM veya web parolası yöntemiyle giriş yapar; plaintext parola kodda kalıcı tutulmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" });
   * @description SDK yöntemi parametreleri: options: { username: string, password: string }.
   * @returns {Promise<Session>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, INVALID_CREDENTIALS, RATE_LIMITED, ENROLLMENT_REQUIRED, AUTH_UNAVAILABLE.
   */
  login: (API) => API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" }),

  // API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" }); — İlk PAM doğrulamasından sonra tek kullanımlık kayıt yetkisi üretir; dosya erişim oturumu vermez. Proof ID browser'a verilmez; expiresAt epoch saniyesidir.
  /**
   * İlk PAM doğrulamasından sonra tek kullanımlık kayıt yetkisi üretir; dosya erişim oturumu vermez. Proof ID browser'a verilmez; expiresAt epoch saniyesidir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" });
   * @description SDK yöntemi parametreleri: options: { username: string, linuxPassword: string }.
   * @returns {Promise<{ enrollmentRequired: true, expiresAt: number }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, INVALID_CREDENTIALS, ALREADY_AUTHENTICATED, FEATURE_DISABLED, RATE_LIMITED.
   */
  beginEnrollment: (API) => API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" }),

  // API.auth.completeEnrollment({ webPassword: "<yeni web parolası>" }); — Web parolasını oluşturur; kayıt yetkisi tek kullanımlıktır. Aynı WS'de kalır; private proof sunucudadır. Başarıda HttpOnly session bağlanır.
  /**
   * Web parolasını oluşturur; kayıt yetkisi tek kullanımlıktır. Aynı WS'de kalır; private proof sunucudadır. Başarıda HttpOnly session bağlanır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.completeEnrollment({ webPassword: "<yeni web parolası>" });
   * @description SDK yöntemi parametreleri: options: { webPassword: string }.
   * @returns {Promise<Session>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, ENROLLMENT_REQUIRED, INVALID_ARGUMENT, INVALID_CREDENTIALS, CONNECTION_CLOSED.
   */
  completeEnrollment: (API) => API.auth.completeEnrollment({ webPassword: "<yeni web parolası>" }),

  // API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" }); — Web parolasını değiştirir ve eski oturum/biletleri iptal eder; yeniden giriş gerekir.
  /**
   * Web parolasını değiştirir ve eski oturum/biletleri iptal eder; yeniden giriş gerekir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" });
   * @description SDK yöntemi parametreleri: options: { currentPassword: string, newPassword: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_CREDENTIALS, PASSWORD_POLICY_DENIED, AUDIT_UNAVAILABLE, FEATURE_DISABLED.
   */
  changePassword: (API) => API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" }),

  // API.auth.logout(); — Mevcut oturumu sonlandırır; hesabın web erişimini kalıcı kapatmaz.
  /**
   * Mevcut oturumu sonlandırır; hesabın web erişimini kalıcı kapatmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.logout();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, CONNECTION_FAILED.
   */
  logout: (API) => API.auth.logout(),

  // API.auth.revokeSelf(); — Yalnızca gerçek Linux kullanıcısı: kendi web erişimini ve tüm oturum/biletlerini iptal eder; Linux hesabını silmez.
  /**
   * Yalnızca gerçek Linux kullanıcısının kendi erişimini iptal eder; sanal hesapta admin gerekir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.revokeSelf();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVOKE_NOT_ALLOWED, AUDIT_UNAVAILABLE.
   */
  revokeSelf: (API) => API.auth.revokeSelf(),

  // API.files.list({ rootId: "home", path: "", limit: 100, cursor: null }); — Kök içindeki dizini sayfalı listeler; yasaklı öğeleri göstermez.
  /**
   * Kök içindeki dizini sayfalı listeler; yasaklı öğeleri göstermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.list({ rootId: "home", path: "", limit: 100, cursor: null });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: FileEntry[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, NOT_FOUND, PERMISSION_DENIED.
   */
  list: (API) => API.files.list({ rootId: "home", path: "", limit: 100, cursor: null }),

  // API.files.stat({ rootId: "home", path: "projects/app/config.toml" }); — Dosya metadata, revision ve işlem yeteneklerini döndürür.
  /**
   * Dosya metadata, revision ve işlem yeteneklerini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.stat({ rootId: "home", path: "projects/app/config.toml" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<FileEntry>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, NOT_FOUND, PERMISSION_DENIED.
   */
  stat: (API) => API.files.stat({ rootId: "home", path: "projects/app/config.toml" }),

  // API.files.readText({ rootId: "home", path: "projects/app/.env" }); — Metni ve aynı içeriğe ait revision tokenini döndürür.
  /**
   * Metni ve aynı içeriğe ait revision tokenini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.readText({ rootId: "home", path: "projects/app/.env" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<{ content: string, revision: string, metadata: object, capabilities: object }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED, FILE_TOO_LARGE, INVALID_ENCODING, NOT_FOUND.
   */
  readText: (API) => API.files.readText({ rootId: "home", path: "projects/app/.env" }),

  // API.files.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "<readText sonucundaki 64 karakter revision>" }); — Beklenen sürüm hâlâ geçerliyse metni kaydeder; conflict durumunda üzerine yazmaz.
  /**
   * Beklenen sürüm hâlâ geçerliyse metni kaydeder; conflict durumunda üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "<readText sonucundaki 64 karakter revision>" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, content: string, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED, FILE_TOO_LARGE.
   */
  writeText: (API) => API.files.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "<readText sonucundaki 64 karakter revision>" }),

  // API.files.createFolder({ rootId: "home", path: "projects/new-app" }); — Yeni klasör oluşturur; mevcut hedef üzerine yazmaz.
  /**
   * Yeni klasör oluşturur; mevcut hedef üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.createFolder({ rootId: "home", path: "projects/new-app" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  createFolder: (API) => API.files.createFolder({ rootId: "home", path: "projects/new-app" }),

  // API.files.rename({ rootId: "home", path: "old.md", newName: "new.md" }); — Aynı dizinde ad değiştirir; newName yol değil tek dosya adıdır.
  /**
   * Aynı dizinde ad değiştirir; newName yol değil tek dosya adıdır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.rename({ rootId: "home", path: "old.md", newName: "new.md" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, newName: string, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_NAME, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED.
   */
  rename: (API) => API.files.rename({ rootId: "home", path: "old.md", newName: "new.md" }),

  // API.files.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" } }); — Dosya/klasörü taşır; kaynak, hedef ve alt içerik politikaları uygulanır.
  /**
   * Dosya/klasörü taşır; kaynak, hedef ve alt içerik politikaları uygulanır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" } });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  move: (API) => API.files.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" } }),

  // API.files.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" } }); — Dosya/klasörü kopyalar; yasaklı içerik dolaylı olarak okunamaz.
  /**
   * Dosya/klasörü kopyalar; yasaklı içerik dolaylı olarak okunamaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" } });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  copy: (API) => API.files.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" } }),

  // API.files.chmod({ rootId: "home", path: "notes.md", mode: 0o640 }); — Normal izin bitlerini değiştirir; özel bitler yasaktır. UI riskli seçimde önceden uyarır.
  /**
   * Normal izin bitlerini değiştirir; özel bitler yasaktır. UI riskli seçimde önceden uyarır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.chmod({ rootId: "home", path: "notes.md", mode: 0o640 });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, mode: number (0..0o777), expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_MODE, SPECIAL_BITS_DENIED, REVISION_CONFLICT, PERMISSION_DENIED.
   */
  chmod: (API) => API.files.chmod({ rootId: "home", path: "notes.md", mode: 0o640 }),

  // API.files.search({ rootId: "home", path: "projects", query: "config", limit: 100 }); — İzinli ağaçta dosya adı arar; yasaklı dizinlere girmez.
  /**
   * İzinli ağaçta dosya adı arar; yasaklı dizinlere girmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.search({ rootId: "home", path: "projects", query: "config", limit: 100 });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, query: string, limit?: number }.
   * @returns {Promise<{ items: FileEntry[], truncated: boolean }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, PERMISSION_DENIED, INVALID_QUERY.
   */
  search: (API) => API.files.search({ rootId: "home", path: "projects", query: "config", limit: 100 }),

  // API.files.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) }); — File/Blob transferini yürütür; başarı yalnızca publish onayından sonra döner.
  /**
   * File/Blob transferini yürütür; başarı yalnızca publish onayından sonra döner.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) });
   * @description SDK yöntemi parametreleri: options: { rootId: string, directory: string, file: File, signal?: AbortSignal, onProgress?: function }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FILE_TOO_LARGE, QUOTA_EXCEEDED, PERMISSION_DENIED, ABORTED, TRANSFER_FAILED.
   */
  upload: (API) => API.files.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) }),

  // API.files.download({ rootId: "home", path: "uploads/report.pdf" }); — Tarayıcı indirmesini başlatır; dosyanın tamamını JS belleğine almak zorunlu değildir. Sonuç kullanıcı diske kaydetti demek değildir.
  /**
   * Tarayıcı indirmesini başlatır; dosyanın tamamını JS belleğine almak zorunlu değildir. Sonuç kullanıcı diske kaydetti demek değildir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.download({ rootId: "home", path: "uploads/report.pdf" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, PERMISSION_DENIED, TRANSFER_FAILED.
   */
  download: (API) => API.files.download({ rootId: "home", path: "uploads/report.pdf" }),

  // API.files.archives.create({ rootId: "home", sources: ["notes"], path: "notes.zip", format: "zip" }); — İzinli içerikten arşiv oluşturur; yasaklı alt dosya sızdırmaz.
  /**
   * İzinli içerikten arşiv oluşturur; yasaklı alt dosya sızdırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.archives.create({ rootId: "home", sources: ["notes"], path: "notes.zip", format: "zip" });
   * @description SDK yöntemi parametreleri: options: { sources: FileRef[], destination: FileRef, format: "zip"|"tar.gz" }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, PERMISSION_DENIED, QUOTA_EXCEEDED, ALREADY_EXISTS.
   */
  archiveCreate: (API) => API.files.archives.create({ rootId: "home", sources: ["notes"], path: "notes.zip", format: "zip" }),

  // API.files.archives.extract({ rootId: "home", path: "notes.zip", destination: "extracted" }); — Arşivi hedefe güvenli açar; path escape ve kota denetlenir.
  /**
   * Arşivi hedefe güvenli açar; path escape ve kota denetlenir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.archives.extract({ rootId: "home", path: "notes.zip", destination: "extracted" });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, INVALID_ARCHIVE, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  archiveExtract: (API) => API.files.archives.extract({ rootId: "home", path: "notes.zip", destination: "extracted" }),

  // API.files.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" }); — Dosya/klasörü kendi çöp kutusuna taşır; kalıcı silmez.
  /**
   * Dosya/klasörü kendi çöp kutusuna taşır; kalıcı silmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, expectedRevision: string }.
   * @returns {Promise<{ operationId: string, trashId: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  trashMove: (API) => API.files.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" }),

  // API.files.trash.list({ rootId: "home", limit: 50, cursor: null }); — Kendi çöp kutusunu listeler; iç depo yolunu açmaz.
  /**
   * Kendi çöp kutusunu listeler; iç depo yolunu açmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.trash.list({ rootId: "home", limit: 50, cursor: null });
   * @description SDK yöntemi parametreleri: options: { rootId: string, limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: TrashEntry[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED.
   */
  trashList: (API) => API.files.trash.list({ rootId: "home", limit: 50, cursor: null }),

  // API.files.trash.restore({ rootId: "home", trashId: "trash_example" }); — Saklanan içerik ve güncel politika uygunsa geri yükler; dolu hedefin üzerine yazmaz.
  /**
   * Saklanan içerik ve güncel politika uygunsa geri yükler; dolu hedefin üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.trash.restore({ rootId: "home", trashId: "trash_example" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, trashId: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, ALREADY_EXISTS, REVISION_CONFLICT, PERMISSION_DENIED.
   */
  trashRestore: (API) => API.files.trash.restore({ rootId: "home", trashId: "trash_example" }),

  // API.files.trash.deletePermanently({ rootId: "home", trashId: "trash_example" }); — İzin verilmişse çöp kutusu içeriğini kalıcı siler.
  /**
   * İzin verilmişse çöp kutusu içeriğini kalıcı siler.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.trash.deletePermanently({ rootId: "home", trashId: "trash_example" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, trashId: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, PERMISSION_DENIED, NOT_FOUND.
   */
  trashDelete: (API) => API.files.trash.deletePermanently({ rootId: "home", trashId: "trash_example" }),

  // API.files.audit.list({ limit: 50, cursor: null, action: "chmod" }); — Yalnızca kendi audit olaylarını sayfalı sorgular; başka username/UID parametresi yoktur.
  /**
   * Yalnızca kendi audit olaylarını sayfalı sorgular; başka username/UID parametresi yoktur.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.audit.list({ limit: 50, cursor: null, action: "chmod" });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null, action?: string }.
   * @returns {Promise<{ events: AuditEvent[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_FILTER, AUDIT_UNAVAILABLE.
   */
  auditList: (API) => API.files.audit.list({ limit: 50, cursor: null, action: "chmod" }),

  // API.files.undo.getPreferences(); — null tercih sunucunun güncel üst sınırını izler.
  /** @returns {Promise<{hours:number|null,effectiveHours:number,maximumHours:number,revision:number}>} Saat ve CAS sürümü. */
  undoPreferences: (API) => API.files.undo.getPreferences(),
  // API.files.undo.setPreferences({ hours: 6, expectedRevision: preferences.revision }); — Yalnız kendi tercihini değiştirir; oturum iptal edilmez.
  /** @throws {TealDriveError} INVALID_ARGUMENT, CONFLICT, AUTH_REQUIRED, FEATURE_DISABLED. */
  setUndoPreferences: (API, preferences) => API.files.undo.setPreferences({ hours: Math.min(6, preferences.maximumHours), expectedRevision: preferences.revision }),

  // API.files.undo.list({ limit: 20, cursor: null }); — Geri alma adayları available/unavailableReason/expectedState ile döner.
  /**
   * Geri alma adayları available/unavailableReason/expectedState ile döner.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.undo.list({ limit: 20, cursor: null });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: UndoCandidate[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED.
   */
  undoList: (API) => API.files.undo.list({ limit: 20, cursor: null }),

  // API.files.undo.apply({ operationId: candidate.operation.id, expectedState: candidate.expectedState }); — Beklenen durumu helper uygulama anında tekrar doğrular; eski audit silinmez.
  /**
   * Beklenen durumu helper uygulama anında tekrar doğrular; eski audit silinmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.undo.apply({ operationId: candidate.operation.id, expectedState: candidate.expectedState });
   * @description SDK yöntemi parametreleri: options: { operationId: string, expectedState: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, REVISION_CONFLICT, CONFLICT, PERMISSION_DENIED, ALREADY_EXISTS.
   */
  undoApply: (API, candidate) => API.files.undo.apply({ operationId: candidate.operation.id, expectedState: candidate.expectedState }),

  // API.files.policy.get(); — Etkin kişisel/admin/root kural kaynaklarını güvenli biçimde döndürür.
  /**
   * Etkin kişisel/admin/root kural kaynaklarını güvenli biçimde döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.policy.get();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<PolicyView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED.
   */
  policyGet: (API) => API.files.policy.get(),

  // API.files.policy.addRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] }); — Yalnızca kişisel katmana kısıt ekler; yeni yetki vermez.
  /**
   * Yalnızca kişisel katmana kısıt ekler; yeni yetki vermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.policy.addRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] });
   * @description SDK yöntemi parametreleri: options: { path: string, recursive: boolean, deny: string[] }.
   * @returns {Promise<{ ruleId: string, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_RULE, AUDIT_UNAVAILABLE.
   */
  policyAdd: (API) => API.files.policy.addRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] }),

  // API.files.policy.removeRule({ ruleId: "personal_0", expectedRevision: "current_generation" }); — Kendi kuralını kaldırır; üst katman yasakları geçerli kalır.
  /**
   * Kendi kuralını kaldırır; üst katman yasakları geçerli kalır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.policy.removeRule({ ruleId: "personal_0", expectedRevision: "current_generation" });
   * @description SDK yöntemi parametreleri: options: { ruleId: string, expectedRevision: string }.
   * @returns {Promise<{ revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED, REVISION_CONFLICT, AUDIT_UNAVAILABLE.
   */
  policyRemove: async (API) => { const policy = await API.files.policy.get(); return API.files.policy.removeRule({ ruleId: policy.rules.personalRules[0].ruleId, expectedRevision: policy.revision }); },

  // API.files.quota.get(); — Kota DB kayıtlarından, gerçek kullanım helper ölçümünden gelir; eşik örneğin yüzde 80.
  /**
   * Kota DB kayıtlarından, gerçek kullanım helper ölçümünden gelir; eşik örneğin yüzde 80.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.files.quota.get();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<QuotaView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, QUOTA_MEASUREMENT_UNAVAILABLE.
   */
  quotaGet: (API) => API.files.quota.get(),

  // API.admin.settings.get(); — Panelin görebildiği config snapshot döner; kritik alanları değiştirmek server tarafından reddedilir.
  /**
   * Panelin görebildiği config snapshot döner; kritik alanları değiştirmek server tarafından reddedilir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.settings.get();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<{ settings: object, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED.
   */
  settingsGet: (API) => API.admin.settings.get(),

  // API.admin.settings.patch({ changes: { quota_warning_percent: 80 } }); — Yalnızca izinli çalışma ayarlarını tavan içinde günceller.
  /**
   * Yalnızca izinli çalışma ayarlarını tavan içinde günceller.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.settings.patch({ changes: { quota_warning_percent: 80 } });
   * @description SDK yöntemi parametreleri: options: { expectedRevision: string, changes: object }.
   * @returns {Promise<{ settings: object, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, CRITICAL_SETTING_DENIED, REVISION_CONFLICT, VALIDATION_FAILED.
   */
  settingsPatch: (API) => API.admin.settings.patch({ changes: { quota_warning_percent: 80 } }),

  // API.admin.systemUsers.list(); — Servis/sistem kullanıcıları filtrelenmiş yerel adayları döndürür; erişim açmaz.
  /**
   * @param {Object} API Handshake ve login tamamlanmış SDK istemcisi.
   * @returns {Promise<UserCandidates>} {users:[{username,uid,gid}],truncated}; en çok1024 yerel aday.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED, SERVICE_UNAVAILABLE.
   * @example API.admin.systemUsers.list();
   */
  adminUsersList: (API) => API.admin.systemUsers.list(),

  // API.admin.systemUsers.accounts(); — Kayıtlı Linux hesaplarını ve web erişim durumlarını döndürür.
  /**
   * @param {Object} API Yetkili global Linux admin SDK istemcisi.
   * @returns {Promise<AccountMetadata[]>} Password hash/grant yok; passwordEnrolled kayıt tamamlanmasını gösterir.
   * @throws {TealDriveError} FEATURE_DISABLED, AUTH_REQUIRED, PERMISSION_DENIED, SERVICE_UNAVAILABLE.
   * @example API.admin.systemUsers.accounts();
   */
  adminUsersAccounts: (API) => API.admin.systemUsers.accounts(),

  // API.admin.systemUsers.approve({username:"example-user"}); — İlk PAM kaydı için mevcut Linux kullanıcısını onaylar.
  /**
   * Kullanıcı PAM ile Linux kimliğini doğrular ve web parolasını kendisi oluşturur.
   * Root deny üstün; admin UID/home/role/parola seçmez. Var olan hesapta409: allow/deny kullanılır.
   * @param {Object} API Root roster + DB Linux/Admin yetkisindeki SDK istemcisi.
   * @returns {Promise<AccountMetadata>} role:"user", passwordEnrolled:false.
   * @throws {TealDriveError} FEATURE_DISABLED, INVALID_ARGUMENT, AUTH_REQUIRED, PERMISSION_DENIED, CONFLICT, SERVICE_UNAVAILABLE.
   * @example API.admin.systemUsers.approve({username:"example-user"});
   */
  adminUsersApprove: (API) => API.admin.systemUsers.approve({username:"example-user"}),

  // API.admin.systemUsers.allow({accountId:"account-uuid"}); — Mevcut hesabın girişini Root kuralları içinde yeniden açar.
  /**
   * Eski grant/cookie/PAM proof geri açılmaz; yeni giriş veya yeni PAM kanıtı gerekir.
   * @param {Object} API Yetkili global Linux admin SDK istemcisi.
   * @returns {Promise<AccountMetadata>} Yeni generation ve erişim durumu.
   * @throws {TealDriveError} FEATURE_DISABLED, AUTH_REQUIRED, PERMISSION_DENIED, CONFLICT, SERVICE_UNAVAILABLE.
   * @example API.admin.systemUsers.allow({accountId:"account-uuid"});
   */
  adminUsersAllow: (API) => API.admin.systemUsers.allow({accountId:"account-uuid"}),

  // API.admin.systemUsers.deny({accountId:"account-uuid"}); — Web erişimini askıya alır; aktif grant/proof iptal olur.
  /**
   * Linux hesabı ve dosyalar tutulur; role/own-admin değişikliği bu API'den yapılmaz.
   * @param {Object} API Yetkili global Linux admin SDK istemcisi.
   * @returns {Promise<AccountMetadata>} status:"suspended", loginEnabled:false, yeni generation.
   * @throws {TealDriveError} FEATURE_DISABLED, AUTH_REQUIRED, PERMISSION_DENIED, CONFLICT, SERVICE_UNAVAILABLE.
   * @example API.admin.systemUsers.deny({accountId:"account-uuid"});
   */
  adminUsersDeny: (API) => API.admin.systemUsers.deny({accountId:"account-uuid"}),

  // API.admin.virtualUsers.list(); — Yalnızca yönettiği sanal alt hesapları listeler.
  /**
   * Yalnızca yönettiği sanal alt hesapları listeler.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.list();
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null }.
   * @returns {Promise<AccountMetadata[]>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED.
   */
  virtualList: (API) => API.admin.virtualUsers.list(),

  // API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, password: "<kullanıcıdan alınan ilk web parolası>", idempotencyKey: crypto.randomUUID() }); — Ayrı locked/nologin UID/GID ve ilk web parolasıyla User alt hesap sağlar; admin oluşturamaz.
  /**
   * Ayrı locked/nologin UID/GID ve ilk web parolasıyla User alt hesap sağlar; admin oluşturamaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, password: "<kullanıcıdan alınan ilk web parolası>", idempotencyKey: crypto.randomUUID() });
   * @description SDK yöntemi parametreleri: options: { name: string, password: string, quotaBytes: number, idempotencyKey?: UUID }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, QUOTA_BUDGET_EXCEEDED, INVALID_NAME, AUDIT_UNAVAILABLE, PROVISIONING_FAILED.
   */
  virtualCreate: (API) => API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, password: "<kullanıcıdan alınan ilk web parolası>", idempotencyKey: crypto.randomUUID() }),

  // API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedGeneration: 1 }); — Admin bütçesi içinde sanal alt hesabın kotasını değiştirir; root havuzunu değiştirmez.
  /**
   * Admin bütçesi içinde sanal alt hesabın kotasını değiştirir; root havuzunu değiştirmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedGeneration: 1 });
   * @description SDK yöntemi parametreleri: options: { accountId: string, quotaBytes: number, expectedRevision: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, QUOTA_BUDGET_EXCEEDED, QUOTA_BELOW_USAGE, REVISION_CONFLICT.
   */
  virtualQuota: (API) => API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedGeneration: 1 }),

  // API.admin.virtualUsers.disable({ accountId: "acct_customer" }); — Sanal alt hesabı kapatır ve oturumlarını iptal eder; veriyi silmez.
  /**
   * Sanal alt hesabı kapatır ve oturumlarını iptal eder; veriyi silmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.disable({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  virtualDisable: (API) => API.admin.virtualUsers.disable({ accountId: "acct_customer" }),

  // API.admin.virtualUsers.enable({ accountId: "acct_customer" }); — Kendi alt hesabını root engeli yoksa etkinleştirir; eski biletleri canlandırmaz.
  /**
   * Kendi alt hesabını root engeli yoksa etkinleştirir; eski biletleri canlandırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.enable({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  virtualEnable: (API) => API.admin.virtualUsers.enable({ accountId: "acct_customer" }),

  // API.admin.policy.get({ accountId: "acct_customer" }); — Atanmış kapsamdaki hesabın güvenli politika görünümünü döndürür.
  /**
   * Atanmış kapsamdaki hesabın güvenli politika görünümünü döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.get({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<PolicyView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED.
   */
  adminPolicyGet: (API) => API.admin.policy.get({ accountId: "acct_customer" }),

  // API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] }); — Sadece admin katmanına kural ekler; root tavanını değiştirmez.
  /**
   * Sadece admin katmanına kural ekler; root tavanını değiştirmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] });
   * @description SDK yöntemi parametreleri: options: { accountId: string, path: string, recursive: boolean, deny: string[] }.
   * @returns {Promise<{ ruleId: string, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, INVALID_RULE, AUDIT_UNAVAILABLE.
   */
  adminPolicyAdd: (API) => API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] }),

  // API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: "admin_0", expectedRevision: "current_generation" }); — Yönetmeye yetkili olduğu admin kuralını kaldırır; root/kullanıcı kurallarını kaldırmaz.
  /**
   * Yönetmeye yetkili olduğu admin kuralını kaldırır; root/kullanıcı kurallarını kaldırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: "admin_0", expectedRevision: "current_generation" });
   * @description SDK yöntemi parametreleri: options: { accountId: string, ruleId: string, expectedRevision: string }.
   * @returns {Promise<{ revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, REVISION_CONFLICT, AUDIT_UNAVAILABLE.
   */
  adminPolicyRemove: async (API) => { const policy = await API.admin.policy.get({ accountId: "acct_customer" }); return API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: policy.rules.adminRules[0].ruleId, expectedRevision: policy.revision }); },

  // API.admin.quota.getBudget(); — Root atanmış bütçe, alt hesap tahsisleri ve helper kullanım özetini döndürür.
  /**
   * Root atanmış bütçe, alt hesap tahsisleri ve helper kullanım özetini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.quota.getBudget();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<BudgetView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, QUOTA_MEASUREMENT_UNAVAILABLE.
   */
  adminBudget: (API) => API.admin.quota.getBudget(),


  // API.files.createText({rootId:"home",path:"notes.txt",content:"Yeni içerik"}); — Yalnız yeni dosya; mevcut adı ezmez.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} ALREADY_EXISTS, POLICY_DENIED, DISK_FULL */
  createText: API => API.files.createText({rootId:"home",path:"notes.txt",content:"Yeni içerik"}),
  // API.files.refreshState(); — Kayıtlı dizini helper ile tekrar doğrular; silindiyse en yakın üst dizine gider.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} AUTH_REQUIRED, STATE_DISABLED */
  refreshState: API => API.files.refreshState(),
  // API.files.getState(); — Immutable UI snapshot; stale=true güncel disk garantisi değildir.
  /** @param {Object} API @returns {Object} */
  getState: API => API.files.getState(),
  // API.files.getDownloadUrl({rootId:"home",path:"large.zip"}); — Native browser indirme URL'si; secret query'ye konmaz.
  /** @param {Object} API @returns {string} @throws {TealDriveError} INVALID_PATH, INVALID_ROOT, AUTH_REQUIRED */
  downloadUrl: API => API.files.getDownloadUrl({rootId:"home",path:"large.zip"}),
  // API.files.getPreviewUrl({rootId:"home",path:"photo.jpg"}); — Media element URL'si; erişim/size/type backend'de denetlenir.
  /** @param {Object} API @returns {string} @throws {TealDriveError} INVALID_PATH, INVALID_ROOT, AUTH_REQUIRED */
  previewUrl: API => API.files.getPreviewUrl({rootId:"home",path:"photo.jpg"}),
  // API.admin.settings.inspect(); — {config, mode} döner; mode=split Root ceiling/runtime ayrımıdır.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} PERMISSION_DENIED */
  settingsInspect: API => API.admin.settings.inspect(),
  // API.admin.settings.reload(); — Root policy ve runtime overlay doğrulanarak yeniden okunur; Root dosyasına yazmaz.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} PERMISSION_DENIED, VALIDATION_FAILED */
  settingsReload: API => API.admin.settings.reload(),
  // API.admin.virtualUsers.delete({accountId:"ACCOUNT_ID",expectedGeneration:1}); — Root izinliyse retained delete; OS purge Root lifecycle işidir.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} FEATURE_DISABLED, CONFLICT, PERMISSION_DENIED */
  virtualDelete: API => API.admin.virtualUsers.delete({accountId:"ACCOUNT_ID",expectedGeneration:1}),

  // API.admin.systemUsers.accountsPage({limit:100,filter:"example",status:"active"}); — Kayıtlı sistem hesaplarını filtreleyip sayfalar; yetki sunucuda doğrulanır.
  /** @param {Object} API @returns {Promise<{items:Array,nextCursor:string|null,hasMore:boolean}>} @throws {TealDriveError} FEATURE_DISABLED, PERMISSION_DENIED, INVALID_ARGUMENT */
  systemAccountsPage: API => API.admin.systemUsers.accountsPage({limit:100,filter:"example",status:"active"}),
  // API.admin.virtualUsers.listPage({limit:100,cursor:"LAST_USERNAME",enrolled:true}); — Admin yalnız kendi hesaplarını görür; dönen nextCursor ile sonraki sayfa alınır.
  /** @param {Object} API @returns {Promise<{items:Array,nextCursor:string|null,hasMore:boolean}>} @throws {TealDriveError} FEATURE_DISABLED, PERMISSION_DENIED, INVALID_ARGUMENT */
  virtualAccountsPage: API => API.admin.virtualUsers.listPage({limit:100,enrolled:true}),
  // API.files.audit.list({limit:100,status:"failed",path:"projects/",from:0,to:Math.floor(Date.now()/1000)}); — Kendi geçmişi; güncel helper politikası, source/target filtresi ve saniye cinsinden zaman aralığı.
  /** @param {Object} API @returns {Promise<{items:Array,nextCursor:string|null,hasMore:boolean}>} @throws {TealDriveError} FEATURE_DISABLED, AUTH_REQUIRED, INVALID_ARGUMENT */
  auditFilteredPage: API => API.files.audit.list({limit:100,status:"failed",path:"projects/",from:0,to:Math.floor(Date.now()/1000)}),
  // API.files.audit.list({requestId:operationResult.requestId}); — İşlem yanıtının isteğini korumalı helper kaydıyla eşleştirir; başka hesaba erişim vermez.
  /** @param {Object} API @param {{requestId:string}} operationResult @returns {Promise<Object>} @throws {TealDriveError} FEATURE_DISABLED, INVALID_ARGUMENT */
  auditForRequest: (API, operationResult) => API.files.audit.list({requestId:operationResult.requestId}),

  // API.files.audit.list({requestId: response.headers.get("x-tealdrive-request-id")}); — HTTP indirme/önizleme yanıtını own helper kaydıyla ilişkilendirir; baytlar helper teslimidir.
  /**
   * Akış tüketildikten sonra toplam helper teslimini sorgulayın; tarayıcı diske kaydetme kanıtı değildir.
   * @param {Object} API Handshake/login tamamlanmış SDK.
   * @param {Response} response API.files.download sonucu; response body ayrı tüketilir.
   * @returns {Promise<Object>} Kendi audit sayfası.
   * @throws {TealDriveError} FEATURE_DISABLED, INVALID_ARGUMENT, AUTH_REQUIRED.
   */
  auditForDownload: (API, response) => API.files.audit.list({requestId:response.headers.get("x-tealdrive-request-id")}),

  // API.files.deletePermanently({rootId:"home",path:"obsolete.bin"}); — Canlı path'i geri alınamaz siler; Root izin/politika şartı vardır.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} FEATURE_DISABLED, POLICY_DENIED, NOT_FOUND */
  deleteLivePath: API => API.files.deletePermanently({rootId:"home",path:"obsolete.bin"}),

};


/**
 * SDK1.1: encrypted WS, reconnect ve public UUID örnekleri.
 * Bu fonksiyon çağrılmadan ağ veya dosya işlemi yapmaz.
 * @param {import('./src/ui/sdk/client.js').TealDriveClient} API
 * @param {File} file
 * @param {AbortSignal} signal
 * @example await transferExamples(API, selectedFile, controller.signal);
 * @throws {TealDriveError} QUOTA_EXCEEDED, RESUME_FILE_MISMATCH, ABORTED,
 * UPLOAD_COMMIT_UNCONFIRMED (yayın gerçekleşmiş olabilir; otomatik tekrar etmeyin).
 */
export async function transferExamples(API, file, signal) {
  let resumeTransferId;
  try {
    await API.files.upload({directory:'/', file, signal, resumable:true,
      onTransfer: transfer => { resumeTransferId=transfer.transferId; },
      onProgress: progress => console.log(progress.transferredBytes, progress.totalBytes)});
  } catch (error) {
    if (!error.params?.resumeTransferId) throw error;
    await API.handshake();
    await API.files.upload({directory:'/', file, signal, resumeTransferId:error.params.resumeTransferId});
  }
  // İptal gerekirse: await API.files.abortUpload({transferId:resumeTransferId});
  // Ticket yalnız aynı HttpOnly session ve server'ın bekletme penceresinde geçerlidir.
}
