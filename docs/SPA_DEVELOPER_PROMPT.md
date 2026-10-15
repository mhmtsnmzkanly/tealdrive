# TealDrive NextGen Single Page Application (SPA) Developer Prompt & Specification

> **Target Audience:** Frontend Architects, Senior SPA Developers (React / Next.js / Vue 3 / Svelte 5 / Nuxt / Vite), and AI Coding Agents.  
> **Mission:** Build a world-class, ultra-modern, responsive, high-performance Web UI for TealDrive that completely supersedes the legacy server-rendered table interface.

---

## 1. Project Overview & Vision

TealDrive is an enterprise-grade, high-security cloud and edge personal storage server powered by a high-throughput Rust backend, Noise-protocol encrypted WebSockets, and granular UNIX/virtual filesystem permissions.

The current interface is functional but dated. Your goal is to build an SPA experience on par with **Google Drive, Nextcloud Hub, and Linear**—featuring:
- Fluid drag-and-drop file management.
- Multi-view file canvas (Icon Grid with thumbnail previews vs. High-density Table).
- Keyboard-first navigation (`Cmd+K` Omnisearch, Arrow keys, `Space` for QuickLook, `F2` for Rename).
- Deep integration with TealDrive's unique features:
  - **Live Storage Quota Tracking** with proactive threshold warnings.
  - **Single-Click Operation Undo** powered by server-side audit logs.
  - **In-App Code & Text Editor** with optimistic lock revision protection (`expectedRevision`).
  - **UNIX Chmod / Permissions Inspector & Editor**.
  - **Virtual & System User Management** for administrators.

---

## 2. Fast Prototyping with the Mock SDK

You can develop, test, and polish the **entire application locally** without running the Rust backend or configuring TLS/Noise encryption.

TealDrive includes a standalone in-memory Mock SDK located at:
```
src/ui/sdk/mock-client.js
```
The Mock SDK implements 100% of the methods, events, and data structures defined in `src/ui/sdk/client.d.ts`.

### Switching Between Mock and Production

In your SPA project, create a single abstraction file (e.g. `src/lib/tealdrive.ts`):

```typescript
import { createClient as createProdClient } from "../sdk/client.js";
import { createMockClient } from "../sdk/mock-client.js";

const USE_MOCK = import.meta.env.VITE_USE_MOCK_SDK === "true" || process.env.NODE_ENV !== "production";

export const tealdrive = USE_MOCK
  ? createMockClient({ latency: 40, persist: true })
  : createProdClient({ encryption: "auto" });
```

---

## 3. Recommended Technology Stack

| Layer | Recommended Choice | Alternatives |
| :--- | :--- | :--- |
| **Framework** | **React 19 / Vite** or **Next.js 15 (App Router)** | Vue 3 + Vite, Svelte 5 + SvelteKit |
| **Styling** | **Tailwind CSS v4** + CSS Variables | Tailwind v3 + `@tailwindcss/typography` |
| **Icons** | **Lucide Icons** (`lucide-react` / `lucide-vue-next`) | Tabler Icons, Radix Icons |
| **Component Primitives** | **Radix UI** or **Shadcn UI** | Headless UI, Melt UI (Svelte) |
| **Server State** | **TanStack Query (React Query)** v5 | SWR, Pinia (Vue) |
| **Client State / Dialogs**| **Zustand** | React Context, Nanostores |
| **Code Editor** | **CodeMirror 6** or **Monaco Editor** | Ace Editor |
| **Drag & Drop** | `@hello-pangea/dnd` or native HTML5 Drag and Drop API | `dnd-kit` |

---

## 4. UI/UX Design System & Tokens

### Color Palette (The "TealDrive" Aesthetic)
- **Primary Brand / Teal**:
  - Light Accent: `#0d9488` (`teal-600`)
  - Dark Accent: `#14b8a6` (`teal-500`)
  - Accent Soft Glow: `rgba(20, 184, 166, 0.15)`
- **Neutrals (Dark Mode First)**:
  - Background Base: `#090d16` (Deep Obsidian)
  - Surface Card: `#0f172a` (Slate 900)
  - Surface Elevated / Modals: `#1e293b` (Slate 800)
  - Borders: `#334155` (`slate-700` / 50%)
  - Text Primary: `#f8fafc` (`slate-50`)
  - Text Secondary: `#94a3b8` (`slate-400`)
- **Status Colors**:
  - Success: `#10b981` (Emerald)
  - Warning: `#f59e0b` (Amber)
  - Danger / Quota Full: `#ef4444` (Rose / Red)
  - Info / Transfer: `#3b82f6` (Sky / Blue)

