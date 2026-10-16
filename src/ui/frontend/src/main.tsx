import {createRoot} from 'react-dom/client';
import App from './App.tsx';
import './index.css';
import { language } from './i18n/index.ts';

document.documentElement.lang = language();

createRoot(document.getElementById('root')!).render(<App />);
