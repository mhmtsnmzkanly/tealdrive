import { createClient } from '../sdk/client.ts';

const prodUrl =
  (typeof window !== 'undefined' && (window as any).__TEALDRIVE_PROD_URL__) ||
  import.meta.env.VITE_TEALDRIVE_URL ||
  '';

export const prodInstance = createClient({ baseUrl: prodUrl, encryption: 'auto' });
export const tealdrive = prodInstance;
export type TealDrive = typeof tealdrive;
