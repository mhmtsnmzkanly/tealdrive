import type { AuditEntry, FileEntry } from './types';

export class AuditLogManager {
  private entries: AuditEntry[] = [];

  constructor(initialEntries: AuditEntry[] = []) {
    this.entries = [...initialEntries];
  }

  getEntries(): AuditEntry[] {
    return [...this.entries];
  }

  record(entry: Omit<AuditEntry, 'id' | 'timestamp'>): AuditEntry {
    const fullEntry: AuditEntry = {
      ...entry,
      id: `audit_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
      timestamp: new Date().toISOString(),
    };
    // Keep most recent first
    this.entries.unshift(fullEntry);
    // Limit to 50 operations
    if (this.entries.length > 50) {
      this.entries.pop();
    }
    return fullEntry;
  }

  find(operationId: string): AuditEntry | undefined {
    return this.entries.find((e) => e.id === operationId);
  }

  markUndone(operationId: string): boolean {
    const item = this.find(operationId);
    if (item && item.undoable) {
      item.undoable = false;
      return true;
    }
    return false;
  }
}