### Layout Wireframe
```
+-----------------------------------------------------------------------------------------+
| [LOGO TealDrive]   [  Search files & folders (Cmd+K)  ]    [View Toggle] [Quota] [User] |
+-----------------------------------------------------------------------------------------+
| [ + New Upload / Folder ] | Path: Home > Documents > Reports                [Filter]    |
|                           +-------------------------------------------------------------+
| * My Drive                |  [ ] Name           Size       Modified       Perms  [...]  |
| * Recent                  |  ---------------------------------------------------------  |
| * Starred                 |  [D] Financial/     --         10 mins ago    0755   [...]  |
| * Trash                   |  [F] Q3_Report.pdf  2.4 MB     Yesterday      0644   [...]  |
| * Audit & Undo            |  [F] Roadmap.docx   512 KB     3 days ago     0644   [...]  |
|                           |                                                             |
| --- Admin ---             |                                                             |
| * Virtual Users           |                                                             |
| * System Users            |                                                             |
|                           +-------------------------------------------------------------+
| Quota: [=====    ] 5.4%   |  [Upload Manager Dock: "Uploading 2 files (45%)..."]        |
+-----------------------------------------------------------------------------------------+
```

---

## 5. Functional Requirements & Implementation Guide

### 5.1 Application Shell & Navigation

1. **Collapsible Sidebar**:
   - Primary action: Giant `+ New` button with dropdown:
     - `Upload Files` (triggers file picker).
     - `Upload Folder` (`webkitdirectory`).
     - `New Folder` (opens modal).
     - `New Text Document` (opens in-app editor).
   - Navigation links:
     - **My Drive** (`/`): Main file explorer.
     - **Recent**: Filtered by recently modified.
     - **Trash**: Soft-deleted files list with restore & permanent delete.
     - **Audit & Undo Timeline**: Audit history of changes with instant undo buttons.
     - **Admin Console** (conditional on `session.isAdmin`): Virtual Users, Linux PAM System Users, Storage Budget, Server Settings.
   - **Storage Quota Widget** (docked at the bottom of the sidebar):
     - Interactive progress bar showing `usedBytes / limitBytes` (e.g. `245 MB of 10 GB used (2.4%)`).
     - Subscribe to real-time quota warnings using `api.files.observeQuotaWarning(callback)`.
     - Alert banner when quota exceeds 80% (Warning) or 95% (Critical).

2. **Top App Bar**:
   - **Breadcrumb Navigation**:
     - Dynamic segment buttons (`Home / Documents / Projects`).
     - Clicking any segment calls `api.files.goTo(path)`.
     - Dragging a file into a breadcrumb folder moves the file there!
   - **Omnisearch (`Cmd+K` / `Ctrl+K`)**:
     - Modal search bar providing instant fuzzy results as the user types.
     - Highlights matching query substring.
     - Direct jump or preview on click.
   - **View Switcher & Density**:
     - Toggle between **Grid View** (cards with rich icon/thumbnail) and **Table View** (columns: Name, Size, Modified Date, Mode, Actions).
     - Sort dropdown: Name (A-Z, Z-A), Size (Small-Large, Large-Small), Modified Date (Newest, Oldest).
     - Category filter pills: `All`, `Folders`, `Documents`, `Images`, `Media`, `Code`, `Archives`.
   - **User Menu**:
     - Displays current username, UID, and role (`Admin` badge).
     - Actions: Change Password, Toggle Dark/Light Mode, Logout.

---

### 5.2 File Explorer & Interactivity

1. **File / Folder Canvas**:
   - **Drag & Drop Upload Target**: Dragging files from the desktop anywhere over the canvas displays a sleek frosted glass drop overlay ("Drop files to upload to /Current/Directory").
   - **Multi-Selection**:
     - Click to select single file.
     - `Ctrl+Click` / `Cmd+Click` to toggle individual items.
     - `Shift+Click` for range selection.
     - Drag selection box (lasso) across items.
     - Batch actions toolbar appears upon selection: `Download Zip`, `Move`, `Delete`.
   - **Context Menu** (Right-click or 3-dots button):
     - `Open` / `Preview` (Spacebar).
     - `Download` (`api.files.download({path})` or `getDownloadUrl`).
     - `Rename` (F2 shortcut).
     - `Move To...` (Opens destination tree picker).
     - `Copy To...`.
     - `Edit Permissions (chmod)` (Opens UNIX permission modal).
     - `Move to Trash` (Soft delete).
     - `Delete Permanently` (Red confirm dialog).

2. **In-App Previewers & Modals**:
   - **Images & Photos**: High-resolution lightbox with zoom in/out, rotate, and full-screen view.
   - **Videos & Audio**: Custom HTML5 media player with seekbar and playback speed controls.
   - **Code & Text Documents**:
     - Fullscreen / drawer modal with syntax highlighting (JavaScript, TypeScript, Rust, Python, Markdown, JSON, HTML, CSS, Shell).
     - Direct in-browser editing.
     - **Optimistic Locking**: Call `api.files.readText({path})` to fetch content and `revision`. When saving with `api.files.writeText({path, content, expectedRevision})`, handle `REVISION_CONFLICT` gracefully by warning the user if the file was modified concurrently.
   - **PDF Viewer**: Embedded native PDF preview with page navigation and download button.
   - **Permissions (chmod) Modal**:
     - Visual 3x3 matrix: Read, Write, Execute for Owner, Group, Others.
     - Live octal calculation (e.g. `0755`, `0644`).
     - Calls `api.files.chmod({path, mode})`.

