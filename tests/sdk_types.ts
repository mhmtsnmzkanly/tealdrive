// Compile-only public SDK consumer; no server or credentials are needed.
import {createClient, type ServerInfo, type AdminBudget} from '../src/ui/sdk/client.js';
async function consumer(publicKey: string, file: File): Promise<void> {
  const api=createClient({encryption:'required',trustedServerKeys:[publicKey]});
  const info:ServerInfo=await api.handshake();
  const pinTrust:'pinned'|'tls-discovery'|'tls'=info.transport.keyTrust;
  const encrypted:boolean=info.transport.encryptedWebSocket;
  const budget:AdminBudget=await api.admin.quota.getBudget();
  const own:number|null|undefined=budget.ownQuotaBytes;
  const mode:'fixed'|'remainder'|undefined=budget.ownQuotaMode;
  await api.files.list({path:'/',filter:{extensions:['js','ts'],type:'file'},sort:{by:'name',direction:'asc'}});
  await api.files.upload({directory:'/',file});
  void [pinTrust,encrypted,own,mode];
  api.disconnect();
}
void consumer;
// @ts-expect-error only the declared transport policies are accepted
createClient({encryption:'fallback-to-plaintext'});
// @ts-expect-error public pins must be strings
createClient({trustedServerKeys:[123]});