---

### 5.3 Upload Manager (Floating Dock)

- When an upload starts via file picker or drag-and-drop, spawn a minimized dock in the bottom-right corner.
- Call `api.files.upload({directory, file, onProgress, onTransfer, signal})`.
- The dock shows:
  - List of active files.
  - Overall progress bar and percentage.
  - Transfer speed and remaining time.
  - Cancel (`AbortController.abort()`) button per transfer.
  - Completed checkmarks with sound / micro-animation.

---

### 5.4 Audit History & Instant "Undo" System

TealDrive records every mutating operation in its server-side audit engine.
- Build an **Audit & Undo** page:
  - Fetches audit items via `api.files.undo.list()`.
  - Displays a visual timeline of recent changes:
    - `Renamed "report.txt" to "final_report.txt" (5 minutes ago)`
    - `Moved "banner.png" to /Media (12 minutes ago)`
    - `Edited "Meeting_Notes.txt" (1 hour ago)`
  - Each candidate has an **Undo** button.
  - Clicking **Undo** invokes `api.files.undo.apply({ operationId })`, rolling back the filesystem change instantly.

---

### 5.5 Admin Console (For Virtual & System Users)

Available if `session.isAdmin === true`:
1. **Virtual Users Tab**:
   - Table of virtual users (`john_doe`, `sarah_connor`, etc.).
   - "Create User" dialog: Username, Password, and Storage Quota (e.g. 5 GB). Calls `api.admin.virtualUsers.create()`.
   - Actions: Edit Quota (`setQuota`), Suspend/Activate (`disable`/`enable`), Delete.
2. **System Linux Users Tab**:
   - Lists candidate Linux accounts on the host machine.
   - Approval button for first-time PAM web login enrollment (`api.admin.systemUsers.approve()`).
3. **Storage Budget Tab**:
   - Visual breakdown of disk space allocated to virtual users vs. system reserve via `api.admin.quota.getBudget()`.
4. **Server Settings Tab**:
   - Configuration viewer and patch inspector via `api.admin.settings.inspect()`.

---

## 6. Ready-to-Use SDK Hook Examples (React)

### Initializing and Subscribing to State

```typescript
// src/hooks/useTealDrive.ts
import { useEffect, useState } from "react";
import { tealdrive } from "../lib/tealdrive";
import type { ClientState, Session } from "../sdk/client";

export function useTealDrive() {
  const [state, setState] = useState<ClientState>(tealdrive.getState());
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // 1. Initial handshake & session check
    tealdrive.handshake().then(() => {
      tealdrive.auth.getSession().then((sess) => {
        setSession(sess);
        setLoading(false);
      });
    });

    // 2. Observe state changes
    const unsubState = tealdrive.observeState((nextState) => {
      setState(nextState);
    });

    return () => {
      unsubState();
    };
  }, []);

  return { tealdrive, state, session, loading };
}
```

### Directory Listing & Navigation Hook

```typescript
// src/hooks/useDirectory.ts
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { tealdrive } from "../lib/tealdrive";

export function useDirectory(path: string) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["files", path],
    queryFn: () => tealdrive.files.list({ path }),
  });

  const createFolder = useMutation({
    mutationFn: (newFolderName: string) =>
      tealdrive.files.createFolder({ path: `${path}/${newFolderName}` }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["files", path] }),
  });

  const deleteItem = useMutation({
    mutationFn: (targetPath: string) =>
      tealdrive.files.trash.move({ path: targetPath }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["files", path] }),
  });

  return {
    entries: query.data?.entries || [],
    isLoading: query.isLoading,
    error: query.error,
    createFolder: createFolder.mutateAsync,
    deleteItem: deleteItem.mutateAsync,
  };
}
```

---

## 7. Deliverables & Definition of Done

1. **SPA Codebase Structure**:
   - Organized cleanly with reusable components (`/components/ui`, `/components/explorer`, `/components/modals`, `/components/admin`).
   - Strict TypeScript definitions matching `client.d.ts`.
2. **Offline / Standalone Verification**:
   - Running `npm run dev` launches the UI immediately populated with rich mock folders and documents.
   - Creating, editing, uploading, deleting, and undoing operations work out-of-the-box.
3. **Responsive & Accessible**:
   - Fully functional on desktop (1920x1080), laptop (1366x768), tablet (iPad portrait/landscape), and mobile (iPhone / Android 375px+).
   - Proper ARIA labels, focus states, and keyboard navigation.
4. **Production Readiness**:
   - Changing `VITE_USE_MOCK_SDK=false` seamlessly connects to the live Rust server running on `https://tdrive.novastrum.dev` or `localhost:13100`.
